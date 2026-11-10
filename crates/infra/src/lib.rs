//! PostgreSQL 實作。
//! 目前用執行期查詢（query_as），不需資料庫即可編譯；
//! 資料庫環境就緒後改用 `sqlx::query!` 搭配 `cargo sqlx prepare` 做編譯期檢查。

pub mod pg;
mod pg_basic_code;
mod pg_change_log;
mod pg_classification;
mod pg_credential;
mod pg_currency;
mod pg_department;
mod pg_dictionary;
mod pg_item;
mod pg_item_group;
mod pg_legacy;
mod pg_menu;
mod pg_password_reset;
mod pg_permission;
mod pg_permission_class;
mod pg_plant;
mod pg_product_class;
mod pg_program;
mod pg_project;
mod pg_tax_type;
mod pg_unit;
mod pg_user;
mod pg_user_permission;
mod plant_schema;
mod qbe_sql;
pub mod report;
pub mod smtp;

pub use pg::PgStore;
pub use plant_schema::{
    PLANT_MIGRATOR, drop_all_plant_schemas, plant_role_exists, plant_schema_version,
};
pub use smtp::SmtpMailer;

use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};

/// 內嵌的 migrations；資料字典的變更紀錄也從這裡讀 SQL 內容
pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        // 異動記錄的操作情境（ADR 0012 §3）：新連線與取出的連線都設定
        .after_connect(|conn, _| Box::pin(plant_schema::set_change_context(conn)))
        .before_acquire(|conn, _| {
            Box::pin(async move { plant_schema::set_change_context(conn).await.map(|()| true) })
        })
        .connect(database_url)
        .await
}

/// 先套用共用 schema 的 migrations，再對每個工廠 schema 套用工廠 migrations（ADR 0007 §4）
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    MIGRATOR.run(pool).await?;
    // 共用層 change_log 的月份分割（CH7）；工廠層在 ttrs_ensure_plant_schema
    sqlx::query("SELECT ttrs_change_log_partitions('public')")
        .execute(pool)
        .await?;
    plant_schema::migrate_plants(pool).await
}
