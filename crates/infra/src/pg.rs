use async_trait::async_trait;
use sqlx::PgPool;
use ttrs_app::{error::RepoError, ports::MasterDataRepository};
use ttrs_domain::plant::PlantId;

#[derive(Clone)]
pub struct PgStore {
    pub(crate) pool: PgPool,
}

impl PgStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

pub(crate) fn db_err(e: sqlx::Error) -> RepoError {
    match e.as_database_error() {
        Some(d) if d.is_unique_violation() => RepoError::Conflict,
        Some(d) if d.is_foreign_key_violation() => RepoError::InUse,
        _ => RepoError::Other(e.to_string()),
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
