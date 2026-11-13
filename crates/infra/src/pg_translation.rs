//! 翻譯工作（translation_job；epub_translate.feature，migration 0053）：PostgreSQL 實作

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use ttrs_app::{
    error::RepoError,
    ports::{JobRecord, TranslationJobRepository},
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct Row {
    job_id: String,
    user_id: String,
    source_sha256: String,
    file_name: String,
    format: String,
    status: String,
    total_batches: i32,
    done_batches: i32,
    total_lines: i32,
    done_lines: i32,
    error_code: Option<String>,
    error_message: Option<String>,
    doc: String,
    result: Option<Vec<u8>>,
    result_file_name: Option<String>,
    result_title: Option<String>,
    result_author: Option<String>,
    result_chapters: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl From<Row> for JobRecord {
    fn from(r: Row) -> Self {
        Self {
            job_id: r.job_id,
            user_id: r.user_id,
            source_sha256: r.source_sha256,
            file_name: r.file_name,
            format: r.format,
            status: r.status,
            total_batches: r.total_batches,
            done_batches: r.done_batches,
            total_lines: r.total_lines,
            done_lines: r.done_lines,
            error_code: r.error_code,
            error_message: r.error_message,
            doc: r.doc,
            result: r.result,
            result_file_name: r.result_file_name,
            result_title: r.result_title,
            result_author: r.result_author,
            result_chapters: r.result_chapters,
            created_at: r.created_at,
            updated_at: r.updated_at,
            expires_at: r.expires_at,
        }
    }
}

#[async_trait]
impl TranslationJobRepository for PgStore {
    async fn create(&self, j: &JobRecord) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO translation_job (job_id, user_id, source_sha256, file_name, format, status, \
             total_batches, done_batches, total_lines, done_lines, error_code, error_message, doc, \
             result, result_file_name, result_title, result_author, result_chapters, created_at, \
             updated_at, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)",
        )
        .bind(&j.job_id)
        .bind(&j.user_id)
        .bind(&j.source_sha256)
        .bind(&j.file_name)
        .bind(&j.format)
        .bind(&j.status)
        .bind(j.total_batches)
        .bind(j.done_batches)
        .bind(j.total_lines)
        .bind(j.done_lines)
        .bind(&j.error_code)
        .bind(&j.error_message)
        .bind(&j.doc)
        .bind(&j.result)
        .bind(&j.result_file_name)
        .bind(&j.result_title)
        .bind(&j.result_author)
        .bind(j.result_chapters)
        .bind(j.created_at)
        .bind(j.updated_at)
        .bind(j.expires_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn get(&self, job_id: &str) -> Result<Option<JobRecord>, RepoError> {
        let r: Option<Row> = sqlx::query_as("SELECT * FROM translation_job WHERE job_id = $1")
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(r.map(Into::into))
    }

    async fn save(&self, j: &JobRecord) -> Result<(), RepoError> {
        sqlx::query(
            "UPDATE translation_job SET status = $2, total_batches = $3, done_batches = $4, \
             total_lines = $5, done_lines = $6, error_code = $7, error_message = $8, doc = $9, \
             result = $10, result_file_name = $11, result_title = $12, result_author = $13, \
             result_chapters = $14, format = $15, updated_at = $16, expires_at = $17 \
             WHERE job_id = $1",
        )
        .bind(&j.job_id)
        .bind(&j.status)
        .bind(j.total_batches)
        .bind(j.done_batches)
        .bind(j.total_lines)
        .bind(j.done_lines)
        .bind(&j.error_code)
        .bind(&j.error_message)
        .bind(&j.doc)
        .bind(&j.result)
        .bind(&j.result_file_name)
        .bind(&j.result_title)
        .bind(&j.result_author)
        .bind(j.result_chapters)
        .bind(&j.format)
        .bind(j.updated_at)
        .bind(j.expires_at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn find_resumable(
        &self,
        user_id: &str,
        source_sha256: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<JobRecord>, RepoError> {
        let r: Option<Row> = sqlx::query_as(
            "SELECT * FROM translation_job \
             WHERE user_id = $1 AND source_sha256 = $2 AND status = 'failed' AND expires_at > $3 \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(user_id)
        .bind(source_sha256)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(r.map(Into::into))
    }

    async fn has_active(&self, since: DateTime<Utc>) -> Result<bool, RepoError> {
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM translation_job \
             WHERE status IN ('queued', 'running') AND updated_at > $1",
        )
        .bind(since)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(n > 0)
    }

    async fn purge_expired(&self, now: DateTime<Utc>) -> Result<u64, RepoError> {
        let r = sqlx::query("DELETE FROM translation_job WHERE expires_at <= $1")
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(r.rows_affected())
    }
}
