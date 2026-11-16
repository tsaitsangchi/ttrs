//! GitHub 來源（github_epub_search.feature GE5、GE9）：以 HTTPS 呼叫 GitHub REST API。
//! token 放 .env 的 TTRS_GITHUB_TOKEN；沒有設定時匿名呼叫（每小時 60 次）。User-Agent 用通用名稱，不帶使用者資料。

use async_trait::async_trait;
use chrono::{Local, TimeZone};
use serde_json::Value;
use ttrs_app::ports::{GithubFile, GithubRepo, GithubSource};
use ttrs_domain::DomainError;

use crate::PgStore;

const API: &str = "https://api.github.com";
const RAW: &str = "https://raw.githubusercontent.com";

fn unavailable(e: impl ToString) -> DomainError {
    DomainError::GithubUnavailable(e.to_string())
}

pub(crate) fn client() -> Result<reqwest::Client, DomainError> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .user_agent("TTRS")
        .build()
        .map_err(unavailable)
}

fn token() -> Option<String> {
    std::env::var("TTRS_GITHUB_TOKEN")
        .ok()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// 送出請求；403／429 且剩餘次數為 0 時回 GITHUB_RATE_LIMITED（附可再查詢的時間）、404 回 GITHUB_NOT_FOUND
async fn get(url: &str, what: &str) -> Result<reqwest::Response, DomainError> {
    let mut req = client()?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(t) = token() {
        req = req.bearer_auth(t);
    }
    let res = req.send().await.map_err(unavailable)?;
    let status = res.status().as_u16();
    if status == 403 || status == 429 {
        let h = |k: &str| {
            res.headers()
                .get(k)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        if h("x-ratelimit-remaining").as_deref() == Some("0") || status == 429 {
            let reset = h("x-ratelimit-reset")
                .and_then(|v| v.parse::<i64>().ok())
                .and_then(|s| Local.timestamp_opt(s, 0).single())
                .map_or_else(
                    || "稍後".to_owned(),
                    |t| t.format("%Y-%m-%d %H:%M").to_string(),
                );
            return Err(DomainError::GithubRateLimited { reset });
        }
    }
    if status == 404 {
        return Err(DomainError::GithubNotFound(what.to_owned()));
    }
    if !res.status().is_success() {
        return Err(unavailable(format!("HTTP {status}")));
    }
    Ok(res)
}

async fn get_json(url: &str, what: &str) -> Result<Value, DomainError> {
    get(url, what).await?.json().await.map_err(unavailable)
}

fn repo_of(v: &Value) -> Option<GithubRepo> {
    Some(GithubRepo {
        full_name: v["full_name"].as_str()?.to_owned(),
        description: v["description"].as_str().map(str::to_owned),
        license: v["license"]["spdx_id"]
            .as_str()
            .filter(|s| *s != "NOASSERTION")
            .map(str::to_owned),
        stars: u32::try_from(v["stargazers_count"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX),
        updated_at: v["pushed_at"].as_str().map(str::to_owned),
        default_branch: v["default_branch"].as_str().unwrap_or("main").to_owned(),
    })
}

/// 網址的路徑段編碼（保留 /）
fn encode_path(p: &str) -> String {
    p.split('/')
        .map(|seg| {
            seg.bytes()
                .map(|b| match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        (b as char).to_string()
                    }
                    _ => format!("%{b:02X}"),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// 只呼叫 GitHub、不碰資料庫的來源（多來源 EPUB 搜尋用）；[`PgStore`] 的實作也委派給它
#[derive(Debug, Clone, Copy, Default)]
pub struct GithubHttp;

#[async_trait]
impl GithubSource for PgStore {
    async fn search_repos(
        &self,
        keyword: &str,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<GithubRepo>, u64), DomainError> {
        GithubHttp.search_repos(keyword, page, per_page).await
    }

    async fn get_repo(&self, full_name: &str) -> Result<GithubRepo, DomainError> {
        GithubHttp.get_repo(full_name).await
    }

    async fn tree(
        &self,
        full_name: &str,
        branch: &str,
    ) -> Result<(Vec<GithubFile>, bool), DomainError> {
        GithubHttp.tree(full_name, branch).await
    }

    async fn download(
        &self,
        full_name: &str,
        branch: &str,
        path: &str,
    ) -> Result<Vec<u8>, DomainError> {
        GithubHttp.download(full_name, branch, path).await
    }
}

#[async_trait]
impl GithubSource for GithubHttp {
    async fn search_repos(
        &self,
        keyword: &str,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<GithubRepo>, u64), DomainError> {
        let url = reqwest::Url::parse_with_params(
            &format!("{API}/search/repositories"),
            &[
                ("q", keyword.to_owned()),
                ("sort", "stars".to_owned()),
                ("per_page", per_page.to_string()),
                ("page", page.to_string()),
            ],
        )
        .map_err(unavailable)?;
        let v = get_json(url.as_str(), keyword).await?;
        let items = v["items"]
            .as_array()
            .map(|a| a.iter().filter_map(repo_of).collect())
            .unwrap_or_default();
        Ok((items, v["total_count"].as_u64().unwrap_or(0)))
    }

    async fn get_repo(&self, full_name: &str) -> Result<GithubRepo, DomainError> {
        let v = get_json(
            &format!("{API}/repos/{}", encode_path(full_name)),
            full_name,
        )
        .await?;
        repo_of(&v).ok_or_else(|| DomainError::GithubNotFound(full_name.to_owned()))
    }

    async fn tree(
        &self,
        full_name: &str,
        branch: &str,
    ) -> Result<(Vec<GithubFile>, bool), DomainError> {
        let v = get_json(
            &format!(
                "{API}/repos/{}/git/trees/{}?recursive=1",
                encode_path(full_name),
                encode_path(branch)
            ),
            full_name,
        )
        .await?;
        let files = v["tree"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|e| e["type"].as_str() == Some("blob"))
                    .filter_map(|e| {
                        Some(GithubFile {
                            path: e["path"].as_str()?.to_owned(),
                            size: e["size"].as_u64().unwrap_or(0),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok((files, v["truncated"].as_bool().unwrap_or(false)))
    }

    async fn download(
        &self,
        full_name: &str,
        branch: &str,
        path: &str,
    ) -> Result<Vec<u8>, DomainError> {
        let url = format!(
            "{RAW}/{}/{}/{}",
            encode_path(full_name),
            encode_path(branch),
            encode_path(path)
        );
        let res = get(&url, &format!("{full_name}/{path}")).await?;
        Ok(res.bytes().await.map_err(unavailable)?.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_paths() {
        assert_eq!(encode_path("a b/c#.epub"), "a%20b/c%23.epub");
        assert_eq!(encode_path("中/x.epub"), "%E4%B8%AD/x.epub");
    }
}
