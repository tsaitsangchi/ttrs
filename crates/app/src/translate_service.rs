//! EPUB 英文翻譯成繁體中文（TTRS 新增的工具作業，TipTop 無對應）
//! 規格：specs/features/azz/epub_translate.feature（EL1–EL13）
//!
//! 翻譯一本書要數分鐘以上，所以做成「工作」：提交後立即回工作編號，背景逐批翻譯並保存進度，
//! 使用者查進度、完成後下載；結果與原文只保留 24 小時。引擎走 [`TranslationEngine`] port（infra 以 Claude Code `claude -p` 實作，
//! 用訂閱額度、不另外收 API 費用）。額度用完時工作失敗但保留已翻譯的批次，重新提交同一本書會從斷點續翻。

use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Duration as Span, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ttrs_domain::DomainError;
use ttrs_shared::TranslateJobDto;

use crate::{
    Actor, AppError, Authorizer,
    epub_pdf::{PdfFonts, SystemFonts},
    epub_service::{self, Content, EpubOutput, Format, MAX_MB},
    error::RepoError,
    ports::{
        AuditEntry, AuditLog, JobRecord, TranslateRequest, TranslationEngine,
        TranslationJobRepository,
    },
};

/// 權限檢查用的程式代號（沿用 EPUB 轉換）與功能
pub const PROGRAM: &str = "epub-to-text";
pub const ACTION: &str = "translate";
/// 單本上限（EL7、EL13）
pub const MAX_CHARS: usize = 300_000;
/// 一批約 2000 字元（EL5）
pub const BATCH_CHARS: usize = 2000;
/// 單一段落超過這個長度就切開翻譯，再接回去
const LINE_MAX: usize = 2400;
/// 引擎暫時失敗的重試次數（EL9）
const RETRIES: usize = 3;
/// 結果與原文保留時數（EL4）
const KEEP_HOURS: i64 = 24;
/// 進行中的工作超過這麼久沒更新就視為已中斷（伺服器重啟等）
const STALE_MINUTES: i64 = 30;
/// 帶給引擎的人名地名對照表最多幾筆（EL5）
const GLOSSARY_MAX: usize = 60;

/// 原文、切段與已翻譯的批次（存在工作的 doc 欄位，JSON）
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Doc {
    stem: String,
    author: Option<String>,
    /// 各章的段落數（不含書名）
    chapter_groups: Vec<usize>,
    /// 每個段落被切成幾片（一般為 1）；第一個是書名
    pieces: Vec<usize>,
    /// 全部片段原文：書名、各章段落
    lines: Vec<String>,
    /// 每批 [起, 迄) 片段
    batches: Vec<(usize, usize)>,
    /// 每批的譯文
    translated: Vec<Option<Vec<String>>>,
    /// (英文, 中文) 人名地名對照表
    glossary: Vec<(String, String)>,
}

#[derive(Clone)]
pub struct TranslationService {
    authz: Arc<dyn Authorizer>,
    jobs: Arc<dyn TranslationJobRepository>,
    audit: Arc<dyn AuditLog>,
    engine: Arc<dyn TranslationEngine>,
    fonts: Arc<dyn PdfFonts>,
    /// 提交後是否自己在背景執行（BDD 改為手動呼叫 [`process`](Self::process)）
    spawn: bool,
    batch_delay: Duration,
    retry_delay: Duration,
}

