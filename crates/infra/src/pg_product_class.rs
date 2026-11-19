//! 產品分類（product_class ← TipTop oba_file；product_classification.feature）。工廠 schema
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::ProductClassRepository};
use ttrs_domain::{
    department::DeptCode,
    plant::PlantId,
    product_class::{ProductClass, ProductClassAttrs, ProductClassCode},
    record::Stamp,
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct Row {
    class_code: String,
    name: Option<String>,
    period_format: Option<String>,
    periods: Option<i32>,
    cost_center: Option<String>,
    revenue_account: Option<String>,
    revenue_account2: Option<String>,
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

macro_rules! select_class {
    ($rest:literal) => {
        concat!(
            "SELECT class_code, name, period_format, periods, cost_center, revenue_account, revenue_account2, ",
            "active, confirmed, confirmed_by, confirmed_at, created_by, created_dept, created_on, updated_by, updated_on, version, created_at, updated_at ",
            "FROM product_class ",
            $rest
        )
    };
}

fn bad(v: &str) -> RepoError {
    RepoError::Other(format!("產品分類資料不正確：{v}"))
}

impl TryFrom<Row> for ProductClass {
    type Error = RepoError;

    fn try_from(r: Row) -> Result<Self, RepoError> {
        Ok(ProductClass {
            code: ProductClassCode::parse(&r.class_code).map_err(|_| bad(&r.class_code))?,
            attrs: ProductClassAttrs {
                name: r.name,
                period_format: r.period_format,
                periods: r.periods,
                cost_center: match r.cost_center.as_deref() {
                    Some(c) => Some(DeptCode::parse(c).map_err(|_| bad(c))?),
                    None => None,
                },
                revenue_account: r.revenue_account,
                revenue_account2: r.revenue_account2,
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
impl ProductClassRepository for PgStore {
    async fn list_product_classes(&self, plant: &PlantId) -> Result<Vec<ProductClass>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<Row> = sqlx::query_as(select_class!("ORDER BY class_code COLLATE \"C\""))
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        rows.into_iter().map(ProductClass::try_from).collect()
    }

    async fn get_product_class(
        &self,
        plant: &PlantId,
        code: &ProductClassCode,
    ) -> Result<Option<ProductClass>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let row: Option<Row> = sqlx::query_as(select_class!("WHERE class_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        row.map(ProductClass::try_from).transpose()
    }

    async fn insert_product_class(
        &self,
        plant: &PlantId,
        c: &ProductClass,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        product_class_insert(&mut tx, c).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_product_class(
        &self,
        plant: &PlantId,
        c: &ProductClass,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        product_class_update(&mut tx, c).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete_product_class(
        &self,
        plant: &PlantId,
        code: &ProductClassCode,
    ) -> Result<bool, RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let n = sqlx::query("DELETE FROM product_class WHERE class_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}

/// 在呼叫端的交易內寫入（基本資料拋轉一個營運中心一個交易，basic_data_carry.feature BC6(d)）
pub(crate) async fn product_class_insert(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    c: &ProductClass,
) -> Result<(), RepoError> {
    let a = &c.attrs;
    sqlx::query(
            "INSERT INTO product_class (class_code, name, period_format, periods, cost_center, revenue_account,
                 revenue_account2, active, created_by, created_dept, created_on, confirmed, confirmed_by, confirmed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
        )
        .bind(c.code.as_str())
        .bind(&a.name)
        .bind(&a.period_format)
        .bind(a.periods)
        .bind(a.cost_center.as_ref().map(DeptCode::as_str))
        .bind(&a.revenue_account)
        .bind(&a.revenue_account2)
        .bind(c.active)
        .bind(&c.created_by)
        .bind(&c.created_dept)
        .bind(c.created_on)
        .bind(c.confirmation.confirmed)
        .bind(&c.confirmation.by)
        .bind(c.confirmation.at)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
    Ok(())
}

/// 在呼叫端的交易內寫入（基本資料拋轉一個營運中心一個交易，basic_data_carry.feature BC6(d)）
pub(crate) async fn product_class_update(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    c: &ProductClass,
) -> Result<(), RepoError> {
    let a = &c.attrs;
    let n = sqlx::query(
            "UPDATE product_class SET name = $2, period_format = $3, periods = $4, cost_center = $5,
                 revenue_account = $6, revenue_account2 = $7, active = $8, updated_by = $9, updated_on = $10,
                 confirmed = $12, confirmed_by = $13, confirmed_at = $14
             WHERE class_code = $1 AND version = $11",
        )
        .bind(c.code.as_str())
        .bind(&a.name)
        .bind(&a.period_format)
        .bind(a.periods)
        .bind(a.cost_center.as_ref().map(DeptCode::as_str))
        .bind(&a.revenue_account)
        .bind(&a.revenue_account2)
        .bind(c.active)
        .bind(&c.updated_by)
        .bind(c.updated_on)
        .bind(c.stamp.version)
        .bind(c.confirmation.confirmed)
        .bind(&c.confirmation.by)
        .bind(c.confirmation.at)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?
        .rows_affected();
    // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
    if n == 0 {
        return Err(RepoError::Stale);
    }
    Ok(())
}
