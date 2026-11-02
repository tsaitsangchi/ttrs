//! BDD 驗收測試入口。
//!
//! 預設使用記憶體 repository；設定 `TTRS_BDD_DATABASE_URL` 則改對 PostgreSQL 執行
//! （每個場景前清空資料表，並改為循序執行）。同一份規格必須在兩種後端都通過。

mod schema_fixture;
mod steps;

use std::{fmt, sync::Arc};

use cucumber::World;
use sqlx::PgPool;
use ttrs_app::{
    Actor, AppError, DataDictionaryService, ItemService, LegacyCatalogService,
    PermissionClassService, PlantService, ProgramPermissionService, ProgramService, UserService,
    memory::InMemoryStore,
};
use ttrs_infra::PgStore;
use ttrs_shared::{
    ColumnInfoDto, ConstraintDto, LegacyTableDto, LegacyTableSummaryDto, MigrationDto, Page,
    ProgramDto, TableDictionaryDto,
};

pub enum Backend {
    Memory(InMemoryStore),
    Pg(PgPool),
}

#[derive(World)]
#[world(init = Self::init)]
pub struct ErpWorld {
    pub backend: Backend,
    pub items: ItemService,
    pub programs: ProgramService,
    /// 目前登入的使用者（程式維護等需要記錄建立者/權限類別的作業）
    pub actor: Actor,
    /// 最近一次「當」步驟的結果
    pub last: Option<Result<(), AppError>>,
    /// 最近一次程式維護查詢的結果
    pub program_page: Option<Page<ProgramDto>>,
    pub dictionary: DataDictionaryService,
    /// 資料字典：最近一次查看的欄位、限制、變更紀錄、匯出內容
    pub dict_columns: Option<Vec<ColumnInfoDto>>,
    pub dict_constraints: Option<Vec<ConstraintDto>>,
    pub dict_history: Option<Vec<MigrationDto>>,
    pub dict_export: Option<String>,
    /// 資料字典多語言：最近一次依語系查看的字典（D9）
    pub displayed_dict: Option<TableDictionaryDto>,
    /// 程式維護多語言：最近一次依語系查看的程式
    pub displayed_program: Option<ProgramDto>,
    pub legacy: LegacyCatalogService,
    /// TipTop 對照目錄：.sch 資料目錄＋命名檔合併後、待匯入的內容
    pub legacy_source: Vec<LegacyTableDto>,
    pub legacy_results: Option<Vec<LegacyTableSummaryDto>>,
    pub legacy_view: Option<LegacyTableDto>,
    pub users: UserService,
    pub classes: PermissionClassService,
    pub plants: PlantService,
    pub permissions: ProgramPermissionService,
    /// 最近一次批次作業的處理筆數（更新所有使用者目錄編號）
    pub updated_count: Option<u64>,
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
        let (backend, items, programs, dictionary, legacy, users, classes, plants, permissions) =
            match database_url() {
                Some(url) => {
                    let pool = ttrs_infra::connect(&url).await.expect("connect PostgreSQL");
                    ttrs_infra::migrate(&pool).await.expect("run migrations");
                    sqlx::query("TRUNCATE user_plant, user_account, legacy_column, legacy_table, column_dictionary_text, table_dictionary_text, table_dictionary, audit_log, query_definition, menu_item, program_permission, program_name, \
                     program, permission_class, module, inventory_txn, item, unit, plant \
                     RESTART IDENTITY CASCADE")
                    .execute(&pool)
                    .await
                    .expect("truncate tables");
                    let store = Arc::new(PgStore::new(pool.clone()));
                    (
                        Backend::Pg(pool),
                        ItemService::new(store.clone(), store.clone()),
                        ProgramService::new(store.clone()),
                        DataDictionaryService::new(store.clone(), store.clone(), store.clone()),
                        LegacyCatalogService::new(store.clone()),
                        UserService::new(
                            store.clone(),
                            store.clone(),
                            store.clone(),
                            store.clone(),
                        ),
                        PermissionClassService::new(store.clone(), store.clone(), store.clone()),
                        PlantService::new(store.clone()),
                        ProgramPermissionService::new(store.clone(), store.clone(), store),
                    )
                }
                None => {
                    let store = InMemoryStore::new();
                    schema_fixture::load(&store);
                    let shared = Arc::new(store.clone());
                    (
                        Backend::Memory(store),
                        ItemService::new(shared.clone(), shared.clone()),
                        ProgramService::new(shared.clone()),
                        DataDictionaryService::new(shared.clone(), shared.clone(), shared.clone()),
                        LegacyCatalogService::new(shared.clone()),
                        UserService::new(
                            shared.clone(),
                            shared.clone(),
                            shared.clone(),
                            shared.clone(),
                        ),
                        PermissionClassService::new(shared.clone(), shared.clone(), shared.clone()),
                        PlantService::new(shared.clone()),
                        ProgramPermissionService::new(shared.clone(), shared.clone(), shared),
                    )
                }
            };
        Self {
            backend,
            items,
            programs,
            actor: Actor::default(),
            last: None,
            program_page: None,
            dictionary,
            dict_columns: None,
            dict_constraints: None,
            dict_history: None,
            dict_export: None,
            displayed_dict: None,
            displayed_program: None,
            legacy,
            legacy_source: Vec::new(),
            legacy_results: None,
            legacy_view: None,
            users,
            classes,
            plants,
            permissions,
            updated_count: None,
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
        // @draft：規格審核中、尚未實作，不納入執行
        cucumber
            .fail_on_skipped()
            .filter_run_and_exit(features, |feature, rule, scenario| {
                let is_draft = |tags: &[String]| tags.iter().any(|t| t == "draft");
                !(is_draft(&feature.tags)
                    || rule.is_some_and(|r| is_draft(&r.tags))
                    || is_draft(&scenario.tags))
            })
            .await;
    });
}
