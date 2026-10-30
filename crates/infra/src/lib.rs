//! PostgreSQL 實作。
//! 目前用執行期查詢（query_as），不需資料庫即可編譯；
//! 資料庫環境就緒後改用 `sqlx::query!` 搭配 `cargo sqlx prepare` 做編譯期檢查。

pub mod pg;
mod pg_dictionary;
mod pg_legacy;
mod pg_program;

pub use pg::PgStore;

use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};

/// 內嵌的 migrations；資料字典的變更紀錄也從這裡讀 SQL 內容
pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await
}