impl TranslationService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        jobs: Arc<dyn TranslationJobRepository>,
        audit: Arc<dyn AuditLog>,
        engine: Arc<dyn TranslationEngine>,
    ) -> Self {
        Self {
            authz,
            jobs,
            audit,
            engine,
            fonts: Arc::new(SystemFonts),
            spawn: true,
            // 批次之間隔 2 秒，不一次吃光訂閱額度（EL13）
            batch_delay: Duration::from_secs(2),
            retry_delay: Duration::from_secs(5),
        }
    }

    pub fn with_engine(mut self, engine: Arc<dyn TranslationEngine>) -> Self {
        self.engine = engine;
        self
    }

    pub fn with_fonts(mut self, fonts: Arc<dyn PdfFonts>) -> Self {
        self.fonts = fonts;
        self
    }

    /// 不自己在背景執行、不等待間隔（BDD：由測試呼叫 [`process`](Self::process)）
    pub fn inline(mut self) -> Self {
        self.spawn = false;
        self.batch_delay = Duration::ZERO;
        self.retry_delay = Duration::ZERO;
        self
    }

    /// 提交翻譯工作（EL1）：回工作編號。同一本書上次失敗的工作還在時，沿用並從斷點續翻（EL13）
    pub async fn submit(
        &self,
        actor: &Actor,
        file_name: &str,
        bytes: &[u8],
        format: Format,
        consent: bool,
    ) -> Result<String, AppError> {
        self.authz.authorize(actor, PROGRAM, ACTION).await?;
        let now = Utc::now();
        let _ = self.jobs.purge_expired(now).await;
        if !consent {
            return Err(DomainError::TranslateConsentRequired.into());
        }
        if bytes.len() > MAX_MB * 1024 * 1024 {
            return Err(DomainError::EpubTooLarge { max_mb: MAX_MB }.into());
        }
        // 先確認 PDF 字型與翻譯服務，沒有就不必解析整本書
        if format == Format::Pdf {
            self.fonts.load()?;
        }
        self.engine.check().await?;
        let book = epub_service::convert(bytes)?;
        let chars: usize = book
            .parts
            .iter()
            .flatten()
            .map(|l| l.chars().count())
            .sum::<usize>()
            + book.title.as_deref().map_or(0, |t| t.chars().count());
        if !looks_english(&book.parts) {
            return Err(DomainError::EpubNotEnglish.into());
        }
        if chars > MAX_CHARS {
            return Err(DomainError::TranslateLimitExceeded {
                max_chars: MAX_CHARS,
            }
            .into());
        }
        if self
            .jobs
            .has_active(now - Span::minutes(STALE_MINUTES))
            .await
            .map_err(infra)?
        {
            return Err(DomainError::TranslateBusy.into());
        }
        let sha = hex(&Sha256::digest(bytes));
        let job_id = match self
            .jobs
            .find_resumable(&actor.user_id, &sha, now)
            .await
            .map_err(infra)?
        {
            Some(mut job) => {
                job.status = "queued".to_owned();
                job.error_code = None;
                job.error_message = None;
                job.format = format.as_str().to_owned();
                job.updated_at = now;
                job.expires_at = now + Span::hours(KEEP_HOURS);
                self.jobs.save(&job).await.map_err(infra)?;
                job.job_id
            }
            None => {
                let job = new_job(actor, file_name, &sha, format, &book, now)?;
                self.jobs.create(&job).await.map_err(infra)?;
                job.job_id
            }
        };
        let _ = self
            .audit
            .record(&AuditEntry {
                program: PROGRAM.to_owned(),
                user_id: actor.user_id.clone(),
                key: file_name.to_owned(),
                action: ACTION.to_owned(),
                // 只記引擎、字數與格式，不記書的內容（EL3）
                detail: Some(format!(
                    "engine={}, chars={chars}, format={}",
                    self.engine.name(),
                    format.as_str()
                )),
            })
            .await;
        if self.spawn {
            let svc = self.clone();
            let id = job_id.clone();
            tokio::spawn(async move {
                if let Err(e) = svc.process(&id).await {
                    tracing::error!("翻譯工作 {id} 發生錯誤：{e}");
                }
            });
        }
        Ok(job_id)
    }

    /// 執行工作：依序翻譯尚未完成的批次，每批存進度；結束時成功存結果、失敗存原因（保留已翻譯的批次）
    pub async fn process(&self, job_id: &str) -> Result<(), AppError> {
        let Some(mut job) = self.jobs.get(job_id).await.map_err(infra)? else {
            return Err(DomainError::TranslateJobNotFound.into());
        };
        if job.status == "done" {
            return Ok(());
        }
        let mut doc: Doc =
            serde_json::from_str(&job.doc).map_err(|e| AppError::Infra(e.to_string()))?;
        job.status = "running".to_owned();
        job.updated_at = Utc::now();
        self.jobs.save(&job).await.map_err(infra)?;
        let batch_count = doc.batches.len();
        for bi in 0..batch_count {
            if doc.translated[bi].is_some() {
                continue;
            }
            let (from, to) = doc.batches[bi];
            let req = TranslateRequest {
                lines: doc.lines[from..to].to_vec(),
                glossary: doc
                    .glossary
                    .iter()
                    .rev()
                    .take(GLOSSARY_MAX)
                    .rev()
                    .cloned()
                    .collect(),
            };
            match self.translate_with_retry(&req).await {
                Ok(resp) => {
                    for (en, zh) in resp.names {
                        if !doc.glossary.iter().any(|(e, _)| *e == en) {
                            doc.glossary.push((en, zh));
                        }
                    }
                    doc.translated[bi] = Some(resp.lines);
                    job.done_batches =
                        i32::try_from(doc.translated.iter().flatten().count()).unwrap_or(i32::MAX);
                    job.done_lines = done_groups(&doc);
                    job.doc =
                        serde_json::to_string(&doc).map_err(|e| AppError::Infra(e.to_string()))?;
                    job.updated_at = Utc::now();
                    self.jobs.save(&job).await.map_err(infra)?;
                    if bi + 1 < batch_count && !self.batch_delay.is_zero() {
                        tokio::time::sleep(self.batch_delay).await;
                    }
                }
                Err(e) => {
                    job.status = "failed".to_owned();
                    job.error_code = Some(e.code().to_owned());
                    job.error_message = Some(e.to_string());
                    job.updated_at = Utc::now();
                    self.jobs.save(&job).await.map_err(infra)?;
                    return Ok(());
                }
            }
        }
        match self.finish(&doc, &job.format).await {
            Ok(out) => {
                job.result = Some(match out.content {
                    Content::Text(t) => t.into_bytes(),
                    Content::Pdf(b) => b,
                });
                job.result_file_name = Some(out.file_name);
                job.result_title = out.title;
                job.result_author = out.author;
                job.result_chapters = i32::try_from(out.chapters).unwrap_or(i32::MAX);
                job.status = "done".to_owned();
            }
            Err(e) => {
                job.status = "failed".to_owned();
                job.error_code = Some(e.code().to_owned());
                job.error_message = Some(e.to_string());
            }
        }
        job.updated_at = Utc::now();
        self.jobs.save(&job).await.map_err(infra)?;
        Ok(())
    }

    /// 呼叫引擎：暫時失敗與段數不符自動重試（EL9）；沒設定、額度用完立刻回報
    async fn translate_with_retry(
        &self,
        req: &TranslateRequest,
    ) -> Result<crate::ports::TranslateResponse, DomainError> {
        let mut last = DomainError::TranslateFailed("未知原因".to_owned());
        for attempt in 0..=RETRIES {
            if attempt > 0 && !self.retry_delay.is_zero() {
                tokio::time::sleep(self.retry_delay).await;
            }
            match self.engine.translate(req).await {
                Ok(resp) if resp.lines.len() == req.lines.len() => return Ok(resp),
                Ok(resp) => {
                    last = DomainError::TranslateFailed(format!(
                        "譯文段數不符（要 {} 段，回 {} 段）",
                        req.lines.len(),
                        resp.lines.len()
                    ));
                }
                Err(DomainError::TranslateFailed(m)) => last = DomainError::TranslateFailed(m),
                Err(other) => return Err(other),
            }
        }
        Err(last)
    }

    /// 全部批次翻完後組成輸出檔（TXT 或 PDF）
    async fn finish(&self, doc: &Doc, format: &str) -> Result<EpubOutput, DomainError> {
        let format = Format::parse(format)?;
        let files = match format {
            Format::Pdf => Some(self.fonts.load()?),
            Format::Txt => None,
        };
        // 全部片段的譯文，依批次接起來
        let flat: Vec<String> = doc
            .translated
            .iter()
            .flat_map(|b| b.clone().unwrap_or_default())
            .collect();
        if flat.len() != doc.lines.len() {
            return Err(DomainError::TranslateFailed(
                "譯文與原文段數不符".to_owned(),
            ));
        }
        // 片段接回段落：[書名, 各章段落…]
        let mut groups: Vec<String> = Vec::new();
        let mut at = 0;
        for &n in &doc.pieces {
            groups.push(flat[at..at + n].join(" "));
            at += n;
        }
        let mut groups = groups.into_iter();
        let title = groups.next();
        let mut parts: Vec<Vec<String>> = Vec::new();
        for &n in &doc.chapter_groups {
            parts.push(groups.by_ref().take(n).collect());
        }
        let author = doc.author.clone();
        let stem = doc.stem.clone();
        tokio::task::spawn_blocking(move || {
            epub_service::build_output(&parts, title, author, &stem, format, files)
        })
        .await
        .map_err(|e| DomainError::PdfGenerationFailed(e.to_string()))?
    }

    /// 查詢工作進度（EL4）；只有提交的人看得到，過期的視為不存在
    pub async fn status(&self, actor: &Actor, job_id: &str) -> Result<TranslateJobDto, AppError> {
        let job = self.own_job(actor, job_id).await?;
        Ok(TranslateJobDto {
            job_id: job.job_id,
            status: job.status,
            done_lines: u32::try_from(job.done_lines).unwrap_or(0),
            total_lines: u32::try_from(job.total_lines).unwrap_or(0),
            done_batches: u32::try_from(job.done_batches).unwrap_or(0),
            total_batches: u32::try_from(job.total_batches).unwrap_or(0),
            error_code: job.error_code,
            error_message: job.error_message,
            file_name: job.result_file_name,
        })
    }

    /// 下載翻譯結果（TXT 或 PDF）
    pub async fn download(&self, actor: &Actor, job_id: &str) -> Result<EpubOutput, AppError> {
        let job = self.own_job(actor, job_id).await?;
        let (Some(bytes), "done") = (job.result, job.status.as_str()) else {
            return Err(DomainError::TranslateFailed("工作尚未完成或沒有結果".to_owned()).into());
        };
        let content = match job.format.as_str() {
            "pdf" => Content::Pdf(bytes),
            _ => Content::Text(String::from_utf8_lossy(&bytes).into_owned()),
        };
        Ok(EpubOutput {
            file_name: job
                .result_file_name
                .unwrap_or_else(|| "book.txt".to_owned()),
            title: job.result_title,
            author: job.result_author,
            chapters: u32::try_from(job.result_chapters).unwrap_or(0),
            content,
        })
    }

    async fn own_job(&self, actor: &Actor, job_id: &str) -> Result<JobRecord, AppError> {
        self.authz.authorize(actor, PROGRAM, ACTION).await?;
        match self.jobs.get(job_id).await.map_err(infra)? {
            Some(j) if j.user_id == actor.user_id && j.expires_at > Utc::now() => Ok(j),
            _ => Err(DomainError::TranslateJobNotFound.into()),
        }
    }

    /// 刪除到期的工作（EL4：24 小時）；提交時也會順便清一次
    pub async fn purge_expired(&self, now: DateTime<Utc>) -> Result<u64, AppError> {
        self.jobs.purge_expired(now).await.map_err(infra)
    }
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// 已翻完的段落數（不含書名）
fn done_groups(doc: &Doc) -> i32 {
    let mut done = 0usize;
    let mut piece_at = 0usize;
    for (gi, &n) in doc.pieces.iter().enumerate() {
        let end = piece_at + n;
        let translated = doc
            .batches
            .iter()
            .enumerate()
            .filter(|(bi, (from, to))| {
                doc.translated[*bi].is_some() && *from < end && *to > piece_at
            })
            .map(|(_, (from, to))| (*from.max(&piece_at), *to.min(&end)))
            .map(|(a, b)| b - a)
            .sum::<usize>();
        if translated == n && gi > 0 {
            done += 1;
        }
        piece_at = end;
    }
    i32::try_from(done).unwrap_or(i32::MAX)
}

