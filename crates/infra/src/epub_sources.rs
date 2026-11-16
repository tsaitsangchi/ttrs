//! EPUB 多來源搜尋的來源（epub_web_search.feature WS2）：Internet Archive、Project Gutenberg（Gutendex）、
//! Wikisource（ws-export 產生 EPUB）、GitHub。都是免費公開 API、不需金鑰；呼叫時帶通用的 User-Agent，不帶使用者資料。

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use ttrs_app::ports::{EpubBook, EpubBookKind, EpubSource, GithubSource};
use ttrs_domain::DomainError;

use crate::github::{GithubHttp, client};

fn unavailable(name: &str) -> DomainError {
    DomainError::EpubSourceUnavailable(name.to_owned())
}

/// 網址的路徑／查詢字串編碼
fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn get_json(name: &str, url: &str) -> Result<Value, DomainError> {
    let res = client()?
        .get(url)
        .send()
        .await
        .map_err(|_| unavailable(name))?;
    if !res.status().is_success() {
        return Err(unavailable(name));
    }
    res.json().await.map_err(|_| unavailable(name))
}

/// 每個關鍵字寫法同時查，各自最多等 8 秒；慢的或失敗的寫法略過，全部都失敗才算來源失敗
/// （Gutendex 搜尋簡體字偶爾要等數十秒，不該拖累繁體的結果）
async fn per_keyword<F, Fut>(
    name: &str,
    keywords: &[String],
    one: F,
) -> Result<Vec<EpubBook>, DomainError>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<EpubBook>, DomainError>> + Send + 'static,
{
    let handles: Vec<_> = keywords
        .iter()
        .map(|k| {
            let fut = one(k.clone());
            tokio::spawn(async move {
                tokio::time::timeout(std::time::Duration::from_secs(8), fut).await
            })
        })
        .collect();
    let (mut out, mut ok) = (Vec::new(), false);
    for h in handles {
        if let Ok(Ok(Ok(books))) = h.await {
            ok = true;
            out.extend(books);
        }
    }
    if ok { Ok(out) } else { Err(unavailable(name)) }
}

