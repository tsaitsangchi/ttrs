//! 單位換算（TipTop smc_file、smd_file；unit_conversion.feature）：
//! 一般換算在共用 schema（migration 0049），料件換算在工廠 schema（migrations/plant/0015），在工廠交易內執行。

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use ttrs_app::{
    error::RepoError,
    ports::{ItemUnitConversionRepository, UnitConversionRepository},
};
use ttrs_domain::{
    confirmation::Confirmation,
    item::ItemNo,
    plant::PlantId,
    record::Stamp,
    unit::UnitCode,
    unit_conversion::{Conversion, ItemUnitConversion, UnitConversion},
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct Row {
    item_no: Option<String>,
    from_unit: String,
    to_unit: String,
    from_quantity: Decimal,
    to_quantity: Decimal,
    note: Option<String>,
    active: bool,
    confirmed: Option<bool>,
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

fn bad(e: ttrs_domain::DomainError) -> RepoError {
    RepoError::Other(e.to_string())
}

impl Row {
    fn conversion(&self) -> Result<Conversion, RepoError> {
        // 資料庫內的資料已檢查過，直接組回（不重跑新增時的檢查）
        Ok(Conversion {
            from_unit: UnitCode::parse(&self.from_unit).map_err(bad)?,
            to_unit: UnitCode::parse(&self.to_unit).map_err(bad)?,
            from_quantity: self.from_quantity,
            to_quantity: self.to_quantity,
            note: self.note.clone(),
        })
    }

    fn stamp(&self) -> Stamp {
        Stamp {
            version: self.version,
            created_at: Some(self.created_at),
            updated_at: self.updated_at,
        }
    }

    fn general(self) -> Result<UnitConversion, RepoError> {
        Ok(UnitConversion {
            conversion: self.conversion()?,
            active: self.active,
            confirmation: Confirmation {
                confirmed: self.confirmed.unwrap_or(false),
                by: self.confirmed_by.clone(),
                at: self.confirmed_at,
            },
            stamp: self.stamp(),
            created_by: self.created_by,
            created_dept: self.created_dept,
            created_on: self.created_on,
            updated_by: self.updated_by,
            updated_on: self.updated_on,
        })
    }

    fn item(self) -> Result<ItemUnitConversion, RepoError> {
        Ok(ItemUnitConversion {
            item: ItemNo::parse(self.item_no.as_deref().unwrap_or_default()).map_err(bad)?,
            conversion: self.conversion()?,
            active: self.active,
            stamp: self.stamp(),
            created_by: self.created_by,
            created_dept: self.created_dept,
            created_on: self.created_on,
            updated_by: self.updated_by,
            updated_on: self.updated_on,
        })
    }
}

#[async_trait]
impl UnitConversionRepository for PgStore {
    async fn list_unit_conversions(&self) -> Result<Vec<UnitConversion>, RepoError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT NULL::text AS item_no, from_unit, to_unit, from_quantity, to_quantity, note, active,
                    confirmed, confirmed_by, confirmed_at, created_by, created_dept, created_on,
                    updated_by, updated_on, version, created_at, updated_at
               FROM unit_conversion ORDER BY from_unit COLLATE \"C\", to_unit COLLATE \"C\"",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?;
        rows.into_iter().map(Row::general).collect()
    }

    async fn insert_unit_conversion(&self, c: &UnitConversion) -> Result<(), RepoError> {
        let v = &c.conversion;
        sqlx::query(
            "INSERT INTO unit_conversion (from_unit, to_unit, from_quantity, to_quantity, note, active,
                 confirmed, confirmed_by, confirmed_at, created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(v.from_unit.as_str())
        .bind(v.to_unit.as_str())
        .bind(v.from_quantity)
        .bind(v.to_quantity)
        .bind(&v.note)
        .bind(c.active)
        .bind(c.confirmation.confirmed)
        .bind(&c.confirmation.by)
        .bind(c.confirmation.at)
        .bind(&c.created_by)
        .bind(&c.created_dept)
        .bind(c.created_on)
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update_unit_conversion(&self, c: &UnitConversion) -> Result<(), RepoError> {
        let v = &c.conversion;
        let n = sqlx::query(
            "UPDATE unit_conversion SET from_quantity = $3, to_quantity = $4, note = $5, active = $6,
                    confirmed = $7, confirmed_by = $8, confirmed_at = $9, updated_by = $10, updated_on = $11
              WHERE from_unit = $1 AND to_unit = $2 AND version = $12",
        )
        .bind(v.from_unit.as_str())
        .bind(v.to_unit.as_str())
        .bind(v.from_quantity)
        .bind(v.to_quantity)
        .bind(&v.note)
        .bind(c.active)
        .bind(c.confirmation.confirmed)
        .bind(&c.confirmation.by)
        .bind(c.confirmation.at)
        .bind(&c.updated_by)
        .bind(c.updated_on)
        .bind(c.stamp.version)
        .execute(&self.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
        if n == 0 {
            return Err(RepoError::Stale);
        }
        Ok(())
    }

    async fn delete_unit_conversion(
        &self,
        from: &UnitCode,
        to: &UnitCode,
    ) -> Result<bool, RepoError> {
        let n = sqlx::query("DELETE FROM unit_conversion WHERE from_unit = $1 AND to_unit = $2")
            .bind(from.as_str())
            .bind(to.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?
            .rows_affected();
        Ok(n > 0)
    }
}

#[async_trait]
impl ItemUnitConversionRepository for PgStore {
    async fn list_item_unit_conversions(
        &self,
        plant: &PlantId,
        item: Option<&ItemNo>,
    ) -> Result<Vec<ItemUnitConversion>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT item_no, from_unit, to_unit, from_quantity, to_quantity, note, active,
                    NULL::boolean AS confirmed, NULL::text AS confirmed_by, NULL::timestamptz AS confirmed_at,
                    created_by, created_dept, created_on, updated_by, updated_on, version, created_at, updated_at
               FROM item_unit_conversion
              WHERE ($1::text IS NULL OR item_no = $1)
              ORDER BY item_no COLLATE \"C\", from_unit COLLATE \"C\", to_unit COLLATE \"C\"",
        )
        .bind(item.map(ItemNo::as_str))
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        rows.into_iter().map(Row::item).collect()
    }

    async fn insert_item_unit_conversion(
        &self,
        plant: &PlantId,
        c: &ItemUnitConversion,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        item_conversion_insert(&mut tx, c).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_item_unit_conversion(
        &self,
        plant: &PlantId,
        c: &ItemUnitConversion,
    ) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        item_conversion_update(&mut tx, c).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete_item_unit_conversion(
        &self,
        plant: &PlantId,
        item: &ItemNo,
        from: &UnitCode,
        to: &UnitCode,
    ) -> Result<bool, RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let n = sqlx::query(
            "DELETE FROM item_unit_conversion WHERE item_no = $1 AND from_unit = $2 AND to_unit = $3",
        )
        .bind(item.as_str())
        .bind(from.as_str())
        .bind(to.as_str())
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }
}

/// 在呼叫端的交易內寫入（料件拋轉一個營運中心一個交易，item_carry.feature IC7）
pub(crate) async fn item_conversion_insert(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    c: &ItemUnitConversion,
) -> Result<(), RepoError> {
    let v = &c.conversion;
    sqlx::query(
            "INSERT INTO item_unit_conversion (item_no, from_unit, to_unit, from_quantity, to_quantity, note,
                 active, created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(c.item.as_str())
        .bind(v.from_unit.as_str())
        .bind(v.to_unit.as_str())
        .bind(v.from_quantity)
        .bind(v.to_quantity)
        .bind(&v.note)
        .bind(c.active)
        .bind(&c.created_by)
        .bind(&c.created_dept)
        .bind(c.created_on)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
    Ok(())
}

/// 在呼叫端的交易內寫入（料件拋轉一個營運中心一個交易，item_carry.feature IC7）
pub(crate) async fn item_conversion_update(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    c: &ItemUnitConversion,
) -> Result<(), RepoError> {
    let v = &c.conversion;
    let n = sqlx::query(
            "UPDATE item_unit_conversion SET from_quantity = $4, to_quantity = $5, note = $6, active = $7,
                    updated_by = $8, updated_on = $9
              WHERE item_no = $1 AND from_unit = $2 AND to_unit = $3 AND version = $10",
        )
        .bind(c.item.as_str())
        .bind(v.from_unit.as_str())
        .bind(v.to_unit.as_str())
        .bind(v.from_quantity)
        .bind(v.to_quantity)
        .bind(&v.note)
        .bind(c.active)
        .bind(&c.updated_by)
        .bind(c.updated_on)
        .bind(c.stamp.version)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?
        .rows_affected();
    if n == 0 {
        return Err(RepoError::Stale);
    }
    Ok(())
}
