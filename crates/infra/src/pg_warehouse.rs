//! 倉庫與儲位（warehouse ← imd_file、warehouse_location ← ime_file；warehouse.feature）。工廠層
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use ttrs_app::{error::RepoError, ports::WarehouseRepository};
use ttrs_domain::{
    department::DeptCode,
    plant::PlantId,
    record::Stamp,
    warehouse::{
        AccountMethod, Location, LocationCode, StorageClass, StorageFlags, Warehouse,
        WarehouseAttrs, WarehouseCode,
    },
};

use crate::{PgStore, pg::db_err};

#[derive(FromRow)]
struct HeadRow {
    warehouse_code: String,
    name: Option<String>,
    address: Option<String>,
    contact: Option<String>,
    phone: Option<String>,
    account: Option<String>,
    account2: Option<String>,
    cost_group: Option<String>,
    cost_center: Option<String>,
    storage_class: String,
    usable: bool,
    mrp_usable: bool,
    bonded: bool,
    issue_priority: i32,
    ship_priority: i32,
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

#[derive(FromRow)]
struct LocRow {
    warehouse_code: String,
    location_code: String,
    name: Option<String>,
    storage_class: String,
    usable: bool,
    mrp_usable: bool,
    bonded: bool,
    issue_priority: i32,
    ship_priority: i32,
    account_method: String,
    account: Option<String>,
    account2: Option<String>,
}

macro_rules! select_head {
    ($rest:literal) => {
        concat!(
            "SELECT warehouse_code, name, address, contact, phone, account, account2, cost_group, cost_center, ",
            "storage_class, usable, mrp_usable, bonded, issue_priority, ship_priority, active, confirmed, ",
            "confirmed_by, confirmed_at, created_by, created_dept, created_on, updated_by, updated_on, version, ",
            "created_at, updated_at FROM warehouse ",
            $rest
        )
    };
}

const SELECT_LOC: &str = "SELECT warehouse_code, location_code, name, storage_class, usable, mrp_usable, bonded, \
     issue_priority, ship_priority, account_method, account, account2 FROM warehouse_location";

fn bad(code: &str) -> RepoError {
    RepoError::Other(format!("倉庫資料不正確：{code}"))
}

fn flags(
    class: &str,
    usable: bool,
    mrp: bool,
    bonded: bool,
    issue: i32,
    ship: i32,
    code: &str,
) -> Result<StorageFlags, RepoError> {
    StorageFlags::new(
        StorageClass::parse(class).map_err(|_| bad(code))?,
        usable,
        mrp,
        bonded,
        issue,
        ship,
    )
    .map_err(|_| bad(code))
}

fn to_domain(h: HeadRow, locs: &[LocRow]) -> Result<Warehouse, RepoError> {
    let code = h.warehouse_code.clone();
    let mut locations = Vec::new();
    for l in locs.iter().filter(|l| l.warehouse_code == code) {
        locations.push(Location {
            code: LocationCode::parse(&l.location_code).map_err(|_| bad(&code))?,
            name: l.name.clone(),
            flags: flags(
                &l.storage_class,
                l.usable,
                l.mrp_usable,
                l.bonded,
                l.issue_priority,
                l.ship_priority,
                &code,
            )?,
            account_method: AccountMethod::parse(&l.account_method).map_err(|_| bad(&code))?,
            account: l.account.clone(),
            account2: l.account2.clone(),
        });
    }
    locations.sort_by(|a, b| a.code.cmp(&b.code));
    Ok(Warehouse {
        code: WarehouseCode::parse(&code).map_err(|_| bad(&code))?,
        attrs: WarehouseAttrs {
            name: h.name,
            address: h.address,
            contact: h.contact,
            phone: h.phone,
            account: h.account,
            account2: h.account2,
            cost_group: h.cost_group,
            cost_center: h
                .cost_center
                .map(|c| DeptCode::parse(&c))
                .transpose()
                .map_err(|_| bad(&code))?,
            flags: flags(
                &h.storage_class,
                h.usable,
                h.mrp_usable,
                h.bonded,
                h.issue_priority,
                h.ship_priority,
                &code,
            )?,
        },
        locations,
        active: h.active,
        confirmation: ttrs_domain::confirmation::Confirmation {
            confirmed: h.confirmed,
            by: h.confirmed_by,
            at: h.confirmed_at,
        },
        created_by: h.created_by,
        created_dept: h.created_dept,
        created_on: h.created_on,
        updated_by: h.updated_by,
        updated_on: h.updated_on,
        stamp: Stamp {
            version: h.version,
            created_at: Some(h.created_at),
            updated_at: h.updated_at,
        },
    })
}

/// 儲位寫入：只寫有變動的列（與記憶體後端相同）
async fn sync_locations(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    w: &Warehouse,
) -> Result<(), RepoError> {
    let codes: Vec<String> = w
        .locations
        .iter()
        .map(|l| l.code.as_str().to_owned())
        .collect();
    sqlx::query(
        "DELETE FROM warehouse_location WHERE warehouse_code = $1 AND NOT (location_code = ANY($2))",
    )
    .bind(w.code.as_str())
    .bind(&codes)
    .execute(&mut **tx)
    .await
    .map_err(db_err)?;
    for l in &w.locations {
        let f = &l.flags;
        sqlx::query(
            "INSERT INTO warehouse_location (warehouse_code, location_code, name, storage_class, usable, mrp_usable,
                 bonded, issue_priority, ship_priority, account_method, account, account2)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
             ON CONFLICT (warehouse_code, location_code) DO UPDATE
                SET name = EXCLUDED.name, storage_class = EXCLUDED.storage_class, usable = EXCLUDED.usable,
                    mrp_usable = EXCLUDED.mrp_usable, bonded = EXCLUDED.bonded,
                    issue_priority = EXCLUDED.issue_priority, ship_priority = EXCLUDED.ship_priority,
                    account_method = EXCLUDED.account_method, account = EXCLUDED.account, account2 = EXCLUDED.account2
              WHERE (warehouse_location.name, warehouse_location.storage_class, warehouse_location.usable,
                     warehouse_location.mrp_usable, warehouse_location.bonded, warehouse_location.issue_priority,
                     warehouse_location.ship_priority, warehouse_location.account_method, warehouse_location.account,
                     warehouse_location.account2)
                    IS DISTINCT FROM
                    (EXCLUDED.name, EXCLUDED.storage_class, EXCLUDED.usable, EXCLUDED.mrp_usable, EXCLUDED.bonded,
                     EXCLUDED.issue_priority, EXCLUDED.ship_priority, EXCLUDED.account_method, EXCLUDED.account,
                     EXCLUDED.account2)",
        )
        .bind(w.code.as_str())
        .bind(l.code.as_str())
        .bind(&l.name)
        .bind(f.class.code())
        .bind(f.usable)
        .bind(f.mrp_usable)
        .bind(f.bonded)
        .bind(f.issue_priority)
        .bind(f.ship_priority)
        .bind(l.account_method.code())
        .bind(&l.account)
        .bind(&l.account2)
        .execute(&mut **tx)
        .await
        .map_err(db_err)?;
    }
    Ok(())
}

