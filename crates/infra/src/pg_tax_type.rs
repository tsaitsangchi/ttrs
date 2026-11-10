//! 稅別（tax_type ← TipTop gec_file；tax_type_maintenance.feature）。共用 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::TaxTypeRepository};
use ttrs_domain::{
    confirmation::Confirmation,
    record::Stamp,
    tax_type::{InvoiceCopies, TaxClass, TaxCode, TaxDirection, TaxKey, TaxType, TaxTypeAttrs},
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct TaxRow {
    tax_code: String,
    direction: String,
    name: Option<String>,
    account: Option<String>,
    account2: Option<String>,
    invoice_copies: Option<String>,
    tax_class: Option<String>,
    rate: Decimal,
    include_tax: bool,
    filing_format: String,
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

macro_rules! select_tax {
    ($rest:literal) => {
        concat!(
            "SELECT tax_code, direction, name, account, account2, invoice_copies, tax_class, rate, include_tax, ",
            "filing_format, active, confirmed, confirmed_by, confirmed_at, created_by, created_dept, created_on, ",
            "updated_by, updated_on, version, created_at, updated_at FROM tax_type ",
            $rest
        )
    };
}

impl TryFrom<TaxRow> for TaxType {
    type Error = RepoError;

    fn try_from(r: TaxRow) -> Result<Self, RepoError> {
        let bad = |v: &str| RepoError::Other(format!("稅別資料不正確：{v}"));
        let code = TaxCode::parse(&r.tax_code).map_err(|_| bad(&r.tax_code))?;
        let direction = TaxDirection::from_code(&r.direction).ok_or_else(|| bad(&r.direction))?;
        let invoice_copies = match r.invoice_copies.as_deref() {
            Some(v) => Some(InvoiceCopies::from_code(v).ok_or_else(|| bad(v))?),
            None => None,
        };
        let tax_class = match r.tax_class.as_deref() {
            Some(v) => Some(TaxClass::from_code(v).ok_or_else(|| bad(v))?),
            None => None,
        };
        Ok(TaxType {
            key: TaxKey { code, direction },
            attrs: TaxTypeAttrs {
                name: r.name,
                account: r.account,
                account2: r.account2,
                invoice_copies,
                tax_class,
                rate: r.rate.normalize(),
                include_tax: r.include_tax,
                filing_format: r.filing_format,
            },
            active: r.active,
            confirmation: Confirmation {
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
impl TaxTypeRepository for PgStore {
    async fn list_tax_types(&self) -> Result<Vec<TaxType>, RepoError> {
        sqlx::query_as::<_, TaxRow>(select_tax!("ORDER BY tax_code COLLATE \"C\", direction"))
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(TaxType::try_from)
            .collect()
    }

    async fn get_tax_type(&self, key: &TaxKey) -> Result<Option<TaxType>, RepoError> {
        sqlx::query_as::<_, TaxRow>(select_tax!("WHERE tax_code = $1 AND direction = $2"))
            .bind(key.code.as_str())
            .bind(key.direction.code())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .map(TaxType::try_from)
            .transpose()
    }

    async fn insert_tax_type(&self, t: &TaxType) -> Result<(), RepoError> {
        let a = &t.attrs;
        sqlx::query(
            "INSERT INTO tax_type (tax_code, direction, name, account, account2, invoice_copies, tax_class,
                 rate, include_tax, filing_format, active, created_by, created_dept, created_on,
                 confirmed, confirmed_by, confirmed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
        )
        .bind(t.key.code.as_str())
        .bind(t.key.direction.code())
        .bind(&a.name)
        .bind(&a.account)
        .bind(&a.account2)
        .bind(a.invoice_copies.map(InvoiceCopies::code))
        .bind(a.tax_class.map(TaxClass::code))
        .bind(a.rate)
        .bind(a.include_tax)
        .bind(&a.filing_format)
        .bind(t.active)
        .bind(&t.created_by)
        .bind(&t.created_dept)
        .bind(t.created_on)
        .bind(t.confirmation.confirmed)
        .bind(&t.confirmation.by)
        .bind(t.confirmation.at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_tax_type(&self, t: &TaxType) -> Result<(), RepoError> {
        let a = &t.attrs;
        let n = sqlx::query(
            "UPDATE tax_type SET name = $3, account = $4, account2 = $5, invoice_copies = $6, tax_class = $7,
                 rate = $8, include_tax = $9, filing_format = $10, active = $11, updated_by = $12,
                 updated_on = $13, confirmed = $15, confirmed_by = $16, confirmed_at = $17
             WHERE tax_code = $1 AND direction = $2 AND version = $14",
        )
        .bind(t.key.code.as_str())
        .bind(t.key.direction.code())
        .bind(&a.name)
        .bind(&a.account)
        .bind(&a.account2)
        .bind(a.invoice_copies.map(InvoiceCopies::code))
        .bind(a.tax_class.map(TaxClass::code))
        .bind(a.rate)
        .bind(a.include_tax)
        .bind(&a.filing_format)
        .bind(t.active)
        .bind(&t.updated_by)
        .bind(t.updated_on)
        .bind(t.stamp.version)
        .bind(t.confirmation.confirmed)
        .bind(&t.confirmation.by)
        .bind(t.confirmation.at)
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

    async fn delete_tax_type(&self, key: &TaxKey) -> Result<bool, RepoError> {
        // 被引用時外鍵擋下，由 db_err 轉成 InUse（TX7；引用的表移植後加外鍵）
        let n = sqlx::query("DELETE FROM tax_type WHERE tax_code = $1 AND direction = $2")
            .bind(key.code.as_str())
            .bind(key.direction.code())
            .execute(&self.pool)
            .await
            .map_err(db_err)?
            .rows_affected();
        Ok(n > 0)
    }
}
