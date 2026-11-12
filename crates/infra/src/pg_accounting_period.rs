//! 會計期間（TipTop azm_file；accounting_periods.feature）：accounting_year 表頭＋accounting_period 一期一列（migration 0048）

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::AccountingPeriodRepository};
use ttrs_domain::{
    accounting_period::{AccountingYear, Period},
    confirmation::Confirmation,
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct YearRow {
    year: i32,
    period_count: i16,
    confirmed: bool,
    confirmed_by: Option<String>,
    confirmed_at: Option<DateTime<Utc>>,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

#[derive(FromRow)]
struct PeriodRow {
    year: i32,
    period_no: i16,
    start_date: NaiveDate,
    end_date: NaiveDate,
    quarter: i16,
}

fn to_domain(y: YearRow, periods: &[PeriodRow]) -> AccountingYear {
    AccountingYear {
        year: y.year,
        period_count: y.period_count,
        periods: periods
            .iter()
            .filter(|p| p.year == y.year)
            .map(|p| Period {
                no: p.period_no,
                start: p.start_date,
                end: p.end_date,
                quarter: p.quarter,
            })
            .collect(),
        confirmation: Confirmation {
            confirmed: y.confirmed,
            by: y.confirmed_by,
            at: y.confirmed_at,
        },
        created_by: y.created_by,
        created_dept: y.created_dept,
        created_on: y.created_on,
        updated_by: y.updated_by,
        updated_on: y.updated_on,
        stamp: Stamp {
            version: y.version,
            created_at: y.created_at,
            updated_at: y.updated_at,
        },
    }
}

#[async_trait]
impl AccountingPeriodRepository for PgStore {
    async fn list_accounting_years(&self) -> Result<Vec<AccountingYear>, RepoError> {
        let years: Vec<YearRow> = sqlx::query_as(
            "SELECT year, period_count, confirmed, confirmed_by, confirmed_at, created_by, created_dept,
                    created_on, updated_by, updated_on, version, created_at, updated_at
               FROM accounting_year ORDER BY year",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        let periods: Vec<PeriodRow> = sqlx::query_as(
            "SELECT year, period_no, start_date, end_date, quarter FROM accounting_period
              ORDER BY year, period_no",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(years.into_iter().map(|y| to_domain(y, &periods)).collect())
    }

    async fn get_accounting_year(&self, year: i32) -> Result<Option<AccountingYear>, RepoError> {
        let y: Option<YearRow> = sqlx::query_as(
            "SELECT year, period_count, confirmed, confirmed_by, confirmed_at, created_by, created_dept,
                    created_on, updated_by, updated_on, version, created_at, updated_at
               FROM accounting_year WHERE year = $1",
        )
        .bind(year)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?;
        let Some(y) = y else { return Ok(None) };
        let periods: Vec<PeriodRow> = sqlx::query_as(
            "SELECT year, period_no, start_date, end_date, quarter FROM accounting_period
              WHERE year = $1 ORDER BY period_no",
        )
        .bind(year)
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(Some(to_domain(y, &periods)))
    }

    async fn insert_accounting_year(&self, y: &AccountingYear) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query(
            "INSERT INTO accounting_year (year, period_count, confirmed, confirmed_by, confirmed_at,
                 created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(y.year)
        .bind(y.period_count)
        .bind(y.confirmation.confirmed)
        .bind(&y.confirmation.by)
        .bind(y.confirmation.at)
        .bind(&y.created_by)
        .bind(&y.created_dept)
        .bind(y.created_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        for p in &y.periods {
            sqlx::query(
                "INSERT INTO accounting_period (year, period_no, start_date, end_date, quarter)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(y.year)
            .bind(p.no)
            .bind(p.start)
            .bind(p.end)
            .bind(p.quarter)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn update_accounting_year(&self, y: &AccountingYear) -> Result<(), RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        let n = sqlx::query(
            "UPDATE accounting_year SET confirmed = $2, confirmed_by = $3, confirmed_at = $4,
                    updated_by = $5, updated_on = $6
              WHERE year = $1 AND version = $7",
        )
        .bind(y.year)
        .bind(y.confirmation.confirmed)
        .bind(&y.confirmation.by)
        .bind(y.confirmation.at)
        .bind(&y.updated_by)
        .bind(y.updated_on)
        .bind(y.stamp.version)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        // 只更新有變動的期（異動記錄只記變動的期，與記憶體後端相同）
        for p in &y.periods {
            sqlx::query(
                "UPDATE accounting_period SET start_date = $3, end_date = $4, quarter = $5
                  WHERE year = $1 AND period_no = $2
                    AND (start_date, end_date, quarter) IS DISTINCT FROM ($3, $4, $5)",
            )
            .bind(y.year)
            .bind(p.no)
            .bind(p.start)
            .bind(p.end)
            .bind(p.quarter)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        }
        tx.commit().await.map_err(db_err)
    }

    async fn delete_accounting_year(&self, year: i32) -> Result<bool, RepoError> {
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query("DELETE FROM accounting_period WHERE year = $1")
            .bind(year)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        let n = sqlx::query("DELETE FROM accounting_year WHERE year = $1")
            .bind(year)
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}
