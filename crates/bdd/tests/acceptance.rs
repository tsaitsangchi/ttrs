//! BDD 驗收測試入口。
//!
//! 預設使用記憶體 repository；設定 `TTRS_BDD_DATABASE_URL` 則改對 PostgreSQL 執行
//! （每個場景前清空資料表，並改為循序執行）。同一份規格必須在兩種後端都通過。

mod steps;

use std::{fmt, sync::Arc};

use cucumber::World;
use sqlx::PgPool;
use ttrs_app::{AppError, ItemService, memory::InMemoryStore};
use ttrs_infra::PgStore;

pub enum Backend {
    Memory(InMemoryStore),
    Pg(PgPool),
}

#[derive(World)]
#[world(init = Self::init)]
pub struct ErpWorld {
    pub backend: Backend,
    pub items: ItemService,
    /// 最近一次「當」步驟的結果
    pub last: Option<Result<(), AppError>>,
}

impl fmt::Debug for ErpWorld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErpWorld")
            .field("last", &self.last)
            .finish()
    }
}

impl ErpWorld {
    async fn init() -> Self {
        let (backend, items) = match database_url() {
            Some(url) => {
                let pool = ttrs_infra::connect(&url).await.expect("connect PostgreSQL");
                ttrs_infra::migrate(&pool).await.expect("run migrations");
                sqlx::query("TRUNCATE inventory_txn, item, unit, plant RESTART IDENTITY CASCADE")
                    .execute(&pool)
                    .await
                    .expect("truncate tables");
                let store = Arc::new(PgStore::new(pool.clone()));
                (Backend::Pg(pool), ItemService::new(store.clone(), store))
            }
            None => {
                let store = InMemoryStore::new();
                let shared = Arc::new(store.clone());
                (
                    Backend::Memory(store),
                    ItemService::new(shared.clone(), shared),
                )
            }
        };
        Self {
            backend,
            items,
            last: None,
        }
    }
}

fn database_url() -> Option<String> {
    std::env::var("TTRS_BDD_DATABASE_URL").ok()
}

fn main() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/../../specs/features");
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let mut cucumber = ErpWorld::cucumber();
        if database_url().is_some() {
            cucumber = cucumber.max_concurrent_scenarios(1);
        }
        cucumber.fail_on_skipped().run_and_exit(features).await;
    });
}