/// 書的內容大多是英文字母（CJK 字不到一成）才算英文（EL6）
fn looks_english(parts: &[Vec<String>]) -> bool {
    let (mut latin, mut cjk) = (0usize, 0usize);
    for c in parts.iter().flatten().flat_map(|l| l.chars()) {
        if c.is_ascii_alphabetic() {
            latin += 1;
        } else if matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF)
        {
            cjk += 1;
        }
    }
    latin > 0 && cjk * 10 <= latin
}

/// 把過長的段落切成不超過 [`LINE_MAX`] 字元的片段（盡量在句尾或空白處）
fn split_long(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= LINE_MAX {
        return vec![text.to_owned()];
    }
    let mut out = Vec::new();
    let mut start = 0;
    while chars.len() - start > LINE_MAX {
        let window = &chars[start..start + BATCH_CHARS];
        let cut = window
            .iter()
            .rposition(|c| matches!(c, '.' | '!' | '?' | '。' | '！' | '？'))
            .or_else(|| window.iter().rposition(|c| *c == ' '))
            .map_or(BATCH_CHARS, |i| i + 1);
        out.push(chars[start..start + cut].iter().collect::<String>());
        start += cut;
    }
    out.push(chars[start..].iter().collect());
    out
}

fn new_job(
    actor: &Actor,
    file_name: &str,
    sha: &str,
    format: Format,
    book: &epub_service::EpubText,
    now: DateTime<Utc>,
) -> Result<JobRecord, AppError> {
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(s, _)| s)
        .trim();
    let stem = if stem.is_empty() { "book" } else { stem }.to_owned();
    let mut lines: Vec<String> = Vec::new();
    let mut pieces: Vec<usize> = Vec::new();
    let mut push = |text: &str| {
        let p = split_long(text);
        pieces.push(p.len());
        lines.extend(p);
    };
    // 書名也一起翻（沒有書名時用檔名）
    push(book.title.as_deref().unwrap_or(&stem));
    let mut chapter_groups = Vec::new();
    for chapter in &book.parts {
        chapter_groups.push(chapter.len());
        for l in chapter {
            push(l);
        }
    }
    // 依字元數切批：每批約 2000 字元，單一長段落自成一批
    let mut batches: Vec<(usize, usize)> = Vec::new();
    let (mut from, mut size) = (0usize, 0usize);
    for (i, l) in lines.iter().enumerate() {
        let n = l.chars().count();
        if i > from && size + n > BATCH_CHARS {
            batches.push((from, i));
            from = i;
            size = 0;
        }
        size += n;
    }
    if from < lines.len() {
        batches.push((from, lines.len()));
    }
    let total_groups = i32::try_from(pieces.len().saturating_sub(1)).unwrap_or(i32::MAX);
    let doc = Doc {
        stem,
        author: book.author.clone(),
        chapter_groups,
        pieces,
        translated: vec![None; batches.len()],
        batches: batches.clone(),
        lines,
        glossary: Vec::new(),
    };
    let doc_json = serde_json::to_string(&doc).map_err(|e| AppError::Infra(e.to_string()))?;
    Ok(JobRecord {
        job_id: new_id()?,
        user_id: actor.user_id.clone(),
        source_sha256: sha.to_owned(),
        file_name: file_name.to_owned(),
        format: format.as_str().to_owned(),
        status: "queued".to_owned(),
        total_batches: i32::try_from(batches.len()).unwrap_or(i32::MAX),
        done_batches: 0,
        total_lines: total_groups,
        done_lines: 0,
        error_code: None,
        error_message: None,
        doc: doc_json,
        result: None,
        result_file_name: None,
        result_title: None,
        result_author: None,
        result_chapters: 0,
        created_at: now,
        updated_at: now,
        expires_at: now + Span::hours(KEEP_HOURS),
    })
}

