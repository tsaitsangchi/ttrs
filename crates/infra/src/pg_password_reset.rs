//! 忘記密碼的重設權杖與異動紀錄寫入（password_reset_token、audit_log；migration 0021，ADR 0010）

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ttrs_app::{
    error::RepoError,
    ports::{AuditEntry, AuditLog, PasswordResetTokenRepository, ResetToken},
};
use ttrs_domain::user::UserId;

use crate::{PgStore, pg::db_err};

#[derive(sqlx::FromRow)]
struct TokenRow {
    user_id: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    used_at: Option<DateTime<Utc>>,
}

#[async_trait]
impl PasswordResetTokenRepository for PgStore {
    /// 同一交易內先讓舊權杖失效再新增
    async fn issue_reset_token(
        &self,
        token_hash: &str,
        token: &ResetToken,
    ) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query(
            "UPDATE password_reset_token SET used_at = $2
              WHERE user_id = $1 AND used_at IS NULL",
        )
        .bind(&token.user_id)
        .bind(token.created_at)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sqlx::query(
            "INSERT INTO password_reset_token (token_hash, user_id, created_at, expires_at, used_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(token_hash)
        .bind(&token.user_id)
        .bind(token.created_at)
        .bind(token.expires_at)
        .bind(token.used_at)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn reset_token(&self, token_hash: &str) -> Result<Option<ResetToken>, RepoError> {
        let row: Option<TokenRow> = sqlx::query_as(
            "SELECT user_id, created_at, expires_at, used_at
               FROM password_reset_token WHERE token_hash = $1",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(|r| ResetToken {
            user_id: r.user_id,
            created_at: r.created_at,
            expires_at: r.expires_at,
            used_at: r.used_at,
        }))
    }

    async fn reset_requests_since(
        &self,
        user: &UserId,
        since: DateTime<Utc>,
    ) -> Result<u64, RepoError> {
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM password_reset_token WHERE user_id = $1 AND created_at > $2",
        )
        .bind(user.as_str())
        .bind(since)
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    async fn revoke_reset_tokens(&self, user: &UserId, at: DateTime<Utc>) -> Result<(), RepoError> {
        sqlx::query(
            "UPDATE password_reset_token SET used_at = $2
              WHERE user_id = $1 AND used_at IS NULL",
        )
        .bind(user.as_str())
        .bind(at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }
}

#[async_trait]
impl AuditLog for PgStore {
    async fn record(&self, entry: &AuditEntry) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO audit_log (program, user_id, key, action) VALUES ($1, $2, $3, $4)",
        )
        .bind(&entry.program)
        .bind(&entry.user_id)
        .bind(&entry.key)
        .bind(&entry.action)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }
}
