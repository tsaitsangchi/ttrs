//! 以網址下載 EPUB（TTRS 新增的工具作業，TipTop 無對應）
//! 規格：specs/features/azz/epub_url_download.feature（UD1–UD8）
//!
//! 使用者貼一個網址：是 EPUB 檔就視為一本書；是網頁就只讀那一頁、列出上面副檔名為 .epub 的連結（不往下爬）。
//! 由伺服器代為連線，所以必須防止被拿去連內部網路（SSRF，UD4）：只允許 http／https，網域解析後的每個位址都必須是公開位址，
//! 轉址最多 5 次且每次都重新檢查，連線時固定連到已檢查過的位址。系統不自己搜尋、爬站，也不繞過登入、付費牆或 DRM。

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::Arc,
};

use ttrs_domain::DomainError;
use ttrs_shared::{EpubUrlDto, EpubUrlLinkDto};
use url::{Host, Url};

use crate::{
    Actor, AppError, Authorizer,
    epub_pdf::{PdfFonts, SystemFonts},
    epub_service::{self, ConvertOptions, EpubOutput, MAX_MB},
    ports::{AuditEntry, AuditLog, HttpHop, WebClient},
};

/// 權限檢查用的程式代號（沿用 EPUB 搜尋）
pub const PROGRAM: &str = "github-epub-search";
/// 轉址次數上限（UD4）
const MAX_REDIRECTS: usize = 5;
/// 網頁最大讀取量（UD2）
const PAGE_MAX_BYTES: usize = 2 * 1024 * 1024;
/// 一個網頁最多列出幾個 EPUB 連結（UD2）
const MAX_LINKS: usize = 100;
/// robots.txt 最大讀取量
const ROBOTS_MAX_BYTES: u64 = 512 * 1024;

fn ipv4_public(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || o[0] == 0
        // 100.64.0.0/10（共享位址）、198.18.0.0/15（測試）、192.0.0.0/24（IETF 保留）、240.0.0.0/4（保留）
        || (o[0] == 100 && (64..=127).contains(&o[1]))
        || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)
        || o[0] >= 240)
}

fn ipv6_public(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return ipv4_public(v4);
    }
    let s = ip.segments();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        // fc00::/7 唯一本地、fe80::/10 連結本地、2001:db8::/32 文件用
        || (s[0] & 0xfe00) == 0xfc00
        || (s[0] & 0xffc0) == 0xfe80
        || (s[0] == 0x2001 && s[1] == 0x0db8))
}

/// 公開位址才可連線（UD4）：本機、私有、連結本地（含雲端中繼資料 169.254.*）、保留位址都不行
pub fn ip_is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => ipv4_public(v4),
        IpAddr::V6(v6) => ipv6_public(v6),
    }
}

