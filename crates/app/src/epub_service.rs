//! EPUB to Text EPUB 轉 TXT（TTRS 新增的工具作業，TipTop 無對應）
//! 規格：specs/features/azz/epub_to_text.feature（ET1–ET8）
//!
//! 上傳的 EPUB 只在記憶體轉換，不存資料庫、不留檔（ET3）；稽核紀錄只記使用者、檔名與大小。

use std::{
    io::{Cursor, Read},
    sync::Arc,
};

use ttrs_domain::DomainError;
use ttrs_shared::EpubTextDto;

use crate::{
    Actor, AppError, Authorizer,
    epub_pdf::{self, Book, PdfFonts, SystemFonts},
    ports::{AuditEntry, AuditLog},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "epub-to-text";
/// 單一檔案上限（ET4）
pub const MAX_MB: usize = 50;

/// 輸出格式（EP1）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    #[default]
    Txt,
    Pdf,
}

impl Format {
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "txt" => Ok(Self::Txt),
            "pdf" => Ok(Self::Pdf),
            _ => Err(DomainError::EpubFormatInvalid(s.to_owned())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Txt => "txt",
            Self::Pdf => "pdf",
        }
    }
}

/// 轉換選項：輸出格式與是否把簡體轉成繁體（EP1、EP2）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConvertOptions {
    pub format: Format,
    pub s2t: bool,
}

/// 轉出的內容
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// 文字（CRLF 換行）
    Text(String),
    /// PDF 位元組
    Pdf(Vec<u8>),
}

/// 一本轉換的結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpubOutput {
    pub file_name: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub chapters: u32,
    pub content: Content,
}

/// 簡體轉繁體（EP2、EP3：OpenCC s2tw，字形加台灣異體字）；辭典載入一次，之後共用
fn to_traditional(text: &str) -> String {
    use std::sync::OnceLock;
    static CC: OnceLock<opencc_jieba_rs::OpenCC> = OnceLock::new();
    CC.get_or_init(opencc_jieba_rs::OpenCC::new)
        .convert(text, "s2tw", false)
}

/// 轉換結果（未換行轉換：段落以 `\n` 分隔）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpubText {
    pub title: Option<String>,
    pub author: Option<String>,
    pub chapters: usize,
    pub text: String,
    /// 各章的行（段落）；PDF 排版用
    pub parts: Vec<Vec<String>>,
}

fn invalid(why: &str) -> DomainError {
    DomainError::EpubInvalid(why.to_owned())
}

/// 把 EPUB 位元組轉成文字（ET5）：container.xml → OPF → spine 順序串接各章，章節之間空一行
pub fn convert(bytes: &[u8]) -> Result<EpubText, DomainError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| invalid("不是 zip 格式"))?;
    if zip.by_name("META-INF/encryption.xml").is_ok() {
        return Err(DomainError::EpubEncrypted);
    }
    let container = read_text(&mut zip, "META-INF/container.xml")
        .ok_or_else(|| invalid("缺少 META-INF/container.xml"))?;
    let opf_path = {
        let doc = roxmltree::Document::parse(&container)
            .map_err(|_| invalid("container.xml 格式錯誤"))?;
        doc.descendants()
            .find(|n| n.has_tag_name("rootfile"))
            .and_then(|n| n.attribute("full-path"))
            .map(str::to_owned)
            .ok_or_else(|| invalid("container.xml 沒有指定 OPF"))?
    };
    let opf = read_text(&mut zip, &opf_path).ok_or_else(|| invalid("找不到 OPF"))?;
    let base = opf_path.rsplit_once('/').map_or("", |(d, _)| d).to_owned();
    let doc = roxmltree::Document::parse(&opf).map_err(|_| invalid("OPF 格式錯誤"))?;
    let meta = |tag: &str| {
        doc.descendants()
            .find(|n| {
                n.tag_name().name() == tag
                    && n.tag_name()
                        .namespace()
                        .is_some_and(|ns| ns.contains("purl.org/dc"))
            })
            .and_then(|n| n.text())
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
    };
    let (title, author) = (meta("title"), meta("creator"));
    let manifest: Vec<(String, String)> = doc
        .descendants()
        .filter(|n| n.has_tag_name("item") || n.tag_name().name() == "item")
        .filter_map(|n| {
            Some((
                n.attribute("id")?.to_owned(),
                n.attribute("href")?.to_owned(),
            ))
        })
        .collect();
    let spine: Vec<String> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "itemref")
        .filter_map(|n| n.attribute("idref").map(str::to_owned))
        .collect();
    let mut chapters = Vec::new();
    for idref in spine {
        let Some((_, href)) = manifest.iter().find(|(id, _)| *id == idref) else {
            continue;
        };
        let path = join(&base, href);
        let Some(xhtml) = read_text(&mut zip, &path) else {
            continue;
        };
        let text = html_to_text(&xhtml);
        if !text.is_empty() {
            chapters.push(text);
        }
    }
    Ok(EpubText {
        title,
        author,
        chapters: chapters.len(),
        text: chapters.join("\n\n"),
        parts: chapters
            .iter()
            .map(|c| c.lines().map(str::to_owned).collect())
            .collect(),
    })
}

