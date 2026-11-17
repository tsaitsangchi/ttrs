//! 編碼年對照（encoding_year_code ← TipTop gff_file；encoding_year.feature）。共用 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::EncodingYearRepository};
use ttrs_domain::{encoding_year::EncodingYear, record::Stamp};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct YearRow {
    year: i32,
    length: i16,
    code_value: String,
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

macro_rules! select_year {
    ($rest:literal) => {
        concat!(
            "SELECT year, length, code_value, active, confirmed, confirmed_by, confirmed_at, created_by, created_dept, ",
            "created_on, updated_by, updated_on, version, created_at, updated_at FROM encoding_year_code ",
            $rest
        )
    };
}

impl From<YearRow> for EncodingYear {
    fn from(r: YearRow) -> Self {
        EncodingYear {
            year: r.year,
            length: r.length,
            code_value: r.code_value,
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
        }
    }
}

#[async_trait]
impl EncodingYearRepository for PgStore {
    async fn list_encoding_years(&self) -> Result<Vec<EncodingYear>, RepoError> {
        Ok(
            sqlx::query_as::<_, YearRow>(select_year!("ORDER BY year, length"))
                .fetch_all(&self.pool)
                .await
                .map_err(db_err)?
                .into_iter()
                .map(EncodingYear::from)
                .collect(),
        )
    }

    async fn get_encoding_year(
        &self,
        year: i32,
        length: i16,
    ) -> Result<Option<EncodingYear>, RepoError> {
        Ok(
            sqlx::query_as::<_, YearRow>(select_year!("WHERE year = $1 AND length = $2"))
                .bind(year)
                .bind(length)
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?
                .map(EncodingYear::from),
        )
    }

    async fn insert_encoding_year(&self, y: &EncodingYear) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO encoding_year_code (year, length, code_value, active, created_by, created_dept, created_on,
                 confirmed, confirmed_by, confirmed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(y.year)
        .bind(y.length)
        .bind(&y.code_value)
        .bind(y.active)
        .bind(&y.created_by)
        .bind(&y.created_dept)
        .bind(y.created_on)
        .bind(y.confirmation.confirmed)
        .bind(&y.confirmation.by)
        .bind(y.confirmation.at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_encoding_year(&self, y: &EncodingYear) -> Result<(), RepoError> {
        let n = sqlx::query(
            "UPDATE encoding_year_code SET code_value = $3, active = $4, updated_by = $5, updated_on = $6,
                 confirmed = $8, confirmed_by = $9, confirmed_at = $10
             WHERE year = $1 AND length = $2 AND version = $7",
        )
        .bind(y.year)
        .bind(y.length)
        .bind(&y.code_value)
        .bind(y.active)
        .bind(&y.updated_by)
        .bind(y.updated_on)
        .bind(y.stamp.version)
        .bind(y.confirmation.confirmed)
        .bind(&y.confirmation.by)
        .bind(y.confirmation.at)
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

    async fn delete_encoding_year(&self, year: i32, length: i16) -> Result<bool, RepoError> {
        let n = sqlx::query("DELETE FROM encoding_year_code WHERE year = $1 AND length = $2")
            .bind(year)
            .bind(length)
            .execute(&self.pool)
            .await
            .map_err(db_err)?
            .rows_affected();
        Ok(n > 0)
    }
}
