//! EPUB 多來源搜尋（TTRS 新增的工具作業，TipTop 無對應）
//! 規格：specs/features/azz/epub_web_search.feature（WS1–WS10）
//!
//! 同時查詢多個合法來源（Internet Archive、Project Gutenberg、Wikisource、GitHub），合併、去重並排序：
//! 繁體中文排最前面，同層可直接下載的在前、再依下載次數由高到低。搜尋時關鍵字同時查繁體與簡體寫法。
//! 只收可合法下載的書：預設只顯示公共領域、開放授權（取消過濾才看得到其他，並標示授權）。

use std::{cmp::Reverse, sync::Arc, time::Duration};

use ttrs_domain::DomainError;
use ttrs_shared::{EpubBookDto, EpubSearchPage};

use crate::{
    Actor, AppError, Authorizer,
    epub_pdf::{PdfFonts, SystemFonts},
    epub_service::{self, ConvertOptions, EpubOutput, MAX_MB, to_simplified, to_traditional},
    github_epub_service::is_open_license,
    ports::{AuditEntry, AuditLog, EpubBook, EpubBookKind, EpubSource},
};

/// 權限檢查用的程式代號（沿用 GitHub EPUB 搜尋，畫面名稱改為「EPUB 搜尋」）
pub const PROGRAM: &str = "github-epub-search";
/// 一頁筆數
pub const PER_PAGE: usize = 30;
/// 每個來源最多取幾筆再合併排序（WS6）
pub const PER_SOURCE: usize = 30;
/// 每個來源最多等多久（WS6）
const SOURCE_TIMEOUT: Duration = Duration::from_secs(10);

/// 文字種類（WS3、WS4）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Traditional,
    Simplified,
    /// 是中文但看不出繁簡
    OtherChinese,
    Other,
}

impl Script {
    fn tier(self) -> u8 {
        match self {
            Self::Traditional => 0,
            Self::Simplified | Self::OtherChinese => 1,
            Self::Other => 2,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Traditional => "traditional",
            Self::Simplified => "simplified",
            Self::OtherChinese => "other_chinese",
            Self::Other => "other",
        }
    }
}

fn has_cjk(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF))
}

/// 由書名的繁體專用字、簡體專用字判斷繁簡（OpenCC 轉換後有變就表示有該種專用字）
fn script_by_text(text: &str) -> Script {
    if !has_cjk(text) {
        return Script::Other;
    }
    let has_trad = to_simplified(text) != text;
    let has_simp = to_traditional(text) != text;
    match (has_trad, has_simp) {
        (true, false) => Script::Traditional,
        (false, true) => Script::Simplified,
        _ => Script::OtherChinese,
    }
}

/// 先看來源標示的語言，標示不明確（zh、chi）時看書名（WS4）
pub fn script_of(language: Option<&str>, title: &str) -> Script {
    let lang = language.unwrap_or_default().trim().to_lowercase();
    if ["zh-tw", "zh-hant", "zh-hk", "zh-mo"]
        .iter()
        .any(|p| lang.starts_with(p))
    {
        return Script::Traditional;
    }
    if ["zh-cn", "zh-hans", "zh-sg"]
        .iter()
        .any(|p| lang.starts_with(p))
    {
        return Script::Simplified;
    }
    let chinese = ["zh", "chi", "zho", "chinese", "中文", "漢語", "汉语"]
        .iter()
        .any(|p| lang.starts_with(p));
    if chinese || lang.is_empty() {
        return script_by_text(title);
    }
    Script::Other
}

/// 開放授權：公共領域、CC0、CC BY、CC BY-SA、CC BY-NC、CC BY-NC-SA 與 GitHub 的開放授權（WS5）。
/// 含 ND（禁止改作）的不算，因為轉成 TXT／PDF 屬於改作
pub fn license_is_open(raw: Option<&str>) -> bool {
    let Some(raw) = raw.map(str::trim).filter(|r| !r.is_empty()) else {
        return false;
    };
    let lower = raw.to_lowercase();
    if lower.contains("publicdomain")
        || lower.contains("public domain")
        || lower.contains("公共領域")
        || lower.contains("cc0")
    {
        return true;
    }
    if let Some((_, rest)) = lower.split_once("creativecommons.org/licenses/") {
        let kind = rest.split('/').next().unwrap_or_default();
        return !kind.split('-').any(|t| t == "nd");
    }
    is_open_license(Some(raw))
}

/// 授權的顯示文字：Creative Commons 網址轉成「公共領域」或「CC BY-NC 4.0」，其他保持原樣
pub fn license_label(raw: &str) -> String {
    let lower = raw.to_lowercase();
    if lower.contains("creativecommons.org/publicdomain") {
        return "公共領域".to_owned();
    }
    if let Some((_, rest)) = lower.split_once("creativecommons.org/licenses/") {
        let mut it = rest.split('/');
        let kind = it.next().unwrap_or_default().to_uppercase();
        return match it.next().filter(|v| !v.is_empty()) {
            Some(ver) => format!("CC {kind} {ver}"),
            None => format!("CC {kind}"),
        };
    }
    raw.to_owned()
}

