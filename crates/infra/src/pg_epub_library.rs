//! 我的 GitHub 書庫（epub_github_library；epub_github_library.feature，migration 0055）：PostgreSQL 實作

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use ttrs_app::{
    error::RepoError,
    ports::{EpubLibraryEntry, EpubLibraryRepository},
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct Row {
    user_id: String,
    repo: String,
    created_at: DateTime<Utc>,
    version: i32,
}

#[async_trait]
impl EpubLibraryRepository for PgStore {
    async fn list_epub_library(&self, user_id: &str) -> Result<Vec<EpubLibraryEntry>, RepoError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT user_id, repo, created_at, version FROM epub_github_library
              WHERE user_id = $1 ORDER BY created_at, repo",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|r| EpubLibraryEntry {
                user_id: r.user_id,
                repo: r.repo,
                created_at: r.created_at,
                version: r.version,
            })
            .collect())
    }

    async fn insert_epub_library(&self, user_id: &str, repo: &str) -> Result<(), RepoError> {
        sqlx::query("INSERT INTO epub_github_library (user_id, repo) VALUES ($1, $2)")
            .bind(user_id)
            .bind(repo)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn delete_epub_library(
        &self,
        user_id: &str,
        repo: &str,
        version: i32,
    ) -> Result<bool, RepoError> {
        let n = sqlx::query(
            "DELETE FROM epub_github_library WHERE user_id = $1 AND repo = $2 AND version = $3",
        )
        .bind(user_id)
        .bind(repo)
        .bind(version)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        if n > 0 {
            return Ok(true);
        }
        let exists: Option<i32> = sqlx::query_scalar(
            "SELECT version FROM epub_github_library WHERE user_id = $1 AND repo = $2",
        )
        .bind(user_id)
        .bind(repo)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        match exists {
            Some(_) => Err(RepoError::Stale),
            None => Ok(false),
        }
    }
}
