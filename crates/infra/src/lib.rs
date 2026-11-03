//! PostgreSQL 實作。
//! 目前用執行期查詢（query_as），不需資料庫即可編譯；
//! 資料庫環境就緒後改用 `sqlx::query!` 搭配 `cargo sqlx prepare` 做編譯期檢查。

pub mod pg;
mod pg_department;
mod pg_dictionary;
mod pg_legacy;
mod pg_permission;
mod pg_permission_class;
mod pg_plant;
mod pg_program;
mod pg_user;
mod pg_user_permission;
mod plant_schema;
mod qbe_sql;

pub use pg::PgStore;
pub use plant_schema::{
    PLANT_MIGRATOR, drop_all_plant_schemas, plant_role_exists, plant_schema_version,
};

use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};

/// 內嵌的 migrations；資料字典的變更紀錄也從這裡讀 SQL 內容
pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

/// 先套用共用 schema 的 migrations，再對每個工廠 schema 套用工廠 migrations（ADR 0007 §4）
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await?;
    plant_schema::migrate_plants(pool).await
}
