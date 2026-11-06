//! 一廠一個 PostgreSQL schema（docs/adr/0007-multi-plant.md）：工廠 migration 與工廠交易。
//! schema 與角色的建立、移除寫在資料庫函式（migrations/0016），這裡的 SQL 仍是字面值＋bind 參數。

use sqlx::{PgConnection, PgPool, Postgres, Transaction, migrate::Migrator};
use ttrs_app::error::RepoError;
use ttrs_domain::plant::PlantId;

use crate::{PgStore, pg::db_err};

/// 工廠層的 migrations：對每個工廠 schema 各套用一次，套用紀錄在該 schema 的 `_sqlx_migrations`
pub static PLANT_MIGRATOR: Migrator = sqlx::migrate!("../../migrations/plant");

/// 建立（或補齊）工廠 schema 與資料庫角色，並套用全部工廠 migration；在呼叫端的交易內執行（P14）
pub(crate) async fn prepare(
    conn: &mut PgConnection,
    schema: &str,
) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::query("SELECT ttrs_ensure_plant_schema($1)")
        .bind(schema)
        .execute(&mut *conn)
        .await?;
    sqlx::query("SELECT set_config('search_path', $1, true)")
        .bind(schema)
        .execute(&mut *conn)
        .await?;
    // run_direct：避開 run(&mut conn) 在 async_trait 內的 Acquire 生命週期問題
    PLANT_MIGRATOR.run_direct(None, &mut *conn, false).await?;
    sqlx::query("SELECT set_config('search_path', 'public', true)")
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// 對全部營運中心的工廠 schema 套用工廠 migration（API 啟動時，任何一個失敗就回錯誤）
pub(crate) async fn migrate_plants(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    let schemas: Vec<String> =
        sqlx::query_scalar("SELECT schema_name FROM plant ORDER BY schema_name")
            .fetch_all(pool)
            .await?;
    for schema in schemas {
        let mut tx = pool.begin().await?;
        prepare(&mut tx, &schema).await?;
        tx.commit().await?;
    }
    Ok(())
}

/// 工廠 schema 已套用的工廠 migration 數；schema 不存在時為 None（測試驗證用）
pub async fn plant_schema_version(
    pool: &PgPool,
    schema: &str,
) -> Result<Option<usize>, sqlx::Error> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = $1)")
            .bind(schema)
            .fetch_one(pool)
            .await?;
    if !exists {
        return Ok(None);
    }
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT set_config('search_path', $1, true)")
        .bind(schema)
        .execute(&mut *tx)
        .await?;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success")
        .fetch_one(&mut *tx)
        .await?;
    tx.rollback().await?;
    Ok(Some(usize::try_from(n).unwrap_or(0)))
}

/// 工廠 schema 的資料庫角色是否存在（測試驗證用）
pub async fn plant_role_exists(pool: &PgPool, schema: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = ttrs_plant_role($1))")
        .bind(schema)
        .fetch_one(pool)
        .await
}

/// 測試用：移除全部工廠 schema 與角色（含上一輪殘留、不在 plant 的 schema）
pub async fn drop_all_plant_schemas(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "SELECT ttrs_drop_plant_schema(nspname) FROM pg_namespace WHERE nspname ~ '^ds[0-9]{2}$'",
    )
    .execute(pool)
    .await?;
    Ok(())
}

impl PgStore {
    /// 工廠交易：切換到營運中心的資料庫角色與 schema（ADR 0007 §3）。營運中心不存在時為 None。
    /// 交易內只看得到該廠的 schema，共用 schema 的表要寫明 `public.`
    pub(crate) async fn plant_tx(
        &self,
        plant: &PlantId,
    ) -> Result<Option<Transaction<'static, Postgres>>, RepoError> {
        let schema: Option<String> =
            sqlx::query_scalar("SELECT schema_name FROM plant WHERE plant_id = $1")
                .bind(plant.as_str())
                .fetch_optional(&self.pool)
                .await
                .map_err(db_err)?;
        let Some(schema) = schema else {
            return Ok(None);
        };
        let mut tx = self.pool.begin().await.map_err(db_err)?;
        sqlx::query(
            "SELECT set_config('role', ttrs_plant_role($1), true), set_config('search_path', $1, true)",
        )
        .bind(&schema)
        .execute(&mut *tx)
        .await
        .map_err(db_err)?;
        set_change_context(&mut tx).await?;
        Ok(Some(tx))
    }
}

/// 異動記錄的操作情境（ADR 0012 §3）：寫進交易範圍的 `ttrs.*` 設定，由觸發器 ttrs_change_log 讀取。
/// 不在 use case 的操作情境內時不設定（觸發器記為 db）
pub(crate) async fn set_change_context(conn: &mut PgConnection) -> Result<(), RepoError> {
    let Some(c) = ttrs_app::change_log::current() else {
        return Ok(());
    };
    sqlx::query(
        "SELECT set_config('ttrs.user_id', $1, true), set_config('ttrs.user_dept', $2, true),
                set_config('ttrs.actor_kind', $3, true), set_config('ttrs.on_behalf_of', $4, true),
                set_config('ttrs.program', $5, true), set_config('ttrs.action', $6, true),
                set_config('ttrs.operation', $7, true), set_config('ttrs.request_id', $8, true),
                set_config('ttrs.client_ip', $9, true), set_config('ttrs.user_agent', $10, true),
                set_config('ttrs.reason', $11, true)",
    )
    .bind(&c.user_id)
    .bind(&c.user_dept)
    .bind(c.actor_kind)
    .bind(c.on_behalf_of.unwrap_or_default())
    .bind(&c.program)
    .bind(&c.action)
    .bind(c.operation.unwrap_or_default())
    .bind(&c.request.request_id)
    .bind(c.request.client_ip.unwrap_or_default())
    .bind(c.request.user_agent.unwrap_or_default())
    .bind(c.reason.unwrap_or_default())
    .execute(&mut *conn)
    .await
    .map_err(db_err)?;
    Ok(())
}