/// 下載到記憶體，超過 `max_bytes` 就中止（先看 Content-Length，再邊收邊算）
async fn download_limited(name: &str, url: &str, max_bytes: u64) -> Result<Vec<u8>, DomainError> {
    let too_large = || DomainError::EpubTooLarge {
        max_mb: usize::try_from(max_bytes / 1024 / 1024).unwrap_or(0),
    };
    let mut res = client()?
        .get(url)
        .send()
        .await
        .map_err(|_| unavailable(name))?;
    if res.status().as_u16() == 404 {
        return Err(DomainError::EpubBookNotFound(name.to_owned()));
    }
    if !res.status().is_success() {
        return Err(unavailable(name));
    }
    if res.content_length().is_some_and(|n| n > max_bytes) {
        return Err(too_large());
    }
    let mut out = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(|_| unavailable(name))? {
        if out.len() as u64 + chunk.len() as u64 > max_bytes {
            return Err(too_large());
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Array(a) => a.first().and_then(text),
        _ => None,
    }
}

// ----- Internet Archive -----

pub struct InternetArchive;

const IA: &str = "Internet Archive";

#[async_trait]
impl EpubSource for InternetArchive {
    fn name(&self) -> String {
        IA.to_owned()
    }

    async fn search(
        &self,
        keywords: &[String],
        open_only: bool,
        limit: usize,
    ) -> Result<Vec<EpubBook>, DomainError> {
        let terms = keywords
            .iter()
            .map(|k| format!("\"{}\"", k.replace('"', " ")))
            .collect::<Vec<_>>()
            .join(" OR ");
        let mut q =
            format!("mediatype:texts AND format:EPUB AND (title:({terms}) OR creator:({terms}))");
        if open_only {
            // 只收公共領域與 Creative Commons：Internet Archive 任何人都能上傳，沒有授權資料的多半版權不明
            q.push_str(" AND (licenseurl:*creativecommons* OR licenseurl:*publicdomain*)");
        }
        let url = format!(
            "https://archive.org/advancedsearch.php?q={}&fl%5B%5D=identifier&fl%5B%5D=title&fl%5B%5D=creator\
             &fl%5B%5D=language&fl%5B%5D=downloads&fl%5B%5D=licenseurl&sort%5B%5D=downloads+desc&rows={limit}&output=json",
            enc(&q)
        );
        let v = get_json(IA, &url).await?;
        Ok(v["response"]["docs"]
            .as_array()
            .map(|docs| {
                docs.iter()
                    .filter_map(|d| {
                        let id = d["identifier"].as_str()?.to_owned();
                        Some(EpubBook {
                            source: IA.to_owned(),
                            url: Some(format!("https://archive.org/details/{id}")),
                            title: text(&d["title"])?,
                            author: text(&d["creator"]),
                            language: text(&d["language"]),
                            license: text(&d["licenseurl"]),
                            downloads: d["downloads"].as_u64(),
                            kind: EpubBookKind::Book,
                            id,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn download(&self, id: &str, max_bytes: u64) -> Result<Vec<u8>, DomainError> {
        let meta = get_json(IA, &format!("https://archive.org/metadata/{}", enc(id))).await?;
        let file = meta["files"]
            .as_array()
            .and_then(|files| {
                files
                    .iter()
                    .filter(|f| {
                        f["format"].as_str() == Some("EPUB")
                            || f["name"]
                                .as_str()
                                .is_some_and(|n| n.to_lowercase().ends_with(".epub"))
                    })
                    .max_by_key(|f| {
                        f["size"]
                            .as_str()
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(0)
                    })
            })
            .ok_or_else(|| DomainError::EpubBookNotFound(id.to_owned()))?;
        let name = file["name"].as_str().unwrap_or_default();
        let url = format!(
            "https://archive.org/download/{}/{}",
            enc(id),
            name.split('/').map(enc).collect::<Vec<_>>().join("/")
        );
        download_limited(IA, &url, max_bytes).await
    }
}

// ----- Project Gutenberg（Gutendex） -----

pub struct Gutenberg;

const PG: &str = "Gutenberg";

#[async_trait]
impl EpubSource for Gutenberg {
    fn name(&self) -> String {
        PG.to_owned()
    }

    async fn search(
        &self,
        keywords: &[String],
        _open_only: bool,
        limit: usize,
    ) -> Result<Vec<EpubBook>, DomainError> {
        let found = per_keyword(PG, keywords, |k| async move {
            let v = get_json(
                PG,
                &format!("https://gutendex.com/books/?search={}", enc(&k)),
            )
            .await?;
            let mut books = Vec::new();
            for b in v["results"].as_array().into_iter().flatten() {
                let Some(id) = b["id"].as_u64() else { continue };
                // 沒有 EPUB 格式的不收
                if !b["formats"]
                    .as_object()
                    .is_some_and(|f| f.keys().any(|m| m.starts_with("application/epub")))
                {
                    continue;
                }
                let id = id.to_string();
                books.push(EpubBook {
                    source: PG.to_owned(),
                    url: Some(format!("https://www.gutenberg.org/ebooks/{id}")),
                    title: b["title"].as_str().unwrap_or_default().to_owned(),
                    author: b["authors"][0]["name"].as_str().map(str::to_owned),
                    language: text(&b["languages"]),
                    // Gutendex 的 copyright=false 表示在美國為公共領域
                    license: match b["copyright"].as_bool() {
                        Some(false) => Some("公共領域".to_owned()),
                        Some(true) => Some("版權保留".to_owned()),
                        None => None,
                    },
                    downloads: b["download_count"].as_u64(),
                    kind: EpubBookKind::Book,
                    id,
                });
            }
            Ok(books)
        })
        .await?;
        let mut out: Vec<EpubBook> = Vec::new();
        for b in found {
            if !out.iter().any(|x| x.id == b.id) {
                out.push(b);
            }
        }
        out.truncate(limit);
        Ok(out)
    }

    async fn download(&self, id: &str, max_bytes: u64) -> Result<Vec<u8>, DomainError> {
        download_limited(
            PG,
            &format!("https://www.gutenberg.org/ebooks/{}.epub3.images", enc(id)),
            max_bytes,
        )
        .await
    }
}

// ----- Wikisource（維基文庫） -----

pub struct Wikisource;

const WS: &str = "Wikisource";

#[async_trait]
impl EpubSource for Wikisource {
    fn name(&self) -> String {
        WS.to_owned()
    }

    async fn search(
        &self,
        keywords: &[String],
        _open_only: bool,
        limit: usize,
    ) -> Result<Vec<EpubBook>, DomainError> {
        let found = per_keyword(WS, keywords, |k| async move {
            let v = get_json(
                WS,
                &format!(
                    "https://zh.wikisource.org/w/api.php?action=query&list=search&srsearch={}&srnamespace=0&srlimit=10&format=json",
                    enc(&k)
                ),
            )
            .await?;
            let mut books = Vec::new();
            for r in v["query"]["search"].as_array().into_iter().flatten() {
                let Some(title) = r["title"].as_str() else { continue };
                // 子頁面（章回）不是一本書，略過
                if title.contains('/') || title.contains("消歧義") {
                    continue;
                }
                books.push(EpubBook {
                    source: WS.to_owned(),
                    id: title.to_owned(),
                    title: title.to_owned(),
                    author: None,
                    // 維基文庫的文字不固定繁簡，由書名判斷
                    language: Some("zh".to_owned()),
                    license: Some("公共領域".to_owned()),
                    downloads: None,
                    kind: EpubBookKind::Book,
                    url: Some(format!("https://zh.wikisource.org/wiki/{}", enc(title))),
                });
            }
            Ok(books)
        })
        .await?;
        let mut out: Vec<EpubBook> = Vec::new();
        for b in found {
            if !out.iter().any(|x| x.id == b.id) {
                out.push(b);
            }
        }
        out.truncate(limit);
        Ok(out)
    }

    async fn download(&self, id: &str, max_bytes: u64) -> Result<Vec<u8>, DomainError> {
        download_limited(
            WS,
            &format!(
                "https://ws-export.wmcloud.org/?format=epub&lang=zh&page={}",
                enc(id)
            ),
            max_bytes,
        )
        .await
    }
}

// ----- GitHub（以 repo 為單位，按「列出 EPUB」才看到檔案） -----

pub struct GithubRepos(pub Arc<dyn GithubSource>);

const GH: &str = "GitHub";

#[async_trait]
impl EpubSource for GithubRepos {
    fn name(&self) -> String {
        GH.to_owned()
    }

    async fn search(
        &self,
        keywords: &[String],
        _open_only: bool,
        limit: usize,
    ) -> Result<Vec<EpubBook>, DomainError> {
        let Some(k) = keywords.first() else {
            return Ok(Vec::new());
        };
        let (repos, _) = self
            .0
            .search_repos(
                &format!("{k} epub"),
                1,
                u32::try_from(limit.min(100)).unwrap_or(30),
            )
            .await
            .map_err(|_| unavailable(GH))?;
        Ok(repos
            .into_iter()
            .map(|r| EpubBook {
                source: GH.to_owned(),
                url: Some(format!("https://github.com/{}", r.full_name)),
                title: r.full_name.clone(),
                author: None,
                language: None,
                license: r.license,
                downloads: None,
                kind: EpubBookKind::Repo,
                id: r.full_name,
            })
            .collect())
    }

    async fn download(&self, id: &str, _max_bytes: u64) -> Result<Vec<u8>, DomainError> {
        // repo 要先「列出 EPUB」再選檔案，不能直接下載
        Err(DomainError::EpubBookNotFound(id.to_owned()))
    }
}

/// 預設的來源（API 啟動時使用）
pub fn default_epub_sources() -> Vec<Arc<dyn EpubSource>> {
    vec![
        Arc::new(InternetArchive),
        Arc::new(Gutenberg),
        Arc::new(Wikisource),
        Arc::new(GithubRepos(Arc::new(GithubHttp))),
    ]
}
