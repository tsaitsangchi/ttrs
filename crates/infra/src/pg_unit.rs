//! 單位（unit ← TipTop gfe_file；unit_maintenance.feature）。共用 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::UnitRepository};
use ttrs_domain::{
    record::Stamp,
    unit::{Unit, UnitAttrs, UnitCode},
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct UnitRow {
    unit_code: String,
    name: Option<String>,
    decimals: i16,
    active: bool,
    created_by: String,
    created_dept: String,
    created_on: NaiveDate,
    updated_by: Option<String>,
    updated_on: Option<NaiveDate>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

macro_rules! select_unit {
    ($rest:literal) => {
        concat!(
            "SELECT unit_code, name, decimals, active, created_by, created_dept, created_on, ",
            "updated_by, updated_on, version, created_at, updated_at FROM unit ",
            $rest
        )
    };
}

impl TryFrom<UnitRow> for Unit {
    type Error = RepoError;

    fn try_from(r: UnitRow) -> Result<Self, RepoError> {
        Ok(Unit {
            code: UnitCode::parse(&r.unit_code)
                .map_err(|_| RepoError::Other(format!("單位資料不正確：{}", r.unit_code)))?,
            attrs: UnitAttrs {
                name: r.name,
                decimals: r.decimals,
            },
            active: r.active,
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
impl UnitRepository for PgStore {
    async fn list_units(&self) -> Result<Vec<Unit>, RepoError> {
        sqlx::query_as::<_, UnitRow>(select_unit!("ORDER BY unit_code COLLATE \"C\""))
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(Unit::try_from)
            .collect()
    }

    async fn get_unit(&self, code: &UnitCode) -> Result<Option<Unit>, RepoError> {
        sqlx::query_as::<_, UnitRow>(select_unit!("WHERE unit_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .map(Unit::try_from)
            .transpose()
    }

    async fn insert_unit(&self, u: &Unit) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO unit (unit_code, name, decimals, active, created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(u.code.as_str())
        .bind(&u.attrs.name)
        .bind(u.attrs.decimals)
        .bind(u.active)
        .bind(&u.created_by)
        .bind(&u.created_dept)
        .bind(u.created_on)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_unit(&self, u: &Unit) -> Result<(), RepoError> {
        let n = sqlx::query(
            "UPDATE unit SET name = $2, decimals = $3, active = $4, updated_by = $5, updated_on = $6
             WHERE unit_code = $1 AND version = $7",
        )
        .bind(u.code.as_str())
        .bind(&u.attrs.name)
        .bind(u.attrs.decimals)
        .bind(u.active)
        .bind(&u.updated_by)
        .bind(u.updated_on)
        .bind(u.stamp.version)
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

    async fn delete_unit(&self, code: &UnitCode) -> Result<bool, RepoError> {
        // 被料件等資料引用時外鍵擋下，由 db_err 轉成 InUse（UN5）
        let n = sqlx::query("DELETE FROM unit WHERE unit_code = $1")
            .bind(code.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?
            .rows_affected();
        Ok(n > 0)
    }
}
