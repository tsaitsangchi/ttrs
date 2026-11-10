//! BDD 驗收測試入口。
//!
//! 預設使用記憶體 repository；設定 `TTRS_BDD_DATABASE_URL` 則改對 PostgreSQL 執行
//! （每個場景前清空資料表，並改為循序執行）。同一份規格必須在兩種後端都通過。

mod schema_fixture;
mod steps;
mod toggle_authorizer;

use std::{fmt, sync::Arc};

use cucumber::World;
use sqlx::PgPool;
use ttrs_app::{
    Actor, AppError, ClassificationCodeService, DataDictionaryService, DepartmentService,
    EmergencyService, EmployeeService, ItemService, LegacyCatalogService, PasswordResetService,
    PermissionClassService, PlantService, ProgramPermissionService, ProgramService, Services,
    SignInService, UserPermissionService, UserService,
    memory::{InMemoryStore, Outbox},
};
use ttrs_infra::PgStore;

use crate::toggle_authorizer::ToggleAuthorizer;
use ttrs_shared::{
    ColumnInfoDto, ConstraintDto, LegacyTableDto, LegacyTableSummaryDto, MigrationDto,
    MyPermissionDto, ProgramDto, QbeCondition, SessionDto, TableDictionaryDto,
};

pub enum Backend {
    Memory(InMemoryStore),
    Pg(PgPool),
}