/// 網址的檔名主體（最後一段路徑、去副檔名、百分比解碼），沒有就用 "book"
fn file_stem(url: &Url) -> String {
    let last = url
        .path_segments()
        .and_then(|mut s| s.next_back())
        .unwrap_or_default();
    let decoded = percent_decode(last);
    let stem = decoded
        .rsplit_once('.')
        .map_or(decoded.as_str(), |(s, _)| s)
        .trim();
    if stem.is_empty() {
        "book".to_owned()
    } else {
        stem.to_owned()
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = if b[i] == b'%' && i + 2 < b.len() {
            std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        } else {
            None
        };
        match hex {
            Some(v) => {
                out.push(v);
                i += 3;
            }
            None => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn is_epub_path(url: &Url) -> bool {
    url.path().to_lowercase().ends_with(".epub")
}

/// 稽核用的網址：只留網域與路徑，不含查詢字串與片段（UD6）
fn audit_target(url: &Url) -> String {
    format!("{}{}", url.host_str().unwrap_or_default(), url.path())
}

/// 找出頁面上的 EPUB 連結（UD2）：`href` 屬性、相對網址依頁面網址補成完整、副檔名 .epub（不分大小寫）、去重、最多 100 個
fn epub_links(html: &str, base: &Url) -> Vec<EpubUrlLinkDto> {
    let lower = html.to_lowercase();
    let mut out: Vec<EpubUrlLinkDto> = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find("href") {
        let start = from + at + 4;
        from = start;
        let rest = html[start..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let (quote, body) = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => (Some(q), &rest[1..]),
            _ => (None, rest),
        };
        let end = match quote {
            Some(q) => body.find(q),
            None => body.find(|c: char| c.is_whitespace() || c == '>'),
        };
        let Some(end) = end else { continue };
        let href = body[..end].trim();
        // HTML 字元參照最常見的是 &amp;
        let href = href.replace("&amp;", "&");
        let Ok(url) = base.join(&href) else { continue };
        if !matches!(url.scheme(), "http" | "https") || !is_epub_path(&url) {
            continue;
        }
        let name = percent_decode(
            url.path_segments()
                .and_then(|mut s| s.next_back())
                .unwrap_or_default(),
        );
        if out.iter().all(|l| l.url != url.as_str()) {
            out.push(EpubUrlLinkDto {
                name,
                url: url.to_string(),
            });
        }
        if out.len() >= MAX_LINKS {
            break;
        }
    }
    out
}

/// robots.txt 是否允許我們（User-agent: * 或 TTRS）讀取 `path`；取最長符合的規則，同長度時 Allow 優先
fn robots_allows(robots: &str, path: &str) -> bool {
    let mut applies = false;
    let mut in_group_header = false;
    let mut best: Option<(usize, bool)> = None;
    for line in robots.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (key, value) = (key.trim().to_lowercase(), value.trim());
        match key.as_str() {
            "user-agent" => {
                let agent = value.to_lowercase();
                let hit = agent == "*" || agent == "ttrs";
                if in_group_header {
                    applies |= hit;
                } else {
                    applies = hit;
                }
                in_group_header = true;
            }
            "allow" | "disallow" => {
                in_group_header = false;
                if !applies || value.is_empty() {
                    continue;
                }
                if path.starts_with(value) {
                    let allow = key == "allow";
                    let len = value.len();
                    if best.is_none_or(|(l, a)| len > l || (len == l && allow && !a)) {
                        best = Some((len, allow));
                    }
                }
            }
            _ => in_group_header = false,
        }
    }
    best.is_none_or(|(_, allow)| allow)
}

#[derive(Clone)]
pub struct EpubUrlService {
    authz: Arc<dyn Authorizer>,
    audit: Arc<dyn AuditLog>,
    web: Arc<dyn WebClient>,
    fonts: Arc<dyn PdfFonts>,
}

impl EpubUrlService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        audit: Arc<dyn AuditLog>,
        web: Arc<dyn WebClient>,
    ) -> Self {
        Self {
            authz,
            audit,
            web,
            fonts: Arc::new(SystemFonts),
        }
    }

    pub fn with_web(mut self, web: Arc<dyn WebClient>) -> Self {
        self.web = web;
        self
    }

    pub fn with_fonts(mut self, fonts: Arc<dyn PdfFonts>) -> Self {
        self.fonts = fonts;
        self
    }

    /// 檢查網址並取得內容（UD4）：只允許 http／https；網域解析後每個位址都必須是公開位址；
    /// 轉址最多 5 次，每次都重新檢查；連線固定到檢查過的位址
    async fn get_checked(&self, raw: &str, max_bytes: u64) -> Result<(Url, HttpHop), DomainError> {
        let mut current = Url::parse(raw.trim()).map_err(|_| DomainError::UrlNotAllowed)?;
        for _ in 0..=MAX_REDIRECTS {
            let ip = self.checked_ip(&current).await?;
            let hop = self.web.get(current.as_str(), ip, max_bytes).await?;
            match hop.status {
                301 | 302 | 303 | 307 | 308 => {
                    let next = hop
                        .location
                        .as_deref()
                        .and_then(|l| current.join(l).ok())
                        .ok_or(DomainError::UrlUnreachable)?;
                    current = next;
                }
                200..=299 => return Ok((current, hop)),
                _ => return Err(DomainError::UrlUnreachable),
            }
        }
        // 轉址次數超過上限
        Err(DomainError::UrlUnreachable)
    }

    /// 這個網址要連的位址：不是 http／https、或有任何一個位址不是公開位址就拒絕
    async fn checked_ip(&self, url: &Url) -> Result<IpAddr, DomainError> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(DomainError::UrlNotAllowed);
        }
        let ips: Vec<IpAddr> = match url.host().ok_or(DomainError::UrlNotAllowed)? {
            Host::Ipv4(ip) => vec![IpAddr::V4(ip)],
            Host::Ipv6(ip) => vec![IpAddr::V6(ip)],
            Host::Domain(d) => {
                // localhost 一律拒絕，不必等解析
                if d.eq_ignore_ascii_case("localhost") || d.to_lowercase().ends_with(".localhost") {
                    return Err(DomainError::UrlNotAllowed);
                }
                self.web.resolve(d).await?
            }
        };
        if ips.is_empty() {
            return Err(DomainError::UrlUnreachable);
        }
        if ips.iter().any(|ip| !ip_is_public(*ip)) {
            return Err(DomainError::UrlNotAllowed);
        }
        Ok(ips[0])
    }

    /// 讀網頁前先看 robots.txt（UD5）；讀不到 robots.txt 視為允許
    async fn check_robots(&self, url: &Url) -> Result<(), DomainError> {
        let Ok(robots_url) = url.join("/robots.txt") else {
            return Ok(());
        };
        match self
            .get_checked(robots_url.as_str(), ROBOTS_MAX_BYTES)
            .await
        {
            Ok((_, hop)) => {
                let text = String::from_utf8_lossy(&hop.body);
                let mut path = url.path().to_owned();
                if let Some(q) = url.query() {
                    path.push('?');
                    path.push_str(q);
                }
                if robots_allows(&text, &path) {
                    Ok(())
                } else {
                    Err(DomainError::UrlRobotsDisallowed)
                }
            }
            // 內部位址、轉址到內部位址的錯誤要往外傳；其他（沒有 robots.txt）視為允許
            Err(DomainError::UrlNotAllowed) => Err(DomainError::UrlNotAllowed),
            Err(_) => Ok(()),
        }
    }

    /// 讀取網址（UD1、UD2）：EPUB 檔回一本書，網頁回頁面上的 EPUB 連結
    pub async fn read(&self, actor: &Actor, raw: &str) -> Result<EpubUrlDto, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        let url = Url::parse(raw.trim()).map_err(|_| DomainError::UrlNotAllowed)?;
        // 直接指向 .epub 的連結不檢查 robots.txt；其他網址先檢查
        if !is_epub_path(&url) {
            self.check_robots(&url).await?;
        }
        let max = u64::try_from(MAX_MB * 1024 * 1024).unwrap_or(u64::MAX);
        let (final_url, hop) = self.get_checked(url.as_str(), max).await?;
        let size = hop.body.len();
        self.audit_target(actor, "query", &final_url, size).await;
        // EPUB 是 zip：以檔頭判斷，不信任副檔名或 content-type
        if hop.body.starts_with(b"PK") {
            let book = epub_service::convert(&hop.body)?;
            return Ok(EpubUrlDto {
                kind: "book".to_owned(),
                url: final_url.to_string(),
                title: book.title,
                author: book.author,
                links: Vec::new(),
            });
        }
        let is_html = hop
            .content_type
            .as_deref()
            .is_some_and(|c| c.to_lowercase().contains("html"))
            || hop.body.iter().take(512).any(|&b| b == b'<');
        if !is_html || size > PAGE_MAX_BYTES {
            return Err(DomainError::EpubInvalid("網址的內容不是 EPUB 或網頁".to_owned()).into());
        }
        let html = String::from_utf8_lossy(&hop.body);
        Ok(EpubUrlDto {
            kind: "links".to_owned(),
            url: final_url.to_string(),
            title: None,
            author: None,
            links: epub_links(&html, &final_url),
        })
    }

    /// 下載一個網址的 EPUB（UD3）：回（檔名主體, 位元組）；內容不是 EPUB 回 EPUB_INVALID
    pub async fn fetch(&self, actor: &Actor, raw: &str) -> Result<(String, Vec<u8>), AppError> {
        self.authz.authorize(actor, PROGRAM, "run").await?;
        let max = u64::try_from(MAX_MB * 1024 * 1024).unwrap_or(u64::MAX);
        let (final_url, hop) = self.get_checked(raw, max).await?;
        if !hop.body.starts_with(b"PK") {
            return Err(DomainError::EpubInvalid("網址的內容不是 EPUB".to_owned()).into());
        }
        self.audit_target(actor, "run", &final_url, hop.body.len())
            .await;
        Ok((file_stem(&final_url), hop.body))
    }

    /// 下載原始 EPUB
    pub async fn download_raw(
        &self,
        actor: &Actor,
        raw: &str,
    ) -> Result<(String, Vec<u8>), AppError> {
        let (stem, bytes) = self.fetch(actor, raw).await?;
        Ok((format!("{stem}.epub"), bytes))
    }

    /// 把已下載的 EPUB 依選項轉成 TXT 或 PDF
    pub async fn render_bytes(
        &self,
        bytes: &[u8],
        stem: &str,
        opts: ConvertOptions,
    ) -> Result<EpubOutput, AppError> {
        Ok(epub_service::render(bytes, stem, opts, self.fonts.clone()).await?)
    }

    /// 下載並轉成 TXT 或 PDF（UD3）
    pub async fn convert(
        &self,
        actor: &Actor,
        raw: &str,
        opts: ConvertOptions,
    ) -> Result<EpubOutput, AppError> {
        let (stem, bytes) = self.fetch(actor, raw).await?;
        self.render_bytes(&bytes, &stem, opts).await
    }

    /// 稽核紀錄（UD6）：只記網域、路徑與位元組數，不記查詢字串
    async fn audit_target(&self, actor: &Actor, action: &str, url: &Url, bytes: usize) {
        let target = audit_target(url);
        let _ = self
            .audit
            .record(&AuditEntry {
                program: PROGRAM.to_owned(),
                user_id: actor.user_id.clone(),
                key: target.clone(),
                action: action.to_owned(),
                detail: Some(format!("url={target}; bytes={bytes}")),
            })
            .await;
    }
}

