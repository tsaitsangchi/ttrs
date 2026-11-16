//! 多來源 EPUB 搜尋的 GitHub 來源（epub_web_search.feature WS2(d)、epub_github_filename.feature GF1–GF6）。
//! 只依賴 [`GithubSource`]：API 用 infra 的 HTTPS 實作，BDD 用記憶體假 GitHub。
//! - [`GithubRepos`]：搜 repo 名稱與說明（按「列出 EPUB」才看到檔案）。
//! - [`GithubFiles`]：有 token 時依檔名搜 EPUB 檔，每個檔案列成一本書；GitHub 只收錄部分檔案。
//!
//! 兩者名稱都是「GitHub」：失敗時畫面只提示一次，下載時服務依序嘗試。

use std::sync::Arc;

use async_trait::async_trait;
use ttrs_domain::DomainError;

use crate::ports::{EpubBook, EpubBookKind, EpubSource, GithubSource};

const GH: &str = "GitHub";
/// 一次搜尋最多查幾個 repo 的授權（每個 repo 一次 API 呼叫）
const LICENSE_LOOKUPS: usize = 10;
/// 提示代碼：未設定 token，GitHub 只比對 repo 名稱與說明（GF2）
pub const NO_TOKEN: &str = "GITHUB_NO_TOKEN";

fn unavailable() -> DomainError {
    DomainError::EpubSourceUnavailable(GH.to_owned())
}

/// repo 搜尋（WS2(d)）
pub struct GithubRepos(pub Arc<dyn GithubSource>);

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
            .map_err(|_| unavailable())?;
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

/// 依檔名搜尋（GF1）：識別碼為「擁有者/repo@分支或 commit:路徑」
pub struct GithubFiles(pub Arc<dyn GithubSource>);

/// 組成檔案識別碼
pub fn file_id(full_name: &str, git_ref: &str, path: &str) -> String {
    format!("{full_name}@{git_ref}:{path}")
}

/// 拆開檔案識別碼：(repo, 分支或 commit, 路徑)
fn parse_id(id: &str) -> Option<(&str, &str, &str)> {
    let (repo, rest) = id.split_once('@')?;
    let (git_ref, path) = rest.split_once(':')?;
    (repo.contains('/') && !git_ref.is_empty() && !path.is_empty()).then_some((repo, git_ref, path))
}

/// 書名：檔名去掉 .epub
fn title_of(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let lower = name.to_lowercase();
    if lower.ends_with(".epub") {
        name[..name.len() - 5].to_owned()
    } else {
        name.to_owned()
    }
}

#[async_trait]
impl EpubSource for GithubFiles {
    fn name(&self) -> String {
        GH.to_owned()
    }

    fn notice(&self) -> Option<String> {
        (!self.0.has_token()).then(|| NO_TOKEN.to_owned())
    }

    async fn search(
        &self,
        keywords: &[String],
        _open_only: bool,
        limit: usize,
    ) -> Result<Vec<EpubBook>, DomainError> {
        if !self.0.has_token() {
            return Ok(Vec::new());
        }
        let mut hits = Vec::new();
        for k in keywords {
            let found = self
                .0
                .search_epub_files(k)
                .await
                .map_err(|_| unavailable())?;
            for h in found {
                if !hits.contains(&h) {
                    hits.push(h);
                }
            }
        }
        hits.truncate(limit);
        // 授權取 repo 的授權（GF1）；查不到就「未標示」
        let mut licenses: Vec<(String, Option<String>)> = Vec::new();
        for h in &hits {
            if licenses.iter().any(|(r, _)| *r == h.full_name) {
                continue;
            }
            let license = if licenses.len() < LICENSE_LOOKUPS {
                self.0
                    .get_repo(&h.full_name)
                    .await
                    .ok()
                    .and_then(|r| r.license)
            } else {
                None
            };
            licenses.push((h.full_name.clone(), license));
        }
        Ok(hits
            .into_iter()
            .map(|h| EpubBook {
                source: GH.to_owned(),
                id: file_id(&h.full_name, &h.git_ref, &h.path),
                title: title_of(&h.path),
                author: None,
                language: None,
                license: licenses
                    .iter()
                    .find(|(r, _)| *r == h.full_name)
                    .and_then(|(_, l)| l.clone()),
                downloads: None,
                kind: EpubBookKind::Book,
                url: Some(format!(
                    "https://github.com/{}/blob/{}/{}",
                    h.full_name, h.git_ref, h.path
                )),
            })
            .collect())
    }

    async fn download(&self, id: &str, max_bytes: u64) -> Result<Vec<u8>, DomainError> {
        let (repo, git_ref, path) =
            parse_id(id).ok_or_else(|| DomainError::EpubBookNotFound(id.to_owned()))?;
        let bytes = self.0.download(repo, git_ref, path).await?;
        if bytes.len() as u64 > max_bytes {
            return Err(DomainError::EpubTooLarge {
                max_mb: usize::try_from(max_bytes / 1024 / 1024).unwrap_or(0),
            });
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_titles() {
        let id = file_id("o/r", "abc", "d/庆余年(精校版）.epub");
        assert_eq!(
            parse_id(&id),
            Some(("o/r", "abc", "d/庆余年(精校版）.epub"))
        );
        assert_eq!(parse_id("o/r"), None);
        assert_eq!(title_of("d/庆余年(精校版）.EPUB"), "庆余年(精校版）");
        assert_eq!(title_of("a.txt"), "a.txt");
    }
}