#[derive(World)]
#[world(init = Self::init)]
pub struct ErpWorld {
    pub backend: Backend,
    /// 權限檢查開關（authorization.feature AQ12）：預設不檢查，權限規格的操作步驟才啟用
    pub authz: Arc<ToggleAuthorizer>,
    pub items: ItemService,
    pub programs: ProgramService,
    /// 目前登入的使用者（程式維護等需要記錄建立者/權限類別的作業）
    pub actor: Actor,
    /// 最近一次「當」步驟的結果
    pub last: Option<Result<(), AppError>>,
    /// 最近一次 QBE 查詢的結果：依序的鍵值（程式代號、使用者代號…）與總筆數
    pub query_result: Option<(Vec<String>, u64)>,
    /// 最近一次 QBE 查詢的條件（程式執行權限查詢再以同一組條件取各權限類別的程式）
    pub query_conditions: Vec<QbeCondition>,
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
    pub special: UserPermissionService,
    pub departments: DepartmentService,
    pub employees: EmployeeService,
    /// 分類碼（classification_code.feature）
    pub classification: ClassificationCodeService,
    /// 最近一次批次作業的處理筆數（更新所有使用者目錄編號）
    pub updated_count: Option<u64>,
    /// 最近一次查看的有效權限（authorization.feature）
    pub my_permission: Option<MyPermissionDto>,
    pub sign_in: SignInService,
    /// 目前登入的連線（sign_in.feature）
    pub session: Option<SessionDto>,
    /// 登入畫面輸入使用者代號後的營運中心選項（sign_in.feature SL1–SL8）
    pub sign_in_plants: Option<ttrs_shared::SignInPlantsDto>,
    /// 各使用者目前的 Web 密碼（「使用者 … 已登入」用）
    pub passwords: std::collections::HashMap<String, String>,
    /// 忘記密碼（password_reset.feature）
    pub password_reset: PasswordResetService,
    /// 寄出的信件（「已設定寄信伺服器」時接上）
    pub outbox: Outbox,
    /// 最近收到的重設連結權杖
    pub reset_token: Option<String>,
    /// 伺服器端緊急重設（emergency_password_reset.feature）
    pub emergency: EmergencyService,
    /// 最近一次產生的報表資料（department_report.feature）
    pub report: Option<ttrs_app::report::ReportData>,
    /// 最近一次查詢的異動記錄（change_history.feature）
    pub history: Option<Vec<ttrs_shared::ChangeDto>>,
    /// 共用層作業的異動記錄（ADR 0012 §8）
    pub shared_history: ttrs_app::ChangeHistoryService,
    /// 選單維護與側欄（menu_maintenance.feature）
    pub menus: ttrs_app::MenuService,
    /// 單位維護（unit_maintenance.feature）
    pub units: ttrs_app::UnitService,
    /// 基本代碼：區域別、國別、地區、交運方式、報表抬頭
    pub basic_codes: ttrs_app::BasicCodeService,
    /// 幣別維護（currency_maintenance.feature）
    pub currencies: ttrs_app::CurrencyService,
    /// 稅別維護（tax_type_maintenance.feature）
    pub taxes: ttrs_app::TaxTypeService,
    /// 專案維護（project_maintenance.feature）
    pub projects: ttrs_app::ProjectService,
    /// 產品分類維護（product_classification.feature）
    pub product_classes: ttrs_app::ProductClassService,
    /// 料件分群維護（item_group.feature）
    pub item_groups: ttrs_app::ItemGroupService,
    pub menu_tree: Option<Vec<ttrs_shared::MenuNodeDto>>,
    /// 取得分群預設值的結果（item_maintenance.feature IM-7）
    pub item_defaults: Option<ttrs_shared::ItemInput>,
    /// 以應用程式的資料庫角色修改異動記錄是否被拒（CH-9）
    pub db_denied: Option<bool>,
    /// 最近一次 ttrs-admin 指令的結果
    pub command: Option<Result<ttrs_api::admin_cli::AdminOutput, ttrs_api::admin_cli::AdminError>>,
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
        let (backend, services, authz) = match database_url() {
            Some(url) => {
                let pool = ttrs_infra::connect(&url).await.expect("connect PostgreSQL");
                ttrs_infra::migrate(&pool).await.expect("run migrations");
                // 上一個情境建立的工廠 schema 與角色（ADR 0007）
                ttrs_infra::drop_all_plant_schemas(&pool)
                    .await
                    .expect("drop plant schemas");
                sqlx::query("TRUNCATE change_log, user_permission, user_plant, password_reset_token, user_credential, user_account, legacy_column, legacy_table, column_dictionary_text, table_dictionary_text, table_dictionary, audit_log, query_definition, menu_item, program_permission, program_name, \
                     program, permission_class, module, unit, plant, \
                     report_title_text, report_title, area, country, region, shipping_method, currency, tax_type \
                     RESTART IDENTITY CASCADE")
                    .execute(&pool)
                    .await
                    .expect("truncate tables");
                // 帳號稽核原則回到起始值（migration 0020，ADR 0009 W5）
                for sql in [
                    "DELETE FROM account_policy",
                    "INSERT INTO account_policy (policy_key) VALUES ('0')",
                ] {
                    sqlx::query(sql)
                        .execute(&pool)
                        .await
                        .expect("reset account policy");
                }
                let store = Arc::new(PgStore::new(pool.clone()));
                let authz = ToggleAuthorizer::new(store.clone(), store.clone());
                let services = Services::with_authorizer(store, authz.clone());
                (Backend::Pg(pool), services, authz)
            }
            None => {
                let store = InMemoryStore::new();
                schema_fixture::load(&store);
                let shared = Arc::new(store.clone());
                let authz = ToggleAuthorizer::new(shared.clone(), shared.clone());
                let services = Services::with_authorizer(shared, authz.clone());
                (Backend::Memory(store), services, authz)
            }
        };
        let Services {
            items,
            programs,
            dictionary,
            legacy,
            users,
            classes,
            plants,
            permissions,
            special,
            departments,
            employees,
            classification,
            sign_in,
            password_reset,
            emergency,
            history: shared_history,
            menus,
            units,
            basic_codes,
            currencies,
            taxes,
            projects,
            product_classes,
            item_groups,
            authorizer: _,
        } = services;
        Self {
            backend,
            authz,
            items,
            programs,
            actor: Actor::default(),
            last: None,
            query_result: None,
            query_conditions: Vec::new(),
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
            special,
            departments,
            employees,
            classification,
            updated_count: None,
            my_permission: None,
            sign_in,
            session: None,
            sign_in_plants: None,
            passwords: Default::default(),
            password_reset,
            outbox: Outbox::default(),
            reset_token: None,
            emergency,
            command: None,
            report: None,
            history: None,
            shared_history,
            menus,
            units,
            basic_codes,
            currencies,
            taxes,
            projects,
            product_classes,
            item_groups,
            menu_tree: None,
            item_defaults: None,
            db_denied: None,
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
        // @draft：規格審核中、尚未實作，不納入執行；@pg：只對 PostgreSQL 有意義（資料庫權限、觸發器，CH9）
        let pg = database_url().is_some();
        cucumber
            .fail_on_skipped()
            .filter_run_and_exit(features, move |feature, rule, scenario| {
                let has = |tags: &[String], tag: &str| tags.iter().any(|t| t == tag);
                let tagged = |tag: &str| {
                    has(&feature.tags, tag)
                        || rule.is_some_and(|r| has(&r.tags, tag))
                        || has(&scenario.tags, tag)
                };
                !(tagged("draft") || (!pg && tagged("pg")))
            })
            .await;
    });
}
