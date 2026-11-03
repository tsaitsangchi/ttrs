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
    pub(crate) pool: PgPool,
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

pub(crate) fn db_err(e: sqlx::Error) -> RepoError {
    match e.as_database_error() {
        Some(d) if d.is_unique_violation() => RepoError::Conflict,
        Some(d) if d.is_foreign_key_violation() => RepoError::InUse,
        _ => RepoError::Other(e.to_string()),
    }
}

/// 料件與庫存異動在工廠 schema（ADR 0007）：每個方法在工廠交易內執行
#[async_trait]
impl ItemRepository for PgStore {
    async fn get(&self, plant: &PlantId, no: &ItemNo) -> Result<Option<Item>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(None);
        };
        sqlx::query_as::<_, ItemRow>(
            "SELECT $1::text AS plant_id, item_no, name, spec, stock_unit, status
             FROM item WHERE item_no = $2",
        )
        .bind(plant.as_str())
        .bind(no.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_err)?
        .map(Item::try_from)
        .transpose()
    }

    async fn list(&self, plant: &PlantId) -> Result<Vec<Item>, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(Vec::new());
        };
        sqlx::query_as::<_, ItemRow>(
            "SELECT $1::text AS plant_id, item_no, name, spec, stock_unit, status
             FROM item ORDER BY item_no",
        )
        .bind(plant.as_str())
        .fetch_all(&mut *tx)
        .await
        .map_err(db_err)?
        .into_iter()
        .map(Item::try_from)
        .collect()
    }

    async fn insert(&self, item: &Item) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(&item.plant_id).await?;
        sqlx::query(
            "INSERT INTO item (item_no, name, spec, stock_unit, status) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(item.item_no.as_str())
        .bind(&item.name)
        .bind(&item.spec)
        .bind(&item.stock_unit)
        .bind(status_to_db(item.status))
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn update(&self, item: &Item) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(&item.plant_id).await?;
        sqlx::query(
            "UPDATE item SET name = $2, spec = $3, stock_unit = $4, status = $5, updated_at = now()
             WHERE item_no = $1",
        )
        .bind(item.item_no.as_str())
        .bind(&item.name)
        .bind(&item.spec)
        .bind(&item.stock_unit)
        .bind(status_to_db(item.status))
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn delete(&self, plant: &PlantId, no: &ItemNo) -> Result<(), RepoError> {
        let mut tx = self.required_plant_tx(plant).await?;
        sqlx::query("DELETE FROM item WHERE item_no = $1")
            .bind(no.as_str())
            .execute(&mut *tx)
            .await
            .map_err(db_err)?;
        tx.commit().await.map_err(db_err)
    }

    async fn has_transactions(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, RepoError> {
        let Some(mut tx) = self.plant_tx(plant).await? else {
            return Ok(false);
        };
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM inventory_txn WHERE item_no = $1)")
            .bind(no.as_str())
            .fetch_one(&mut *tx)
            .await
            .map_err(db_err)
    }
}

impl PgStore {
    pub(crate) async fn required_plant_tx(
        &self,
        plant: &PlantId,
    ) -> Result<sqlx::Transaction<'static, sqlx::Postgres>, RepoError> {
        self.plant_tx(plant)
            .await?
            .ok_or_else(|| RepoError::Other(format!("營運中心不存在：{}", plant.as_str())))
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
