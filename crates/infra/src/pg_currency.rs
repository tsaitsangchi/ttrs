//! 幣別（currency ← TipTop azi_file；currency_maintenance.feature）。共用 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::CurrencyRepository};
use ttrs_domain::{
    currency::{Currency, CurrencyAttrs, CurrencyCode, RateMethod},
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct CurrencyRow {
    currency_code: String,
    name: Option<String>,
    price_decimals: i16,
    amount_decimals: i16,
    total_decimals: i16,
    rate_decimals: i16,
    rate_method: String,
    active: bool,
    confirmed: bool,
    confirmed_by: Option<String>,
    confirmed_at: Option<DateTime<Utc>>,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

macro_rules! select_currency {
    ($rest:literal) => {
        concat!(
            "SELECT currency_code, name, price_decimals, amount_decimals, total_decimals, rate_decimals, rate_method, ",
            "active, confirmed, confirmed_by, confirmed_at, created_by, created_dept, created_on, ",
            "updated_by, updated_on, version, created_at, updated_at FROM currency ",
            $rest
        )
    };
}

impl TryFrom<CurrencyRow> for Currency {
    type Error = RepoError;

    fn try_from(r: CurrencyRow) -> Result<Self, RepoError> {
        let bad = |v: &str| RepoError::Other(format!("幣別資料不正確：{v}"));
        Ok(Currency {
            code: CurrencyCode::parse(&r.currency_code).map_err(|_| bad(&r.currency_code))?,
            attrs: CurrencyAttrs {
                name: r.name,
                price_decimals: r.price_decimals,
                amount_decimals: r.amount_decimals,
                total_decimals: r.total_decimals,
                rate_decimals: r.rate_decimals,
                rate_method: RateMethod::parse(&r.rate_method).map_err(|_| bad(&r.rate_method))?,
            },
            active: r.active,
            confirmation: ttrs_domain::confirmation::Confirmation {
                confirmed: r.confirmed,
                by: r.confirmed_by,
                at: r.confirmed_at,
            },
            created_by: r.created_by,
            created_dept: r.created_dept,
            created_on: r.created_on,
            updated_by: r.updated_by,
            updated_on: r.updated_on,
            stamp: Stamp {
                version: r.version,
                created_at: Some(r.created_at),
                updated_at: r.updated_at,
            },
        })
    }
}

#[async_trait]
impl CurrencyRepository for PgStore {
    async fn list_currencies(&self) -> Result<Vec<Currency>, RepoError> {
        sqlx::query_as::<_, CurrencyRow>(select_currency!("ORDER BY currency_code COLLATE \"C\""))
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(Currency::try_from)
            .collect()
    }

    async fn get_currency(&self, code: &CurrencyCode) -> Result<Option<Currency>, RepoError> {
        sqlx::query_as::<_, CurrencyRow>(select_currency!("WHERE currency_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .map(Currency::try_from)
            .transpose()
    }

    async fn insert_currency(&self, c: &Currency) -> Result<(), RepoError> {
        let a = &c.attrs;
        sqlx::query(
            "INSERT INTO currency (currency_code, name, price_decimals, amount_decimals, total_decimals,
                 rate_decimals, rate_method, active, created_by, created_dept, created_on,
                 confirmed, confirmed_by, confirmed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
        )
        .bind(c.code.as_str())
        .bind(&a.name)
        .bind(a.price_decimals)
        .bind(a.amount_decimals)
        .bind(a.total_decimals)
        .bind(a.rate_decimals)
        .bind(a.rate_method.code())
        .bind(c.active)
        .bind(&c.created_by)
        .bind(&c.created_dept)
        .bind(c.created_on)
        .bind(c.confirmation.confirmed)
        .bind(&c.confirmation.by)
        .bind(c.confirmation.at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_currency(&self, c: &Currency) -> Result<(), RepoError> {
        let a = &c.attrs;
        let n = sqlx::query(
            "UPDATE currency SET name = $2, price_decimals = $3, amount_decimals = $4, total_decimals = $5,
                 rate_decimals = $6, rate_method = $7, active = $8, updated_by = $9, updated_on = $10,
                 confirmed = $12, confirmed_by = $13, confirmed_at = $14
             WHERE currency_code = $1 AND version = $11",
        )
        .bind(c.code.as_str())
        .bind(&a.name)
        .bind(a.price_decimals)
        .bind(a.amount_decimals)
        .bind(a.total_decimals)
        .bind(a.rate_decimals)
        .bind(a.rate_method.code())
        .bind(c.active)
        .bind(&c.updated_by)
        .bind(c.updated_on)
        .bind(c.stamp.version)
        .bind(c.confirmation.confirmed)
        .bind(&c.confirmation.by)
        .bind(c.confirmation.at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        Ok(())
    }

    async fn delete_currency(&self, code: &CurrencyCode) -> Result<bool, RepoError> {
        // 被引用時外鍵擋下，由 db_err 轉成 InUse（CY6；引用的表移植後加外鍵）
        let n = sqlx::query("DELETE FROM currency WHERE currency_code = $1")
            .bind(code.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?
            .rows_affected();
        Ok(n > 0)
    }
}