fn new_id() -> Result<String, AppError> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| AppError::Infra(format!("亂數：{e}")))?;
    Ok(hex(&b))
}

/// 沒有設定翻譯引擎時的預設：一律回 TRANSLATE_NOT_CONFIGURED
#[derive(Debug, Clone, Default)]
pub struct NoTranslator;

#[async_trait::async_trait]
impl TranslationEngine for NoTranslator {
    fn name(&self) -> String {
        "none".to_owned()
    }

    async fn check(&self) -> Result<(), DomainError> {
        Err(DomainError::TranslateNotConfigured(
            "尚未設定翻譯引擎".to_owned(),
        ))
    }

    async fn translate(
        &self,
        _req: &TranslateRequest,
    ) -> Result<crate::ports::TranslateResponse, DomainError> {
        self.check().await.map(|()| Default::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_long_lines_at_sentence_ends() {
        let s = format!("{} ", "Hello world.".repeat(300));
        let parts = split_long(&s);
        assert!(parts.len() > 1);
        assert!(parts.iter().all(|p| p.chars().count() <= LINE_MAX));
        assert_eq!(parts.concat(), s);
    }

    #[test]
    fn detects_english() {
        assert!(looks_english(&[vec!["Hello world.".to_owned()]]));
        assert!(!looks_english(&[vec!["這是一個測試。".to_owned()]]));
        assert!(!looks_english(&[]));
    }
}
