//! 編碼性質（encoding_nature ← TipTop geh_file；encoding_nature.feature）。共用 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::EncodingNatureRepository};
use ttrs_domain::{
    encoding_nature::{EncodingNature, NatureAttrs, NatureCategory, NatureCode},
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct NatureRow {
    nature_code: String,
    name: Option<String>,
    category: String,
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

macro_rules! select_nature {
    ($rest:literal) => {
        concat!(
            "SELECT nature_code, name, category, active, confirmed, confirmed_by, confirmed_at, created_by, created_dept, ",
            "created_on, updated_by, updated_on, version, created_at, updated_at FROM encoding_nature ",
            $rest
        )
    };
}

impl TryFrom<NatureRow> for EncodingNature {
    type Error = RepoError;

    fn try_from(r: NatureRow) -> Result<Self, RepoError> {
        let bad = || RepoError::Other(format!("編碼性質資料不正確：{}", r.nature_code));
        Ok(EncodingNature {
            code: NatureCode::parse(&r.nature_code).map_err(|_| bad())?,
            attrs: NatureAttrs {
                name: r.name.clone(),
                category: NatureCategory::parse(&r.category).map_err(|_| bad())?,
            },
            active: r.active,
            confirmation: ttrs_domain::confirmation::Confirmation {
                confirmed: r.confirmed,
                by: r.confirmed_by.clone(),
                at: r.confirmed_at,
            },
            created_by: r.created_by.clone(),
            created_dept: r.created_dept.clone(),
            created_on: r.created_on,
            updated_by: r.updated_by.clone(),
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
impl EncodingNatureRepository for PgStore {
    async fn list_encoding_natures(&self) -> Result<Vec<EncodingNature>, RepoError> {
        sqlx::query_as::<_, NatureRow>(select_nature!("ORDER BY nature_code COLLATE \"C\""))
            .fetch_all(&self.pool)
            .await
            .map_err(db_err)?
            .into_iter()
            .map(EncodingNature::try_from)
            .collect()
    }

    async fn get_encoding_nature(
        &self,
        code: &NatureCode,
    ) -> Result<Option<EncodingNature>, RepoError> {
        sqlx::query_as::<_, NatureRow>(select_nature!("WHERE nature_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?
            .map(EncodingNature::try_from)
            .transpose()
    }

    async fn insert_encoding_nature(&self, n: &EncodingNature) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO encoding_nature (nature_code, name, category, active, created_by, created_dept, created_on,
                 confirmed, confirmed_by, confirmed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(n.code.as_str())
        .bind(&n.attrs.name)
        .bind(n.attrs.category.code())
        .bind(n.active)
        .bind(&n.created_by)
        .bind(&n.created_dept)
        .bind(n.created_on)
        .bind(n.confirmation.confirmed)
        .bind(&n.confirmation.by)
        .bind(n.confirmation.at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_encoding_nature(&self, n: &EncodingNature) -> Result<(), RepoError> {
        let rows = sqlx::query(
            "UPDATE encoding_nature SET name = $2, category = $3, active = $4, updated_by = $5, updated_on = $6,
                 confirmed = $8, confirmed_by = $9, confirmed_at = $10
             WHERE nature_code = $1 AND version = $7",
        )
        .bind(n.code.as_str())
        .bind(&n.attrs.name)
        .bind(n.attrs.category.code())
        .bind(n.active)
        .bind(&n.updated_by)
        .bind(n.updated_on)
        .bind(n.stamp.version)
        .bind(n.confirmation.confirmed)
        .bind(&n.confirmation.by)
        .bind(n.confirmation.at)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if rows == 0 {
            return Err(RepoError::Stale);
        }
        Ok(())
    }

    async fn delete_encoding_nature(&self, code: &NatureCode) -> Result<bool, RepoError> {
        // 被編碼類別引用時由外鍵擋下、db_err 轉成 InUse（EN6；aooi402 移植時加外鍵）
        let n = sqlx::query("DELETE FROM encoding_nature WHERE nature_code = $1")
            .bind(code.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?
            .rows_affected();
        Ok(n > 0)
    }
}