#[async_trait]
impl WarehouseRepository for PgStore {
    async fn list_warehouses(&self, plant: &PlantId) -> Result<Vec<Warehouse>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        let heads: Vec<HeadRow> =
            sqlx::query_as(select_head!("ORDER BY warehouse_code COLLATE \"C\""))
                .fetch_all(&mut *tx)
                .await
                .map_err(db_err)?;
        let locs: Vec<LocRow> = sqlx::query_as(SELECT_LOC)
            .fetch_all(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        heads.into_iter().map(|h| to_domain(h, &locs)).collect()
    }

    async fn get_warehouse(
        &self,
        plant: &PlantId,
        code: &WarehouseCode,
    ) -> Result<Option<Warehouse>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        let head: Option<HeadRow> = sqlx::query_as(select_head!("WHERE warehouse_code = $1"))
            .bind(code.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_err)?;
        let locs: Vec<LocRow> = sqlx::query_as(
            "SELECT warehouse_code, location_code, name, storage_class, usable, mrp_usable, bonded, \
             issue_priority, ship_priority, account_method, account, account2 FROM warehouse_location \
             WHERE warehouse_code = $1",
        )
        .bind(code.as_str())
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        head.map(|h| to_domain(h, &locs)).transpose()
    }

    async fn insert_warehouse(&self, plant: &PlantId, w: &Warehouse) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &w.attrs;
        let f = &a.flags;
        sqlx::query(
            "INSERT INTO warehouse (warehouse_code, name, address, contact, phone, account, account2, cost_group,
                 cost_center, storage_class, usable, mrp_usable, bonded, issue_priority, ship_priority, active,
                 confirmed, confirmed_by, confirmed_at, created_by, created_dept, created_on)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22)",
        )
        .bind(w.code.as_str())
        .bind(&a.name)
        .bind(&a.address)
        .bind(&a.contact)
        .bind(&a.phone)
        .bind(&a.account)
        .bind(&a.account2)
        .bind(&a.cost_group)
        .bind(a.cost_center.as_ref().map(|d| d.as_str()))
        .bind(f.class.code())
        .bind(f.usable)
        .bind(f.mrp_usable)
        .bind(f.bonded)
        .bind(f.issue_priority)
        .bind(f.ship_priority)
        .bind(w.active)
        .bind(w.confirmation.confirmed)
        .bind(&w.confirmation.by)
        .bind(w.confirmation.at)
        .bind(&w.created_by)
        .bind(&w.created_dept)
        .bind(w.created_on)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        sync_locations(&mut tx, w).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn update_warehouse(&self, plant: &PlantId, w: &Warehouse) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        let a = &w.attrs;
        let f = &a.flags;
        let n = sqlx::query(
            "UPDATE warehouse SET name = $2, address = $3, contact = $4, phone = $5, account = $6, account2 = $7,
                    cost_group = $8, cost_center = $9, storage_class = $10, usable = $11, mrp_usable = $12,
                    bonded = $13, issue_priority = $14, ship_priority = $15, active = $16, confirmed = $17,
                    confirmed_by = $18, confirmed_at = $19, updated_by = $20, updated_on = $21
              WHERE warehouse_code = $1 AND version = $22",
        )
        .bind(w.code.as_str())
        .bind(&a.name)
        .bind(&a.address)
        .bind(&a.contact)
        .bind(&a.phone)
        .bind(&a.account)
        .bind(&a.account2)
        .bind(&a.cost_group)
        .bind(a.cost_center.as_ref().map(|d| d.as_str()))
        .bind(f.class.code())
        .bind(f.usable)
        .bind(f.mrp_usable)
        .bind(f.bonded)
        .bind(f.issue_priority)
        .bind(f.ship_priority)
        .bind(w.active)
        .bind(w.confirmation.confirmed)
        .bind(&w.confirmation.by)
        .bind(w.confirmation.at)
        .bind(&w.updated_by)
        .bind(w.updated_on)
        .bind(w.stamp.version)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?
        .rows_affected();
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        if n == 0 {
            return Err(RepoError::Stale);
        }
        sync_locations(&mut tx, w).await?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete_warehouse(
        &self,
        plant: &PlantId,
        code: &WarehouseCode,
    ) -> Result<bool, RepoError> {
        if self.warehouse_in_use(plant, code, None).await? {
            return Err(RepoError::InUse);
        }
        let mut tx = self.required_plant_tx(plant).await?;
        sqlx::query("DELETE FROM warehouse_location WHERE warehouse_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        let n = sqlx::query("DELETE FROM warehouse WHERE warehouse_code = $1")
            .bind(code.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?
            .rows_affected();
        tx.commit().await.map_err(db_err)?;
        Ok(n > 0)
    }

    async fn warehouse_in_use(
        &self,
        plant: &PlantId,
        code: &WarehouseCode,
        location: Option<&LocationCode>,
    ) -> Result<bool, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(false);
        };
        let used: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM item WHERE warehouse = $1 AND ($2::text IS NULL OR location = $2))
                 OR EXISTS (SELECT 1 FROM item_group WHERE warehouse = $1 AND ($2::text IS NULL OR location = $2))
                 OR EXISTS (SELECT 1 FROM item_storage WHERE warehouse_code = $1 AND ($2::text IS NULL OR location_code = $2))",
        )
        .bind(code.as_str())
        .bind(location.map(|l| l.as_str()))
        .fetch_one(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)?;
        Ok(used)
    }
}
