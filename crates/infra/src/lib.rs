//! PostgreSQL 實作。
//! 目前用執行期查詢（query_as），不需資料庫即可編譯；
//! 資料庫環境就緒後改用 `sqlx::query!` 搭配 `cargo sqlx prepare` 做編譯期檢查。

pub mod pg;

pub use pg::PgStore;

use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../migrations").run(pool).await
}