/// 同一本書：書名相同（去頭尾空白、不分大小寫），且作者相同或其中一邊沒有作者資料（例如維基文庫）
fn same_book(a: &EpubBook, b: &EpubBook) -> bool {
    let author = |x: &EpubBook| {
        x.author
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_lowercase()
    };
    a.title.trim().to_lowercase() == b.title.trim().to_lowercase()
        && (author(a) == author(b) || author(a).is_empty() || author(b).is_empty())
}

/// 比對用的文字：轉成繁體、英文小寫、去掉空白與標點（RL1）
fn normalize(s: &str) -> String {
    to_traditional(s)
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// 書名吻合程度（RL1）：0 完全相同、1 開頭就是關鍵字、2 中間含有、3 不含；取各關鍵字寫法中最好的一級
fn relevance(title: &str, keywords: &[String]) -> u8 {
    let t = normalize(title);
    keywords
        .iter()
        .map(|k| normalize(k))
        .filter(|k| !k.is_empty())
        .map(|k| {
            if t == k {
                0
            } else if t.starts_with(&k) {
                1
            } else if t.contains(&k) {
                2
            } else {
                3
            }
        })
        .min()
        .unwrap_or(3)
}

/// 一筆結果與判斷出的文字種類
struct Ranked {
    book: EpubBook,
    relevance: u8,
    script: Script,
    open: bool,
    others: Vec<String>,
}

impl Ranked {
    /// 排序鍵：書名吻合程度（RL2）；再依 WS3——繁體優先；同層可直接下載的在前；有下載次數的在前、由高到低；最後依書名
    fn key(&self) -> (u8, u8, bool, bool, Reverse<u64>, String) {
        (
            self.relevance,
            self.script.tier(),
            self.book.kind != EpubBookKind::Book,
            self.book.downloads.is_none(),
            Reverse(self.book.downloads.unwrap_or(0)),
            self.book.title.clone(),
        )
    }
}

/// 排序、去重（WS3、WS6）：相同書名作者只留排序最前面的一筆，其他來源記在 `others`
fn merge(books: Vec<EpubBook>, keywords: &[String]) -> Vec<Ranked> {
    let mut ranked: Vec<Ranked> = books
        .into_iter()
        .map(|b| Ranked {
            relevance: relevance(&b.title, keywords),
            script: script_of(b.language.as_deref(), &b.title),
            open: license_is_open(b.license.as_deref()),
            book: b,
            others: Vec::new(),
        })
        .collect();
    ranked.sort_by_key(Ranked::key);
    let mut out: Vec<Ranked> = Vec::new();
    for r in ranked {
        match out.iter_mut().find(|x| same_book(&x.book, &r.book)) {
            Some(first) => {
                if first.book.source != r.book.source && !first.others.contains(&r.book.source) {
                    first.others.push(r.book.source);
                }
            }
            None => out.push(r),
        }
    }
    out
}

fn to_dto(r: Ranked) -> EpubBookDto {
    EpubBookDto {
        source: r.book.source,
        id: r.book.id,
        title: r.book.title,
        author: r.book.author,
        language: r.book.language,
        script: r.script.as_str().to_owned(),
        license: r.book.license.as_deref().map(license_label),
        open_license: r.open,
        downloads: r.book.downloads,
        kind: match r.book.kind {
            EpubBookKind::Book => "book",
            EpubBookKind::Repo => "repo",
        }
        .to_owned(),
        url: r.book.url,
        other_sources: r.others,
    }
}

/// 同一個關鍵字的寫法：原樣、繁體、簡體（去除重複）
fn keyword_variants(keyword: &str) -> Vec<String> {
    let mut v = vec![keyword.to_owned()];
    for k in [to_traditional(keyword), to_simplified(keyword)] {
        if !v.contains(&k) {
            v.push(k);
        }
    }
    v
}

/// 檔名不可有的字元換成底線
fn safe_name(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let s = s.trim().to_owned();
    if s.is_empty() { "book".to_owned() } else { s }
}

#[derive(Clone)]
pub struct EpubSearchService {
    authz: Arc<dyn Authorizer>,
    audit: Arc<dyn AuditLog>,
    sources: Vec<Arc<dyn EpubSource>>,
    fonts: Arc<dyn PdfFonts>,
}

impl EpubSearchService {
    pub fn new(authz: Arc<dyn Authorizer>, audit: Arc<dyn AuditLog>) -> Self {
        Self {
            authz,
            audit,
            sources: Vec::new(),
            fonts: Arc::new(SystemFonts),
        }
    }

    /// 設定要查詢的來源（API 啟動時放入真正的來源；BDD 放假來源）
    pub fn with_sources(mut self, sources: Vec<Arc<dyn EpubSource>>) -> Self {
        self.sources = sources;
        self
    }

    pub fn with_fonts(mut self, fonts: Arc<dyn PdfFonts>) -> Self {
        self.fonts = fonts;
        self
    }

    /// 搜尋（WS3、WS5–WS7）：所有來源同時查，單一來源失敗或逾時只在 `warnings` 提示
    pub async fn search(
        &self,
        actor: &Actor,
        keyword: &str,
        open_only: bool,
        page: u32,
    ) -> Result<EpubSearchPage, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        let keyword = keyword.trim();
        let page = page.max(1);
        let variants = keyword_variants(keyword);
        let handles: Vec<_> = self
            .sources
            .iter()
            .map(|s| {
                let (source, variants) = (s.clone(), variants.clone());
                tokio::spawn(async move {
                    let name = source.name();
                    let r = tokio::time::timeout(
                        SOURCE_TIMEOUT,
                        source.search(&variants, open_only, PER_SOURCE),
                    )
                    .await;
                    (name, r)
                })
            })
            .collect();
        let (mut books, mut warnings) = (Vec::new(), Vec::new());
        for (source, h) in self.sources.iter().zip(handles) {
            let failed = match h.await {
                Ok((_, Ok(Ok(found)))) => {
                    books.extend(found);
                    None
                }
                Ok((name, _)) => Some(name),
                Err(_) => Some(source.name()),
            };
            // 同名的來源（GitHub 的 repo 與檔名搜尋）只提示一次
            if let Some(name) = failed.filter(|n| !warnings.contains(n)) {
                warnings.push(name);
            }
        }
        let mut merged = merge(books, &variants);
        if open_only {
            merged.retain(|r| r.open);
        }
        let total = merged.len() as u64;
        let from = (page as usize - 1) * PER_PAGE;
        let items = merged
            .into_iter()
            .skip(from)
            .take(PER_PAGE)
            .map(to_dto)
            .collect();
        let mut notices: Vec<String> = Vec::new();
        for n in self.sources.iter().filter_map(|s| s.notice()) {
            if !notices.contains(&n) {
                notices.push(n);
            }
        }
        let mut names: Vec<String> = self.sources.iter().map(|s| s.name()).collect();
        names.dedup();
        self.audit_detail(
            actor,
            "query",
            keyword,
            Some(format!("keyword={keyword}; sources={}", names.join(","))),
        )
        .await;
        Ok(EpubSearchPage {
            items,
            total,
            page,
            warnings,
            notices,
        })
    }

    /// 依名稱下載：同名的來源（GitHub 的 repo 與檔名搜尋）依序嘗試，回第一個不是「找不到」的結果
    async fn download_from(&self, name: &str, id: &str, max: u64) -> Result<Vec<u8>, DomainError> {
        let mut last = DomainError::EpubBookNotFound(name.to_owned());
        for s in self.sources.iter().filter(|s| s.name() == name) {
            match s.download(id, max).await {
                Err(e @ DomainError::EpubBookNotFound(_)) => last = e,
                other => return other,
            }
        }
        Err(last)
    }

    /// 從來源下載一本 EPUB（WS8）：超過 100 MB 回 EPUB_TOO_LARGE，不保存檔案。回（建議檔名主體, 位元組）
    pub async fn fetch(
        &self,
        actor: &Actor,
        source: &str,
        id: &str,
        title: Option<&str>,
    ) -> Result<(String, Vec<u8>), AppError> {
        self.authz.authorize(actor, PROGRAM, "run").await?;
        let bytes = self
            .download_from(
                source,
                id,
                u64::try_from(MAX_MB * 1024 * 1024).unwrap_or(u64::MAX),
            )
            .await?;
        self.audit_detail(
            actor,
            "run",
            &format!("{source}/{id}"),
            Some(format!("source={source}; id={id}; bytes={}", bytes.len())),
        )
        .await;
        let stem = safe_name(title.unwrap_or(id));
        Ok((stem, bytes))
    }

    /// 下載原始 EPUB（WS8）
    pub async fn download_raw(
        &self,
        actor: &Actor,
        source: &str,
        id: &str,
        title: Option<&str>,
    ) -> Result<(String, Vec<u8>), AppError> {
        let (stem, bytes) = self.fetch(actor, source, id, title).await?;
        Ok((format!("{stem}.epub"), bytes))
    }

    /// 把已下載的 EPUB 依選項轉成 TXT 或 PDF（API 的翻譯與轉換共用）
    pub async fn render_bytes(
        &self,
        bytes: &[u8],
        stem: &str,
        opts: ConvertOptions,
    ) -> Result<EpubOutput, AppError> {
        Ok(epub_service::render(bytes, stem, opts, self.fonts.clone()).await?)
    }

    /// 下載並依選項轉成 TXT 或 PDF、可選簡體轉繁體（WS8）
    pub async fn convert(
        &self,
        actor: &Actor,
        source: &str,
        id: &str,
        title: Option<&str>,
        opts: ConvertOptions,
    ) -> Result<EpubOutput, AppError> {
        let (stem, bytes) = self.fetch(actor, source, id, title).await?;
        Ok(epub_service::render(&bytes, &stem, opts, self.fonts.clone()).await?)
    }

    async fn audit_detail(&self, actor: &Actor, action: &str, key: &str, detail: Option<String>) {
        let _ = self
            .audit
            .record(&AuditEntry {
                program: PROGRAM.to_owned(),
                user_id: actor.user_id.clone(),
                key: key.to_owned(),
                action: action.to_owned(),
                detail,
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(source: &str, title: &str, lang: &str, downloads: Option<u64>) -> EpubBook {
        EpubBook {
            source: source.to_owned(),
            id: title.to_owned(),
            title: title.to_owned(),
            author: Some("某".to_owned()),
            language: Some(lang.to_owned()),
            license: Some("公共領域".to_owned()),
            downloads,
            kind: EpubBookKind::Book,
            url: None,
        }
    }

    #[test]
    fn relevance_tiers() {
        let k = keyword_variants("慶餘年");
        assert_eq!(relevance("慶餘年", &k), 0);
        assert_eq!(relevance("庆余年", &k), 0);
        assert_eq!(relevance("《慶餘年》讀後感", &k), 1);
        assert_eq!(relevance("庆余年第1卷至第7卷-全集", &k), 1);
        assert_eq!(relevance("再讀慶餘年", &k), 2);
        assert_eq!(relevance("余家庆刑事一审刑事判决书", &k), 3);
        assert_eq!(relevance("The Story", &keyword_variants("the story")), 0);
    }

    #[test]
    fn script_detection() {
        assert_eq!(script_of(Some("zh-TW"), "x"), Script::Traditional);
        assert_eq!(script_of(Some("chi"), "紅樓夢"), Script::Traditional);
        assert_eq!(script_of(Some("zh"), "红楼梦"), Script::Simplified);
        assert_eq!(script_of(Some("en"), "紅樓夢"), Script::Other);
        assert_eq!(script_of(None, "Hello"), Script::Other);
    }

    #[test]
    fn licenses() {
        assert!(license_is_open(Some("公共領域")));
        assert!(license_is_open(Some(
            "http://creativecommons.org/publicdomain/mark/1.0/"
        )));
        assert!(license_is_open(Some(
            "https://creativecommons.org/licenses/by-nc/4.0/"
        )));
        assert!(!license_is_open(Some(
            "https://creativecommons.org/licenses/by-nc-nd/4.0/"
        )));
        assert!(license_is_open(Some("MIT")));
        assert!(!license_is_open(Some("版權保留")));
        assert!(!license_is_open(None));
    }

    #[test]
    fn license_labels() {
        assert_eq!(
            license_label("http://creativecommons.org/publicdomain/mark/1.0/"),
            "公共領域"
        );
        assert_eq!(
            license_label("https://creativecommons.org/licenses/by-nc/4.0/"),
            "CC BY-NC 4.0"
        );
        assert_eq!(license_label("MIT"), "MIT");
    }

    #[test]
    fn ranks_and_merges() {
        let r = merge(
            vec![
                book("A", "Dream", "en", Some(20000)),
                book("B", "红楼梦", "zh-CN", Some(9000)),
                book("C", "紅樓夢", "zh-TW", Some(5000)),
                book("D", "紅樓夢", "zh-TW", None),
            ],
            &[],
        );
        let titles: Vec<_> = r.iter().map(|x| x.book.title.as_str()).collect();
        assert_eq!(titles, ["紅樓夢", "红楼梦", "Dream"]);
        assert_eq!(r[0].others, ["D"]);
    }

    #[test]
    fn missing_author_merges_with_known_author() {
        let mut a = book("A", "紅樓夢", "zh-TW", Some(10));
        a.author = Some("曹雪芹".to_owned());
        let mut b = book("B", "紅樓夢", "zh-TW", None);
        b.author = None;
        let r = merge(vec![a, b], &[]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].others, ["B"]);
    }

    #[test]
    fn variants_and_names() {
        assert_eq!(keyword_variants("紅樓夢"), ["紅樓夢", "红楼梦"]);
        assert_eq!(safe_name("a/b:c"), "a_b_c");
    }
}
