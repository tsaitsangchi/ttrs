//! Web 密碼與帳號稽核原則（TipTop zx10、zx16、zx17、zx19、zx20 / gbt_file；migration 0020，ADR 0009）

use async_trait::async_trait;
use chrono::NaiveDate;
use ttrs_app::{
    error::RepoError,
    ports::{AccountPolicyRepository, CredentialRepository},
};
use ttrs_domain::{
    password::{AccountPolicy, Complexity, Credential, ExpiryBasis},
    user::UserId,
};

use crate::{PgStore, pg::db_err};

#[derive(sqlx::FromRow)]
struct CredentialRow {
    password_hash: Option<String>,
    password_changed_on: Option<NaiveDate>,
    password_use_count: i32,
    password_change_required: bool,
    failed_sign_in_count: i32,
}

fn count(v: i32) -> u32 {
    u32::try_from(v).unwrap_or(0)
}

fn to_db(v: u32) -> i32 {
    i32::try_from(v).unwrap_or(i32::MAX)
}

#[async_trait]
impl CredentialRepository for PgStore {
    async fn credential(&self, user: &UserId) -> Result<Option<Credential>, RepoError> {
        let row: Option<CredentialRow> = sqlx::query_as(
            "SELECT password_hash, password_changed_on, password_use_count,
                    password_change_required, failed_sign_in_count
               FROM user_credential WHERE user_id = $1",
        )
        .bind(user.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(row.map(|r| Credential {
            password_hash: r.password_hash,
            changed_on: r.password_changed_on,
            use_count: count(r.password_use_count),
            change_required: r.password_change_required,
            failed_count: count(r.failed_sign_in_count),
        }))
    }

    async fn save_credential(&self, user: &UserId, c: &Credential) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO user_credential (user_id, password_hash, password_changed_on,
                 password_use_count, password_change_required, failed_sign_in_count)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (user_id) DO UPDATE SET
                 password_hash = EXCLUDED.password_hash,
                 password_changed_on = EXCLUDED.password_changed_on,
                 password_use_count = EXCLUDED.password_use_count,
                 password_change_required = EXCLUDED.password_change_required,
                 failed_sign_in_count = EXCLUDED.failed_sign_in_count",
        )
        .bind(user.as_str())
        .bind(&c.password_hash)
        .bind(c.changed_on)
        .bind(to_db(c.use_count))
        .bind(c.change_required)
        .bind(to_db(c.failed_count))
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct PolicyRow {
    expiry_basis: String,
    cycle_days: i32,
    use_limit: i32,
    notify: i32,
    min_length: i32,
    complexity: String,
    allow_same_as_old: bool,
    max_failures: i32,
}

#[async_trait]
impl AccountPolicyRepository for PgStore {
    async fn account_policy(&self) -> Result<AccountPolicy, RepoError> {
        let row: Option<PolicyRow> = sqlx::query_as(
            "SELECT expiry_basis, cycle_days, use_limit, notify, min_length, complexity,
                    allow_same_as_old, max_failures
               FROM account_policy WHERE policy_key = '0'",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        let Some(r) = row else {
            return Ok(AccountPolicy::default());
        };
        Ok(AccountPolicy {
            expiry_basis: match r.expiry_basis.as_str() {
                "1" => ExpiryBasis::Days,
                "2" => ExpiryBasis::Uses,
                _ => ExpiryBasis::Never,
            },
            cycle_days: count(r.cycle_days),
            use_limit: count(r.use_limit),
            notify: count(r.notify),
            min_length: count(r.min_length),
            complexity: Complexity::from_code(&r.complexity).unwrap_or(Complexity::None),
            allow_same_as_old: r.allow_same_as_old,
            max_failures: count(r.max_failures),
        })
    }
}