/// 沒有設定網路連線時的預設：一律回 URL_UNREACHABLE
#[derive(Debug, Clone, Default)]
pub struct NoWeb;

#[async_trait::async_trait]
impl WebClient for NoWeb {
    async fn resolve(&self, _host: &str) -> Result<Vec<IpAddr>, DomainError> {
        Err(DomainError::UrlUnreachable)
    }

    async fn get(&self, _url: &str, _ip: IpAddr, _max: u64) -> Result<HttpHop, DomainError> {
        Err(DomainError::UrlUnreachable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_addresses() {
        for bad in [
            "127.0.0.1",
            "10.10.130.40",
            "172.16.0.1",
            "192.168.1.5",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(!ip_is_public(bad.parse().unwrap()), "{bad}");
        }
        for good in ["93.184.216.34", "8.8.8.8", "2606:4700::1111"] {
            assert!(ip_is_public(good.parse().unwrap()), "{good}");
        }
    }

    #[test]
    fn extracts_links() {
        let base = Url::parse("https://books.example.com/list").unwrap();
        let html = r#"<a href="/files/a.epub">A</a> <a href='https://cdn.example.com/b.EPUB?x=1'>B</a>
            <a href=/c%20d.epub>C</a> <a href="/about.html">x</a> <a href="/files/a.epub">dup</a>"#;
        let links = epub_links(html, &base);
        let urls: Vec<_> = links.iter().map(|l| l.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://books.example.com/files/a.epub",
                "https://cdn.example.com/b.EPUB?x=1",
                "https://books.example.com/c%20d.epub"
            ]
        );
        assert_eq!(links[2].name, "c d.epub");
    }

    #[test]
    fn robots_rules() {
        assert!(!robots_allows("User-agent: *\nDisallow: /", "/list"));
        assert!(robots_allows("User-agent: *\nDisallow: /private", "/list"));
        assert!(robots_allows(
            "User-agent: *\nDisallow: /\nAllow: /list",
            "/list"
        ));
        assert!(robots_allows("User-agent: bot\nDisallow: /", "/list"));
        assert!(robots_allows("", "/list"));
    }

    #[test]
    fn names() {
        let u = Url::parse("https://x.com/a/%E7%B4%85%E6%A8%93%E5%A4%A2.epub?t=1").unwrap();
        assert_eq!(file_stem(&u), "紅樓夢");
        assert_eq!(audit_target(&u), "x.com/a/%E7%B4%85%E6%A8%93%E5%A4%A2.epub");
    }
}
