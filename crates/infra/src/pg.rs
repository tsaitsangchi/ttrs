use async_trait::async_trait;
use sqlx::{FromRow, PgPool};
use ttrs_app::{
    error::RepoError,
    ports::{ItemRepository, MasterDataRepository},
};
use ttrs_domain::{
    item::{Item, ItemNo, ItemStatus},
    plant::PlantId,
};

#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(FromRow)]
struct ItemRow {
    plant_id: String,
    item_no: String,
    name: String,
    spec: Option<String>,
    stock_unit: String,
    status: String,
}

impl TryFrom<ItemRow> for Item {
    type Error = RepoError;

    fn try_from(r: ItemRow) -> Result<Self, Self::Error> {
        let bad = |e: ttrs_domain::DomainError| RepoError::Other(format!("資料庫內容不合法：{e}"));
        Ok(Item {
            plant_id: PlantId::parse(&r.plant_id).map_err(bad)?,
            item_no: ItemNo::parse(&r.item_no).map_err(bad)?,
            name: r.name,
            spec: r.spec,
            stock_unit: r.stock_unit,
            status: status_from_db(&r.status)?,
        })
    }
}

fn status_to_db(s: ItemStatus) -> &'static str {
    match s {
        ItemStatus::Unconfirmed => "unconfirmed",
        ItemStatus::Confirmed => "confirmed",
        ItemStatus::Voided => "voided",
    }
}

fn status_from_db(s: &str) -> Result<ItemStatus, RepoError> {
    match s {
        "unconfirmed" => Ok(ItemStatus::Unconfirmed),
        "confirmed" => Ok(ItemStatus::Confirmed),
        "voided" => Ok(ItemStatus::Voided),
        other => Err(RepoError::Other(format!("未知的料件狀態：{other}"))),
    }
}

fn db_err(e: sqlx::Error) -> RepoError {
    match e.as_database_error() {
        Some(d) if d.is_unique_violation() => RepoError::Conflict,
        _ => RepoError::Other(e.to_string()),
    }
}

#[async_trait]
impl ItemRepository for PgStore {
    async fn get(&self, plant: &PlantId, no: &ItemNo) -> Result<Option<Item>, RepoError> {
        sqlx::query_as::<_, ItemRow>(
            "SELECT plant_id, item_no, name, spec, stock_unit, status
             FROM item WHERE plant_id = $1 AND item_no = $2",
        )
        .bind(plant.as_str())
        .bind(no.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_err)?
        .map(Item::try_from)
        .transpose()
    }

    async fn list(&self, plant: &PlantId) -> Result<Vec<Item>, RepoError> {
        sqlx::query_as::<_, ItemRow>(
            "SELECT plant_id, item_no, name, spec, stock_unit, status
             FROM item WHERE plant_id = $1 ORDER BY item_no",
        )
        .bind(plant.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(db_err)?
        .into_iter()
        .map(Item::try_from)
        .collect()
    }

    async fn insert(&self, item: &Item) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO item (plant_id, item_no, name, spec, stock_unit, status)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(item.plant_id.as_str())
        .bind(item.item_no.as_str())
        .bind(&item.name)
        .bind(&item.spec)
        .bind(&item.stock_unit)
        .bind(status_to_db(item.status))
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn update(&self, item: &Item) -> Result<(), RepoError> {
        sqlx::query(
            "UPDATE item SET name = $3, spec = $4, stock_unit = $5, status = $6, updated_at = now()
             WHERE plant_id = $1 AND item_no = $2",
        )
        .bind(item.plant_id.as_str())
        .bind(item.item_no.as_str())
        .bind(&item.name)
        .bind(&item.spec)
        .bind(&item.stock_unit)
        .bind(status_to_db(item.status))
        .execute(&self.pool)
        .await
        .map_err(db_err)?;
        Ok(())
    }

    async fn delete(&self, plant: &PlantId, no: &ItemNo) -> Result<(), RepoError> {
        sqlx::query("DELETE FROM item WHERE plant_id = $1 AND item_no = $2")
            .bind(plant.as_str())
            .bind(no.as_str())
            .execute(&self.pool)
            .await
            .map_err(db_err)?;
        Ok(())
    }

    async fn has_transactions(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, RepoError> {
        sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM inventory_txn WHERE plant_id = $1 AND item_no = $2)",
        )
        .bind(plant.as_str())
        .bind(no.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(db_err)
    }
}

#[async_trait]
impl MasterDataRepository for PgStore {
    async fn plant_active(&self, plant: &PlantId) -> Result<bool, RepoError> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM plant WHERE plant_id = $1 AND active)")
            .bind(plant.as_str())
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }

    async fn unit_exists(&self, unit: &str) -> Result<bool, RepoError> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM unit WHERE unit_code = $1)")
            .bind(unit.to_uppercase())
            .fetch_one(&self.pool)
            .await
            .map_err(db_err)
    }
}