fn read_text<R: Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, name: &str) -> Option<String> {
    let mut f = zip.by_name(name).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let s = String::from_utf8_lossy(&buf);
    Some(s.trim_start_matches('\u{feff}').to_owned())
}

/// OPF 所在資料夾＋相對路徑（處理 `../`、URL 的 %20 與 # 片段）
fn join(base: &str, href: &str) -> String {
    let href = href
        .split('#')
        .next()
        .unwrap_or_default()
        .replace("%20", " ");
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for p in href.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// 會換行的區塊標籤
const BLOCK_TAGS: &[&str] = &[
    "p",
    "div",
    "br",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "tr",
    "blockquote",
    "section",
    "article",
    "header",
    "footer",
    "pre",
    "hr",
    "dt",
    "dd",
    "table",
    "ul",
    "ol",
    "title",
];

/// XHTML → 純文字：去掉標籤與 script、style、head；區塊標籤換行；HTML entity 解碼；連續空行最多一行
pub fn html_to_text(html: &str) -> String {
    let body = match html.find("<body") {
        Some(i) => &html[i..],
        None => html,
    };
    let mut out = String::new();
    let mut rest = body;
    while let Some(lt) = rest.find('<') {
        out.push_str(&decode_entities(&rest[..lt]));
        rest = &rest[lt..];
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[1..gt];
        rest = &rest[gt + 1..];
        if tag.starts_with("!--") {
            if let Some(end) = rest.find("-->") {
                rest = &rest[end + 3..];
            }
            continue;
        }
        let closing = tag.starts_with('/');
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == ':')
            .collect::<String>()
            .to_ascii_lowercase();
        let name = name.rsplit(':').next().unwrap_or_default().to_owned();
        if !closing && (name == "script" || name == "style") && !tag.ends_with('/') {
            let close = format!("</{name}");
            match rest.to_ascii_lowercase().find(&close) {
                Some(end) => {
                    rest = &rest[end..];
                    if let Some(gt) = rest.find('>') {
                        rest = &rest[gt + 1..];
                    }
                }
                None => rest = "",
            }
            continue;
        }
        if BLOCK_TAGS.contains(&name.as_str()) {
            out.push('\n');
        }
    }
    out.push_str(&decode_entities(rest));
    // 整理：每行去頭尾空白（含全形空白以外的空白）、空行合併
    let mut lines: Vec<String> = Vec::new();
    for line in out.lines() {
        let l = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if l.is_empty() {
            if lines.last().is_some_and(|x: &String| !x.is_empty()) {
                lines.push(String::new());
            }
        } else {
            lines.push(l);
        }
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    // 區塊之間不留空行（段落各一行）；原文的空行由章節分隔處理
    lines.retain(|l| !l.is_empty());
    lines.join("\n")
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "hellip" => Some('…'),
            "lsquo" => Some('‘'),
            "rsquo" => Some('’'),
            "ldquo" => Some('“'),
            "rdquo" => Some('”'),
            e if e.starts_with("#x") || e.starts_with("#X") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// 轉換作業（ET7：功能 run）
#[derive(Clone)]
pub struct EpubService {
    authz: Arc<dyn Authorizer>,
    audit: Arc<dyn AuditLog>,
    fonts: Arc<dyn PdfFonts>,
}

impl EpubService {
    pub fn new(authz: Arc<dyn Authorizer>, audit: Arc<dyn AuditLog>) -> Self {
        Self {
            authz,
            audit,
            fonts: Arc::new(SystemFonts),
        }
    }

    /// 換掉 PDF 字型來源（BDD 驗證找不到字型）
    pub fn with_fonts(mut self, fonts: Arc<dyn PdfFonts>) -> Self {
        self.fonts = fonts;
        self
    }

    /// 轉成文字（ET2–ET6）：超過 50 MB 回 EPUB_TOO_LARGE；文字換行為 CRLF、UTF-8 不含 BOM
    pub async fn convert(
        &self,
        actor: &Actor,
        file_name: &str,
        bytes: &[u8],
    ) -> Result<EpubTextDto, AppError> {
        let out = self
            .convert_with(actor, file_name, bytes, ConvertOptions::default())
            .await?;
        match out.content {
            Content::Text(text) => Ok(EpubTextDto {
                file_name: out.file_name,
                title: out.title,
                author: out.author,
                chapters: out.chapters,
                text,
            }),
            Content::Pdf(_) => unreachable!("預設選項輸出文字"),
        }
    }

    /// 依選項轉成 TXT 或 PDF，可選簡體轉繁體（EP1–EP10）
    pub async fn convert_with(
        &self,
        actor: &Actor,
        file_name: &str,
        bytes: &[u8],
        opts: ConvertOptions,
    ) -> Result<EpubOutput, AppError> {
        self.authz.authorize(actor, PROGRAM, "run").await?;
        if bytes.len() > MAX_MB * 1024 * 1024 {
            return Err(DomainError::EpubTooLarge { max_mb: MAX_MB }.into());
        }
        let stem = file_name
            .rsplit_once('.')
            .map_or(file_name, |(s, _)| s)
            .trim();
        let out = render(bytes, stem, opts, self.fonts.clone()).await?;
        let _ = self
            .audit
            .record(&AuditEntry {
                program: PROGRAM.to_owned(),
                user_id: actor.user_id.clone(),
                key: file_name.to_owned(),
                action: "run".to_owned(),
                detail: Some(format!(
                    "{} bytes, format={}, s2t={}",
                    bytes.len(),
                    opts.format.as_str(),
                    opts.s2t
                )),
            })
            .await;
        Ok(out)
    }
}

/// 解析 EPUB 並依選項輸出（EpubService 與 GitHub 轉換共用）。轉換與排版放在背景執行緒（EP7）
pub(crate) async fn render(
    bytes: &[u8],
    stem: &str,
    opts: ConvertOptions,
    fonts: Arc<dyn PdfFonts>,
) -> Result<EpubOutput, DomainError> {
    let bytes = bytes.to_vec();
    let stem = if stem.is_empty() { "book" } else { stem }.to_owned();
    tokio::task::spawn_blocking(move || render_blocking(&bytes, &stem, opts, fonts.as_ref()))
        .await
        .map_err(|e| DomainError::PdfGenerationFailed(e.to_string()))?
}

fn render_blocking(
    bytes: &[u8],
    stem: &str,
    opts: ConvertOptions,
    fonts: &dyn PdfFonts,
) -> Result<EpubOutput, DomainError> {
    // 先讀字型：PDF 沒有字型時不必先解析整本書
    let files = match opts.format {
        Format::Pdf => Some(fonts.load()?),
        Format::Txt => None,
    };
    let mut r = convert(bytes)?;
    if opts.s2t {
        r.title = r.title.as_deref().map(to_traditional);
        r.author = r.author.as_deref().map(to_traditional);
        for part in &mut r.parts {
            let joined = to_traditional(&part.join("\n"));
            *part = joined.lines().map(str::to_owned).collect();
        }
    }
    build_output(&r.parts, r.title, r.author, stem, opts.format, files)
}

/// 由各章的行組出輸出檔（TXT 以 CRLF、章節之間空一行；PDF 依 [`epub_pdf`] 排版）。
/// 簡轉繁與英文翻譯共用：兩者都是先處理文字，再交給這裡
pub(crate) fn build_output(
    parts: &[Vec<String>],
    title: Option<String>,
    author: Option<String>,
    stem: &str,
    format: Format,
    files: Option<Vec<epub_pdf::FontFile>>,
) -> Result<EpubOutput, DomainError> {
    let chapters = u32::try_from(parts.len()).unwrap_or(u32::MAX);
    let content = match files {
        Some(files) => {
            let book = Book {
                title: title.as_deref().unwrap_or(stem),
                author: author.as_deref().unwrap_or_default(),
                chapters: parts,
            };
            Content::Pdf(epub_pdf::render(&book, &files)?)
        }
        None => Content::Text(
            parts
                .iter()
                .map(|p| p.join("\n"))
                .collect::<Vec<_>>()
                .join("\n\n")
                .replace('\n', "\r\n"),
        ),
    };
    Ok(EpubOutput {
        file_name: format!("{stem}.{}", format.as_str()),
        title,
        author,
        chapters,
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_decodes() {
        let t = html_to_text(
            "<html><head><title>x</title></head><body><h1>標題</h1><p>甲&amp;乙&#x4E00;</p><p>a<br/>b</p><script>s()</script></body></html>",
        );
        assert_eq!(t, "標題\n甲&乙一\na\nb");
    }

    #[test]
    fn joins_paths() {
        assert_eq!(join("OEBPS", "text/c1.xhtml#p1"), "OEBPS/text/c1.xhtml");
        assert_eq!(join("OEBPS/text", "../c1.xhtml"), "OEBPS/c1.xhtml");
        assert_eq!(join("", "c1.xhtml"), "c1.xhtml");
    }

    #[test]
    fn rejects_non_zip() {
        assert_eq!(convert(b"hello").unwrap_err().code(), "EPUB_INVALID");
    }
}
