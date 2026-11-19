//! 料件預設倉儲（item_storage ← imf_file；item_storage.feature）。工廠層
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::ItemStorageRepository};
use ttrs_domain::{
    item::ItemNo,
    item_storage::ItemStorage,
    plant::PlantId,
    record::Stamp,
    warehouse::{LocationCode, WarehouseCode},
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct Row {
    item_no: String,
    warehouse_code: String,
    location_code: String,
    max_qty: Option<Decimal>,
    version: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

fn to_domain(r: Row) -> Result<ItemStorage, RepoError> {
    let bad =
        |e: ttrs_domain::DomainError| RepoError::Other(format!("料件預設倉儲資料不正確：{e}"));
    let location = if r.location_code.is_empty() {
        None
    } else {
        Some(LocationCode::parse(&r.location_code).map_err(bad)?)
    };
    Ok(ItemStorage {
        item: ItemNo::parse(&r.item_no).map_err(bad)?,
        warehouse: WarehouseCode::parse(&r.warehouse_code).map_err(bad)?,
        location,
        max_qty: r.max_qty,
        stamp: Stamp {
            version: r.version,
            created_at: Some(r.created_at),
            updated_at: r.updated_at,
        },
    })
}

#[async_trait]
impl ItemStorageRepository for PgStore {
    async fn list_item_storages(
        &self,
        plant: &PlantId,
        item: Option<&ItemNo>,
    ) -> Result<Vec<ItemStorage>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT item_no, warehouse_code, location_code, max_qty, version, created_at, updated_at
               FROM item_storage
              WHERE $1::text IS NULL OR item_no = $1
              ORDER BY item_no COLLATE \"C\", warehouse_code COLLATE \"C\", location_code COLLATE \"C\"",
        )
        .bind(item.map(|i| i.as_str()))
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        rows.into_iter().map(to_domain).collect()
    }

    async fn insert_item_storage(&self, plant: &PlantId, s: &ItemStorage) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        sqlx::query(
            "INSERT INTO item_storage (item_no, warehouse_code, location_code, max_qty)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(s.item.as_str())
        .bind(s.warehouse.as_str())
        .bind(s.location_code())
        .bind(s.max_qty)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_item_storage(
        &self,
        plant: &PlantId,
        old: (&WarehouseCode, Option<&LocationCode>),
        s: &ItemStorage,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let n = sqlx::query(
            "UPDATE item_storage SET warehouse_code = $4, location_code = $5, max_qty = $6
              WHERE item_no = $1 AND warehouse_code = $2 AND location_code = $3 AND version = $7",
        )
        .bind(s.item.as_str())
        .bind(old.0.as_str())
        .bind(old.1.map_or("", |l| l.as_str()))
        .bind(s.warehouse.as_str())
        .bind(s.location_code())
        .bind(s.max_qty)
        .bind(s.stamp.version)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        tx.commit().await.map_err(db_err)
    }

    async fn delete_item_storage(&self, plant: &PlantId, s: &ItemStorage) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let n = sqlx::query(
            "DELETE FROM item_storage
              WHERE item_no = $1 AND warehouse_code = $2 AND location_code = $3 AND version = $4",
        )
        .bind(s.item.as_str())
        .bind(s.warehouse.as_str())
        .bind(s.location_code())
        .bind(s.stamp.version)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        if n == 0 {
            return Err(RepoError::Stale);
        }
        tx.commit().await.map_err(db_err)
    }
}
