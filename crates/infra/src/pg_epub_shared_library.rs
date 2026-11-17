//! 共用 GitHub 書庫（epub_github_shared_library；epub_github_shared_library.feature，migration 0056）：PostgreSQL 實作

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use ttrs_app::{
    error::RepoError,
    ports::{EpubSharedLibraryEntry, EpubSharedLibraryRepository},
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct Row {
    repo: String,
    created_by: String,
    created_dept: String,
    created_at: DateTime<Utc>,
    version: i32,
}

#[async_trait]
impl EpubSharedLibraryRepository for PgStore {
    async fn list_epub_shared_library(&self) -> Result<Vec<EpubSharedLibraryEntry>, RepoError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT repo, created_by, created_dept, created_at, version
               FROM epub_github_shared_library ORDER BY created_at, repo",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(rows
            .into_iter()
            .map(|r| EpubSharedLibraryEntry {
                repo: r.repo,
                created_by: r.created_by,
                created_dept: r.created_dept,
                created_at: r.created_at,
                version: r.version,
            })
            .collect())
    }

    async fn insert_epub_shared_library(
        &self,
        repo: &str,
        created_by: &str,
        created_dept: &str,
    ) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO epub_github_shared_library (repo, created_by, created_dept) VALUES ($1, $2, $3)",
        )
        .bind(repo)
        .bind(created_by)
        .bind(created_dept)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn delete_epub_shared_library(
        &self,
        repo: &str,
        version: i32,
    ) -> Result<bool, RepoError> {
        let n =
            sqlx::query("DELETE FROM epub_github_shared_library WHERE repo = $1 AND version = $2")
                .bind(repo)
                .bind(version)
                .execute(&self.pool)
                .await
                .map_err(db_err)?
                .rows_affected();
        if n > 0 {
            return Ok(true);
        }
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT version FROM epub_github_shared_library WHERE repo = $1")
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
