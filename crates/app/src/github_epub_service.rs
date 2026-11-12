//! GitHub EPUB Search GitHub EPUB 搜尋（TTRS 新增的工具作業，TipTop 無對應）
//! 規格：specs/features/azz/github_epub_search.feature（GE1–GE9）
//!
//! 搜尋 repo、列出 repo 裡的 .epub、一次下載一本轉成 TXT（轉換沿用 [`crate::epub_service::convert`]）；不批次下載、不保存檔案（GE4）。

use std::sync::Arc;

use ttrs_domain::DomainError;
use ttrs_shared::{
    EpubTextDto, GithubEpubFileDto, GithubEpubListDto, GithubRepoDto, GithubRepoPage,
};

use crate::{
    Actor, AppError, Authorizer,
    epub_service::{self, MAX_MB},
    ports::{AuditEntry, AuditLog, GithubRepo, GithubSource},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "github-epub-search";
/// 一頁筆數（GE2）
pub const PER_PAGE: u32 = 30;

/// 開放授權（GE2；GitHub 回傳的 SPDX 代碼）
const OPEN_LICENSES: &[&str] = &[
    "CC0-1.0",
    "CC-BY-4.0",
    "CC-BY-SA-4.0",
    "CC-BY-3.0",
    "CC-BY-SA-3.0",
    "Unlicense",
    "MIT",
    "Apache-2.0",
    "GPL-2.0",
    "GPL-3.0",
    "LGPL-2.1",
    "LGPL-3.0",
    "AGPL-3.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "MPL-2.0",
    "OFL-1.1",
];

pub fn is_open_license(spdx: Option<&str>) -> bool {
    spdx.is_some_and(|s| OPEN_LICENSES.iter().any(|o| o.eq_ignore_ascii_case(s)))
}

#[derive(Clone)]
pub struct GithubEpubService {
    authz: Arc<dyn Authorizer>,
    source: Arc<dyn GithubSource>,
    audit: Arc<dyn AuditLog>,
}

impl GithubEpubService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        source: Arc<dyn GithubSource>,
        audit: Arc<dyn AuditLog>,
    ) -> Self {
        Self {
            authz,
            source,
            audit,
        }
    }

    /// 換掉 GitHub 來源（BDD 對 PostgreSQL 也用記憶體來源，不連外網，GE8）
    pub fn with_source(mut self, source: Arc<dyn GithubSource>) -> Self {
        self.source = source;
        self
    }

    /// 以關鍵字搜尋 repo（GE2）；`open_only` 只留開放授權
    pub async fn search(
        &self,
        actor: &Actor,
        keyword: &str,
        open_only: bool,
        page: u32,
    ) -> Result<GithubRepoPage, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        let keyword = keyword.trim();
        let page = page.max(1);
        let (repos, total) = self.source.search_repos(keyword, page, PER_PAGE).await?;
        self.audit(actor, "query", keyword).await;
        let items = repos
            .into_iter()
            .map(repo_dto)
            .filter(|r| !open_only || r.open_license)
            .collect();
        Ok(GithubRepoPage { items, total, page })
    }

    /// 列出 repo 預設分支裡的 .epub（GE3），依路徑排序
    pub async fn files(&self, actor: &Actor, repo: &str) -> Result<GithubEpubListDto, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        let info = self.source.get_repo(repo.trim()).await?;
        let (files, truncated) = self
            .source
            .tree(&info.full_name, &info.default_branch)
            .await?;
        let mut files: Vec<GithubEpubFileDto> = files
            .into_iter()
            .filter(|f| f.path.to_ascii_lowercase().ends_with(".epub"))
            .map(|f| GithubEpubFileDto {
                html_url: format!(
                    "https://github.com/{}/blob/{}/{}",
                    info.full_name, info.default_branch, f.path
                ),
                path: f.path,
                size: f.size,
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(GithubEpubListDto {
            repo: info.full_name,
            license: info.license,
            files,
            truncated,
        })
    }

    /// 下載一本並轉成 TXT（GE4）：超過 50 MB 不下載
    pub async fn convert(
        &self,
        actor: &Actor,
        repo: &str,
        path: &str,
    ) -> Result<EpubTextDto, AppError> {
        self.authz.authorize(actor, PROGRAM, "run").await?;
        let info = self.source.get_repo(repo.trim()).await?;
        let (files, _) = self
            .source
            .tree(&info.full_name, &info.default_branch)
            .await?;
        let file = files
            .iter()
            .find(|f| f.path == path)
            .ok_or_else(|| DomainError::GithubNotFound(format!("{}/{path}", info.full_name)))?;
        if file.size > u64::try_from(MAX_MB * 1024 * 1024).unwrap_or(u64::MAX) {
            return Err(DomainError::EpubTooLarge { max_mb: MAX_MB }.into());
        }
        let bytes = self
            .source
            .download(&info.full_name, &info.default_branch, path)
            .await?;
        if bytes.len() > MAX_MB * 1024 * 1024 {
            return Err(DomainError::EpubTooLarge { max_mb: MAX_MB }.into());
        }
        let r = epub_service::convert(&bytes)?;
        self.audit(actor, "run", &format!("{}/{path}", info.full_name))
            .await;
        let name = path.rsplit('/').next().unwrap_or(path);
        let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
        Ok(EpubTextDto {
            file_name: format!("{stem}.txt"),
            title: r.title,
            author: r.author,
            chapters: u32::try_from(r.chapters).unwrap_or(u32::MAX),
            text: r.text.replace('\n', "\r\n"),
        })
    }

    /// 稽核紀錄（GE6）
    async fn audit(&self, actor: &Actor, action: &str, key: &str) {
        let _ = self
            .audit
            .record(&AuditEntry {
                program: PROGRAM.to_owned(),
                user_id: actor.user_id.clone(),
                key: key.to_owned(),
                action: action.to_owned(),
                detail: None,
            })
            .await;
    }
}

fn repo_dto(r: GithubRepo) -> GithubRepoDto {
    GithubRepoDto {
        html_url: format!("https://github.com/{}", r.full_name),
        open_license: is_open_license(r.license.as_deref()),
        full_name: r.full_name,
        description: r.description,
        license: r.license,
        stars: r.stars,
        updated_at: r.updated_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_licenses() {
        assert!(is_open_license(Some("CC0-1.0")));
        assert!(is_open_license(Some("mit")));
        assert!(!is_open_license(Some("NOASSERTION")));
        assert!(!is_open_license(None));
    }
}
