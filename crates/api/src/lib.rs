//! 路由與錯誤對應。handler 只做轉譯，業務邏輯一律在 ttrs-app。

pub mod admin_cli;
pub mod rate_job;
pub mod session;

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{FromRequestParts, Path, Query, State},
    http::request::Parts,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use ttrs_app::{
    Actor, ActorKind, AppError, Authorizer, ClassificationCodeService, DataDictionaryService,
    DepartmentService, EmployeeService, ItemService, LegacyCatalogService, PasswordResetService,
    PermissionClassService, PlantService, ProgramPermissionService, ProgramService, Services,
    SignInService, UserPermissionService, UserService, program_service::DEFAULT_LANG,
};
use ttrs_shared::{
    ApiError, ChangePasswordInput, ClassificationCodeDto, ClassificationCodeInput, ColumnInfoDto,
    ColumnTextDto, ConstraintDto, CopyPermissionsRequest, CopyUserPermissionsRequest,
    DepartmentAccountingInput, DepartmentDto, DepartmentInput, EmployeeDto, EmployeeInput,
    GeneratePermissionsRequest, GrantRequest, IndexDto, ItemDto, LegacyImportResultDto,
    LegacyTableDto, LegacyTableSummaryDto, MenuPermissionsRequest, MigrationDto, MyPermissionDto,
    NewPlantIdRequest, NewProgramNoRequest, NewUserIdRequest, Page, PasswordResetInput,
    PasswordResetRequestInput, PasswordResetStatusDto, PermissionBatchResultDto,
    PermissionClassDto, PermissionClassInput, PlantDto, PlantInput, ProgramDto, ProgramInput,
    ProgramNameDto, ProgramPermissionDto, ProgramPermissionInput, QueryRequest, SessionDto,
    SignInInput, SignInPlantsDto, SwitchPlantInput, TableDictionaryDto, TableDictionaryInput,
    TableTextDto, UpdatedCountDto, UserDto, UserInput, UserPermissionDto, UserPermissionInput,
};

#[derive(Clone)]
pub struct AppState {
    pub items: ItemService,
    pub programs: ProgramService,
    pub dictionary: DataDictionaryService,
    pub legacy: LegacyCatalogService,
    pub users: UserService,
    pub classes: PermissionClassService,
    pub plants: PlantService,
    pub permissions: ProgramPermissionService,
    pub special: UserPermissionService,
    pub departments: DepartmentService,
    pub employees: EmployeeService,
    pub classification: ClassificationCodeService,
    pub sign_in: SignInService,
    /// 忘記密碼（ADR 0010 §A）；緊急重設（EmergencyService）只給 ttrs-admin，不放在 API（R11）
    pub password_reset: PasswordResetService,
    /// 共用層作業的異動記錄（ADR 0012 §8）
    pub history: ttrs_app::ChangeHistoryService,
    /// 選單維護與側欄（p_zm；menu_maintenance.feature）
    pub menus: ttrs_app::MenuService,
    /// 單位維護（aooi101；unit_maintenance.feature）
    pub units: ttrs_app::UnitService,
    pub encoding_natures: ttrs_app::EncodingNatureService,
    pub encoding_segments: ttrs_app::EncodingSegmentService,
    /// 集團中心維護（aooi600；group_center.feature）
    pub group_centers: ttrs_app::GroupCenterService,
    /// 資料控管原則維護（aooi601；data_control.feature）
    pub data_controls: ttrs_app::DataControlService,
    /// 資料中心拋轉設定（aooi602；carry_setting.feature）
    pub carry_settings: ttrs_app::CarrySettingService,
    /// 基本資料拋轉（aoop603）與拋轉歷史查詢（aooq604；basic_data_carry.feature）
    pub basic_carry: ttrs_app::BasicCarryService,
    /// 料件拋轉（aimi100 carry；item_carry.feature）
    pub item_carry: ttrs_app::ItemCarryService,
    /// 倉庫儲位維護（aimi201、aimi200；warehouse.feature）
    pub warehouses: ttrs_app::WarehouseService,
    pub item_storages: ttrs_app::ItemStorageService,
    pub inventory_parameters: ttrs_app::InventoryParameterService,
    pub document_types: ttrs_app::DocumentTypeService,
    /// 雜項發料 aimt301、雜項收料 aimt302、雜項報廢 aimt303（三支獨立作業）
    pub misc_issues: ttrs_app::MiscIssueService,
    pub misc_receipts: ttrs_app::MiscReceiptService,
    pub misc_scraps: ttrs_app::MiscScrapService,
    /// 庫存直接調撥 aimt324
    pub warehouse_transfers: ttrs_app::WarehouseTransferService,
    /// 料件庫存數量查詢 aimq400
    pub item_stock: ttrs_app::ItemStockQueryService,
    /// 料件 BIN 卡查詢 aimq231
    pub bin_cards: ttrs_app::ItemBinCardService,
    pub encoding_years: ttrs_app::EncodingYearService,
    pub encoding_classes: ttrs_app::EncodingClassService,
    /// 基本代碼：區域別、國別、地區、交運方式、報表抬頭（aooi100、090、110、160、210；共用）
    pub basic_codes: ttrs_app::BasicCodeService,
    /// 幣別維護（aooi050；共用）
    pub currencies: ttrs_app::CurrencyService,
    pub rates: ttrs_app::ExchangeRateService,
    /// 整體管理系統參數（aoos010）
    pub general_parameters: ttrs_app::GeneralParameterService,
    /// 會計期間維護（aoos020）
    pub accounting_periods: ttrs_app::AccountingPeriodService,
    /// 單位換算（aooi102）、料件單位換算（aooi103）
    pub unit_conversions: ttrs_app::UnitConversionService,
    pub item_unit_conversions: ttrs_app::ItemUnitConversionService,
    /// EPUB 轉 TXT（工具作業）
    pub epub: ttrs_app::EpubService,
    /// GitHub EPUB 搜尋（工具作業）
    pub github_epub: ttrs_app::GithubEpubService,
    /// EPUB 英文翻譯（工具作業）
    pub translate: ttrs_app::TranslationService,
    /// EPUB 多來源搜尋（工具作業）
    pub epub_search: ttrs_app::EpubSearchService,
    /// 以網址下載 EPUB（工具作業）
    pub epub_url: ttrs_app::EpubUrlService,
    /// 稅別維護（aooi150；共用）
    pub taxes: ttrs_app::TaxTypeService,
    /// 專案維護（aooi200；工廠層）
    pub projects: ttrs_app::ProjectService,
    /// 產品分類維護（axmi110；工廠層）
    pub product_classes: ttrs_app::ProductClassService,
    /// 料件分群維護（aimi110；工廠層）
    pub item_groups: ttrs_app::ItemGroupService,
    /// 權限檢查（ADR 0006 §4）：各 use case 已在 app 層檢查，這裡只供查詢有效權限
    pub authorizer: Arc<dyn Authorizer>,
    /// 操作者：固定身分時為 `actor_from_env`；開發用登入時由 [`Ctx`] 換成連線的使用者
    pub actor: Actor,
    pub auth: AuthMode,
}

/// 登入方式（ADR 0009）
#[derive(Clone)]
pub enum AuthMode {
    /// 所有請求都以 `AppState::actor` 執行（API 契約測試、`TTRS_AUTH=fixed` 的本機截圖）
    Fixed,
    /// 使用者代號＋Web 密碼登入，連線存在伺服器端
    Session {
        sessions: session::Sessions,
        /// cookie 加 Secure（經 HTTPS 時）
        secure_cookie: bool,
    },
}

/// handler 用的狀態：登入模式下帶入連線的操作者，未登入回 401 NOT_SIGNED_IN
pub struct Ctx(pub AppState);

impl FromRequestParts<AppState> for Ctx {
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, s: &AppState) -> Result<Self, Self::Rejection> {
        match &s.auth {
            AuthMode::Fixed => Ok(Ctx(s.clone())),
            AuthMode::Session { sessions, .. } => {
                let token = session::token(&parts.headers)
                    .ok_or(AppError::from(ttrs_domain::DomainError::NotSignedIn))?;
                let actor = sessions.actor(&token, &s.sign_in).await?;
                let mut state = s.clone();
                state.actor = actor;
                Ok(Ctx(state))
            }
        }
    }
}

impl AppState {
    pub fn new(services: Services, actor: Actor) -> Self {
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
            emergency: _,
            history,
            menus,
            units,
            encoding_natures,
            encoding_segments,
            group_centers,
            data_controls,
            carry_settings,
            basic_carry,
            item_carry,
            warehouses,
            item_storages,
            inventory_parameters,
            document_types,
            misc_issues,
            misc_receipts,
            misc_scraps,
            warehouse_transfers,
            item_stock,
            bin_cards,
            encoding_years,
            encoding_classes,
            basic_codes,
            currencies,
            rates,
            general_parameters,
            accounting_periods,
            unit_conversions,
            item_unit_conversions,
            epub,
            github_epub,
            translate,
            epub_search,
            epub_url,
            taxes,
            projects,
            product_classes,
            item_groups,
            authorizer,
        } = services;
        Self {
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
            history,
            menus,
            units,
            encoding_natures,
            encoding_segments,
            group_centers,
            data_controls,
            carry_settings,
            basic_carry,
            item_carry,
            warehouses,
            item_storages,
            inventory_parameters,
            document_types,
            misc_issues,
            misc_receipts,
            misc_scraps,
            warehouse_transfers,
            item_stock,
            bin_cards,
            encoding_years,
            encoding_classes,
            basic_codes,
            currencies,
            rates,
            general_parameters,
            accounting_periods,
            unit_conversions,
            item_unit_conversions,
            epub,
            github_epub,
            translate,
            epub_search,
            epub_url,
            taxes,
            projects,
            product_classes,
            item_groups,
            authorizer,
            actor,
            auth: AuthMode::Fixed,
        }
    }

    /// 以使用者代號＋Web 密碼登入（ADR 0009）
    pub fn with_sign_in(services: Services, secure_cookie: bool) -> Self {
        Self {
            auth: AuthMode::Session {
                sessions: session::Sessions::default(),
                secure_cookie,
            },
            ..Self::new(services, Actor::default())
        }
    }
}

/// 開發期的操作者身分：`TTRS_USER`、`TTRS_DEPT`、`TTRS_PERMISSION_CLASS`。
/// 導入登入後改由認證資訊取得。
pub fn actor_from_env() -> Actor {
    let var = |k: &str, default: &str| std::env::var(k).unwrap_or_else(|_| default.to_owned());
    Actor {
        user_id: var("TTRS_USER", "tiptop"),
        dept: var("TTRS_DEPT", "MIS"),
        permission_class: var("TTRS_PERMISSION_CLASS", "ADMIN"),
        kind: ActorKind::Person,
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        // 目前的連線與切換營運中心（sign_in.feature）。公司 SSO（ADR 0006 §1）接上前以開發用身分為登入者
        .route(
            "/api/session",
            get(current_session).post(sign_in).delete(sign_out),
        )
        .route("/api/password", post(change_password))
        // 登入畫面輸入使用者代號後的營運中心選項（sign_in.feature SL1–SL8）：登入前使用
        .route("/api/sign-in/plants", get(sign_in_plants))
        // 忘記密碼（ADR 0010 §A）：登入前使用
        .route(
            "/api/password-reset",
            get(password_reset_status).post(request_password_reset),
        )
        .route("/api/password-reset/confirm", post(confirm_password_reset))
        .route("/api/session/plant", axum::routing::put(switch_plant))
        // 目前使用者的有效權限（authorization.feature AQ13）
        .route("/api/me/permissions/{program}", get(my_permission))
        // Item Maintenance 料件基本資料維護（TipTop aimi100；工廠層）
        .route(
            "/api/plants/{plant}/items",
            get(list_items).post(create_item),
        )
        // QBE 查詢（item_maintenance_query.feature IQ1–IQ10）
        .route("/api/plants/{plant}/items/query", post(query_items))
        // 料件自動編號（item_auto_number.feature AN2–AN6）：取號選單、取料號（不保留號碼）
        .route(
            "/api/plants/{plant}/items/auto-number",
            get(item_number_options).post(next_item_number),
        )
        // 列印、匯出 Excel（ADR 0013 SA3；item_maintenance_report.feature）
        .route(
            "/api/plants/{plant}/items/report.{format}",
            get(item_report),
        )
        .route(
            "/api/plants/{plant}/items/{no}",
            get(get_item).put(update_item).delete(delete_item),
        )
        .route("/api/plants/{plant}/items/{no}/history", get(item_history))
        .route("/api/plants/{plant}/items/{no}/copy", post(copy_item))
        .route(
            "/api/plants/{plant}/items/{no}/texts",
            axum::routing::put(set_item_text),
        )
        .route("/api/plants/{plant}/items/{no}/{action}", post(item_action))
        .route(
            "/api/plants/{plant}/item-groups/{code}/item-defaults",
            get(item_group_defaults),
        )
        .route("/api/programs", get(list_programs).post(create_program))
        .route("/api/programs/query", post(query_programs))
        // 列印、匯出 Excel（ADR 0013；program_maintenance_standard_actions.feature）
        .route("/api/programs/report.{format}", get(program_report))
        .route(
            "/api/program-permissions/report.{format}",
            get(program_permission_report),
        )
        .route("/api/user-permissions/query", post(query_user_permissions))
        .route(
            "/api/user-permissions/report.{format}",
            get(user_permission_report),
        )
        .route(
            "/api/dictionary/tables/{table}/report.{format}",
            get(dictionary_report),
        )
        .route(
            "/api/programs/{no}",
            get(get_program).put(update_program).delete(delete_program),
        )
        .route("/api/programs/{no}/copy", post(copy_program))
        .route(
            "/api/programs/{no}/names",
            axum::routing::put(set_program_name),
        )
        .route(
            "/api/programs/{no}/permissions",
            get(list_permissions).post(grant_permission),
        )
        // Data Dictionary Maintenance 資料字典維護（TipTop p_zta）；結構唯讀，不執行 DDL
        .route("/api/dictionary/schema-tables", get(schema_tables))
        .route(
            "/api/dictionary/tables",
            get(list_dictionaries).post(create_dictionary),
        )
        .route(
            "/api/dictionary/tables/{table}",
            get(get_dictionary)
                .put(update_dictionary)
                .delete(delete_dictionary),
        )
        .route(
            "/api/dictionary/tables/{table}/texts",
            get(table_texts).put(set_table_text),
        )
        .route("/api/dictionary/tables/{table}/columns", get(table_columns))
        .route(
            "/api/dictionary/tables/{table}/column-texts",
            get(column_texts).put(set_column_text),
        )
        .route("/api/dictionary/tables/{table}/indexes", get(table_indexes))
        .route(
            "/api/dictionary/tables/{table}/constraints",
            get(table_constraints),
        )
        .route("/api/dictionary/tables/{table}/history", get(table_history))
        .route(
            "/api/dictionary/tables/{table}/export",
            get(export_dictionary),
        )
        .route("/api/dictionary/tables/{table}/legacy", get(table_legacy))
        // TipTop 對照目錄（legacy_catalog.feature）；匯入由 make legacy-catalog 呼叫
        .route("/api/legacy-catalog", get(search_legacy))
        .route("/api/legacy-catalog/import", post(import_legacy))
        .route("/api/legacy-catalog/{legacy}", get(get_legacy))
        // User Maintenance 使用者維護（TipTop p_zx）
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/query", post(query_users))
        .route(
            "/api/users/{id}",
            get(get_user).put(update_user).delete(delete_user),
        )
        .route("/api/users/{id}/copy", post(copy_user))
        .route("/api/users/{id}/deactivate", post(deactivate_user))
        .route("/api/users/{id}/activate", post(activate_user))
        // 確認／取消確認、列印、匯出 Excel（ADR 0013；user_maintenance_standard_actions.feature）
        .route("/api/users/{id}/confirm", post(confirm_user))
        .route("/api/users/{id}/unconfirm", post(unconfirm_user))
        .route("/api/users/report.{format}", get(user_report))
        .route("/api/users/{id}/unlock", post(unlock_user))
        // User Special Permission Maintenance 使用者特殊權限維護（TipTop p_zxw）
        .route(
            "/api/users/{id}/permissions",
            get(list_user_permissions)
                .post(add_user_permission)
                .delete(remove_all_user_permissions),
        )
        .route(
            "/api/users/{id}/permissions/programs/{program}",
            axum::routing::put(update_user_program_permission),
        )
        .route(
            "/api/users/{id}/permissions/{line}",
            axum::routing::delete(remove_user_permission),
        )
        .route(
            "/api/users/{id}/copy-permissions",
            post(copy_user_permissions),
        )
        // Item Group 料件分群維護（TipTop aimi110；工廠層）
        .route(
            "/api/plants/{plant}/item-groups",
            get(list_item_groups).post(create_item_group),
        )
        .route(
            "/api/plants/{plant}/item-groups/query",
            post(query_item_groups),
        )
        .route(
            "/api/plants/{plant}/item-groups/report.{format}",
            get(item_group_report),
        )
        .route(
            "/api/plants/{plant}/item-groups/{code}",
            get(get_item_group)
                .put(update_item_group)
                .delete(delete_item_group),
        )
        .route(
            "/api/plants/{plant}/item-groups/{code}/history",
            get(item_group_history),
        )
        .route(
            "/api/plants/{plant}/item-groups/{code}/copy",
            post(copy_item_group),
        )
        .route(
            "/api/plants/{plant}/item-groups/{code}/{state}",
            post(set_item_group_active),
        )
        // Project Maintenance 專案維護（TipTop aooi200；工廠層）
        .route(
            "/api/plants/{plant}/projects",
            get(list_projects).post(create_project),
        )
        .route("/api/plants/{plant}/projects/query", post(query_projects))
        .route(
            "/api/plants/{plant}/projects/report.{format}",
            get(project_report),
        )
        .route(
            "/api/plants/{plant}/projects/{code}/copy",
            post(copy_project),
        )
        .route(
            "/api/plants/{plant}/projects/{code}",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route(
            "/api/plants/{plant}/projects/{code}/history",
            get(project_history),
        )
        .route(
            "/api/plants/{plant}/projects/{code}/{state}",
            post(set_project_state),
        )
        // Product Classification 產品分類維護（TipTop axmi110；工廠層）
        .route(
            "/api/plants/{plant}/product-classes",
            get(list_product_classes).post(create_product_class),
        )
        // 查詢、複製、列印、匯出 Excel（ADR 0013；product_classification_standard_actions.feature）
        // Warehouse Maintenance 倉庫儲位維護（TipTop aimi201、aimi200；工廠層；warehouse.feature）
        .route("/api/plants/{plant}/warehouses", post(create_warehouse))
        .route(
            "/api/plants/{plant}/warehouses/query",
            post(query_warehouses),
        )
        .route(
            "/api/plants/{plant}/warehouses/report.{format}",
            get(warehouse_report),
        )
        .route(
            "/api/plants/{plant}/warehouses/{code}",
            get(get_warehouse)
                .put(update_warehouse)
                .delete(delete_warehouse),
        )
        .route(
            "/api/plants/{plant}/warehouses/{code}/copy",
            post(copy_warehouse),
        )
        .route(
            "/api/plants/{plant}/warehouses/{code}/history",
            get(warehouse_history),
        )
        .route(
            "/api/plants/{plant}/warehouses/{code}/locations",
            post(add_warehouse_location).put(update_warehouse_location),
        )
        .route(
            "/api/plants/{plant}/warehouses/{code}/locations/{loc}",
            axum::routing::delete(remove_warehouse_location),
        )
        .route(
            "/api/plants/{plant}/warehouses/{code}/{state}",
            post(set_warehouse_state),
        )
        // Document Type Maintenance 單別維護（TipTop asmi300；工廠層；document_type.feature）
        .route(
            "/api/plants/{plant}/document-types",
            post(create_document_type),
        )
        .route(
            "/api/plants/{plant}/document-types/query",
            post(query_document_types),
        )
        .route(
            "/api/plants/{plant}/document-types/report.{format}",
            get(document_type_report),
        )
        .route(
            "/api/plants/{plant}/document-types/{code}",
            get(get_document_type)
                .put(update_document_type)
                .delete(delete_document_type),
        )
        .route(
            "/api/plants/{plant}/document-types/{code}/copy",
            post(copy_document_type),
        )
        .route(
            "/api/plants/{plant}/document-types/{code}/history",
            get(document_type_history),
        )
        .route(
            "/api/plants/{plant}/document-types/{code}/storages",
            axum::routing::put(set_document_type_storages),
        )
        .route(
            "/api/plants/{plant}/document-types/{code}/access",
            axum::routing::put(set_document_type_access),
        )
        .route(
            "/api/plants/{plant}/document-types/{code}/{state}",
            post(set_document_type_state),
        )
        // Misc Issue 雜項發料（TipTop aimt301；工廠層；misc_issue.feature）
        .route("/api/plants/{plant}/misc-issues", post(create_misc_issue))
        .route(
            "/api/plants/{plant}/misc-issues/query",
            post(query_misc_issues),
        )
        .route(
            "/api/plants/{plant}/misc-issues/report.{format}",
            get(misc_issue_report),
        )
        .route(
            "/api/plants/{plant}/misc-issues/{no}",
            get(get_misc_issue)
                .put(update_misc_issue)
                .delete(delete_misc_issue),
        )
        .route(
            "/api/plants/{plant}/misc-issues/{no}/copy",
            post(copy_misc_issue),
        )
        .route(
            "/api/plants/{plant}/misc-issues/{no}/history",
            get(misc_issue_history),
        )
        .route(
            "/api/plants/{plant}/misc-issues/{no}/post",
            post(post_misc_issue),
        )
        .route(
            "/api/plants/{plant}/misc-issues/{no}/undo-post",
            post(undo_post_misc_issue),
        )
        .route(
            "/api/plants/{plant}/misc-issues/{no}/txn-records",
            get(misc_issue_txn_records),
        )
        .route(
            "/api/plants/{plant}/misc-issues/{no}/{state}",
            post(set_misc_issue_state),
        )
        // Misc Receipt 雜項收料（TipTop aimt302；工廠層；misc_receipt.feature）
        .route(
            "/api/plants/{plant}/misc-receipts",
            post(create_misc_receipt),
        )
        .route(
            "/api/plants/{plant}/misc-receipts/query",
            post(query_misc_receipts),
        )
        .route(
            "/api/plants/{plant}/misc-receipts/new-storages",
            post(misc_receipt_new_storages),
        )
        .route(
            "/api/plants/{plant}/misc-receipts/report.{format}",
            get(misc_receipt_report),
        )
        .route(
            "/api/plants/{plant}/misc-receipts/{no}",
            get(get_misc_receipt)
                .put(update_misc_receipt)
                .delete(delete_misc_receipt),
        )
        .route(
            "/api/plants/{plant}/misc-receipts/{no}/copy",
            post(copy_misc_receipt),
        )
        .route(
            "/api/plants/{plant}/misc-receipts/{no}/history",
            get(misc_receipt_history),
        )
        .route(
            "/api/plants/{plant}/misc-receipts/{no}/post",
            post(post_misc_receipt),
        )
        .route(
            "/api/plants/{plant}/misc-receipts/{no}/undo-post",
            post(undo_post_misc_receipt),
        )
        .route(
            "/api/plants/{plant}/misc-receipts/{no}/txn-records",
            get(misc_receipt_txn_records),
        )
        .route(
            "/api/plants/{plant}/misc-receipts/{no}/{state}",
            post(set_misc_receipt_state),
        )
        // Misc Scrap 雜項報廢（TipTop aimt303；工廠層；misc_scrap.feature）
        .route("/api/plants/{plant}/misc-scraps", post(create_misc_scrap))
        .route(
            "/api/plants/{plant}/misc-scraps/query",
            post(query_misc_scraps),
        )
        .route(
            "/api/plants/{plant}/misc-scraps/report.{format}",
            get(misc_scrap_report),
        )
        .route(
            "/api/plants/{plant}/misc-scraps/{no}",
            get(get_misc_scrap)
                .put(update_misc_scrap)
                .delete(delete_misc_scrap),
        )
        .route(
            "/api/plants/{plant}/misc-scraps/{no}/copy",
            post(copy_misc_scrap),
        )
        .route(
            "/api/plants/{plant}/misc-scraps/{no}/history",
            get(misc_scrap_history),
        )
        .route(
            "/api/plants/{plant}/misc-scraps/{no}/post",
            post(post_misc_scrap),
        )
        .route(
            "/api/plants/{plant}/misc-scraps/{no}/undo-post",
            post(undo_post_misc_scrap),
        )
        .route(
            "/api/plants/{plant}/misc-scraps/{no}/txn-records",
            get(misc_scrap_txn_records),
        )
        .route(
            "/api/plants/{plant}/misc-scraps/{no}/{state}",
            post(set_misc_scrap_state),
        )
        // Warehouse Transfer 庫存直接調撥（TipTop aimt324；工廠層；warehouse_transfer.feature）
        .route(
            "/api/plants/{plant}/warehouse-transfers",
            post(create_warehouse_transfer),
        )
        .route(
            "/api/plants/{plant}/warehouse-transfers/query",
            post(query_warehouse_transfers),
        )
        .route(
            "/api/plants/{plant}/warehouse-transfers/new-storages",
            post(warehouse_transfer_new_storages),
        )
        .route(
            "/api/plants/{plant}/warehouse-transfers/report.{format}",
            get(warehouse_transfer_report),
        )
        .route(
            "/api/plants/{plant}/warehouse-transfers/{no}",
            get(get_warehouse_transfer)
                .put(update_warehouse_transfer)
                .delete(delete_warehouse_transfer),
        )
        .route(
            "/api/plants/{plant}/warehouse-transfers/{no}/copy",
            post(copy_warehouse_transfer),
        )
        .route(
            "/api/plants/{plant}/warehouse-transfers/{no}/history",
            get(warehouse_transfer_history),
        )
        .route(
            "/api/plants/{plant}/warehouse-transfers/{no}/txn-records",
            get(warehouse_transfer_txn_records),
        )
        .route(
            "/api/plants/{plant}/warehouse-transfers/{no}/{state}",
            post(set_warehouse_transfer_state),
        )
        // Item Bin Card 料件 BIN 卡查詢（TipTop aimq231；工廠層；item_bin_card.feature）
        .route("/api/plants/{plant}/bin-cards/query", post(query_bin_cards))
        .route(
            "/api/plants/{plant}/bin-cards/report.{format}",
            get(bin_card_report),
        )
        // Item Stock Query 料件庫存數量查詢（TipTop aimq400；工廠層；item_stock_query.feature）
        .route(
            "/api/plants/{plant}/item-stock/query",
            post(query_item_stock),
        )
        .route(
            "/api/plants/{plant}/item-stock/report.{format}",
            get(item_stock_report),
        )
        // Inventory Parameters 庫存參數（TipTop asms230；工廠層；inventory_parameters.feature）
        .route(
            "/api/plants/{plant}/inventory-parameters",
            get(get_inventory_parameters).put(update_inventory_parameters),
        )
        .route(
            "/api/plants/{plant}/inventory-parameters/history",
            get(inventory_parameters_history),
        )
        .route(
            "/api/plants/{plant}/inventory-parameters/report.{format}",
            get(inventory_parameters_report),
        )
        // Item Storage Maintenance 料件預設倉儲維護（TipTop aimi202；工廠層；item_storage.feature）
        .route(
            "/api/plants/{plant}/item-storages/query",
            post(query_item_storages),
        )
        .route(
            "/api/plants/{plant}/item-storages/report.{format}",
            get(item_storage_report),
        )
        .route(
            "/api/plants/{plant}/item-storages/batch",
            post(batch_item_storages),
        )
        .route(
            "/api/plants/{plant}/item-storages/{item}",
            get(get_item_storage),
        )
        // 單身：倉庫、儲位放在查詢參數（儲位可空白，IS3(b)）
        .route(
            "/api/plants/{plant}/item-storages/{item}/storages",
            post(create_item_storage)
                .put(update_item_storage)
                .delete(delete_item_storage),
        )
        .route(
            "/api/plants/{plant}/item-storages/{item}/copy",
            post(copy_item_storage),
        )
        .route(
            "/api/plants/{plant}/item-storages/{item}/history",
            get(item_storage_history),
        )
        .route(
            "/api/plants/{plant}/product-classes/query",
            post(query_product_classes),
        )
        .route(
            "/api/plants/{plant}/product-classes/report.{format}",
            get(product_class_report),
        )
        .route(
            "/api/plants/{plant}/product-classes/{code}/copy",
            post(copy_product_class),
        )
        .route(
            "/api/plants/{plant}/product-classes/{code}",
            get(get_product_class)
                .put(update_product_class)
                .delete(delete_product_class),
        )
        .route(
            "/api/plants/{plant}/product-classes/{code}/history",
            get(product_class_history),
        )
        .route(
            "/api/plants/{plant}/product-classes/{code}/{state}",
            post(set_product_class_active),
        )
        // Unit Maintenance 單位維護（TipTop aooi101；共用）
        .route("/api/units", get(list_units).post(create_unit))
        // 查詢、複製、列印、匯出 Excel（ADR 0013；unit_maintenance_standard_actions.feature）
        .route("/api/units/query", post(query_units))
        .route("/api/units/report.{format}", get(unit_report))
        .route("/api/units/{code}/copy", post(copy_unit))
        .route(
            "/api/units/{code}",
            get(get_unit).put(update_unit).delete(delete_unit),
        )
        .route("/api/units/{code}/{state}", post(set_unit_active))
        // Encoding Nature Maintenance 編碼性質維護（TipTop aooi401；共用；encoding_nature.feature）
        .route("/api/encoding-natures", post(create_encoding_nature))
        .route("/api/encoding-natures/query", post(query_encoding_natures))
        .route(
            "/api/encoding-natures/report.{format}",
            get(encoding_nature_report),
        )
        .route(
            "/api/encoding-natures/{code}/copy",
            post(copy_encoding_nature),
        )
        .route(
            "/api/encoding-natures/{code}",
            get(get_encoding_nature)
                .put(update_encoding_nature)
                .delete(delete_encoding_nature),
        )
        .route(
            "/api/encoding-natures/{code}/{state}",
            post(set_encoding_nature_state),
        )
        // Encoding Class Maintenance 編碼類別維護（TipTop aooi402；共用；encoding_class.feature）
        .route("/api/encoding-classes", post(create_encoding_class))
        .route("/api/encoding-classes/query", post(query_encoding_classes))
        .route(
            "/api/encoding-classes/report.{format}",
            get(encoding_class_report),
        )
        .route(
            "/api/encoding-classes/{code}/copy",
            post(copy_encoding_class),
        )
        // 編碼試算（encoding_rule.feature ER4；取代 aooi400 與 s_auno）
        .route(
            "/api/encoding-classes/{code}/preview",
            post(preview_encoding_class),
        )
        .route(
            "/api/encoding-classes/{code}/segments",
            post(add_encoding_class_segment),
        )
        .route(
            "/api/encoding-classes/{code}/segments/renumber",
            post(renumber_encoding_class),
        )
        .route(
            "/api/encoding-classes/{code}/segments/{no}",
            axum::routing::put(update_encoding_class_segment).delete(remove_encoding_class_segment),
        )
        .route(
            "/api/encoding-classes/{code}/segments/{no}/insert",
            post(insert_encoding_class_segment),
        )
        .route(
            "/api/encoding-classes/{code}",
            get(get_encoding_class)
                .put(update_encoding_class)
                .delete(delete_encoding_class),
        )
        .route(
            "/api/encoding-classes/{code}/{state}",
            post(set_encoding_class_state),
        )
        // Encoding Year Maintenance 編碼年對照維護（TipTop aooi404；共用；encoding_year.feature）：鍵值為年＋長度
        .route("/api/encoding-years", post(create_encoding_year))
        .route("/api/encoding-years/query", post(query_encoding_years))
        .route(
            "/api/encoding-years/report.{format}",
            get(encoding_year_report),
        )
        .route(
            "/api/encoding-years/{year}/{length}",
            get(get_encoding_year)
                .put(update_encoding_year)
                .delete(delete_encoding_year),
        )
        .route(
            "/api/encoding-years/{year}/{length}/copy",
            post(copy_encoding_year),
        )
        .route(
            "/api/encoding-years/{year}/{length}/{state}",
            post(set_encoding_year_state),
        )
        // Encoding Segment Maintenance 編碼獨立段維護（TipTop aooi403；共用；encoding_segment.feature）
        .route("/api/encoding-segments", post(create_encoding_segment))
        .route(
            "/api/encoding-segments/query",
            post(query_encoding_segments),
        )
        .route(
            "/api/encoding-segments/report.{format}",
            get(encoding_segment_report),
        )
        .route(
            "/api/encoding-segments/{code}/copy",
            post(copy_encoding_segment),
        )
        .route(
            "/api/encoding-segments/{code}/values",
            post(add_encoding_segment_value),
        )
        .route(
            "/api/encoding-segments/{code}/values/{line}",
            axum::routing::put(update_encoding_segment_value).delete(remove_encoding_segment_value),
        )
        .route(
            "/api/encoding-segments/{code}",
            get(get_encoding_segment)
                .put(update_encoding_segment)
                .delete(delete_encoding_segment),
        )
        .route(
            "/api/encoding-segments/{code}/{state}",
            post(set_encoding_segment_state),
        )
        // 料件拋轉（TipTop aimi100 carry；item_carry.feature）
        .route("/api/plants/{plant}/items-carry", get(item_carry_info))
        .route("/api/plants/{plant}/items-carry/{no}", post(run_item_carry))
        .route("/api/items-carry-history/{no}", get(item_carry_history))
        // Basic Data Carry 基本資料拋轉（TipTop aoop603）、Carry History 拋轉歷史查詢（aooq604；basic_data_carry.feature）
        .route(
            "/api/plants/{plant}/basic-data-carry",
            get(basic_carry_info).post(run_basic_carry),
        )
        .route("/api/carry-history/query", post(query_carry_history))
        .route(
            "/api/carry-history/report.{format}",
            get(carry_history_report),
        )
        // Carry Setting Maintenance 資料中心拋轉設定（TipTop aooi602；共用；carry_setting.feature）
        .route("/api/carry-settings", post(create_carry_setting))
        .route("/api/carry-settings/query", post(query_carry_settings))
        .route(
            "/api/carry-settings/report.{format}",
            get(carry_setting_report),
        )
        .route(
            "/api/carry-settings/{center}/{data_type}",
            get(get_carry_setting)
                .put(update_carry_setting)
                .delete(delete_carry_setting),
        )
        .route(
            "/api/carry-settings/{center}/{data_type}/copy",
            post(copy_carry_setting),
        )
        .route(
            "/api/carry-settings/{center}/{data_type}/targets",
            post(add_carry_target).put(update_carry_target),
        )
        .route(
            "/api/carry-settings/{center}/{data_type}/targets/{plant}",
            axum::routing::delete(remove_carry_target),
        )
        .route(
            "/api/carry-settings/{center}/{data_type}/targets/{plant}/conditions",
            post(add_carry_condition).put(update_carry_condition),
        )
        .route(
            "/api/carry-settings/{center}/{data_type}/targets/{plant}/conditions/{field}",
            axum::routing::delete(remove_carry_condition),
        )
        .route(
            "/api/carry-settings/{center}/{data_type}/targets/{plant}/defaults",
            post(add_carry_default).put(update_carry_default),
        )
        .route(
            "/api/carry-settings/{center}/{data_type}/targets/{plant}/defaults/{field}",
            axum::routing::delete(remove_carry_default),
        )
        // Data Control Maintenance 資料控管原則維護（TipTop aooi601；共用；data_control.feature）
        .route("/api/data-controls/query", post(query_data_controls))
        .route(
            "/api/data-controls/report.{format}",
            get(data_control_report),
        )
        .route("/api/data-controls/plants", post(add_data_control_plant))
        .route("/api/data-controls/{data_type}", get(get_data_control))
        .route(
            "/api/data-controls/{data_type}/plants/{plant}",
            axum::routing::delete(remove_data_control_plant),
        )
        .route(
            "/api/data-controls/{data_type}/plants/{plant}/copy",
            post(copy_data_control_rules),
        )
        .route(
            "/api/data-controls/{data_type}/plants/{plant}/rules",
            post(add_data_control_rule).put(update_data_control_rule),
        )
        .route(
            "/api/data-controls/{data_type}/plants/{plant}/rules/{field}",
            axum::routing::delete(remove_data_control_rule),
        )
        // Group Center Maintenance 集團中心維護（TipTop aooi600；共用；group_center.feature）
        .route("/api/group-centers", post(create_group_center))
        .route("/api/group-centers/query", post(query_group_centers))
        .route(
            "/api/group-centers/report.{format}",
            get(group_center_report),
        )
        .route("/api/group-centers/{code}/copy", post(copy_group_center))
        .route(
            "/api/group-centers/{code}/data-types",
            post(add_group_center_data_type),
        )
        .route(
            "/api/group-centers/{code}/data-types/{data_type}/{plant}",
            axum::routing::delete(remove_group_center_data_type),
        )
        .route(
            "/api/group-centers/{code}",
            get(get_group_center)
                .put(update_group_center)
                .delete(delete_group_center),
        )
        .route(
            "/api/group-centers/{code}/{state}",
            post(set_group_center_state),
        )
        // Tax Type Maintenance 稅別維護（TipTop aooi150；共用）：鍵值為稅別碼＋進銷項
        .route("/api/tax-types", get(list_tax_types).post(create_tax_type))
        .route("/api/tax-types/query", post(query_tax_types))
        .route("/api/tax-types/report.{format}", get(tax_type_report))
        .route(
            "/api/tax-types/{code}/{direction}/copy",
            post(copy_tax_type),
        )
        .route(
            "/api/tax-types/{code}/{direction}",
            get(get_tax_type)
                .put(update_tax_type)
                .delete(delete_tax_type),
        )
        .route(
            "/api/tax-types/{code}/{direction}/{state}",
            post(set_tax_type_state),
        )
        // 每月匯率（TipTop aooi060；共用，ER1）
        .route("/api/monthly-rates", post(create_monthly_rate))
        .route("/api/monthly-rates/query", post(query_monthly_rates))
        .route(
            "/api/monthly-rates/report.{format}",
            get(monthly_rate_report),
        )
        // 月底重評價（MR7）
        .route("/api/monthly-rates/revaluate", post(revaluate_rates))
        .route(
            "/api/monthly-rates/{currency}/{period}",
            get(get_monthly_rate)
                .put(update_monthly_rate)
                .delete(delete_monthly_rate),
        )
        .route(
            "/api/monthly-rates/{currency}/{period}/copy",
            post(copy_monthly_rate),
        )
        .route(
            "/api/monthly-rates/{currency}/{period}/{state}",
            post(set_monthly_rate_state),
        )
        // 每日匯率（TipTop aooi070；共用）
        .route("/api/daily-rates", post(create_daily_rate))
        .route("/api/daily-rates/query", post(query_daily_rates))
        .route("/api/daily-rates/report.{format}", get(daily_rate_report))
        // 新增時的預設匯率（DR4）、統一匯率（DR7）
        .route(
            "/api/daily-rates/defaults/{currency}",
            get(daily_rate_defaults),
        )
        .route("/api/daily-rates/unify", post(unify_rates))
        // 匯率自動匯入（aooi070 ★ auto_import；exchange_rate_import.feature）：台銀（可上傳 CSV）、海關
        .route("/api/daily-rates/import/bank", post(import_bank_rates))
        .route(
            "/api/daily-rates/import/customs",
            post(import_customs_rates),
        )
        .route(
            "/api/daily-rates/{currency}/{date}",
            get(get_daily_rate)
                .put(update_daily_rate)
                .delete(delete_daily_rate),
        )
        .route(
            "/api/daily-rates/{currency}/{date}/copy",
            post(copy_daily_rate),
        )
        .route(
            "/api/daily-rates/{currency}/{date}/{state}",
            post(set_daily_rate_state),
        )
        // 每日匯率複製（TipTop aoop010，CP1）與結果的列印、匯出 Excel
        .route("/api/daily-rate-fill", post(fill_daily_rates))
        .route("/api/daily-rate-fill/report.{format}", post(fill_report))
        // General Management Parameters 整體管理系統參數（TipTop aoos010；共用、只有一筆）
        .route(
            "/api/general-parameters",
            get(get_general_parameters).put(update_general_parameters),
        )
        .route(
            "/api/general-parameters/report.{format}",
            get(general_parameters_report),
        )
        // EPUB to Text（TTRS 工具作業，epub_to_text.feature）：上傳 EPUB 位元組，回傳文字；上限 100 MB（ET4）另留緩衝
        .route(
            "/api/epub-to-text",
            post(epub_to_text).layer(axum::extract::DefaultBodyLimit::max(
                (ttrs_app::epub_service::MAX_MB + 5) * 1024 * 1024,
            )),
        )
        // EPUB 英文翻譯（TTRS 工具作業，epub_translate.feature）：提交、查進度、下載
        .route(
            "/api/epub-translate",
            post(epub_translate_submit).layer(axum::extract::DefaultBodyLimit::max(
                (ttrs_app::epub_service::MAX_MB + 5) * 1024 * 1024,
            )),
        )
        .route("/api/epub-translate/{id}", get(epub_translate_status))
        .route(
            "/api/epub-translate/{id}/download",
            get(epub_translate_download),
        )
        // 以網址下載 EPUB（TTRS 工具作業，epub_url_download.feature）
        .route("/api/epub-url", get(epub_url_read))
        .route("/api/epub-url/download", get(epub_url_download))
        .route("/api/epub-url/convert", post(epub_url_convert))
        // EPUB 多來源搜尋（TTRS 工具作業，epub_web_search.feature）
        .route("/api/epub-search", get(epub_search))
        .route("/api/epub-search/download", get(epub_search_download))
        .route("/api/epub-search/convert", post(epub_search_convert))
        // 共用 GitHub 書庫（epub_github_shared_library.feature SL3）
        .route(
            "/api/epub-search/shared-library",
            get(epub_shared_library)
                .post(epub_shared_library_add)
                .delete(epub_shared_library_remove),
        )
        // 我的 GitHub 書庫（epub_github_library.feature ML2）
        .route(
            "/api/epub-search/library",
            get(epub_library)
                .post(epub_library_add)
                .delete(epub_library_remove),
        )
        // GitHub EPUB Search（TTRS 工具作業，github_epub_search.feature）
        .route("/api/github-epub/repos", get(github_epub_repos))
        .route("/api/github-epub/files", get(github_epub_files))
        .route("/api/github-epub/convert", post(github_epub_convert))
        // Unit Conversion 單位換算（TipTop aooi102；共用）
        .route("/api/unit-conversions", post(create_unit_conversion))
        .route("/api/unit-conversions/query", post(query_unit_conversions))
        .route(
            "/api/unit-conversions/report.{format}",
            get(unit_conversion_report),
        )
        .route(
            "/api/unit-conversions/{from}/{to}",
            get(get_unit_conversion)
                .put(update_unit_conversion)
                .delete(delete_unit_conversion),
        )
        .route(
            "/api/unit-conversions/{from}/{to}/copy",
            post(copy_unit_conversion),
        )
        .route(
            "/api/unit-conversions/{from}/{to}/{state}",
            post(set_unit_conversion_state),
        )
        // Item Unit Conversion 料件單位換算（TipTop aooi103；工廠層）與換算率（s_umfchk）
        .route(
            "/api/plants/{plant}/items/{no}/unit-conversions",
            get(list_item_unit_conversions).post(create_item_unit_conversion),
        )
        .route(
            "/api/plants/{plant}/items/{no}/unit-conversions/history",
            get(item_unit_conversion_history),
        )
        .route(
            "/api/plants/{plant}/items/{no}/unit-conversions/{from}/{to}",
            axum::routing::put(update_item_unit_conversion).delete(delete_item_unit_conversion),
        )
        .route(
            "/api/plants/{plant}/items/{no}/unit-conversions/{from}/{to}/{state}",
            post(set_item_unit_conversion_state),
        )
        .route(
            "/api/plants/{plant}/item-unit-conversions/report.{format}",
            get(item_unit_conversion_report),
        )
        .route(
            "/api/plants/{plant}/unit-conversion-factor",
            get(unit_conversion_factor),
        )
        // Accounting Periods 會計期間維護（TipTop aoos020；aoos030、aooq010 由即時計算取代，PD3）
        .route("/api/accounting-years", post(create_accounting_year))
        .route("/api/accounting-years/query", post(query_accounting_years))
        .route(
            "/api/accounting-years/defaults",
            get(accounting_year_defaults),
        )
        .route(
            "/api/accounting-years/report.{format}",
            get(accounting_year_report),
        )
        .route(
            "/api/accounting-years/{year}",
            get(get_accounting_year)
                .put(update_accounting_year)
                .delete(delete_accounting_year),
        )
        .route(
            "/api/accounting-years/{year}/copy",
            post(copy_accounting_year),
        )
        .route(
            "/api/accounting-years/{year}/periods/{period}",
            get(accounting_period_dates),
        )
        .route(
            "/api/accounting-years/{year}/{state}",
            post(set_accounting_year_state),
        )
        .route("/api/accounting-days", get(accounting_days))
        // 匯率維護畫面的設定與提示（GP7）
        .route("/api/monthly-rates/settings", get(monthly_rate_settings))
        .route("/api/daily-rates/settings", get(daily_rate_settings))
        // Currency Maintenance 幣別維護（TipTop aooi050；共用）
        .route(
            "/api/currencies",
            get(list_currencies).post(create_currency),
        )
        .route("/api/currencies/query", post(query_currencies))
        .route("/api/currencies/report.{format}", get(currency_report))
        .route("/api/currencies/{code}/copy", post(copy_currency))
        .route(
            "/api/currencies/{code}",
            get(get_currency)
                .put(update_currency)
                .delete(delete_currency),
        )
        .route("/api/currencies/{code}/{state}", post(set_currency_state))
        // 基本代碼：區域別、國別、地區、交運方式、報表抬頭（aooi100、090、110、160、210；共用）
        .route(
            "/api/basic-codes/{kind}",
            get(list_basic_codes).post(create_basic_code),
        )
        .route("/api/basic-codes/{kind}/query", post(query_basic_codes))
        .route(
            "/api/basic-codes/{kind}/report.{format}",
            get(basic_code_report),
        )
        .route("/api/basic-codes/{kind}/{code}/copy", post(copy_basic_code))
        .route(
            "/api/basic-codes/{kind}/{code}",
            get(get_basic_code)
                .put(update_basic_code)
                .delete(delete_basic_code),
        )
        .route(
            "/api/basic-codes/{kind}/{code}/{state}",
            post(set_basic_code_state),
        )
        // Menu Maintenance 選單維護（TipTop p_zm）與側欄樹狀選單（cl_create_4sm）
        .route("/api/menus", get(list_menus))
        .route("/api/menus/tree", get(menu_tree))
        .route("/api/menus/{menu}", axum::routing::delete(delete_menu))
        .route(
            "/api/menus/{menu}/items",
            get(menu_items).post(add_menu_item),
        )
        .route(
            "/api/menus/{menu}/items/{program}",
            axum::routing::delete(remove_menu_item),
        )
        .route(
            "/api/menus/{menu}/items/{program}/{direction}",
            post(shift_menu_item),
        )
        .route("/api/menus/{menu}/renumber", post(renumber_menu))
        .route("/api/menus/{menu}/copy", post(copy_menu))
        // 共用層作業的異動記錄（ADR 0012 §8；功能 history）
        .route("/api/history/{program}/{key}", get(shared_history))
        // Department Maintenance 部門維護（TipTop aooi030；工廠層）
        // Classification Code 分類碼維護（TipTop aooi300）：工廠層，依類別
        .route(
            "/api/plants/{plant}/classification-codes/{kind}",
            get(list_codes).post(create_code),
        )
        .route(
            "/api/plants/{plant}/classification-codes/{kind}/{code}",
            axum::routing::put(update_code).delete(delete_code),
        )
        .route(
            "/api/plants/{plant}/classification-codes/{kind}/{code}/deactivate",
            post(deactivate_code),
        )
        .route(
            "/api/plants/{plant}/classification-codes/{kind}/{code}/activate",
            post(activate_code),
        )
        // 查詢、複製、確認／取消確認、列印、匯出 Excel（ADR 0013；classification_code_standard_actions.feature）
        .route(
            "/api/plants/{plant}/classification-codes/{kind}/query",
            post(query_codes),
        )
        .route(
            "/api/plants/{plant}/classification-codes/{kind}/report.{format}",
            get(code_report),
        )
        .route(
            "/api/plants/{plant}/classification-codes/{kind}/{code}/copy",
            post(copy_code),
        )
        .route(
            "/api/plants/{plant}/classification-codes/{kind}/{code}/confirm",
            post(confirm_code),
        )
        .route(
            "/api/plants/{plant}/classification-codes/{kind}/{code}/unconfirm",
            post(unconfirm_code),
        )
        .route(
            "/api/plants/{plant}/classification-codes/{kind}/{code}/history",
            get(code_history),
        )
        .route("/api/plants/{plant}/departments", post(create_department))
        .route(
            "/api/plants/{plant}/departments/query",
            post(query_departments),
        )
        // 列印、匯出 Excel（aooi030 output／exporttoexcel；department_report.feature，ADR 0011）
        .route(
            "/api/plants/{plant}/departments/report.{format}",
            get(department_report),
        )
        .route(
            "/api/plants/{plant}/departments/{code}",
            get(get_department)
                .put(update_department)
                .delete(delete_department),
        )
        .route(
            "/api/plants/{plant}/departments/{code}/accounting",
            axum::routing::put(set_department_accounting),
        )
        .route(
            "/api/plants/{plant}/departments/{code}/deactivate",
            post(deactivate_department),
        )
        .route(
            "/api/plants/{plant}/departments/{code}/activate",
            post(activate_department),
        )
        // 複製、確認／取消確認（ADR 0013；department_employee_standard_actions.feature）
        .route(
            "/api/plants/{plant}/departments/{code}/copy",
            post(copy_department),
        )
        .route(
            "/api/plants/{plant}/departments/{code}/{state}",
            post(confirm_department),
        )
        .route(
            "/api/plants/{plant}/departments/{code}/history",
            get(department_history),
        )
        // Employee Maintenance 員工維護（TipTop aooi040；工廠層）
        .route("/api/plants/{plant}/employees", post(create_employee))
        .route("/api/plants/{plant}/employees/query", post(query_employees))
        .route(
            "/api/plants/{plant}/employees/{id}",
            get(get_employee)
                .put(update_employee)
                .delete(delete_employee),
        )
        .route(
            "/api/plants/{plant}/employees/{id}/deactivate",
            post(deactivate_employee),
        )
        .route(
            "/api/plants/{plant}/employees/{id}/activate",
            post(activate_employee),
        )
        // 複製、確認／取消確認、列印、匯出 Excel（ADR 0013）
        .route(
            "/api/plants/{plant}/employees/{id}/copy",
            post(copy_employee),
        )
        .route(
            "/api/plants/{plant}/employees/{id}/{state}",
            post(confirm_employee),
        )
        .route(
            "/api/plants/{plant}/employees/report.{format}",
            get(employee_report),
        )
        .route(
            "/api/plants/{plant}/employees/{id}/history",
            get(employee_history),
        )
        // Plant Maintenance 營運中心維護（TipTop aooi901）
        .route("/api/plants", get(list_plants).post(create_plant))
        .route("/api/plants/query", post(query_plants))
        .route(
            "/api/plants/{id}",
            get(get_plant).put(update_plant).delete(delete_plant),
        )
        .route("/api/plants/{id}/copy", post(copy_plant))
        .route("/api/plants/{id}/deactivate", post(deactivate_plant))
        .route("/api/plants/{id}/activate", post(activate_plant))
        // 確認／取消確認、列印、匯出 Excel（ADR 0013；plant_maintenance_standard_actions.feature）
        .route("/api/plants/{id}/confirm", post(confirm_plant))
        .route("/api/plants/{id}/unconfirm", post(unconfirm_plant))
        .route("/api/plants/report.{format}", get(plant_report))
        // Permission Class Maintenance 權限類別維護（TipTop p_zw）
        .route(
            "/api/permission-classes",
            get(list_classes).post(create_class),
        )
        .route("/api/permission-classes/query", post(query_classes))
        .route(
            "/api/permission-classes/update-user-menus",
            post(update_user_menus),
        )
        .route(
            "/api/permission-classes/{code}",
            get(get_class).put(update_class).delete(delete_class),
        )
        .route(
            "/api/permission-classes/{code}/deactivate",
            post(deactivate_class),
        )
        .route(
            "/api/permission-classes/{code}/activate",
            post(activate_class),
        )
        // 複製、確認／取消確認、列印、匯出 Excel（ADR 0013；permission_class_maintenance_standard_actions.feature）
        .route("/api/permission-classes/{code}/copy", post(copy_class))
        .route(
            "/api/permission-classes/{code}/confirm",
            post(confirm_class),
        )
        .route(
            "/api/permission-classes/{code}/unconfirm",
            post(unconfirm_class),
        )
        .route("/api/permission-classes/report.{format}", get(class_report))
        // Program Permission Maintenance 程式執行權限維護（TipTop p_zy）
        .route(
            "/api/program-permissions/query",
            post(query_program_permissions),
        )
        .route(
            "/api/permission-classes/{code}/permission-query",
            post(query_class_permissions),
        )
        .route(
            "/api/permission-classes/{code}/permissions",
            get(list_class_permissions)
                .post(grant_class_permission)
                .delete(revoke_all_permissions),
        )
        .route(
            "/api/permission-classes/{code}/permissions/{program}",
            axum::routing::put(update_class_permission).delete(revoke_class_permission),
        )
        .route(
            "/api/permission-classes/{code}/copy-permissions",
            post(copy_permissions),
        )
        .route(
            "/api/permission-classes/{code}/generate-permissions",
            post(generate_permissions),
        )
        .route(
            "/api/permission-classes/{code}/generate-permissions-by-menu",
            post(generate_permissions_by_menu),
        )
        .route(
            "/api/permission-classes/{code}/delete-permissions-by-menu",
            post(delete_permissions_by_menu),
        )
        .layer(axum::middleware::from_fn(change_request))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// 跨來源白名單。Web 版經 nginx / dx proxy 走同源，正常不需要 CORS；
/// 只有前端與 API 不同網域時才設定 `TTRS_CORS_ORIGINS`（逗號分隔）。
pub fn cors_from_env() -> Option<CorsLayer> {
    let raw = std::env::var("TTRS_CORS_ORIGINS").ok()?;
    let origins: Vec<HeaderValue> = raw
        .split(',')
        .map(str::trim)
        .filter(|o| !o.is_empty())
        .filter_map(|o| o.parse().ok())
        .collect();
    if origins.is_empty() {
        return None;
    }
    Some(
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any),
    )
}

// ----- TipTop 對照目錄（legacy_catalog.feature） -----

#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
}

async fn search_legacy(
    Ctx(s): Ctx,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<Vec<LegacyTableSummaryDto>>> {
    Ok(Json(s.legacy.search(&s.actor, &q.q).await?))
}

async fn get_legacy(Ctx(s): Ctx, Path(legacy): Path<String>) -> ApiResult<Json<LegacyTableDto>> {
    Ok(Json(s.legacy.get(&s.actor, &legacy).await?))
}

/// 整批取代（C2）；名稱不合法或重複時 422 且不寫入（C6）
async fn import_legacy(
    Ctx(s): Ctx,
    Json(req): Json<Vec<LegacyTableDto>>,
) -> ApiResult<Json<LegacyImportResultDto>> {
    Ok(Json(s.legacy.import(&s.actor, req).await?))
}

/// 已移植資料表對應的 TipTop 舊表；沒有對照時回傳 null（C1）
async fn table_legacy(
    Ctx(s): Ctx,
    Path(table): Path<String>,
) -> ApiResult<Json<Option<LegacyTableDto>>> {
    Ok(Json(s.legacy.for_table(&s.actor, &table).await?))
}

// ----- User Maintenance 使用者維護（TipTop p_zx） -----

async fn list_users(Ctx(s): Ctx) -> ApiResult<Json<Vec<UserDto>>> {
    Ok(Json(s.users.list().await?))
}

/// QBE 查詢（TipTop p_zx 的 Q.查詢）；條件格式見 ttrs_domain::qbe
async fn query_users(Ctx(s): Ctx, Json(req): Json<QueryRequest>) -> ApiResult<Json<Page<UserDto>>> {
    Ok(Json(s.users.query(&s.actor, req).await?))
}

async fn create_user(
    Ctx(s): Ctx,
    Json(req): Json<UserInput>,
) -> ApiResult<(StatusCode, Json<UserDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.users.create(&s.actor, req).await?),
    ))
}

async fn get_user(Ctx(s): Ctx, Path(id): Path<String>) -> ApiResult<Json<UserDto>> {
    Ok(Json(s.users.get(&s.actor, &id).await?))
}

async fn update_user(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Json(req): Json<UserInput>,
) -> ApiResult<Json<UserDto>> {
    require_version(req.version)?;
    Ok(Json(s.users.update(&s.actor, &id, req).await?))
}

async fn delete_user(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    s.users
        .delete(&s.actor, &id, Some(require_version(v.version)?))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_user(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Json(req): Json<NewUserIdRequest>,
) -> ApiResult<(StatusCode, Json<UserDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.users.copy(&s.actor, &id, &req.new_user_id).await?),
    ))
}

async fn deactivate_user(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<UserDto>> {
    Ok(Json(
        s.users
            .deactivate(&s.actor, &id, Some(require_version(v.version)?))
            .await?,
    ))
}

/// 解除鎖定（p_zx user_unlock）
async fn unlock_user(Ctx(s): Ctx, Path(id): Path<String>) -> ApiResult<StatusCode> {
    s.users.unlock(&s.actor, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn confirm_user(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<UserDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(s.users.confirm(&s.actor, &id, true, version).await?))
}

async fn unconfirm_user(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<UserDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(s.users.confirm(&s.actor, &id, false, version).await?))
}

async fn user_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .users
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn activate_user(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<UserDto>> {
    Ok(Json(
        s.users
            .activate(&s.actor, &id, Some(require_version(v.version)?))
            .await?,
    ))
}

// ----- Plant Maintenance 營運中心維護（TipTop aooi901） -----

async fn list_plants(Ctx(s): Ctx) -> ApiResult<Json<Vec<PlantDto>>> {
    Ok(Json(s.plants.list().await?))
}

/// QBE 查詢（TipTop aooi901 的 Q.查詢）
async fn query_plants(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<PlantDto>>> {
    Ok(Json(s.plants.query(&s.actor, req).await?))
}

async fn create_plant(
    Ctx(s): Ctx,
    Json(req): Json<PlantInput>,
) -> ApiResult<(StatusCode, Json<PlantDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.plants.create(&s.actor, req).await?),
    ))
}

async fn get_plant(Ctx(s): Ctx, Path(id): Path<String>) -> ApiResult<Json<PlantDto>> {
    Ok(Json(s.plants.get(&s.actor, &id).await?))
}

async fn update_plant(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Json(req): Json<PlantInput>,
) -> ApiResult<Json<PlantDto>> {
    require_version(req.version)?;
    Ok(Json(s.plants.update(&s.actor, &id, req).await?))
}

async fn delete_plant(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    s.plants
        .delete(&s.actor, &id, Some(require_version(v.version)?))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_plant(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Json(req): Json<NewPlantIdRequest>,
) -> ApiResult<(StatusCode, Json<PlantDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.plants.copy(&s.actor, &id, &req.new_plant_id).await?),
    ))
}

async fn deactivate_plant(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<PlantDto>> {
    Ok(Json(
        s.plants
            .deactivate(&s.actor, &id, Some(require_version(v.version)?))
            .await?,
    ))
}

async fn confirm_plant(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<PlantDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(s.plants.confirm(&s.actor, &id, true, version).await?))
}

async fn unconfirm_plant(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<PlantDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(s.plants.confirm(&s.actor, &id, false, version).await?))
}

async fn plant_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .plants
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn activate_plant(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<PlantDto>> {
    Ok(Json(
        s.plants
            .activate(&s.actor, &id, Some(require_version(v.version)?))
            .await?,
    ))
}

// ----- Permission Class Maintenance 權限類別維護（TipTop p_zw） -----

async fn list_classes(Ctx(s): Ctx) -> ApiResult<Json<Vec<PermissionClassDto>>> {
    Ok(Json(s.classes.list().await?))
}

/// QBE 查詢（TipTop p_zw 的 Q.查詢）
async fn query_classes(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<PermissionClassDto>>> {
    Ok(Json(s.classes.query(&s.actor, req).await?))
}

async fn create_class(
    Ctx(s): Ctx,
    Json(req): Json<PermissionClassInput>,
) -> ApiResult<(StatusCode, Json<PermissionClassDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.classes.create(&s.actor, req).await?),
    ))
}

async fn get_class(Ctx(s): Ctx, Path(code): Path<String>) -> ApiResult<Json<PermissionClassDto>> {
    Ok(Json(s.classes.get(&s.actor, &code).await?))
}

async fn update_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<PermissionClassInput>,
) -> ApiResult<Json<PermissionClassDto>> {
    require_version(req.version)?;
    Ok(Json(s.classes.update(&s.actor, &code, req).await?))
}

async fn delete_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    s.classes
        .delete(&s.actor, &code, Some(require_version(v.version)?))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn deactivate_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<PermissionClassDto>> {
    Ok(Json(
        s.classes
            .deactivate(&s.actor, &code, Some(require_version(v.version)?))
            .await?,
    ))
}

#[derive(Deserialize)]
struct NewClassCode {
    new_class_code: String,
}

async fn copy_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<NewClassCode>,
) -> ApiResult<(StatusCode, Json<PermissionClassDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.classes.copy(&s.actor, &code, &req.new_class_code).await?),
    ))
}

async fn confirm_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<PermissionClassDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.classes.confirm(&s.actor, &code, true, version).await?,
    ))
}

async fn unconfirm_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<PermissionClassDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.classes.confirm(&s.actor, &code, false, version).await?,
    ))
}

async fn class_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .classes
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn activate_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<PermissionClassDto>> {
    Ok(Json(
        s.classes
            .activate(&s.actor, &code, Some(require_version(v.version)?))
            .await?,
    ))
}

async fn update_user_menus(Ctx(s): Ctx) -> ApiResult<Json<UpdatedCountDto>> {
    let updated = s.classes.update_user_menus(&s.actor).await?;
    Ok(Json(UpdatedCountDto { updated }))
}

// ----- Program Permission Maintenance 程式執行權限維護（TipTop p_zy） -----

/// QBE 查詢（TipTop p_zy 的 Q.查詢）：符合條件的權限類別
async fn query_program_permissions(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<PermissionClassDto>>> {
    Ok(Json(s.permissions.query(&s.actor, req).await?))
}

/// 權限類別符合同一組查詢條件的程式（單身，YQ3）
async fn query_class_permissions(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Vec<ProgramPermissionDto>>> {
    Ok(Json(s.permissions.query_class(&s.actor, &code, req).await?))
}

async fn list_class_permissions(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ProgramPermissionDto>>> {
    let lang = q.lang.unwrap_or_else(|| DEFAULT_LANG.to_owned());
    Ok(Json(s.permissions.list(&s.actor, &code, &lang).await?))
}

async fn grant_class_permission(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<ProgramPermissionInput>,
) -> ApiResult<(StatusCode, Json<ProgramPermissionDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.permissions.grant(&s.actor, &code, req).await?),
    ))
}

async fn update_class_permission(
    Ctx(s): Ctx,
    Path((code, program)): Path<(String, String)>,
    Json(req): Json<ProgramPermissionInput>,
) -> ApiResult<Json<ProgramPermissionDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.permissions.update(&s.actor, &code, &program, req).await?,
    ))
}

async fn revoke_class_permission(
    Ctx(s): Ctx,
    Path((code, program)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    s.permissions.revoke(&s.actor, &code, &program).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn revoke_all_permissions(
    Ctx(s): Ctx,
    Path(code): Path<String>,
) -> ApiResult<Json<PermissionBatchResultDto>> {
    Ok(Json(s.permissions.revoke_all(&s.actor, &code).await?))
}

async fn copy_permissions(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<CopyPermissionsRequest>,
) -> ApiResult<Json<PermissionBatchResultDto>> {
    Ok(Json(
        s.permissions
            .copy(&s.actor, &code, &req.target_class)
            .await?,
    ))
}

async fn generate_permissions(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<GeneratePermissionsRequest>,
) -> ApiResult<Json<PermissionBatchResultDto>> {
    Ok(Json(
        s.permissions
            .generate(&s.actor, &code, &req.program_nos)
            .await?,
    ))
}

async fn generate_permissions_by_menu(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<MenuPermissionsRequest>,
) -> ApiResult<Json<PermissionBatchResultDto>> {
    Ok(Json(
        s.permissions
            .generate_by_menu(&s.actor, &code, &req.menu, req.reset_existing)
            .await?,
    ))
}

async fn delete_permissions_by_menu(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<MenuPermissionsRequest>,
) -> ApiResult<Json<PermissionBatchResultDto>> {
    Ok(Json(
        s.permissions
            .delete_by_menu(&s.actor, &code, &req.menu)
            .await?,
    ))
}

// ----- User Special Permission Maintenance 使用者特殊權限維護（TipTop p_zxw） -----

async fn list_user_permissions(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<UserPermissionDto>>> {
    let lang = q.lang.unwrap_or_else(|| DEFAULT_LANG.to_owned());
    Ok(Json(s.special.list(&s.actor, &id, &lang).await?))
}

async fn add_user_permission(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Json(req): Json<UserPermissionInput>,
) -> ApiResult<(StatusCode, Json<Vec<UserPermissionDto>>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.special.add(&s.actor, &id, req).await?),
    ))
}

async fn update_user_program_permission(
    Ctx(s): Ctx,
    Path((id, program)): Path<(String, String)>,
    Json(req): Json<UserPermissionInput>,
) -> ApiResult<Json<UserPermissionDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.special
            .update_program(&s.actor, &id, &program, req)
            .await?,
    ))
}

async fn remove_user_permission(
    Ctx(s): Ctx,
    Path((id, line)): Path<(String, i32)>,
) -> ApiResult<StatusCode> {
    s.special.remove(&s.actor, &id, line).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_all_user_permissions(
    Ctx(s): Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<PermissionBatchResultDto>> {
    Ok(Json(s.special.remove_all(&s.actor, &id).await?))
}

async fn copy_user_permissions(
    Ctx(s): Ctx,
    Path(id): Path<String>,
    Json(req): Json<CopyUserPermissionsRequest>,
) -> ApiResult<Json<PermissionBatchResultDto>> {
    Ok(Json(s.special.copy(&s.actor, &id, &req.target_user).await?))
}

pub struct HttpError(AppError);

impl From<AppError> for HttpError {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let status = match self.0.code() {
            "ITEM_NOT_FOUND"
            | "PROGRAM_NOT_FOUND"
            | "DICT_NOT_FOUND"
            | "LEGACY_TABLE_NOT_FOUND"
            | "USER_NOT_FOUND"
            | "PERMISSION_NOT_FOUND"
            | "DEPARTMENT_NOT_FOUND"
            | "EMPLOYEE_NOT_FOUND"
            | "CODE_NOT_FOUND"
            | "UNIT_NOT_FOUND"
            | "ENCODING_NATURE_NOT_FOUND"
            | "GROUP_CENTER_NOT_FOUND"
            | "DATA_CONTROL_PLANT_NOT_FOUND"
            | "CARRY_SETTING_NOT_FOUND"
            | "WAREHOUSE_LOCATION_NOT_FOUND"
            | "ITEM_STORAGE_NOT_FOUND"
            | "DOCUMENT_TYPE_NOT_FOUND"
            | "INVENTORY_TRANSACTION_NOT_FOUND"
            | "CARRY_TARGET_NOT_FOUND"
            | "CARRY_CONDITION_NOT_FOUND"
            | "CARRY_DEFAULT_NOT_FOUND"
            | "DATA_CONTROL_RULE_NOT_FOUND"
            | "DATA_CENTER_TYPE_NOT_FOUND"
            | "ENCODING_SEGMENT_NOT_FOUND"
            | "ENCODING_YEAR_NOT_FOUND"
            | "ENCODING_CLASS_NOT_FOUND"
            | "ENCODING_CLASS_LINE_NOT_FOUND"
            | "ENCODING_SEGMENT_LINE_NOT_FOUND"
            | "PRODUCT_CLASS_NOT_FOUND"
            | "ITEM_GROUP_NOT_FOUND"
            | "CURRENCY_NOT_FOUND"
            | "EXCHANGE_RATE_NOT_FOUND"
            | "ACCOUNTING_YEAR_NOT_FOUND"
            | "UNIT_CONVERSION_NOT_FOUND"
            | "ACCOUNTING_PERIOD_NOT_FOUND"
            | "TAX_TYPE_NOT_FOUND"
            | "PROJECT_NOT_FOUND"
            | "REGION_NOT_FOUND"
            | "COUNTRY_NOT_FOUND"
            | "AREA_NOT_FOUND"
            | "SHIPPING_METHOD_NOT_FOUND"
            | "REPORT_TITLE_NOT_FOUND" => StatusCode::NOT_FOUND,
            "PASSWORD_RESET_UNAVAILABLE" => StatusCode::SERVICE_UNAVAILABLE,
            "EPUB_TOO_LARGE" => StatusCode::PAYLOAD_TOO_LARGE,
            "EPUB_FORMAT_INVALID" | "TRANSLATE_CONSENT_REQUIRED" => StatusCode::BAD_REQUEST,
            "EPUB_NOT_ENGLISH" => StatusCode::UNPROCESSABLE_ENTITY,
            "TRANSLATE_LIMIT_EXCEEDED" => StatusCode::PAYLOAD_TOO_LARGE,
            "TRANSLATE_BUSY" => StatusCode::CONFLICT,
            "TRANSLATE_NOT_CONFIGURED" => StatusCode::SERVICE_UNAVAILABLE,
            "TRANSLATE_QUOTA_EXHAUSTED" => StatusCode::TOO_MANY_REQUESTS,
            "TRANSLATE_FAILED" => StatusCode::BAD_GATEWAY,
            "TRANSLATE_JOB_NOT_FOUND" | "EPUB_BOOK_NOT_FOUND" => StatusCode::NOT_FOUND,
            "EPUB_SOURCE_UNAVAILABLE" | "URL_UNREACHABLE" => StatusCode::BAD_GATEWAY,
            "URL_NOT_ALLOWED" | "URL_ROBOTS_DISALLOWED" => StatusCode::FORBIDDEN,
            "PDF_FONT_MISSING" => StatusCode::SERVICE_UNAVAILABLE,
            "GITHUB_RATE_LIMITED" => StatusCode::TOO_MANY_REQUESTS,
            "GITHUB_UNAVAILABLE" => StatusCode::BAD_GATEWAY,
            "GITHUB_NOT_FOUND" => StatusCode::NOT_FOUND,
            "EPUB_LIBRARY_DUPLICATE" | "EPUB_LIBRARY_FULL" => StatusCode::CONFLICT,
            // 匯率來源（台銀、關務署網站）無法取得
            "EXCHANGE_RATE_SOURCE_UNAVAILABLE" => StatusCode::BAD_GATEWAY,
            "NOT_SIGNED_IN"
            | "SIGN_IN_FAILED"
            | "USER_LOCKED"
            | "USER_NOT_REGISTERED"
            | "USER_INACTIVE"
            | "USER_EXPIRED" => StatusCode::UNAUTHORIZED,
            "PASSWORD_CHANGE_REQUIRED" => StatusCode::FORBIDDEN,
            "PLANT_NOT_ALLOWED" | "NO_USABLE_PLANT" => StatusCode::FORBIDDEN,
            "REPORT_TOO_LARGE" => StatusCode::PAYLOAD_TOO_LARGE,
            "RECORD_CHANGED"
            | "RECORD_ALREADY_CONFIRMED"
            | "RECORD_NOT_CONFIRMED"
            | "RECORD_CONFIRMED_NOT_DELETABLE" => StatusCode::CONFLICT,
            "MENU_NOT_FOUND" | "MENU_ITEM_NOT_FOUND" => StatusCode::NOT_FOUND,
            "MENU_SEQ_DUPLICATE"
            | "MENU_ITEM_DUPLICATE"
            | "MENU_IN_USE"
            | "MENU_ALREADY_HAS_ITEMS"
            | "MENU_NOT_EMPTY"
            | "UNIT_ALREADY_EXISTS"
            | "UNIT_IN_USE"
            | "ENCODING_NATURE_ALREADY_EXISTS"
            | "ENCODING_NATURE_IN_USE"
            | "ENCODING_SEGMENT_ALREADY_EXISTS"
            | "GROUP_CENTER_ALREADY_EXISTS"
            | "DATA_CONTROL_PLANT_ALREADY_EXISTS"
            | "CARRY_SETTING_ALREADY_EXISTS"
            | "WAREHOUSE_ALREADY_EXISTS"
            | "WAREHOUSE_IN_USE"
            | "WAREHOUSE_INACTIVE"
            | "WAREHOUSE_LOCATION_ALREADY_EXISTS"
            | "WAREHOUSE_LOCATION_IN_USE"
            | "ITEM_STORAGE_ALREADY_EXISTS"
            | "DOCUMENT_TYPE_ALREADY_EXISTS"
            | "DOCUMENT_TYPE_INACTIVE"
            | "DOC_CODE_LENGTH_IN_USE"
            | "DOC_SERIAL_LENGTH_IN_USE"
            | "INVENTORY_TRANSACTION_ALREADY_EXISTS"
            | "INVENTORY_TRANSACTION_CONFIRMED"
            | "INVENTORY_TRANSACTION_VOIDED"
            | "INVENTORY_TRANSACTION_POSTED"
            | "INVENTORY_TRANSACTION_NOT_POSTED"
            | "INVENTORY_PERIOD_CLOSED"
            | "INVENTORY_PERIOD_NOT_OPEN"
            | "WAREHOUSE_TRANSFER_SAME_STORAGE"
            | "WAREHOUSE_TRANSFER_STORAGE_NOT_ASSIGNED"
            | "ITEM_STORAGE_COPY_TARGET_NOT_EMPTY"
            | "CARRY_SETTING_IN_USE"
            | "BASIC_CARRY_NOT_SOURCE"
            | "ITEM_CARRY_NOT_SOURCE"
            | "ITEM_CARRY_NOT_CONFIRMED"
            | "ITEM_CARRIED_NOT_EDITABLE"
            | "ITEM_CARRIED_NOT_DELETABLE"
            | "BASIC_CARRY_NO_TARGET"
            | "CARRY_TARGET_ALREADY_EXISTS"
            | "CARRY_CONDITION_ALREADY_EXISTS"
            | "CARRY_DEFAULT_ALREADY_EXISTS"
            | "DATA_CONTROL_RULE_ALREADY_EXISTS"
            | "DATA_CONTROL_NO_DATA_CENTER"
            | "DATA_CONTROL_RULE_NOT_ALLOWED"
            | "GROUP_CENTER_IN_USE"
            | "GROUP_CENTER_HAS_DATA_TYPES"
            | "DATA_CENTER_TYPE_ALREADY_EXISTS"
            | "ENCODING_YEAR_ALREADY_EXISTS"
            | "ENCODING_CLASS_ALREADY_EXISTS"
            | "ENCODING_CLASS_IN_USE"
            | "ENCODING_CLASS_INACTIVE"
            | "ENCODING_CLASS_SERIAL_DUPLICATE"
            | "ENCODING_RULE_SERIAL_EXHAUSTED"
            | "ITEM_AUTO_NUMBER_DISABLED"
            | "ENCODING_CLASS_LINE_DUPLICATE"
            | "ENCODING_SEGMENT_IN_USE"
            | "ENCODING_SEGMENT_INACTIVE"
            | "ENCODING_SEGMENT_LENGTH_HAS_VALUES"
            | "ENCODING_SEGMENT_VALUE_DUPLICATE"
            | "ENCODING_SEGMENT_LINE_DUPLICATE"
            | "PRODUCT_CLASS_ALREADY_EXISTS"
            | "PRODUCT_CLASS_IN_USE"
            | "ITEM_GROUP_ALREADY_EXISTS"
            | "ITEM_GROUP_IN_USE"
            | "CURRENCY_ALREADY_EXISTS"
            | "CURRENCY_IN_USE"
            | "EXCHANGE_RATE_ALREADY_EXISTS"
            | "EXCHANGE_RATE_IMPORT_OVERWRITE_REQUIRED"
            | "BASE_CURRENCY_IN_USE"
            | "ACCOUNTING_YEAR_ALREADY_EXISTS"
            | "UNIT_CONVERSION_ALREADY_EXISTS"
            | "ACCOUNTING_YEAR_OVERLAP"
            | "ACCOUNTING_PERIODS_IN_USE"
            | "TAX_TYPE_ALREADY_EXISTS"
            | "TAX_TYPE_IN_USE"
            | "PROJECT_ALREADY_EXISTS"
            | "PROJECT_IN_USE"
            | "REGION_ALREADY_EXISTS"
            | "REGION_IN_USE"
            | "COUNTRY_ALREADY_EXISTS"
            | "COUNTRY_IN_USE"
            | "AREA_ALREADY_EXISTS"
            | "AREA_IN_USE"
            | "SHIPPING_METHOD_ALREADY_EXISTS"
            | "SHIPPING_METHOD_IN_USE"
            | "REPORT_TITLE_ALREADY_EXISTS"
            | "REPORT_TITLE_IN_USE" => StatusCode::CONFLICT,
            "VERSION_REQUIRED" => StatusCode::BAD_REQUEST,
            "ITEM_ALREADY_EXISTS"
            | "ITEM_INVALID_TRANSITION"
            | "ITEM_HAS_TRANSACTIONS"
            | "ITEM_INACTIVE"
            | "PROGRAM_ALREADY_EXISTS"
            | "DICT_ALREADY_EXISTS"
            | "USER_ALREADY_EXISTS"
            | "IDP_ACCOUNT_IN_USE"
            | "PERMISSION_CLASS_ALREADY_EXISTS"
            | "PERMISSION_CLASS_IN_USE"
            | "PERMISSION_CLASS_HAS_PERMISSIONS"
            | "PLANT_ALREADY_EXISTS"
            | "PLANT_IN_USE"
            | "PLANT_LAST_ACTIVE"
            | "CODE_ALREADY_EXISTS"
            | "CODE_IN_USE"
            | "PLANT_SCHEMA_DUPLICATE"
            | "DEPARTMENT_ALREADY_EXISTS"
            | "DEPARTMENT_IN_USE"
            | "EMPLOYEE_ALREADY_EXISTS"
            | "PERMISSION_ALREADY_EXISTS"
            | "USER_PERMISSION_DUPLICATE"
            | "USER_HAS_SPECIAL_PERMISSIONS"
            | "LAST_USER_ADMIN" => StatusCode::CONFLICT,
            "PERMISSION_DENIED" => StatusCode::FORBIDDEN,
            "INFRA_ERROR" => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        if status.is_server_error() {
            tracing::error!(error = %self.0, "request failed");
        }
        let body = ApiError {
            code: self.0.code().to_owned(),
            message: self.0.to_string(),
        };
        (status, Json(body)).into_response()
    }
}

type ApiResult<T> = Result<T, HttpError>;

async fn current_session(Ctx(s): Ctx) -> ApiResult<Json<SessionDto>> {
    Ok(Json(s.sign_in.revalidate(&s.actor.user_id).await?))
}

/// 登入（weblogin，ADR 0009）：成功時建立連線並設定 cookie
async fn sign_in(
    State(s): State<AppState>,
    Json(input): Json<SignInInput>,
) -> Result<Response, HttpError> {
    let AuthMode::Session {
        sessions,
        secure_cookie,
    } = &s.auth
    else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let result = s
        .sign_in
        .sign_in_at(&input.user_id, &input.password, input.plant_id.as_deref())
        .await;
    match &result {
        Ok(session) => {
            tracing::info!(user = %session.user_id, plant = %session.plant, "sign-in")
        }
        Err(e) => tracing::info!(user = %input.user_id, code = e.code(), "sign-in failed"),
    }
    let session = result?;
    let token = sessions.open(&session);
    Ok((
        [(
            header::SET_COOKIE,
            session::set_cookie(Some(&token), *secure_cookie),
        )],
        Json(session),
    )
        .into_response())
}

async fn sign_out(State(s): State<AppState>, headers: axum::http::HeaderMap) -> Response {
    let secure = match &s.auth {
        AuthMode::Session {
            sessions,
            secure_cookie,
        } => {
            if let Some(token) = session::token(&headers) {
                sessions.close(&token);
            }
            *secure_cookie
        }
        AuthMode::Fixed => false,
    };
    (
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, session::set_cookie(None, secure))],
    )
        .into_response()
}

/// 變更密碼（webpasswd）：以舊密碼驗證，須改密碼時尚未登入也能使用
async fn change_password(
    State(s): State<AppState>,
    Json(input): Json<ChangePasswordInput>,
) -> ApiResult<StatusCode> {
    s.sign_in
        .change_password(
            &input.user_id,
            &input.old_password,
            &input.new_password,
            &input.confirm_password,
        )
        .await?;
    tracing::info!(user = %input.user_id, "password changed");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SignInPlantsQuery {
    user_id: String,
}

/// 營運中心選項：不需密碼（SQ11 選項 A），每次查詢都記錄
async fn sign_in_plants(
    State(s): State<AppState>,
    Query(q): Query<SignInPlantsQuery>,
) -> ApiResult<Json<SignInPlantsDto>> {
    let options = s.sign_in.plant_options(&q.user_id).await?;
    tracing::info!(user = %q.user_id, count = options.plants.len(), "sign-in plant options");
    Ok(Json(options))
}

/// 忘記密碼是否開放（R9）
async fn password_reset_status(State(s): State<AppState>) -> Json<PasswordResetStatusDto> {
    Json(PasswordResetStatusDto {
        available: s.password_reset.available(),
    })
}

/// 申請重設（R2）：帳號不存在、沒有 Email 也一樣回 204，不透露帳號是否存在
async fn request_password_reset(
    State(s): State<AppState>,
    Json(input): Json<PasswordResetRequestInput>,
) -> ApiResult<StatusCode> {
    s.password_reset.request(&input.user_id).await?;
    tracing::info!(user = %input.user_id, "password reset requested");
    Ok(StatusCode::NO_CONTENT)
}

/// 以重設連結設定新密碼；成功後該使用者原本的連線在下一次請求時失效（R6）
async fn confirm_password_reset(
    State(s): State<AppState>,
    Json(input): Json<PasswordResetInput>,
) -> ApiResult<StatusCode> {
    s.password_reset
        .reset(&input.token, &input.new_password, &input.confirm_password)
        .await?;
    tracing::info!("password reset with link");
    Ok(StatusCode::NO_CONTENT)
}

async fn switch_plant(
    Ctx(s): Ctx,
    Json(input): Json<SwitchPlantInput>,
) -> ApiResult<Json<SessionDto>> {
    Ok(Json(
        s.sign_in
            .switch_plant(&s.actor.user_id, &input.plant_id)
            .await?,
    ))
}

async fn my_permission(
    Ctx(s): Ctx,
    Path(program): Path<String>,
) -> ApiResult<Json<MyPermissionDto>> {
    Ok(Json(
        ttrs_app::authorization::my_permission(s.authorizer.as_ref(), &s.actor, &program).await?,
    ))
}

// ----- Item Maintenance 料件基本資料維護（aimi100；工廠層） -----

async fn query_items(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ItemDto>>> {
    Ok(Json(s.items.query(&s.actor, &plant, req).await?))
}

async fn list_items(Ctx(s): Ctx, Path(plant): Path<String>) -> ApiResult<Json<Vec<ItemDto>>> {
    Ok(Json(s.items.list(&s.actor, &plant).await?))
}

async fn get_item(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<ItemDto>> {
    Ok(Json(
        s.items
            .get(&s.actor, &plant, &no, q.lang.as_deref())
            .await?,
    ))
}

async fn create_item(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ttrs_shared::ItemInput>,
) -> ApiResult<(StatusCode, Json<ItemDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.items.create(&s.actor, &plant, req).await?),
    ))
}

async fn update_item(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::ItemInput>,
) -> ApiResult<Json<ItemDto>> {
    require_version(req.version)?;
    Ok(Json(s.items.update(&s.actor, &plant, &no, req).await?))
}

/// 確認 confirm、取消確認 unconfirm、作廢／恢復 void、訂單無效切換 order-invalid（須帶 ?version=）
async fn item_action(
    Ctx(s): Ctx,
    Path((plant, no, action)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let dto = match action.as_str() {
        "confirm" => s.items.confirm(&s.actor, &plant, &no, version).await?,
        "unconfirm" => s.items.unconfirm(&s.actor, &plant, &no, version).await?,
        "void" => s.items.toggle_void(&s.actor, &plant, &no, version).await?,
        "order-invalid" => {
            s.items
                .toggle_order_invalid(&s.actor, &plant, &no, version)
                .await?
        }
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn copy_item(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::CopyItemRequest>,
) -> ApiResult<(StatusCode, Json<ItemDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.items.copy(&s.actor, &plant, &no, req).await?),
    ))
}

async fn set_item_text(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::ItemTextDto>,
) -> ApiResult<Json<ItemDto>> {
    Ok(Json(s.items.set_text(&s.actor, &plant, &no, req).await?))
}

async fn delete_item(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.items.delete(&s.actor, &plant, &no, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn item_history(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.items
            .history(&s.actor, &plant, &no, q.lang.as_deref())
            .await?,
    ))
}

/// 分群碼帶入料件的預設值（IM25）
async fn item_group_defaults(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
) -> ApiResult<Json<ttrs_shared::ItemInput>> {
    Ok(Json(s.items.group_defaults(&s.actor, &plant, &code).await?))
}

// ----- Program Maintenance 程式維護（TipTop p_zz） -----

async fn list_programs(Ctx(s): Ctx) -> ApiResult<Json<Vec<ProgramDto>>> {
    Ok(Json(s.programs.list().await?))
}

/// QBE 查詢（比照 TipTop p_zz 的 Q.查詢）；條件格式見 ttrs_domain::qbe
async fn query_programs(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ProgramDto>>> {
    Ok(Json(s.programs.query(&s.actor, req).await?))
}

async fn create_program(
    Ctx(s): Ctx,
    Json(req): Json<ProgramInput>,
) -> ApiResult<(StatusCode, Json<ProgramDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.programs.create(&s.actor, req).await?),
    ))
}

/// `?lang=en` 依語系顯示名稱（沒有該語系時退回繁中）
async fn get_program(
    Ctx(s): Ctx,
    Path(no): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<ProgramDto>> {
    Ok(Json(match q.lang {
        Some(lang) => s.programs.get_localized(&s.actor, &no, &lang).await?,
        None => s.programs.get(&s.actor, &no).await?,
    }))
}

/// 維護某語系的程式名稱與報表抬頭（gaz_file）
async fn set_program_name(
    Ctx(s): Ctx,
    Path(no): Path<String>,
    Json(req): Json<ProgramNameDto>,
) -> ApiResult<StatusCode> {
    s.programs.set_name(&s.actor, &no, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_program(
    Ctx(s): Ctx,
    Path(no): Path<String>,
    Json(req): Json<ProgramInput>,
) -> ApiResult<Json<ProgramDto>> {
    require_version(req.version)?;
    Ok(Json(s.programs.update(&s.actor, &no, req).await?))
}

async fn delete_program(
    Ctx(s): Ctx,
    Path(no): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    s.programs
        .delete(&s.actor, &no, Some(require_version(v.version)?))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_program(
    Ctx(s): Ctx,
    Path(no): Path<String>,
    Json(req): Json<NewProgramNoRequest>,
) -> ApiResult<(StatusCode, Json<ProgramDto>)> {
    let dto = s.programs.copy(&s.actor, &no, &req.new_no).await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

async fn list_permissions(Ctx(s): Ctx, Path(no): Path<String>) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(s.programs.permission_classes(&s.actor, &no).await?))
}

async fn grant_permission(
    Ctx(s): Ctx,
    Path(no): Path<String>,
    Json(req): Json<GrantRequest>,
) -> ApiResult<StatusCode> {
    s.programs
        .grant(&s.actor, &no, &req.permission_class)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- Data Dictionary Maintenance 資料字典維護（TipTop p_zta） -----

async fn schema_tables(Ctx(s): Ctx) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(s.dictionary.schema_tables(&s.actor).await?))
}

/// `?lang=` 決定顯示語系（ADR 0005）；回應的 `texts` 含所有語系
async fn list_dictionaries(
    Ctx(s): Ctx,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<TableDictionaryDto>>> {
    let lang = q.lang.as_deref().unwrap_or(DEFAULT_LANG);
    Ok(Json(s.dictionary.list_localized(&s.actor, lang).await?))
}

async fn create_dictionary(
    Ctx(s): Ctx,
    Json(req): Json<TableDictionaryInput>,
) -> ApiResult<(StatusCode, Json<TableDictionaryDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.dictionary.create(&s.actor, req).await?),
    ))
}

async fn get_dictionary(
    Ctx(s): Ctx,
    Path(table): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<TableDictionaryDto>> {
    let lang = q.lang.as_deref().unwrap_or(DEFAULT_LANG);
    Ok(Json(
        s.dictionary.get_localized(&s.actor, &table, lang).await?,
    ))
}

async fn update_dictionary(
    Ctx(s): Ctx,
    Path(table): Path<String>,
    Json(req): Json<TableDictionaryInput>,
) -> ApiResult<Json<TableDictionaryDto>> {
    require_version(req.version)?;
    Ok(Json(s.dictionary.update(&s.actor, &table, req).await?))
}

async fn delete_dictionary(
    Ctx(s): Ctx,
    Path(table): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    s.dictionary
        .delete(&s.actor, &table, Some(require_version(v.version)?))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn table_texts(Ctx(s): Ctx, Path(table): Path<String>) -> ApiResult<Json<Vec<TableTextDto>>> {
    Ok(Json(s.dictionary.table_texts(&s.actor, &table).await?))
}

async fn set_table_text(
    Ctx(s): Ctx,
    Path(table): Path<String>,
    Json(req): Json<TableTextDto>,
) -> ApiResult<StatusCode> {
    s.dictionary.set_table_text(&s.actor, &table, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn table_columns(
    Ctx(s): Ctx,
    Path(table): Path<String>,
) -> ApiResult<Json<Vec<ColumnInfoDto>>> {
    Ok(Json(s.dictionary.columns(&s.actor, &table).await?))
}

async fn column_texts(
    Ctx(s): Ctx,
    Path(table): Path<String>,
) -> ApiResult<Json<Vec<ColumnTextDto>>> {
    Ok(Json(s.dictionary.column_texts(&s.actor, &table).await?))
}

async fn set_column_text(
    Ctx(s): Ctx,
    Path(table): Path<String>,
    Json(req): Json<ColumnTextDto>,
) -> ApiResult<StatusCode> {
    s.dictionary.set_column_text(&s.actor, &table, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn table_indexes(Ctx(s): Ctx, Path(table): Path<String>) -> ApiResult<Json<Vec<IndexDto>>> {
    Ok(Json(s.dictionary.indexes(&s.actor, &table).await?))
}

async fn table_constraints(
    Ctx(s): Ctx,
    Path(table): Path<String>,
) -> ApiResult<Json<Vec<ConstraintDto>>> {
    Ok(Json(s.dictionary.constraints(&s.actor, &table).await?))
}

async fn table_history(
    Ctx(s): Ctx,
    Path(table): Path<String>,
) -> ApiResult<Json<Vec<MigrationDto>>> {
    Ok(Json(s.dictionary.history(&s.actor, &table).await?))
}

#[derive(Deserialize)]
struct LangQuery {
    lang: Option<String>,
}

/// 匯出 CSV（D6）；開頭加 UTF-8 BOM，Excel 才會以 UTF-8 開啟中文
async fn program_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .programs
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn program_permission_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .permissions
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn query_user_permissions(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::SpecialPermissionUserDto>>> {
    Ok(Json(s.special.query(&s.actor, req).await?))
}

async fn user_permission_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .special
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn dictionary_report(
    Ctx(s): Ctx,
    Path((table, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, _, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .dictionary
        .report_in(&s.actor, &table, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn export_dictionary(
    Ctx(s): Ctx,
    Path(table): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Response> {
    let lang = q.lang.unwrap_or_else(|| DEFAULT_LANG.to_owned());
    let csv = s.dictionary.export_csv(&s.actor, &table, &lang).await?;
    let filename = format!("attachment; filename=\"{table}_{lang}.csv\"");
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (header::CONTENT_DISPOSITION, filename),
        ],
        format!("\u{feff}{csv}"),
    )
        .into_response())
}

// ----- 部門維護（aooi030）、員工維護（aooi040）：工廠層，路徑帶營運中心 -----

async fn query_departments(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<DepartmentDto>>> {
    Ok(Json(s.departments.query(&s.actor, &plant, req).await?))
}

// ----- Classification Code 分類碼維護（TipTop aooi300） -----

async fn list_codes(
    Ctx(s): Ctx,
    Path((plant, kind)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ClassificationCodeDto>>> {
    Ok(Json(s.classification.list(&s.actor, &plant, &kind).await?))
}

async fn create_code(
    Ctx(s): Ctx,
    Path((plant, kind)): Path<(String, String)>,
    Json(req): Json<ClassificationCodeInput>,
) -> ApiResult<(StatusCode, Json<ClassificationCodeDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(
            s.classification
                .create(&s.actor, &plant, &kind, req)
                .await?,
        ),
    ))
}

async fn update_code(
    Ctx(s): Ctx,
    Path((plant, kind, code)): Path<(String, String, String)>,
    Json(req): Json<ClassificationCodeInput>,
) -> ApiResult<Json<ClassificationCodeDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.classification
            .update(&s.actor, &plant, &kind, &code, req)
            .await?,
    ))
}

async fn delete_code(
    Ctx(s): Ctx,
    Path((plant, kind, code)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    s.classification
        .delete(
            &s.actor,
            &plant,
            &kind,
            &code,
            Some(require_version(v.version)?),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn deactivate_code(
    Ctx(s): Ctx,
    Path((plant, kind, code)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<ClassificationCodeDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.classification
            .set_active(&s.actor, &plant, &kind, &code, false, version)
            .await?,
    ))
}

async fn query_codes(
    Ctx(s): Ctx,
    Path((plant, kind)): Path<(String, String)>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ClassificationCodeDto>>> {
    Ok(Json(
        s.classification.query(&s.actor, &plant, &kind, req).await?,
    ))
}

async fn copy_code(
    Ctx(s): Ctx,
    Path((plant, kind, code)): Path<(String, String, String)>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<ClassificationCodeDto>)> {
    let c = s
        .classification
        .copy(&s.actor, &plant, &kind, &code, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(c)))
}

async fn confirm_code(
    Ctx(s): Ctx,
    Path((plant, kind, code)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<ClassificationCodeDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.classification
            .confirm(&s.actor, &plant, &kind, &code, true, version)
            .await?,
    ))
}

async fn unconfirm_code(
    Ctx(s): Ctx,
    Path((plant, kind, code)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<ClassificationCodeDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.classification
            .confirm(&s.actor, &plant, &kind, &code, false, version)
            .await?,
    ))
}

async fn code_report(
    Ctx(s): Ctx,
    Path((plant, kind, format)): Path<(String, String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .classification
        .report_in(&s.actor, &plant, &kind, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn activate_code(
    Ctx(s): Ctx,
    Path((plant, kind, code)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<ClassificationCodeDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.classification
            .set_active(&s.actor, &plant, &kind, &code, true, version)
            .await?,
    ))
}

#[derive(Deserialize)]
struct ReportQuery {
    /// 查詢條件（QbeCondition 陣列的 JSON）；空白為全部
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    lang: Option<String>,
}

/// 報表檔：PDF 以內嵌方式開啟（瀏覽器預覽、列印），Excel 為下載
fn report_response(
    data: &ttrs_app::report::ReportData,
    format: ttrs_app::report::ReportFormat,
) -> ApiResult<Response> {
    use ttrs_app::report::ReportFormat;
    let (bytes, mime, disposition) = match format {
        ReportFormat::Pdf => (
            ttrs_infra::report::render_pdf(data),
            "application/pdf",
            "inline",
        ),
        ReportFormat::Excel => (
            ttrs_infra::report::render_xlsx(data),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "attachment",
        ),
    };
    let bytes = bytes.map_err(|e| HttpError::from(AppError::Infra(e)))?;
    let file = format!("{}-{}.{}", data.program, data.plant, format.code());
    Ok((
        [
            (header::CONTENT_TYPE, mime.to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("{disposition}; filename=\"{file}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

/// 報表請求：格式（pdf／xlsx，其他為 None → 404）、查詢條件、報表語系
type ReportArgs = (
    ttrs_app::report::ReportFormat,
    Vec<ttrs_shared::QbeCondition>,
    ttrs_domain::lang::Lang,
);

fn report_args(format: &str, q: &ReportQuery) -> ApiResult<Option<ReportArgs>> {
    use ttrs_app::report::ReportFormat;
    let format = match format {
        "pdf" => ReportFormat::Pdf,
        "xlsx" => ReportFormat::Excel,
        _ => return Ok(None),
    };
    let conditions: Vec<ttrs_shared::QbeCondition> =
        match q.q.as_deref().filter(|v| !v.trim().is_empty()) {
            Some(json) => serde_json::from_str(json).map_err(|e| {
                HttpError::from(AppError::from(ttrs_domain::DomainError::QbeInvalidSyntax(
                    e.to_string(),
                )))
            })?,
            None => Vec::new(),
        };
    let lang = ttrs_domain::lang::Lang::parse(q.lang.as_deref().unwrap_or(DEFAULT_LANG))
        .unwrap_or_else(|_| ttrs_domain::lang::Lang::default_lang());
    Ok(Some((format, conditions, lang)))
}

async fn item_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .items
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn item_group_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .item_groups
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn department_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .departments
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn create_department(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<DepartmentInput>,
) -> ApiResult<(StatusCode, Json<DepartmentDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.departments.create(&s.actor, &plant, req).await?),
    ))
}

async fn get_department(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
) -> ApiResult<Json<DepartmentDto>> {
    Ok(Json(s.departments.get(&s.actor, &plant, &code).await?))
}

async fn update_department(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<DepartmentInput>,
) -> ApiResult<Json<DepartmentDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.departments.update(&s.actor, &plant, &code, req).await?,
    ))
}

async fn set_department_accounting(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<DepartmentAccountingInput>,
) -> ApiResult<Json<DepartmentDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.departments
            .set_accounting(&s.actor, &plant, &code, req)
            .await?,
    ))
}

async fn deactivate_department(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<DepartmentDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.departments
            .deactivate(&s.actor, &plant, &code, version)
            .await?,
    ))
}

#[derive(Deserialize)]
struct NewCode {
    new_code: String,
}

/// 確認 confirm、取消確認 unconfirm（ADR 0013 SA9）
fn confirm_state(state: &str) -> Option<bool> {
    match state {
        "confirm" => Some(true),
        "unconfirm" => Some(false),
        _ => None,
    }
}

async fn copy_department(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<DepartmentDto>)> {
    let d = s
        .departments
        .copy(&s.actor, &plant, &code, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(d)))
}

async fn confirm_department(
    Ctx(s): Ctx,
    Path((plant, code, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let Some(confirm) = confirm_state(&state) else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let version = Some(require_version(v.version)?);
    let d = s
        .departments
        .confirm(&s.actor, &plant, &code, confirm, version)
        .await?;
    Ok(Json(d).into_response())
}

async fn copy_employee(
    Ctx(s): Ctx,
    Path((plant, id)): Path<(String, String)>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<EmployeeDto>)> {
    let e = s
        .employees
        .copy(&s.actor, &plant, &id, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(e)))
}

async fn confirm_employee(
    Ctx(s): Ctx,
    Path((plant, id, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let Some(confirm) = confirm_state(&state) else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let version = Some(require_version(v.version)?);
    let e = s
        .employees
        .confirm(&s.actor, &plant, &id, confirm, version)
        .await?;
    Ok(Json(e).into_response())
}

async fn employee_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .employees
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn activate_department(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<DepartmentDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.departments
            .activate(&s.actor, &plant, &code, version)
            .await?,
    ))
}

async fn delete_department(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.departments
        .delete(&s.actor, &plant, &code, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn query_employees(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<EmployeeDto>>> {
    Ok(Json(s.employees.query(&s.actor, &plant, req).await?))
}

async fn create_employee(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<EmployeeInput>,
) -> ApiResult<(StatusCode, Json<EmployeeDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.employees.create(&s.actor, &plant, req).await?),
    ))
}

async fn get_employee(
    Ctx(s): Ctx,
    Path((plant, id)): Path<(String, String)>,
) -> ApiResult<Json<EmployeeDto>> {
    Ok(Json(s.employees.get(&s.actor, &plant, &id).await?))
}

async fn update_employee(
    Ctx(s): Ctx,
    Path((plant, id)): Path<(String, String)>,
    Json(req): Json<EmployeeInput>,
) -> ApiResult<Json<EmployeeDto>> {
    require_version(req.version)?;
    Ok(Json(s.employees.update(&s.actor, &plant, &id, req).await?))
}

async fn deactivate_employee(
    Ctx(s): Ctx,
    Path((plant, id)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<EmployeeDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.employees
            .deactivate(&s.actor, &plant, &id, version)
            .await?,
    ))
}

async fn activate_employee(
    Ctx(s): Ctx,
    Path((plant, id)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<EmployeeDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.employees.activate(&s.actor, &plant, &id, version).await?,
    ))
}

async fn delete_employee(
    Ctx(s): Ctx,
    Path((plant, id)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.employees.delete(&s.actor, &plant, &id, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- 資料異動記錄（ADR 0012；change_history.feature） -----

/// 修改、刪除、啟用／停用的版本號（樂觀鎖，§4）
#[derive(Deserialize)]
struct VersionQuery {
    #[serde(default)]
    version: Option<i32>,
}

/// 請求一律要帶版本號（CH4）；沒帶回 400
fn require_version(v: Option<i32>) -> Result<i32, HttpError> {
    v.ok_or_else(|| AppError::from(ttrs_domain::DomainError::VersionRequired).into())
}

// ----- Item Group 料件分群維護（aimi110；工廠層） -----

async fn query_item_groups(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::ItemGroupDto>>> {
    Ok(Json(s.item_groups.query(&s.actor, &plant, req).await?))
}

async fn list_item_groups(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
) -> ApiResult<Json<Vec<ttrs_shared::ItemGroupDto>>> {
    Ok(Json(s.item_groups.list(&s.actor, &plant).await?))
}

async fn get_item_group(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
) -> ApiResult<Json<ttrs_shared::ItemGroupDto>> {
    Ok(Json(s.item_groups.get(&s.actor, &plant, &code).await?))
}

async fn create_item_group(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ttrs_shared::ItemGroupInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::ItemGroupDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.item_groups.create(&s.actor, &plant, req).await?),
    ))
}

async fn update_item_group(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::ItemGroupInput>,
) -> ApiResult<Json<ttrs_shared::ItemGroupDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.item_groups.update(&s.actor, &plant, &code, req).await?,
    ))
}

#[derive(Deserialize)]
struct CopyItemGroup {
    new_code: String,
}

async fn copy_item_group(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<CopyItemGroup>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::ItemGroupDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(
            s.item_groups
                .copy(&s.actor, &plant, &code, &req.new_code)
                .await?,
        ),
    ))
}

async fn set_item_group_active(
    Ctx(s): Ctx,
    Path((plant, code, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let g = &s.item_groups;
    let dto = match state.as_str() {
        "activate" => g.set_active(&s.actor, &plant, &code, true, version).await?,
        "deactivate" => {
            g.set_active(&s.actor, &plant, &code, false, version)
                .await?
        }
        // 確認／取消確認（ADR 0013 SA9）
        "confirm" => g.confirm(&s.actor, &plant, &code, true, version).await?,
        "unconfirm" => g.confirm(&s.actor, &plant, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn delete_item_group(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.item_groups
        .delete(&s.actor, &plant, &code, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn item_group_history(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.item_groups
            .history(&s.actor, &plant, &code, q.lang.as_deref())
            .await?,
    ))
}

// ----- Product Classification 產品分類維護（axmi110；工廠層） -----

async fn list_product_classes(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
) -> ApiResult<Json<Vec<ttrs_shared::ProductClassDto>>> {
    Ok(Json(s.product_classes.list(&s.actor, &plant).await?))
}

async fn get_product_class(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
) -> ApiResult<Json<ttrs_shared::ProductClassDto>> {
    Ok(Json(s.product_classes.get(&s.actor, &plant, &code).await?))
}

async fn create_product_class(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ttrs_shared::ProductClassInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::ProductClassDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.product_classes.create(&s.actor, &plant, req).await?),
    ))
}

async fn update_product_class(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::ProductClassInput>,
) -> ApiResult<Json<ttrs_shared::ProductClassDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.product_classes
            .update(&s.actor, &plant, &code, req)
            .await?,
    ))
}

async fn set_product_class_active(
    Ctx(s): Ctx,
    Path((plant, code, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let c = &s.product_classes;
    let dto = match state.as_str() {
        "activate" => c.set_active(&s.actor, &plant, &code, true, version).await?,
        "deactivate" => {
            c.set_active(&s.actor, &plant, &code, false, version)
                .await?
        }
        // 確認／取消確認（ADR 0013 SA9）
        "confirm" => c.confirm(&s.actor, &plant, &code, true, version).await?,
        "unconfirm" => c.confirm(&s.actor, &plant, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn query_product_classes(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::ProductClassDto>>> {
    Ok(Json(s.product_classes.query(&s.actor, &plant, req).await?))
}

async fn copy_product_class(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::ProductClassDto>)> {
    let c = s
        .product_classes
        .copy(&s.actor, &plant, &code, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(c)))
}

async fn product_class_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .product_classes
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn delete_product_class(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.product_classes
        .delete(&s.actor, &plant, &code, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn product_class_history(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.product_classes
            .history(&s.actor, &plant, &code, q.lang.as_deref())
            .await?,
    ))
}

// ----- Warehouse Maintenance 倉庫儲位維護（aimi201、aimi200；工廠層） -----

use ttrs_shared::{WarehouseDto, WarehouseInput, WarehouseLocationInput};

async fn query_warehouses(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<WarehouseDto>>> {
    Ok(Json(s.warehouses.query(&s.actor, &plant, req).await?))
}

async fn get_warehouse(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
) -> ApiResult<Json<WarehouseDto>> {
    Ok(Json(s.warehouses.get(&s.actor, &plant, &code).await?))
}

async fn create_warehouse(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<WarehouseInput>,
) -> ApiResult<(StatusCode, Json<WarehouseDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.warehouses.create(&s.actor, &plant, req).await?),
    ))
}

async fn update_warehouse(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<WarehouseInput>,
) -> ApiResult<Json<WarehouseDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.warehouses.update(&s.actor, &plant, &code, req).await?,
    ))
}

async fn delete_warehouse(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.warehouses
        .delete(&s.actor, &plant, &code, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_warehouse(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<WarehouseDto>)> {
    let w = s
        .warehouses
        .copy(&s.actor, &plant, &code, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(w)))
}

async fn warehouse_history(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.warehouses
            .history(&s.actor, &plant, &code, q.lang.as_deref())
            .await?,
    ))
}

/// 新增儲位（detail）
async fn add_warehouse_location(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<WarehouseLocationInput>,
) -> ApiResult<Json<WarehouseDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.warehouses
            .add_location(&s.actor, &plant, &code, req)
            .await?,
    ))
}

/// 修改儲位（detail）
async fn update_warehouse_location(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<WarehouseLocationInput>,
) -> ApiResult<Json<WarehouseDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.warehouses
            .update_location(&s.actor, &plant, &code, req)
            .await?,
    ))
}

/// 刪除儲位（detail）
async fn remove_warehouse_location(
    Ctx(s): Ctx,
    Path((plant, code, loc)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<WarehouseDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.warehouses
            .remove_location(&s.actor, &plant, &code, &loc, version)
            .await?,
    ))
}

/// 停用（deactivate）、啟用（activate）、確認（confirm）、取消確認（unconfirm）
async fn set_warehouse_state(
    Ctx(s): Ctx,
    Path((plant, code, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let g = &s.warehouses;
    let dto = match state.as_str() {
        "activate" => g.set_active(&s.actor, &plant, &code, true, version).await?,
        "deactivate" => {
            g.set_active(&s.actor, &plant, &code, false, version)
                .await?
        }
        "confirm" => g.confirm(&s.actor, &plant, &code, true, version).await?,
        "unconfirm" => g.confirm(&s.actor, &plant, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn warehouse_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .warehouses
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- Item Storage Maintenance 料件預設倉儲維護（aimi202；工廠層） -----

use ttrs_shared::{
    ItemStorageBatchRequest, ItemStorageBatchResult, ItemStorageInput, ItemStorageItemDto,
};

/// 單身鍵值：倉庫、儲位（空白為整個倉庫）；刪除時帶版本
#[derive(Deserialize)]
struct StoragePlace {
    warehouse: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    version: Option<i32>,
}

async fn query_item_storages(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ItemStorageItemDto>>> {
    Ok(Json(s.item_storages.query(&s.actor, &plant, req).await?))
}

async fn get_item_storage(
    Ctx(s): Ctx,
    Path((plant, item)): Path<(String, String)>,
) -> ApiResult<Json<ItemStorageItemDto>> {
    Ok(Json(s.item_storages.get(&s.actor, &plant, &item).await?))
}

async fn create_item_storage(
    Ctx(s): Ctx,
    Path((plant, item)): Path<(String, String)>,
    Json(req): Json<ItemStorageInput>,
) -> ApiResult<(StatusCode, Json<ItemStorageItemDto>)> {
    let d = s.item_storages.create(&s.actor, &plant, &item, req).await?;
    Ok((StatusCode::CREATED, Json(d)))
}

async fn update_item_storage(
    Ctx(s): Ctx,
    Path((plant, item)): Path<(String, String)>,
    Query(p): Query<StoragePlace>,
    Json(req): Json<ItemStorageInput>,
) -> ApiResult<Json<ItemStorageItemDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.item_storages
            .update(&s.actor, &plant, &item, (&p.warehouse, &p.location), req)
            .await?,
    ))
}

async fn delete_item_storage(
    Ctx(s): Ctx,
    Path((plant, item)): Path<(String, String)>,
    Query(p): Query<StoragePlace>,
) -> ApiResult<Json<ItemStorageItemDto>> {
    let version = Some(require_version(p.version)?);
    Ok(Json(
        s.item_storages
            .delete(
                &s.actor,
                &plant,
                &item,
                (&p.warehouse, &p.location),
                version,
            )
            .await?,
    ))
}

/// 複製到另一個料件（reproduce）；new_code 為目的料號
async fn copy_item_storage(
    Ctx(s): Ctx,
    Path((plant, item)): Path<(String, String)>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<ItemStorageItemDto>)> {
    let d = s
        .item_storages
        .copy(&s.actor, &plant, &item, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(d)))
}

/// 整批產生（batch_generate，IS7）
async fn batch_item_storages(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ItemStorageBatchRequest>,
) -> ApiResult<Json<ItemStorageBatchResult>> {
    Ok(Json(
        s.item_storages
            .batch_generate(&s.actor, &plant, req)
            .await?,
    ))
}

async fn item_storage_history(
    Ctx(s): Ctx,
    Path((plant, item)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.item_storages
            .history(&s.actor, &plant, &item, q.lang.as_deref())
            .await?,
    ))
}

async fn item_storage_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .item_storages
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- Project Maintenance 專案維護（aooi200；工廠層） -----

async fn list_projects(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
) -> ApiResult<Json<Vec<ttrs_shared::ProjectDto>>> {
    let req = QueryRequest {
        page_size: Some(u32::MAX),
        ..Default::default()
    };
    Ok(Json(s.projects.query(&s.actor, &plant, req).await?.items))
}

async fn get_project(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
) -> ApiResult<Json<ttrs_shared::ProjectDto>> {
    Ok(Json(s.projects.get(&s.actor, &plant, &code).await?))
}

async fn create_project(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ttrs_shared::ProjectInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::ProjectDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.projects.create(&s.actor, &plant, req).await?),
    ))
}

async fn update_project(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::ProjectInput>,
) -> ApiResult<Json<ttrs_shared::ProjectDto>> {
    require_version(req.version)?;
    Ok(Json(s.projects.update(&s.actor, &plant, &code, req).await?))
}

async fn set_project_state(
    Ctx(s): Ctx,
    Path((plant, code, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let c = &s.projects;
    let dto = match state.as_str() {
        "activate" => c.set_active(&s.actor, &plant, &code, true, version).await?,
        "deactivate" => {
            c.set_active(&s.actor, &plant, &code, false, version)
                .await?
        }
        // 確認／取消確認（ADR 0013 SA9）
        "confirm" => c.confirm(&s.actor, &plant, &code, true, version).await?,
        "unconfirm" => c.confirm(&s.actor, &plant, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn query_projects(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::ProjectDto>>> {
    Ok(Json(s.projects.query(&s.actor, &plant, req).await?))
}

async fn copy_project(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::ProjectDto>)> {
    let c = s
        .projects
        .copy(&s.actor, &plant, &code, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(c)))
}

async fn project_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .projects
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn delete_project(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.projects.delete(&s.actor, &plant, &code, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn project_history(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.projects
            .history(&s.actor, &plant, &code, q.lang.as_deref())
            .await?,
    ))
}

// ----- Unit Maintenance 單位維護（aooi101） -----

async fn list_units(Ctx(s): Ctx) -> ApiResult<Json<Vec<ttrs_shared::UnitDto>>> {
    Ok(Json(s.units.list(&s.actor).await?))
}

async fn get_unit(Ctx(s): Ctx, Path(code): Path<String>) -> ApiResult<Json<ttrs_shared::UnitDto>> {
    Ok(Json(s.units.get(&s.actor, &code).await?))
}

async fn create_unit(
    Ctx(s): Ctx,
    Json(req): Json<ttrs_shared::UnitInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::UnitDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.units.create(&s.actor, req).await?),
    ))
}

async fn update_unit(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<ttrs_shared::UnitInput>,
) -> ApiResult<Json<ttrs_shared::UnitDto>> {
    require_version(req.version)?;
    Ok(Json(s.units.update(&s.actor, &code, req).await?))
}

/// 停用（deactivate）、啟用（activate）
async fn set_unit_active(
    Ctx(s): Ctx,
    Path((code, state)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let u = &s.units;
    let dto = match state.as_str() {
        "activate" => u.set_active(&s.actor, &code, true, version).await?,
        "deactivate" => u.set_active(&s.actor, &code, false, version).await?,
        // 確認／取消確認（ADR 0013 SA9）
        "confirm" => u.confirm(&s.actor, &code, true, version).await?,
        "unconfirm" => u.confirm(&s.actor, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn query_units(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::UnitDto>>> {
    Ok(Json(s.units.query(&s.actor, req).await?))
}

async fn copy_unit(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::UnitDto>)> {
    let u = s.units.copy(&s.actor, &code, &req.new_code).await?;
    Ok((StatusCode::CREATED, Json(u)))
}

async fn unit_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .units
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn delete_unit(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.units.delete(&s.actor, &code, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- Encoding Nature Maintenance 編碼性質維護（aooi401） -----

use ttrs_shared::{EncodingNatureDto, EncodingNatureInput};

async fn get_encoding_nature(
    Ctx(s): Ctx,
    Path(code): Path<String>,
) -> ApiResult<Json<EncodingNatureDto>> {
    Ok(Json(s.encoding_natures.get(&s.actor, &code).await?))
}

async fn create_encoding_nature(
    Ctx(s): Ctx,
    Json(req): Json<EncodingNatureInput>,
) -> ApiResult<(StatusCode, Json<EncodingNatureDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.encoding_natures.create(&s.actor, req).await?),
    ))
}

async fn update_encoding_nature(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<EncodingNatureInput>,
) -> ApiResult<Json<EncodingNatureDto>> {
    require_version(req.version)?;
    Ok(Json(s.encoding_natures.update(&s.actor, &code, req).await?))
}

/// 停用（deactivate）、啟用（activate）、確認（confirm）、取消確認（unconfirm）
async fn set_encoding_nature_state(
    Ctx(s): Ctx,
    Path((code, state)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let n = &s.encoding_natures;
    let dto = match state.as_str() {
        "activate" => n.set_active(&s.actor, &code, true, version).await?,
        "deactivate" => n.set_active(&s.actor, &code, false, version).await?,
        "confirm" => n.confirm(&s.actor, &code, true, version).await?,
        "unconfirm" => n.confirm(&s.actor, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn query_encoding_natures(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<EncodingNatureDto>>> {
    Ok(Json(s.encoding_natures.query(&s.actor, req).await?))
}

async fn copy_encoding_nature(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<EncodingNatureDto>)> {
    let n = s
        .encoding_natures
        .copy(&s.actor, &code, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(n)))
}

async fn encoding_nature_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .encoding_natures
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn delete_encoding_nature(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.encoding_natures.delete(&s.actor, &code, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- Encoding Class Maintenance 編碼類別維護（aooi402） -----

use ttrs_shared::{EncodingClassDto, EncodingClassInput, EncodingClassSegmentInput};

async fn get_encoding_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
) -> ApiResult<Json<EncodingClassDto>> {
    Ok(Json(s.encoding_classes.get(&s.actor, &code).await?))
}

async fn create_encoding_class(
    Ctx(s): Ctx,
    Json(req): Json<EncodingClassInput>,
) -> ApiResult<(StatusCode, Json<EncodingClassDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.encoding_classes.create(&s.actor, req).await?),
    ))
}

async fn update_encoding_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<EncodingClassInput>,
) -> ApiResult<Json<EncodingClassDto>> {
    require_version(req.version)?;
    Ok(Json(s.encoding_classes.update(&s.actor, &code, req).await?))
}

/// 單身新增一段（detail）
async fn add_encoding_class_segment(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<EncodingClassSegmentInput>,
) -> ApiResult<Json<EncodingClassDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.encoding_classes.add_segment(&s.actor, &code, req).await?,
    ))
}

/// 單身在指定段次插入一段，之後的段次加 1（detail；addno）
async fn insert_encoding_class_segment(
    Ctx(s): Ctx,
    Path((code, no)): Path<(String, i32)>,
    Json(req): Json<EncodingClassSegmentInput>,
) -> ApiResult<Json<EncodingClassDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.encoding_classes
            .insert_segment(&s.actor, &code, no, req)
            .await?,
    ))
}

/// 單身修改一段（detail）
async fn update_encoding_class_segment(
    Ctx(s): Ctx,
    Path((code, no)): Path<(String, i32)>,
    Json(req): Json<EncodingClassSegmentInput>,
) -> ApiResult<Json<EncodingClassDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.encoding_classes
            .update_segment(&s.actor, &code, no, req)
            .await?,
    ))
}

/// 單身刪除一段（detail）
async fn remove_encoding_class_segment(
    Ctx(s): Ctx,
    Path((code, no)): Path<(String, i32)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<EncodingClassDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.encoding_classes
            .remove_segment(&s.actor, &code, no, version)
            .await?,
    ))
}

/// 段次重排（detail；lineno_from_1_resort）
async fn renumber_encoding_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<EncodingClassDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.encoding_classes
            .renumber(&s.actor, &code, version)
            .await?,
    ))
}

/// 停用（deactivate）、啟用（activate）、確認（confirm）、取消確認（unconfirm）
async fn set_encoding_class_state(
    Ctx(s): Ctx,
    Path((code, state)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let g = &s.encoding_classes;
    let dto = match state.as_str() {
        "activate" => g.set_active(&s.actor, &code, true, version).await?,
        "deactivate" => g.set_active(&s.actor, &code, false, version).await?,
        "confirm" => g.confirm(&s.actor, &code, true, version).await?,
        "unconfirm" => g.confirm(&s.actor, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

/// 編碼試算：依類別組下一個號碼與名稱，不保留號碼（ER4、ER5）
async fn preview_encoding_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<ttrs_shared::EncodingPreviewRequest>,
) -> ApiResult<Json<ttrs_shared::EncodingPreviewDto>> {
    Ok(Json(
        s.encoding_classes
            .preview(&s.actor, &req.plant, &code, &req.selections)
            .await?,
    ))
}

/// 料件取號選單（AN2、AN6）：參數未開啟時 enabled＝false
async fn item_number_options(
    Ctx(s): Ctx,
    Path(_plant): Path<String>,
) -> ApiResult<Json<ttrs_shared::ItemNumberOptionsDto>> {
    Ok(Json(
        s.encoding_classes.item_number_options(&s.actor).await?,
    ))
}

/// 取料號（AN2–AN5）：不保留號碼
async fn next_item_number(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ttrs_shared::ItemNumberRequest>,
) -> ApiResult<Json<ttrs_shared::EncodingPreviewDto>> {
    Ok(Json(
        s.encoding_classes
            .next_item_number(&s.actor, &plant, &req.class_code, &req.selections)
            .await?,
    ))
}

async fn query_encoding_classes(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<EncodingClassDto>>> {
    Ok(Json(s.encoding_classes.query(&s.actor, req).await?))
}

async fn copy_encoding_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<EncodingClassDto>)> {
    let c = s
        .encoding_classes
        .copy(&s.actor, &code, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(c)))
}

async fn encoding_class_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .encoding_classes
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn delete_encoding_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.encoding_classes.delete(&s.actor, &code, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- Encoding Year Maintenance 編碼年對照維護（aooi404） -----

use ttrs_shared::{EncodingYearCopyRequest, EncodingYearDto, EncodingYearInput};

async fn get_encoding_year(
    Ctx(s): Ctx,
    Path((year, length)): Path<(i32, i16)>,
) -> ApiResult<Json<EncodingYearDto>> {
    Ok(Json(s.encoding_years.get(&s.actor, year, length).await?))
}

async fn create_encoding_year(
    Ctx(s): Ctx,
    Json(req): Json<EncodingYearInput>,
) -> ApiResult<(StatusCode, Json<EncodingYearDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.encoding_years.create(&s.actor, req).await?),
    ))
}

async fn update_encoding_year(
    Ctx(s): Ctx,
    Path((year, length)): Path<(i32, i16)>,
    Json(req): Json<EncodingYearInput>,
) -> ApiResult<Json<EncodingYearDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.encoding_years.update(&s.actor, year, length, req).await?,
    ))
}

/// 停用（deactivate）、啟用（activate）、確認（confirm）、取消確認（unconfirm）
async fn set_encoding_year_state(
    Ctx(s): Ctx,
    Path((year, length, state)): Path<(i32, i16, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let y = &s.encoding_years;
    let a = &s.actor;
    let dto = match state.as_str() {
        "activate" => y.set_active(a, year, length, true, version).await?,
        "deactivate" => y.set_active(a, year, length, false, version).await?,
        "confirm" => y.confirm(a, year, length, true, version).await?,
        "unconfirm" => y.confirm(a, year, length, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn query_encoding_years(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<EncodingYearDto>>> {
    Ok(Json(s.encoding_years.query(&s.actor, req).await?))
}

async fn copy_encoding_year(
    Ctx(s): Ctx,
    Path((year, length)): Path<(i32, i16)>,
    Json(req): Json<EncodingYearCopyRequest>,
) -> ApiResult<(StatusCode, Json<EncodingYearDto>)> {
    let y = s
        .encoding_years
        .copy(&s.actor, year, length, req.new_year)
        .await?;
    Ok((StatusCode::CREATED, Json(y)))
}

async fn encoding_year_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .encoding_years
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn delete_encoding_year(
    Ctx(s): Ctx,
    Path((year, length)): Path<(i32, i16)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.encoding_years
        .delete(&s.actor, year, length, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- Encoding Segment Maintenance 編碼獨立段維護（aooi403） -----

use ttrs_shared::{EncodingSegmentDto, EncodingSegmentInput, EncodingSegmentValueInput};

async fn get_encoding_segment(
    Ctx(s): Ctx,
    Path(code): Path<String>,
) -> ApiResult<Json<EncodingSegmentDto>> {
    Ok(Json(s.encoding_segments.get(&s.actor, &code).await?))
}

async fn create_encoding_segment(
    Ctx(s): Ctx,
    Json(req): Json<EncodingSegmentInput>,
) -> ApiResult<(StatusCode, Json<EncodingSegmentDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.encoding_segments.create(&s.actor, req).await?),
    ))
}

async fn update_encoding_segment(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<EncodingSegmentInput>,
) -> ApiResult<Json<EncodingSegmentDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.encoding_segments.update(&s.actor, &code, req).await?,
    ))
}

/// 單身新增一列（detail）
async fn add_encoding_segment_value(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<EncodingSegmentValueInput>,
) -> ApiResult<Json<EncodingSegmentDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.encoding_segments.add_value(&s.actor, &code, req).await?,
    ))
}

/// 單身修改一列（detail）
async fn update_encoding_segment_value(
    Ctx(s): Ctx,
    Path((code, line)): Path<(String, i32)>,
    Json(req): Json<EncodingSegmentValueInput>,
) -> ApiResult<Json<EncodingSegmentDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.encoding_segments
            .update_value(&s.actor, &code, line, req)
            .await?,
    ))
}

/// 單身刪除一列（detail）
async fn remove_encoding_segment_value(
    Ctx(s): Ctx,
    Path((code, line)): Path<(String, i32)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<EncodingSegmentDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.encoding_segments
            .remove_value(&s.actor, &code, line, version)
            .await?,
    ))
}

/// 停用（deactivate）、啟用（activate）、確認（confirm）、取消確認（unconfirm）
async fn set_encoding_segment_state(
    Ctx(s): Ctx,
    Path((code, state)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let g = &s.encoding_segments;
    let dto = match state.as_str() {
        "activate" => g.set_active(&s.actor, &code, true, version).await?,
        "deactivate" => g.set_active(&s.actor, &code, false, version).await?,
        "confirm" => g.confirm(&s.actor, &code, true, version).await?,
        "unconfirm" => g.confirm(&s.actor, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn query_encoding_segments(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<EncodingSegmentDto>>> {
    Ok(Json(s.encoding_segments.query(&s.actor, req).await?))
}

async fn copy_encoding_segment(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<EncodingSegmentDto>)> {
    let g = s
        .encoding_segments
        .copy(&s.actor, &code, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(g)))
}

async fn encoding_segment_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .encoding_segments
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn delete_encoding_segment(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.encoding_segments.delete(&s.actor, &code, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- 料件拋轉（aimi100 carry） -----

async fn item_carry_info(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
) -> ApiResult<Json<ttrs_shared::ItemCarryInfoDto>> {
    Ok(Json(s.item_carry.info(&s.actor, &plant).await?))
}

async fn run_item_carry(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::ItemCarryRequest>,
) -> ApiResult<Json<ttrs_shared::ItemCarryResultDto>> {
    Ok(Json(s.item_carry.carry(&s.actor, &plant, &no, req).await?))
}

/// 拋轉歷史（qry_carry_history）
async fn item_carry_history(
    Ctx(s): Ctx,
    Path(no): Path<String>,
) -> ApiResult<Json<Vec<ttrs_shared::CarryLogDto>>> {
    Ok(Json(s.item_carry.history(&s.actor, &no).await?))
}

// ----- Basic Data Carry 基本資料拋轉（aoop603）、Carry History 拋轉歷史查詢（aooq604） -----

use ttrs_shared::{BasicCarryInfoDto, BasicCarryRequest, BasicCarryResultDto, CarryLogDto};

async fn basic_carry_info(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
) -> ApiResult<Json<BasicCarryInfoDto>> {
    Ok(Json(s.basic_carry.info(&s.actor, &plant).await?))
}

/// 執行拋轉（carry）
async fn run_basic_carry(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<BasicCarryRequest>,
) -> ApiResult<Json<BasicCarryResultDto>> {
    Ok(Json(s.basic_carry.carry(&s.actor, &plant, req).await?))
}

async fn query_carry_history(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<CarryLogDto>>> {
    Ok(Json(s.basic_carry.history(&s.actor, req).await?))
}

async fn carry_history_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .basic_carry
        .history_report(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- Carry Setting Maintenance 資料中心拋轉設定（aooi602） -----

use ttrs_shared::{
    CarryCopyInput, CarryDefaultInput, CarrySettingDto, CarrySettingInput, CarryTargetInput,
};

async fn create_carry_setting(
    Ctx(s): Ctx,
    Json(req): Json<CarrySettingInput>,
) -> ApiResult<(StatusCode, Json<CarrySettingDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.carry_settings.create(&s.actor, req).await?),
    ))
}

async fn query_carry_settings(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<CarrySettingDto>>> {
    Ok(Json(s.carry_settings.query(&s.actor, req).await?))
}

async fn get_carry_setting(
    Ctx(s): Ctx,
    Path((center, data_type)): Path<(String, String)>,
) -> ApiResult<Json<CarrySettingDto>> {
    Ok(Json(
        s.carry_settings.get(&s.actor, &center, &data_type).await?,
    ))
}

/// 修改拋轉方式（modify）
async fn update_carry_setting(
    Ctx(s): Ctx,
    Path((center, data_type)): Path<(String, String)>,
    Json(req): Json<CarrySettingInput>,
) -> ApiResult<Json<CarrySettingDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.carry_settings
            .update_mode(&s.actor, &center, &data_type, req)
            .await?,
    ))
}

async fn delete_carry_setting(
    Ctx(s): Ctx,
    Path((center, data_type)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.carry_settings
        .delete(&s.actor, &center, &data_type, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_carry_setting(
    Ctx(s): Ctx,
    Path((center, data_type)): Path<(String, String)>,
    Json(req): Json<CarryCopyInput>,
) -> ApiResult<(StatusCode, Json<CarrySettingDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(
            s.carry_settings
                .copy(&s.actor, &center, &data_type, req)
                .await?,
        ),
    ))
}

/// 新增拋轉營運中心（detail）
async fn add_carry_target(
    Ctx(s): Ctx,
    Path((center, data_type)): Path<(String, String)>,
    Json(req): Json<CarryTargetInput>,
) -> ApiResult<Json<CarrySettingDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.carry_settings
            .add_target(&s.actor, &center, &data_type, req)
            .await?,
    ))
}

/// 修改拋轉營運中心（detail）
async fn update_carry_target(
    Ctx(s): Ctx,
    Path((center, data_type)): Path<(String, String)>,
    Json(req): Json<CarryTargetInput>,
) -> ApiResult<Json<CarrySettingDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.carry_settings
            .update_target(&s.actor, &center, &data_type, req)
            .await?,
    ))
}

/// 刪除拋轉營運中心連同條件與預設值（detail）
async fn remove_carry_target(
    Ctx(s): Ctx,
    Path((center, data_type, plant)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<CarrySettingDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.carry_settings
            .remove_target(&s.actor, &center, &data_type, &plant, version)
            .await?,
    ))
}

/// 新增拋轉條件（detail）
async fn add_carry_condition(
    Ctx(s): Ctx,
    Path((center, data_type, plant)): Path<(String, String, String)>,
    Json(req): Json<DataControlRuleInput>,
) -> ApiResult<Json<CarrySettingDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.carry_settings
            .put_condition(&s.actor, &center, &data_type, &plant, req, false)
            .await?,
    ))
}

/// 修改拋轉條件（detail）
async fn update_carry_condition(
    Ctx(s): Ctx,
    Path((center, data_type, plant)): Path<(String, String, String)>,
    Json(req): Json<DataControlRuleInput>,
) -> ApiResult<Json<CarrySettingDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.carry_settings
            .put_condition(&s.actor, &center, &data_type, &plant, req, true)
            .await?,
    ))
}

/// 刪除拋轉條件（detail）
async fn remove_carry_condition(
    Ctx(s): Ctx,
    Path((center, data_type, plant, field)): Path<(String, String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<CarrySettingDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.carry_settings
            .remove_condition(&s.actor, &center, &data_type, &plant, &field, version)
            .await?,
    ))
}

/// 新增預設值（set_default_value）
async fn add_carry_default(
    Ctx(s): Ctx,
    Path((center, data_type, plant)): Path<(String, String, String)>,
    Json(req): Json<CarryDefaultInput>,
) -> ApiResult<Json<CarrySettingDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.carry_settings
            .put_default(&s.actor, &center, &data_type, &plant, req, false)
            .await?,
    ))
}

/// 修改預設值（set_default_value）
async fn update_carry_default(
    Ctx(s): Ctx,
    Path((center, data_type, plant)): Path<(String, String, String)>,
    Json(req): Json<CarryDefaultInput>,
) -> ApiResult<Json<CarrySettingDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.carry_settings
            .put_default(&s.actor, &center, &data_type, &plant, req, true)
            .await?,
    ))
}

/// 刪除預設值（set_default_value）
async fn remove_carry_default(
    Ctx(s): Ctx,
    Path((center, data_type, plant, field)): Path<(String, String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<CarrySettingDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.carry_settings
            .remove_default(&s.actor, &center, &data_type, &plant, &field, version)
            .await?,
    ))
}

async fn carry_setting_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .carry_settings
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- Data Control Maintenance 資料控管原則維護（aooi601） -----

use ttrs_shared::{
    DataControlCopyInput, DataControlDto, DataControlPlantInput, DataControlRuleInput,
};

async fn query_data_controls(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<DataControlDto>>> {
    Ok(Json(s.data_controls.query(&s.actor, req).await?))
}

async fn get_data_control(
    Ctx(s): Ctx,
    Path(data_type): Path<String>,
) -> ApiResult<Json<DataControlDto>> {
    Ok(Json(s.data_controls.get(&s.actor, &data_type).await?))
}

/// 新增受控營運中心（detail）
async fn add_data_control_plant(
    Ctx(s): Ctx,
    Json(req): Json<DataControlPlantInput>,
) -> ApiResult<(StatusCode, Json<DataControlDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.data_controls.add_plant(&s.actor, req).await?),
    ))
}

/// 刪除受控營運中心連同欄位條件（detail）
async fn remove_data_control_plant(
    Ctx(s): Ctx,
    Path((data_type, plant)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<DataControlDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.data_controls
            .remove_plant(&s.actor, &data_type, &plant, version)
            .await?,
    ))
}

/// 把欄位條件複製到另一個營運中心（reproduce）
async fn copy_data_control_rules(
    Ctx(s): Ctx,
    Path((data_type, plant)): Path<(String, String)>,
    Json(req): Json<DataControlCopyInput>,
) -> ApiResult<(StatusCode, Json<DataControlDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(
            s.data_controls
                .copy_rules(&s.actor, &data_type, &plant, req)
                .await?,
        ),
    ))
}

/// 新增欄位條件（condition_detail）
async fn add_data_control_rule(
    Ctx(s): Ctx,
    Path((data_type, plant)): Path<(String, String)>,
    Json(req): Json<DataControlRuleInput>,
) -> ApiResult<Json<DataControlDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.data_controls
            .add_rule(&s.actor, &data_type, &plant, req)
            .await?,
    ))
}

/// 修改欄位條件（condition_detail）
async fn update_data_control_rule(
    Ctx(s): Ctx,
    Path((data_type, plant)): Path<(String, String)>,
    Json(req): Json<DataControlRuleInput>,
) -> ApiResult<Json<DataControlDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.data_controls
            .update_rule(&s.actor, &data_type, &plant, req)
            .await?,
    ))
}

/// 刪除欄位條件（condition_detail）
async fn remove_data_control_rule(
    Ctx(s): Ctx,
    Path((data_type, plant, field)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<DataControlDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.data_controls
            .remove_rule(&s.actor, &data_type, &plant, &field, version)
            .await?,
    ))
}

async fn data_control_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .data_controls
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- Group Center Maintenance 集團中心維護（aooi600） -----

use ttrs_shared::{DataCenterTypeInput, GroupCenterDto, GroupCenterInput};

async fn get_group_center(
    Ctx(s): Ctx,
    Path(code): Path<String>,
) -> ApiResult<Json<GroupCenterDto>> {
    Ok(Json(s.group_centers.get(&s.actor, &code).await?))
}

async fn create_group_center(
    Ctx(s): Ctx,
    Json(req): Json<GroupCenterInput>,
) -> ApiResult<(StatusCode, Json<GroupCenterDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.group_centers.create(&s.actor, req).await?),
    ))
}

async fn update_group_center(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<GroupCenterInput>,
) -> ApiResult<Json<GroupCenterDto>> {
    require_version(req.version)?;
    Ok(Json(s.group_centers.update(&s.actor, &code, req).await?))
}

/// 資料類別設定新增一列（data_type_setting）
async fn add_group_center_data_type(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<DataCenterTypeInput>,
) -> ApiResult<Json<GroupCenterDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.group_centers.add_data_type(&s.actor, &code, req).await?,
    ))
}

/// 資料類別設定刪除一列（data_type_setting）
async fn remove_group_center_data_type(
    Ctx(s): Ctx,
    Path((code, data_type, plant)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<GroupCenterDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.group_centers
            .remove_data_type(&s.actor, &code, &data_type, &plant, version)
            .await?,
    ))
}

/// 停用（deactivate）、啟用（activate）、確認（confirm）、取消確認（unconfirm）
async fn set_group_center_state(
    Ctx(s): Ctx,
    Path((code, state)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let g = &s.group_centers;
    let dto = match state.as_str() {
        "activate" => g.set_active(&s.actor, &code, true, version).await?,
        "deactivate" => g.set_active(&s.actor, &code, false, version).await?,
        "confirm" => g.confirm(&s.actor, &code, true, version).await?,
        "unconfirm" => g.confirm(&s.actor, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn query_group_centers(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<GroupCenterDto>>> {
    Ok(Json(s.group_centers.query(&s.actor, req).await?))
}

async fn copy_group_center(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<GroupCenterDto>)> {
    let g = s.group_centers.copy(&s.actor, &code, &req.new_code).await?;
    Ok((StatusCode::CREATED, Json(g)))
}

async fn group_center_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .group_centers
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn delete_group_center(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.group_centers.delete(&s.actor, &code, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- Tax Type Maintenance 稅別維護（aooi150） -----

use ttrs_shared::{TaxTypeCopyRequest, TaxTypeDto, TaxTypeInput};

async fn list_tax_types(Ctx(s): Ctx) -> ApiResult<Json<Vec<TaxTypeDto>>> {
    let req = QueryRequest {
        page_size: Some(u32::MAX),
        ..Default::default()
    };
    Ok(Json(s.taxes.query(&s.actor, req).await?.items))
}

async fn get_tax_type(
    Ctx(s): Ctx,
    Path((code, direction)): Path<(String, String)>,
) -> ApiResult<Json<TaxTypeDto>> {
    Ok(Json(s.taxes.get(&s.actor, &code, &direction).await?))
}

async fn create_tax_type(
    Ctx(s): Ctx,
    Json(req): Json<TaxTypeInput>,
) -> ApiResult<(StatusCode, Json<TaxTypeDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.taxes.create(&s.actor, req).await?),
    ))
}

async fn update_tax_type(
    Ctx(s): Ctx,
    Path((code, direction)): Path<(String, String)>,
    Json(req): Json<TaxTypeInput>,
) -> ApiResult<Json<TaxTypeDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.taxes.update(&s.actor, &code, &direction, req).await?,
    ))
}

/// 停用 deactivate、啟用 activate、確認 confirm、取消確認 unconfirm
async fn set_tax_type_state(
    Ctx(s): Ctx,
    Path((code, direction, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let t = &s.taxes;
    let a = &s.actor;
    let dto = match state.as_str() {
        "activate" => t.set_active(a, &code, &direction, true, version).await?,
        "deactivate" => t.set_active(a, &code, &direction, false, version).await?,
        "confirm" => t.confirm(a, &code, &direction, true, version).await?,
        "unconfirm" => t.confirm(a, &code, &direction, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn query_tax_types(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<TaxTypeDto>>> {
    Ok(Json(s.taxes.query(&s.actor, req).await?))
}

async fn copy_tax_type(
    Ctx(s): Ctx,
    Path((code, direction)): Path<(String, String)>,
    Json(req): Json<TaxTypeCopyRequest>,
) -> ApiResult<(StatusCode, Json<TaxTypeDto>)> {
    let t = s
        .taxes
        .copy(
            &s.actor,
            &code,
            &direction,
            &req.new_code,
            &req.new_direction,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(t)))
}

async fn tax_type_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .taxes
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn delete_tax_type(
    Ctx(s): Ctx,
    Path((code, direction)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.taxes.delete(&s.actor, &code, &direction, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- 匯率：每月 aooi060、每日 aooi070、每日匯率複製 aoop010 -----

use ttrs_shared::{
    DailyRateCopyRequest, DailyRateDto, DailyRateFillLine, DailyRateFillRequest, DailyRateInput,
    MonthlyRateCopyRequest, MonthlyRateDto, MonthlyRateInput, RateBatchResult, RateImportRequest,
    RateImportResult, RatesDto, RevaluationRequest, UnifyRateRequest,
};

async fn create_monthly_rate(
    Ctx(s): Ctx,
    Json(req): Json<MonthlyRateInput>,
) -> ApiResult<(StatusCode, Json<MonthlyRateDto>)> {
    let dto = s.rates.create_monthly(&s.actor, req).await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

async fn query_monthly_rates(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<MonthlyRateDto>>> {
    Ok(Json(s.rates.query_monthly(&s.actor, req).await?))
}

async fn monthly_rate_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .rates
        .report_monthly(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn revaluate_rates(
    Ctx(s): Ctx,
    Json(req): Json<RevaluationRequest>,
) -> ApiResult<Json<RateBatchResult>> {
    Ok(Json(s.rates.revaluate(&s.actor, req.date).await?))
}

async fn get_monthly_rate(
    Ctx(s): Ctx,
    Path((currency, period)): Path<(String, String)>,
) -> ApiResult<Json<MonthlyRateDto>> {
    Ok(Json(
        s.rates.get_monthly(&s.actor, &currency, &period).await?,
    ))
}

async fn update_monthly_rate(
    Ctx(s): Ctx,
    Path((currency, period)): Path<(String, String)>,
    Json(req): Json<MonthlyRateInput>,
) -> ApiResult<Json<MonthlyRateDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.rates
            .update_monthly(&s.actor, &currency, &period, req)
            .await?,
    ))
}

async fn delete_monthly_rate(
    Ctx(s): Ctx,
    Path((currency, period)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.rates
        .delete_monthly(&s.actor, &currency, &period, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_monthly_rate(
    Ctx(s): Ctx,
    Path((currency, period)): Path<(String, String)>,
    Json(req): Json<MonthlyRateCopyRequest>,
) -> ApiResult<(StatusCode, Json<MonthlyRateDto>)> {
    let dto = s
        .rates
        .copy_monthly(
            &s.actor,
            &currency,
            &period,
            &req.currency_code,
            &req.period,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

/// 停用（deactivate）、啟用（activate）、確認（confirm）、取消確認（unconfirm）
async fn set_monthly_rate_state(
    Ctx(s): Ctx,
    Path((currency, period, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let (r, a) = (&s.rates, &s.actor);
    let dto = match state.as_str() {
        "activate" => {
            r.set_monthly_active(a, &currency, &period, true, version)
                .await?
        }
        "deactivate" => {
            r.set_monthly_active(a, &currency, &period, false, version)
                .await?
        }
        "confirm" => {
            r.confirm_monthly(a, &currency, &period, true, version)
                .await?
        }
        "unconfirm" => {
            r.confirm_monthly(a, &currency, &period, false, version)
                .await?
        }
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

/// 日期路徑參數（YYYY-MM-DD）；格式錯誤視為找不到
fn rate_date(raw: &str) -> Result<ttrs_shared::NaiveDate, HttpError> {
    raw.parse()
        .map_err(|_| AppError::from(ttrs_domain::DomainError::ExchangeRateNotFound).into())
}

async fn create_daily_rate(
    Ctx(s): Ctx,
    Json(req): Json<DailyRateInput>,
) -> ApiResult<(StatusCode, Json<DailyRateDto>)> {
    let dto = s.rates.create_daily(&s.actor, req).await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

async fn query_daily_rates(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<DailyRateDto>>> {
    Ok(Json(s.rates.query_daily(&s.actor, req).await?))
}

async fn daily_rate_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .rates
        .report_daily(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn daily_rate_defaults(
    Ctx(s): Ctx,
    Path(currency): Path<String>,
) -> ApiResult<Json<RatesDto>> {
    Ok(Json(s.rates.daily_defaults(&s.actor, &currency).await?))
}

async fn unify_rates(
    Ctx(s): Ctx,
    Json(req): Json<UnifyRateRequest>,
) -> ApiResult<Json<RateBatchResult>> {
    Ok(Json(s.rates.unify(&s.actor, req.date).await?))
}

/// 匯入台銀牌告匯率（IM1–IM3）：`kind` 為 cash 現金或 spot 即期（預設）；`csv` 有值時用上傳的檔案（IM1 備援）
async fn import_bank_rates(
    Ctx(s): Ctx,
    Json(req): Json<RateImportRequest>,
) -> ApiResult<Json<RateImportResult>> {
    let kind = ttrs_domain::exchange_rate::feed::BankRateType::parse(
        req.kind.as_deref().unwrap_or_default(),
    )
    .map_err(AppError::from)?;
    let csv = req.csv.as_deref().filter(|c| !c.trim().is_empty());
    Ok(Json(
        s.rates
            .import_bank(&s.actor, kind, req.overwrite, csv)
            .await?,
    ))
}

/// 匯入關務署每旬報關匯率（IM4）
async fn import_customs_rates(Ctx(s): Ctx) -> ApiResult<Json<RateImportResult>> {
    Ok(Json(s.rates.import_customs(&s.actor).await?))
}

async fn get_daily_rate(
    Ctx(s): Ctx,
    Path((currency, date)): Path<(String, String)>,
) -> ApiResult<Json<DailyRateDto>> {
    let date = rate_date(&date)?;
    Ok(Json(s.rates.get_daily(&s.actor, &currency, date).await?))
}

async fn update_daily_rate(
    Ctx(s): Ctx,
    Path((currency, date)): Path<(String, String)>,
    Json(req): Json<DailyRateInput>,
) -> ApiResult<Json<DailyRateDto>> {
    require_version(req.version)?;
    let date = rate_date(&date)?;
    Ok(Json(
        s.rates.update_daily(&s.actor, &currency, date, req).await?,
    ))
}

async fn delete_daily_rate(
    Ctx(s): Ctx,
    Path((currency, date)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    let date = rate_date(&date)?;
    s.rates
        .delete_daily(&s.actor, &currency, date, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_daily_rate(
    Ctx(s): Ctx,
    Path((currency, date)): Path<(String, String)>,
    Json(req): Json<DailyRateCopyRequest>,
) -> ApiResult<(StatusCode, Json<DailyRateDto>)> {
    let date = rate_date(&date)?;
    let dto = s
        .rates
        .copy_daily(&s.actor, &currency, date, &req.currency_code, req.rate_date)
        .await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

async fn set_daily_rate_state(
    Ctx(s): Ctx,
    Path((currency, date, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let date = rate_date(&date)?;
    let (r, a) = (&s.rates, &s.actor);
    let dto = match state.as_str() {
        "activate" => {
            r.set_daily_active(a, &currency, date, true, version)
                .await?
        }
        "deactivate" => {
            r.set_daily_active(a, &currency, date, false, version)
                .await?
        }
        "confirm" => r.confirm_daily(a, &currency, date, true, version).await?,
        "unconfirm" => r.confirm_daily(a, &currency, date, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn fill_daily_rates(
    Ctx(s): Ctx,
    Json(req): Json<DailyRateFillRequest>,
) -> ApiResult<Json<Vec<DailyRateFillLine>>> {
    Ok(Json(s.rates.fill(&s.actor, req.until).await?))
}

/// 每日匯率複製結果的列印、匯出 Excel：結果由畫面帶回（CP1）
async fn fill_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
    Json(lines): Json<Vec<DailyRateFillLine>>,
) -> ApiResult<Response> {
    let Some((format, _, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s.rates.report_fill(&s.actor, lines, format, &lang).await?;
    report_response(&data, format)
}

// ----- Currency Maintenance 幣別維護（aooi050） -----

async fn list_currencies(Ctx(s): Ctx) -> ApiResult<Json<Vec<ttrs_shared::CurrencyDto>>> {
    Ok(Json(s.currencies.list(&s.actor).await?))
}

// ----- EPUB 轉 TXT（epub_to_text.feature） -----

#[derive(Deserialize)]
struct EpubQuery {
    #[serde(default)]
    name: Option<String>,
    /// txt（預設）或 pdf（epub_to_pdf.feature EP1、EP8）
    #[serde(default)]
    format: Option<String>,
    /// 簡體轉繁體（EP2）
    #[serde(default)]
    s2t: Option<bool>,
}

fn convert_options(
    format: Option<&str>,
    s2t: Option<bool>,
) -> Result<ttrs_app::ConvertOptions, ttrs_domain::DomainError> {
    Ok(ttrs_app::ConvertOptions {
        format: ttrs_app::Format::parse(format.unwrap_or("txt"))?,
        s2t: s2t.unwrap_or(false),
    })
}

/// 百分比編碼（回應標頭只能放 ASCII）
fn header_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// txt 回 JSON（EpubTextDto）；pdf 回 application/pdf 二進位，檔名、書名、作者、章數放標頭（EP8）
fn epub_response(out: ttrs_app::EpubOutput) -> Response {
    match out.content {
        ttrs_app::Content::Text(text) => Json(ttrs_shared::EpubTextDto {
            file_name: out.file_name,
            title: out.title,
            author: out.author,
            chapters: out.chapters,
            text,
        })
        .into_response(),
        ttrs_app::Content::Pdf(bytes) => {
            let mut res = bytes.into_response();
            let h = res.headers_mut();
            let mut put = |k: &'static str, v: &str| {
                if let Ok(v) = axum::http::HeaderValue::from_str(&header_encode(v)) {
                    h.insert(k, v);
                }
            };
            put("x-epub-file-name", &out.file_name);
            put("x-epub-title", out.title.as_deref().unwrap_or_default());
            put("x-epub-author", out.author.as_deref().unwrap_or_default());
            put("x-epub-chapters", &out.chapters.to_string());
            h.insert(
                axum::http::header::CONTENT_TYPE,
                axum::http::HeaderValue::from_static("application/pdf"),
            );
            h.insert(
                axum::http::header::ACCESS_CONTROL_EXPOSE_HEADERS,
                axum::http::HeaderValue::from_static(
                    "x-epub-file-name, x-epub-title, x-epub-author, x-epub-chapters",
                ),
            );
            res
        }
    }
}

async fn epub_to_text(
    Ctx(s): Ctx,
    Query(q): Query<EpubQuery>,
    body: axum::body::Bytes,
) -> ApiResult<Response> {
    let name = q.name.unwrap_or_else(|| "book.epub".to_owned());
    let opts = convert_options(q.format.as_deref(), q.s2t).map_err(ttrs_app::AppError::from)?;
    Ok(epub_response(
        s.epub.convert_with(&s.actor, &name, &body, opts).await?,
    ))
}

// ----- EPUB 英文翻譯（epub_translate.feature） -----

#[derive(Deserialize)]
struct TranslateQuery {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    format: Option<String>,
    /// 使用者已勾選「書的內容會送到 Claude」（EL3）
    #[serde(default)]
    consent: Option<bool>,
}

async fn epub_translate_submit(
    Ctx(s): Ctx,
    Query(q): Query<TranslateQuery>,
    body: axum::body::Bytes,
) -> ApiResult<(StatusCode, Json<ttrs_shared::TranslateSubmitDto>)> {
    let name = q.name.unwrap_or_else(|| "book.epub".to_owned());
    let format = ttrs_app::Format::parse(q.format.as_deref().unwrap_or("txt"))
        .map_err(ttrs_app::AppError::from)?;
    let job_id = s
        .translate
        .submit(&s.actor, &name, &body, format, q.consent.unwrap_or(false))
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(ttrs_shared::TranslateSubmitDto { job_id }),
    ))
}

async fn epub_translate_status(
    Ctx(s): Ctx,
    Path(id): Path<String>,
) -> ApiResult<Json<ttrs_shared::TranslateJobDto>> {
    Ok(Json(s.translate.status(&s.actor, &id).await?))
}

async fn epub_translate_download(Ctx(s): Ctx, Path(id): Path<String>) -> ApiResult<Response> {
    Ok(epub_response(s.translate.download(&s.actor, &id).await?))
}

// ----- 以網址下載 EPUB（epub_url_download.feature） -----

#[derive(Deserialize)]
struct EpubUrlQuery {
    url: String,
}

async fn epub_url_read(
    Ctx(s): Ctx,
    Query(q): Query<EpubUrlQuery>,
) -> ApiResult<Json<ttrs_shared::EpubUrlDto>> {
    Ok(Json(s.epub_url.read(&s.actor, &q.url).await?))
}

/// 下載原始 EPUB（UD3）：不保存檔案，直接回傳
async fn epub_url_download(Ctx(s): Ctx, Query(q): Query<EpubUrlQuery>) -> ApiResult<Response> {
    let (file_name, bytes) = s.epub_url.download_raw(&s.actor, &q.url).await?;
    let mut res = bytes.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/epub+zip"),
    );
    if let Ok(v) = HeaderValue::from_str(&header_encode(&file_name)) {
        h.insert("x-epub-file-name", v);
    }
    h.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("x-epub-file-name"),
    );
    Ok(res)
}

/// 轉換網址的 EPUB（UD3）：請求的 `id` 欄位放網址；要翻譯時改為提交翻譯工作（202）
async fn epub_url_convert(
    Ctx(s): Ctx,
    Json(req): Json<ttrs_shared::EpubBookConvertRequest>,
) -> ApiResult<Response> {
    let opts = convert_options(req.format.as_deref(), req.s2t).map_err(ttrs_app::AppError::from)?;
    let (stem, bytes) = s.epub_url.fetch(&s.actor, &req.id).await?;
    if req.translate.unwrap_or(false) {
        let job_id = s
            .translate
            .submit(
                &s.actor,
                &format!("{stem}.epub"),
                &bytes,
                opts.format,
                req.consent.unwrap_or(false),
            )
            .await?;
        return Ok((
            StatusCode::ACCEPTED,
            Json(ttrs_shared::TranslateSubmitDto { job_id }),
        )
            .into_response());
    }
    Ok(epub_response(
        s.epub_url.render_bytes(&bytes, &stem, opts).await?,
    ))
}

// ----- EPUB 多來源搜尋（epub_web_search.feature） -----

#[derive(Deserialize)]
struct EpubSearchQuery {
    q: String,
    /// 只看開放授權（預設否：不替使用者過濾，授權標示在每一筆上）
    #[serde(default)]
    open: Option<bool>,
    #[serde(default)]
    page: Option<u32>,
}

async fn epub_search(
    Ctx(s): Ctx,
    Query(q): Query<EpubSearchQuery>,
) -> ApiResult<Json<ttrs_shared::EpubSearchPage>> {
    Ok(Json(
        s.epub_search
            .search(&s.actor, &q.q, q.open.unwrap_or(false), q.page.unwrap_or(1))
            .await?,
    ))
}

/// 列出我的 GitHub 書庫（ML2）
async fn epub_library(Ctx(s): Ctx) -> ApiResult<Json<Vec<ttrs_shared::EpubLibraryRepoDto>>> {
    Ok(Json(s.epub_search.library(&s.actor).await?))
}

/// 加入我的 GitHub 書庫（ML2）
async fn epub_library_add(
    Ctx(s): Ctx,
    Json(req): Json<ttrs_shared::EpubLibraryAddRequest>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::EpubLibraryRepoDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.epub_search.add_library(&s.actor, &req.repo).await?),
    ))
}

#[derive(Deserialize)]
struct EpubLibraryRemoveQuery {
    repo: String,
    #[serde(default)]
    version: Option<i32>,
}

/// 從我的 GitHub 書庫移除（ML2）：需帶版本（ADR 0012）
async fn epub_library_remove(
    Ctx(s): Ctx,
    Query(q): Query<EpubLibraryRemoveQuery>,
) -> ApiResult<StatusCode> {
    s.epub_search
        .remove_library(&s.actor, &q.repo, require_version(q.version)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 列出共用 GitHub 書庫（SL3）
async fn epub_shared_library(Ctx(s): Ctx) -> ApiResult<Json<Vec<ttrs_shared::EpubSharedRepoDto>>> {
    Ok(Json(s.epub_search.shared_library(&s.actor).await?))
}

/// 加入共用 GitHub 書庫（SL3；需 share 功能）
async fn epub_shared_library_add(
    Ctx(s): Ctx,
    Json(req): Json<ttrs_shared::EpubLibraryAddRequest>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::EpubSharedRepoDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(
            s.epub_search
                .add_shared_library(&s.actor, &req.repo)
                .await?,
        ),
    ))
}

/// 從共用 GitHub 書庫移除（SL3；需 share 功能、需帶版本）
async fn epub_shared_library_remove(
    Ctx(s): Ctx,
    Query(q): Query<EpubLibraryRemoveQuery>,
) -> ApiResult<StatusCode> {
    s.epub_search
        .remove_shared_library(&s.actor, &q.repo, require_version(q.version)?)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct EpubDownloadQuery {
    source: String,
    id: String,
    #[serde(default)]
    title: Option<String>,
}

/// 下載原始 EPUB（WS8）：不保存檔案，直接回傳
async fn epub_search_download(
    Ctx(s): Ctx,
    Query(q): Query<EpubDownloadQuery>,
) -> ApiResult<Response> {
    let (file_name, bytes) = s
        .epub_search
        .download_raw(&s.actor, &q.source, &q.id, q.title.as_deref())
        .await?;
    let mut res = bytes.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/epub+zip"),
    );
    if let Ok(v) = HeaderValue::from_str(&header_encode(&file_name)) {
        h.insert("x-epub-file-name", v);
    }
    h.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("x-epub-file-name"),
    );
    Ok(res)
}

/// 轉換搜尋結果中的一本（WS8）：txt／pdf、可簡轉繁；要翻譯時改為提交翻譯工作（202）
async fn epub_search_convert(
    Ctx(s): Ctx,
    Json(req): Json<ttrs_shared::EpubBookConvertRequest>,
) -> ApiResult<Response> {
    let opts = convert_options(req.format.as_deref(), req.s2t).map_err(ttrs_app::AppError::from)?;
    let (stem, bytes) = s
        .epub_search
        .fetch(&s.actor, &req.source, &req.id, req.title.as_deref())
        .await?;
    if req.translate.unwrap_or(false) {
        let job_id = s
            .translate
            .submit(
                &s.actor,
                &format!("{stem}.epub"),
                &bytes,
                opts.format,
                req.consent.unwrap_or(false),
            )
            .await?;
        return Ok((
            StatusCode::ACCEPTED,
            Json(ttrs_shared::TranslateSubmitDto { job_id }),
        )
            .into_response());
    }
    Ok(epub_response(
        s.epub_search.render_bytes(&bytes, &stem, opts).await?,
    ))
}

// ----- GitHub EPUB 搜尋（github_epub_search.feature） -----

#[derive(Deserialize)]
struct GithubSearchQuery {
    q: String,
    #[serde(default)]
    open: Option<bool>,
    #[serde(default)]
    page: Option<u32>,
}

async fn github_epub_repos(
    Ctx(s): Ctx,
    Query(q): Query<GithubSearchQuery>,
) -> ApiResult<Json<ttrs_shared::GithubRepoPage>> {
    Ok(Json(
        s.github_epub
            .search(&s.actor, &q.q, q.open.unwrap_or(false), q.page.unwrap_or(1))
            .await?,
    ))
}

#[derive(Deserialize)]
struct GithubFilesQuery {
    repo: String,
}

async fn github_epub_files(
    Ctx(s): Ctx,
    Query(q): Query<GithubFilesQuery>,
) -> ApiResult<Json<ttrs_shared::GithubEpubListDto>> {
    Ok(Json(s.github_epub.files(&s.actor, &q.repo).await?))
}

async fn github_epub_convert(
    Ctx(s): Ctx,
    Json(req): Json<ttrs_shared::GithubEpubConvertRequest>,
) -> ApiResult<Response> {
    let opts = convert_options(req.format.as_deref(), req.s2t).map_err(ttrs_app::AppError::from)?;
    Ok(epub_response(
        s.github_epub
            .convert_with(&s.actor, &req.repo, &req.path, opts)
            .await?,
    ))
}

// ----- 單位換算（aooi102、aooi103；unit_conversion.feature） -----

async fn query_unit_conversions(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::UnitConversionDto>>> {
    Ok(Json(s.unit_conversions.query(&s.actor, req).await?))
}

async fn create_unit_conversion(
    Ctx(s): Ctx,
    Json(req): Json<ttrs_shared::UnitConversionInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::UnitConversionDto>)> {
    let dto = s.unit_conversions.create(&s.actor, req).await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

async fn get_unit_conversion(
    Ctx(s): Ctx,
    Path((from, to)): Path<(String, String)>,
) -> ApiResult<Json<ttrs_shared::UnitConversionDto>> {
    Ok(Json(s.unit_conversions.get(&s.actor, &from, &to).await?))
}

async fn update_unit_conversion(
    Ctx(s): Ctx,
    Path((from, to)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::UnitConversionInput>,
) -> ApiResult<Json<ttrs_shared::UnitConversionDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.unit_conversions.update(&s.actor, &from, &to, req).await?,
    ))
}

async fn delete_unit_conversion(
    Ctx(s): Ctx,
    Path((from, to)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.unit_conversions
        .delete(&s.actor, &from, &to, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct CopyConversionRequest {
    from_unit: String,
    to_unit: String,
}

async fn copy_unit_conversion(
    Ctx(s): Ctx,
    Path((from, to)): Path<(String, String)>,
    Json(req): Json<CopyConversionRequest>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::UnitConversionDto>)> {
    let dto = s
        .unit_conversions
        .copy(&s.actor, &from, &to, &req.from_unit, &req.to_unit)
        .await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

/// 停用（deactivate）、啟用（activate）、確認（confirm）、取消確認（unconfirm）
async fn set_unit_conversion_state(
    Ctx(s): Ctx,
    Path((from, to, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let u = &s.unit_conversions;
    let a = &s.actor;
    let dto = match state.as_str() {
        "activate" => u.set_active(a, &from, &to, true, version).await?,
        "deactivate" => u.set_active(a, &from, &to, false, version).await?,
        "confirm" => u.confirm(a, &from, &to, true, version).await?,
        "unconfirm" => u.confirm(a, &from, &to, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn unit_conversion_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .unit_conversions
        .report(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn list_item_unit_conversions(
    Ctx(s): Ctx,
    Path((plant, item)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ttrs_shared::ItemUnitConversionDto>>> {
    Ok(Json(
        s.item_unit_conversions
            .list(&s.actor, &plant, &item)
            .await?,
    ))
}

async fn create_item_unit_conversion(
    Ctx(s): Ctx,
    Path((plant, item)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::UnitConversionInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::ItemUnitConversionDto>)> {
    let dto = s
        .item_unit_conversions
        .create(&s.actor, &plant, &item, req)
        .await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

async fn update_item_unit_conversion(
    Ctx(s): Ctx,
    Path((plant, item, from, to)): Path<(String, String, String, String)>,
    Json(req): Json<ttrs_shared::UnitConversionInput>,
) -> ApiResult<Json<ttrs_shared::ItemUnitConversionDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.item_unit_conversions
            .update(&s.actor, &plant, &item, &from, &to, req)
            .await?,
    ))
}

async fn delete_item_unit_conversion(
    Ctx(s): Ctx,
    Path((plant, item, from, to)): Path<(String, String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.item_unit_conversions
        .delete(&s.actor, &plant, &item, &from, &to, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 停用（deactivate）、啟用（activate）
async fn set_item_unit_conversion_state(
    Ctx(s): Ctx,
    Path((plant, item, from, to, state)): Path<(String, String, String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let active = match state.as_str() {
        "activate" => true,
        "deactivate" => false,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    let dto = s
        .item_unit_conversions
        .set_active(&s.actor, &plant, &item, &from, &to, active, version)
        .await?;
    Ok(Json(dto).into_response())
}

async fn item_unit_conversion_history(
    Ctx(s): Ctx,
    Path((plant, item)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.item_unit_conversions
            .history(&s.actor, &plant, &item, q.lang.as_deref())
            .await?,
    ))
}

#[derive(Deserialize)]
struct ItemReportQuery {
    #[serde(default)]
    item: Option<String>,
    #[serde(default)]
    lang: Option<String>,
}

async fn item_unit_conversion_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ItemReportQuery>,
) -> ApiResult<Response> {
    let rq = ReportQuery {
        q: None,
        lang: q.lang.clone(),
    };
    let Some((format, _, lang)) = report_args(&format, &rq)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let item = q.item.as_deref().filter(|i| !i.trim().is_empty());
    let data = s
        .item_unit_conversions
        .report(&s.actor, &plant, item, format, &lang)
        .await?;
    report_response(&data, format)
}

#[derive(Deserialize)]
struct FactorQuery {
    item: String,
    from: String,
    to: String,
}

/// 換算率（s_umfchk，UC7）
async fn unit_conversion_factor(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Query(q): Query<FactorQuery>,
) -> ApiResult<Json<rust_decimal::Decimal>> {
    Ok(Json(
        s.item_unit_conversions
            .factor(&s.actor, &plant, &q.item, &q.from, &q.to)
            .await?,
    ))
}

// ----- 會計期間（aoos020；accounting_periods.feature） -----

async fn query_accounting_years(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::AccountingYearDto>>> {
    Ok(Json(s.accounting_periods.query(&s.actor, req).await?))
}

#[derive(Deserialize)]
struct PeriodDefaultsQuery {
    year: i32,
    start: chrono::NaiveDate,
}

/// 新增時的預設各期（PD5）
async fn accounting_year_defaults(
    Ctx(s): Ctx,
    Query(q): Query<PeriodDefaultsQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::AccountingPeriodLine>>> {
    Ok(Json(
        s.accounting_periods
            .defaults(&s.actor, q.year, q.start)
            .await?,
    ))
}

async fn create_accounting_year(
    Ctx(s): Ctx,
    Json(req): Json<ttrs_shared::AccountingYearInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::AccountingYearDto>)> {
    let dto = s.accounting_periods.create(&s.actor, req).await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

async fn get_accounting_year(
    Ctx(s): Ctx,
    Path(year): Path<i32>,
) -> ApiResult<Json<ttrs_shared::AccountingYearDto>> {
    Ok(Json(s.accounting_periods.get(&s.actor, year).await?))
}

async fn update_accounting_year(
    Ctx(s): Ctx,
    Path(year): Path<i32>,
    Json(req): Json<ttrs_shared::AccountingYearInput>,
) -> ApiResult<Json<ttrs_shared::AccountingYearDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.accounting_periods.update(&s.actor, year, req).await?,
    ))
}

async fn delete_accounting_year(
    Ctx(s): Ctx,
    Path(year): Path<i32>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.accounting_periods.delete(&s.actor, year, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct CopyYearRequest {
    to: i32,
}

/// 複製為另一個年度（PD9）
async fn copy_accounting_year(
    Ctx(s): Ctx,
    Path(year): Path<i32>,
    Json(req): Json<CopyYearRequest>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::AccountingYearDto>)> {
    let dto = s.accounting_periods.copy(&s.actor, year, req.to).await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

/// 確認（confirm）、取消確認（unconfirm）
async fn set_accounting_year_state(
    Ctx(s): Ctx,
    Path((year, state)): Path<(i32, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let u = &s.accounting_periods;
    let dto = match state.as_str() {
        "confirm" => u.confirm(&s.actor, year, true, version).await?,
        "unconfirm" => u.confirm(&s.actor, year, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

/// 年度＋期別的起訖日（s_azm，PD11）
async fn accounting_period_dates(
    Ctx(s): Ctx,
    Path((year, period)): Path<(i32, i16)>,
) -> ApiResult<Json<ttrs_shared::AccountingPeriodLine>> {
    Ok(Json(
        s.accounting_periods
            .period_dates(&s.actor, year, period)
            .await?,
    ))
}

#[derive(Deserialize)]
struct AccountingDaysQuery {
    from: chrono::NaiveDate,
    #[serde(default)]
    to: Option<chrono::NaiveDate>,
}

/// 日期所屬期間（s_gactpd）與週期期間查詢（aooq010，PD3）：只給 from 時回一天
async fn accounting_days(
    Ctx(s): Ctx,
    Query(q): Query<AccountingDaysQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::AccountingDayDto>>> {
    let u = &s.accounting_periods;
    Ok(Json(match q.to {
        Some(to) => u.days(&s.actor, q.from, to).await?,
        None => vec![u.locate(&s.actor, q.from).await?],
    }))
}

async fn accounting_year_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .accounting_periods
        .report(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- 單別維護（asmi300；document_type.feature） -----

use ttrs_shared::{DocTypeAccessInput, DocTypeStorageInput, DocumentTypeDto, DocumentTypeInput};

async fn query_document_types(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<DocumentTypeDto>>> {
    Ok(Json(s.document_types.query(&s.actor, &plant, req).await?))
}

async fn get_document_type(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
) -> ApiResult<Json<DocumentTypeDto>> {
    Ok(Json(s.document_types.get(&s.actor, &plant, &code).await?))
}

async fn create_document_type(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<DocumentTypeInput>,
) -> ApiResult<(StatusCode, Json<DocumentTypeDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.document_types.create(&s.actor, &plant, req).await?),
    ))
}

async fn update_document_type(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<DocumentTypeInput>,
) -> ApiResult<Json<DocumentTypeDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.document_types
            .update(&s.actor, &plant, &code, req)
            .await?,
    ))
}

async fn delete_document_type(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.document_types
        .delete(&s.actor, &plant, &code, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_document_type(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<DocumentTypeDto>)> {
    let d = s
        .document_types
        .copy(&s.actor, &plant, &code, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(d)))
}

async fn document_type_history(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.document_types
            .history(&s.actor, &plant, &code, q.lang.as_deref())
            .await?,
    ))
}

/// 倉庫設限（detail）
async fn set_document_type_storages(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<DocTypeStorageInput>,
) -> ApiResult<Json<DocumentTypeDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.document_types
            .set_storages(&s.actor, &plant, &code, req)
            .await?,
    ))
}

/// 使用者設限（authorization）、部門設限（dept_authorization）
async fn set_document_type_access(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<DocTypeAccessInput>,
) -> ApiResult<Json<DocumentTypeDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.document_types
            .set_access(&s.actor, &plant, &code, req)
            .await?,
    ))
}

/// 停用（deactivate）、啟用（activate）、確認（confirm）、取消確認（unconfirm）
async fn set_document_type_state(
    Ctx(s): Ctx,
    Path((plant, code, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let g = &s.document_types;
    let dto = match state.as_str() {
        "activate" => g.set_active(&s.actor, &plant, &code, true, version).await?,
        "deactivate" => {
            g.set_active(&s.actor, &plant, &code, false, version)
                .await?
        }
        "confirm" => g.confirm(&s.actor, &plant, &code, true, version).await?,
        "unconfirm" => g.confirm(&s.actor, &plant, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn document_type_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .document_types
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- 雜項發料（aimt301；misc_issue.feature） -----

async fn query_misc_issues(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::InventoryTxnDto>>> {
    Ok(Json(s.misc_issues.query(&s.actor, &plant, req).await?))
}

async fn get_misc_issue(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    Ok(Json(s.misc_issues.get(&s.actor, &plant, &no).await?))
}

async fn create_misc_issue(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ttrs_shared::InventoryTxnInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::InventoryTxnDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.misc_issues.create(&s.actor, &plant, req).await?),
    ))
}

async fn update_misc_issue(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::InventoryTxnInput>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.misc_issues.update(&s.actor, &plant, &no, req).await?,
    ))
}

async fn delete_misc_issue(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.misc_issues.delete(&s.actor, &plant, &no, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_misc_issue(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::InventoryTxnCopyInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::InventoryTxnDto>)> {
    let d = s.misc_issues.copy(&s.actor, &plant, &no, req).await?;
    Ok((StatusCode::CREATED, Json(d)))
}

async fn misc_issue_history(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.misc_issues
            .history(&s.actor, &plant, &no, q.lang.as_deref())
            .await?,
    ))
}

/// 確認（confirm）、取消確認（undo-confirm）、作廢（void）、取消作廢（unvoid）
async fn set_misc_issue_state(
    Ctx(s): Ctx,
    Path((plant, no, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let g = &s.misc_issues;
    let dto = match state.as_str() {
        "confirm" => g.confirm(&s.actor, &plant, &no, true, version).await?,
        "undo-confirm" => g.confirm(&s.actor, &plant, &no, false, version).await?,
        "void" => g.set_void(&s.actor, &plant, &no, true, version).await?,
        "unvoid" => g.set_void(&s.actor, &plant, &no, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

/// 庫存過帳（stock_post）：可指定扣帳日（PT2）
async fn post_misc_issue(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
    Json(req): Json<ttrs_shared::InventoryPostInput>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.misc_issues
            .post(&s.actor, &plant, &no, req.posting_date, version)
            .await?,
    ))
}

/// 過帳還原（undo_post；PT8）
async fn undo_post_misc_issue(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.misc_issues
            .undo_post(&s.actor, &plant, &no, version)
            .await?,
    ))
}

/// 本單的庫存異動記錄（PT6）
async fn misc_issue_txn_records(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ttrs_shared::InventoryTxnRecordDto>>> {
    let v = s.misc_issues.txn_records(&s.actor, &plant, &no).await?;
    Ok(Json(v.into_iter().map(txn_record_dto).collect()))
}

async fn misc_issue_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .misc_issues
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- 雜項收料（aimt302；misc_receipt.feature） -----

async fn query_misc_receipts(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::InventoryTxnDto>>> {
    Ok(Json(s.misc_receipts.query(&s.actor, &plant, req).await?))
}

/// 雜收至新倉儲時提示（MT8(b)）：回傳還沒有庫存資料的單身列
async fn misc_receipt_new_storages(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<Vec<ttrs_shared::InventoryTxnLineInput>>,
) -> ApiResult<Json<Vec<usize>>> {
    Ok(Json(
        s.misc_receipts.new_storages(&s.actor, &plant, req).await?,
    ))
}

async fn get_misc_receipt(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    Ok(Json(s.misc_receipts.get(&s.actor, &plant, &no).await?))
}

async fn create_misc_receipt(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ttrs_shared::InventoryTxnInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::InventoryTxnDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.misc_receipts.create(&s.actor, &plant, req).await?),
    ))
}

async fn update_misc_receipt(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::InventoryTxnInput>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.misc_receipts.update(&s.actor, &plant, &no, req).await?,
    ))
}

async fn delete_misc_receipt(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.misc_receipts
        .delete(&s.actor, &plant, &no, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_misc_receipt(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::InventoryTxnCopyInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::InventoryTxnDto>)> {
    let d = s.misc_receipts.copy(&s.actor, &plant, &no, req).await?;
    Ok((StatusCode::CREATED, Json(d)))
}

async fn misc_receipt_history(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.misc_receipts
            .history(&s.actor, &plant, &no, q.lang.as_deref())
            .await?,
    ))
}

/// 確認（confirm）、取消確認（undo-confirm）、作廢（void）、取消作廢（unvoid）
async fn set_misc_receipt_state(
    Ctx(s): Ctx,
    Path((plant, no, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let g = &s.misc_receipts;
    let dto = match state.as_str() {
        "confirm" => g.confirm(&s.actor, &plant, &no, true, version).await?,
        "undo-confirm" => g.confirm(&s.actor, &plant, &no, false, version).await?,
        "void" => g.set_void(&s.actor, &plant, &no, true, version).await?,
        "unvoid" => g.set_void(&s.actor, &plant, &no, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

/// 庫存過帳（stock_post）：可指定扣帳日（PT2）
async fn post_misc_receipt(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
    Json(req): Json<ttrs_shared::InventoryPostInput>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.misc_receipts
            .post(&s.actor, &plant, &no, req.posting_date, version)
            .await?,
    ))
}

/// 過帳還原（undo_post；PT8）
async fn undo_post_misc_receipt(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.misc_receipts
            .undo_post(&s.actor, &plant, &no, version)
            .await?,
    ))
}

/// 本單的庫存異動記錄（PT6）
async fn misc_receipt_txn_records(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ttrs_shared::InventoryTxnRecordDto>>> {
    let v = s.misc_receipts.txn_records(&s.actor, &plant, &no).await?;
    Ok(Json(v.into_iter().map(txn_record_dto).collect()))
}

async fn misc_receipt_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .misc_receipts
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- 雜項報廢（aimt303；misc_scrap.feature） -----

async fn query_misc_scraps(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::InventoryTxnDto>>> {
    Ok(Json(s.misc_scraps.query(&s.actor, &plant, req).await?))
}

async fn get_misc_scrap(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    Ok(Json(s.misc_scraps.get(&s.actor, &plant, &no).await?))
}

async fn create_misc_scrap(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ttrs_shared::InventoryTxnInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::InventoryTxnDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.misc_scraps.create(&s.actor, &plant, req).await?),
    ))
}

async fn update_misc_scrap(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::InventoryTxnInput>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.misc_scraps.update(&s.actor, &plant, &no, req).await?,
    ))
}

async fn delete_misc_scrap(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.misc_scraps.delete(&s.actor, &plant, &no, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_misc_scrap(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::InventoryTxnCopyInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::InventoryTxnDto>)> {
    let d = s.misc_scraps.copy(&s.actor, &plant, &no, req).await?;
    Ok((StatusCode::CREATED, Json(d)))
}

async fn misc_scrap_history(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.misc_scraps
            .history(&s.actor, &plant, &no, q.lang.as_deref())
            .await?,
    ))
}

/// 確認（confirm）、取消確認（undo-confirm）、作廢（void）、取消作廢（unvoid）
async fn set_misc_scrap_state(
    Ctx(s): Ctx,
    Path((plant, no, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let g = &s.misc_scraps;
    let dto = match state.as_str() {
        "confirm" => g.confirm(&s.actor, &plant, &no, true, version).await?,
        "undo-confirm" => g.confirm(&s.actor, &plant, &no, false, version).await?,
        "void" => g.set_void(&s.actor, &plant, &no, true, version).await?,
        "unvoid" => g.set_void(&s.actor, &plant, &no, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

/// 庫存過帳（stock_post）：可指定扣帳日（PT2）
async fn post_misc_scrap(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
    Json(req): Json<ttrs_shared::InventoryPostInput>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.misc_scraps
            .post(&s.actor, &plant, &no, req.posting_date, version)
            .await?,
    ))
}

/// 過帳還原（undo_post；PT8）
async fn undo_post_misc_scrap(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Json<ttrs_shared::InventoryTxnDto>> {
    let version = Some(require_version(v.version)?);
    Ok(Json(
        s.misc_scraps
            .undo_post(&s.actor, &plant, &no, version)
            .await?,
    ))
}

/// 本單的庫存異動記錄（PT6）
async fn misc_scrap_txn_records(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ttrs_shared::InventoryTxnRecordDto>>> {
    let v = s.misc_scraps.txn_records(&s.actor, &plant, &no).await?;
    Ok(Json(v.into_iter().map(txn_record_dto).collect()))
}

async fn misc_scrap_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .misc_scraps
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

fn txn_record_dto(
    r: ttrs_domain::inventory_balance::InventoryTxnRecord,
) -> ttrs_shared::InventoryTxnRecordDto {
    ttrs_shared::InventoryTxnRecordDto {
        doc_no: r.doc_no,
        doc_line: r.doc_line,
        item_no: r.key.item,
        warehouse: r.key.warehouse,
        location: r.key.location,
        lot: r.key.lot,
        direction: r.direction,
        source: r.source,
        dest: r.dest,
        txn_date: r.txn_date,
        qty: r.qty,
        unit: r.unit,
        balance_after: r.balance_after,
        program: r.program,
        reason: r.reason,
        created_by: r.created_by,
        created_at: r.created_at,
    }
}

// ----- 庫存直接調撥（aimt324；warehouse_transfer.feature） -----

async fn query_warehouse_transfers(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::WarehouseTransferDto>>> {
    Ok(Json(
        s.warehouse_transfers.query(&s.actor, &plant, req).await?,
    ))
}

async fn get_warehouse_transfer(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<ttrs_shared::WarehouseTransferDto>> {
    Ok(Json(
        s.warehouse_transfers.get(&s.actor, &plant, &no).await?,
    ))
}

async fn create_warehouse_transfer(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ttrs_shared::WarehouseTransferInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::WarehouseTransferDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.warehouse_transfers.create(&s.actor, &plant, req).await?),
    ))
}

async fn update_warehouse_transfer(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::WarehouseTransferInput>,
) -> ApiResult<Json<ttrs_shared::WarehouseTransferDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.warehouse_transfers
            .update(&s.actor, &plant, &no, req)
            .await?,
    ))
}

async fn delete_warehouse_transfer(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.warehouse_transfers
        .delete(&s.actor, &plant, &no, version)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_warehouse_transfer(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Json(req): Json<ttrs_shared::WarehouseTransferCopyInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::WarehouseTransferDto>)> {
    let d = s
        .warehouse_transfers
        .copy(&s.actor, &plant, &no, req)
        .await?;
    Ok((StatusCode::CREATED, Json(d)))
}

async fn warehouse_transfer_history(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.warehouse_transfers
            .history(&s.actor, &plant, &no, q.lang.as_deref())
            .await?,
    ))
}

async fn warehouse_transfer_txn_records(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<Vec<ttrs_shared::InventoryTxnRecordDto>>> {
    let v = s
        .warehouse_transfers
        .txn_records(&s.actor, &plant, &no)
        .await?;
    Ok(Json(v.into_iter().map(txn_record_dto).collect()))
}

/// 撥入到新倉儲批時提示（WT5(d)）
async fn warehouse_transfer_new_storages(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<Vec<ttrs_shared::WarehouseTransferLineInput>>,
) -> ApiResult<Json<Vec<usize>>> {
    Ok(Json(
        s.warehouse_transfers
            .new_storages(&s.actor, &plant, req)
            .await?,
    ))
}

/// 確認（confirm）、取消確認（undo-confirm）、作廢（void）、取消作廢（unvoid）、過帳（post）、過帳還原（undo-post）
async fn set_warehouse_transfer_state(
    Ctx(s): Ctx,
    Path((plant, no, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let g = &s.warehouse_transfers;
    let dto = match state.as_str() {
        "confirm" => g.confirm(&s.actor, &plant, &no, true, version).await?,
        "undo-confirm" => g.confirm(&s.actor, &plant, &no, false, version).await?,
        "void" => g.set_void(&s.actor, &plant, &no, true, version).await?,
        "unvoid" => g.set_void(&s.actor, &plant, &no, false, version).await?,
        "post" => g.post(&s.actor, &plant, &no, version).await?,
        "undo-post" => g.undo_post(&s.actor, &plant, &no, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn warehouse_transfer_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .warehouse_transfers
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- 料件 BIN 卡查詢（aimq231；item_bin_card.feature） -----

async fn query_bin_cards(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ttrs_shared::BinCardQuery>,
) -> ApiResult<Json<Page<ttrs_shared::BinCardDto>>> {
    Ok(Json(s.bin_cards.query(&s.actor, &plant, req).await?))
}

#[derive(Deserialize)]
struct BinCardPeriod {
    year: i32,
    period: i16,
}

async fn bin_card_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(period): Query<BinCardPeriod>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .bin_cards
        .report_in(
            &s.actor,
            &plant,
            period.year,
            period.period,
            conditions,
            format,
            &lang,
        )
        .await?;
    report_response(&data, format)
}

// ----- 料件庫存數量查詢（aimq400；item_stock_query.feature） -----

async fn query_item_stock(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::ItemStockDto>>> {
    Ok(Json(s.item_stock.query(&s.actor, &plant, req).await?))
}

async fn item_stock_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .item_stock
        .report_in(&s.actor, &plant, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- 庫存參數（asms230；inventory_parameters.feature） -----

async fn get_inventory_parameters(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
) -> ApiResult<Json<ttrs_shared::InventoryParametersDto>> {
    Ok(Json(s.inventory_parameters.get(&s.actor, &plant).await?))
}

async fn update_inventory_parameters(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Json(req): Json<ttrs_shared::InventoryParametersInput>,
) -> ApiResult<Json<ttrs_shared::InventoryParametersDto>> {
    require_version(req.version)?;
    Ok(Json(
        s.inventory_parameters.update(&s.actor, &plant, req).await?,
    ))
}

async fn inventory_parameters_history(
    Ctx(s): Ctx,
    Path(plant): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.inventory_parameters
            .history(&s.actor, &plant, q.lang.as_deref())
            .await?,
    ))
}

async fn inventory_parameters_report(
    Ctx(s): Ctx,
    Path((plant, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, _, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .inventory_parameters
        .report(&s.actor, &plant, format, &lang)
        .await?;
    report_response(&data, format)
}

// ----- 整體管理系統參數（aoos010；general_parameters.feature） -----

async fn get_general_parameters(Ctx(s): Ctx) -> ApiResult<Json<ttrs_shared::GeneralParametersDto>> {
    Ok(Json(s.general_parameters.get(&s.actor).await?))
}

async fn update_general_parameters(
    Ctx(s): Ctx,
    Json(req): Json<ttrs_shared::GeneralParametersInput>,
) -> ApiResult<Json<ttrs_shared::GeneralParametersDto>> {
    require_version(req.version)?;
    Ok(Json(s.general_parameters.update(&s.actor, req).await?))
}

async fn general_parameters_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, _, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s.general_parameters.report(&s.actor, format, &lang).await?;
    report_response(&data, format)
}

async fn monthly_rate_settings(Ctx(s): Ctx) -> ApiResult<Json<ttrs_shared::RateSettingsDto>> {
    Ok(Json(
        s.rates
            .settings(&s.actor, ttrs_app::exchange_rate_service::MONTHLY)
            .await?,
    ))
}

async fn daily_rate_settings(Ctx(s): Ctx) -> ApiResult<Json<ttrs_shared::RateSettingsDto>> {
    Ok(Json(
        s.rates
            .settings(&s.actor, ttrs_app::exchange_rate_service::DAILY)
            .await?,
    ))
}

async fn get_currency(
    Ctx(s): Ctx,
    Path(code): Path<String>,
) -> ApiResult<Json<ttrs_shared::CurrencyDto>> {
    Ok(Json(s.currencies.get(&s.actor, &code).await?))
}

async fn create_currency(
    Ctx(s): Ctx,
    Json(req): Json<ttrs_shared::CurrencyInput>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::CurrencyDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.currencies.create(&s.actor, req).await?),
    ))
}

async fn update_currency(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<ttrs_shared::CurrencyInput>,
) -> ApiResult<Json<ttrs_shared::CurrencyDto>> {
    require_version(req.version)?;
    Ok(Json(s.currencies.update(&s.actor, &code, req).await?))
}

/// 停用（deactivate）、啟用（activate）
async fn set_currency_state(
    Ctx(s): Ctx,
    Path((code, state)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let version = Some(require_version(v.version)?);
    let u = &s.currencies;
    let dto = match state.as_str() {
        "activate" => u.set_active(&s.actor, &code, true, version).await?,
        "deactivate" => u.set_active(&s.actor, &code, false, version).await?,
        // 確認／取消確認（ADR 0013 SA9）
        "confirm" => u.confirm(&s.actor, &code, true, version).await?,
        "unconfirm" => u.confirm(&s.actor, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn query_currencies(
    Ctx(s): Ctx,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ttrs_shared::CurrencyDto>>> {
    Ok(Json(s.currencies.query(&s.actor, req).await?))
}

async fn copy_currency(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<ttrs_shared::CurrencyDto>)> {
    let u = s.currencies.copy(&s.actor, &code, &req.new_code).await?;
    Ok((StatusCode::CREATED, Json(u)))
}

async fn currency_report(
    Ctx(s): Ctx,
    Path(format): Path<String>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .currencies
        .report_in(&s.actor, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn delete_currency(
    Ctx(s): Ctx,
    Path(code): Path<String>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let version = Some(require_version(v.version)?);
    s.currencies.delete(&s.actor, &code, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- 基本代碼：區域別 aooi100、國別 aooi090、地區 aooi110、交運方式 aooi160、報表抬頭 aooi210 -----

use ttrs_domain::basic_code::BasicCodeKind;
use ttrs_shared::{BasicCodeDto, BasicCodeInput};

/// 路徑的種類（regions、countries…）；不認得視為沒有這支作業（404 PROGRAM_NOT_FOUND）
fn basic_kind(slug: &str) -> Result<BasicCodeKind, HttpError> {
    BasicCodeKind::from_slug(slug)
        .ok_or_else(|| HttpError(ttrs_domain::DomainError::ProgramNotFound.into()))
}

async fn list_basic_codes(
    Ctx(s): Ctx,
    Path(kind): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<BasicCodeDto>>> {
    let kind = basic_kind(&kind)?;
    Ok(Json(
        s.basic_codes
            .list(&s.actor, kind, q.lang.as_deref())
            .await?,
    ))
}

async fn get_basic_code(
    Ctx(s): Ctx,
    Path((kind, code)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<BasicCodeDto>> {
    let kind = basic_kind(&kind)?;
    Ok(Json(
        s.basic_codes
            .get(&s.actor, kind, &code, q.lang.as_deref())
            .await?,
    ))
}

async fn create_basic_code(
    Ctx(s): Ctx,
    Path(kind): Path<String>,
    Json(req): Json<BasicCodeInput>,
) -> ApiResult<(StatusCode, Json<BasicCodeDto>)> {
    let kind = basic_kind(&kind)?;
    Ok((
        StatusCode::CREATED,
        Json(s.basic_codes.create(&s.actor, kind, req).await?),
    ))
}

async fn update_basic_code(
    Ctx(s): Ctx,
    Path((kind, code)): Path<(String, String)>,
    Json(req): Json<BasicCodeInput>,
) -> ApiResult<Json<BasicCodeDto>> {
    let kind = basic_kind(&kind)?;
    require_version(req.version)?;
    Ok(Json(
        s.basic_codes.update(&s.actor, kind, &code, req).await?,
    ))
}

/// 停用 deactivate、啟用 activate、確認 confirm、取消確認 unconfirm
async fn set_basic_code_state(
    Ctx(s): Ctx,
    Path((kind, code, state)): Path<(String, String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<Response> {
    let kind = basic_kind(&kind)?;
    let version = Some(require_version(v.version)?);
    let b = &s.basic_codes;
    let dto = match state.as_str() {
        "activate" => b.set_active(&s.actor, kind, &code, true, version).await?,
        "deactivate" => b.set_active(&s.actor, kind, &code, false, version).await?,
        "confirm" => b.confirm(&s.actor, kind, &code, true, version).await?,
        "unconfirm" => b.confirm(&s.actor, kind, &code, false, version).await?,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(dto).into_response())
}

async fn query_basic_codes(
    Ctx(s): Ctx,
    Path(kind): Path<String>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<BasicCodeDto>>> {
    let kind = basic_kind(&kind)?;
    Ok(Json(s.basic_codes.query(&s.actor, kind, req).await?))
}

async fn copy_basic_code(
    Ctx(s): Ctx,
    Path((kind, code)): Path<(String, String)>,
    Json(req): Json<NewCode>,
) -> ApiResult<(StatusCode, Json<BasicCodeDto>)> {
    let kind = basic_kind(&kind)?;
    let c = s
        .basic_codes
        .copy(&s.actor, kind, &code, &req.new_code)
        .await?;
    Ok((StatusCode::CREATED, Json(c)))
}

async fn basic_code_report(
    Ctx(s): Ctx,
    Path((kind, format)): Path<(String, String)>,
    Query(q): Query<ReportQuery>,
) -> ApiResult<Response> {
    let kind = basic_kind(&kind)?;
    let Some((format, conditions, lang)) = report_args(&format, &q)? else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    let data = s
        .basic_codes
        .report_in(&s.actor, kind, conditions, format, &lang)
        .await?;
    report_response(&data, format)
}

async fn delete_basic_code(
    Ctx(s): Ctx,
    Path((kind, code)): Path<(String, String)>,
    Query(v): Query<VersionQuery>,
) -> ApiResult<StatusCode> {
    let kind = basic_kind(&kind)?;
    let version = Some(require_version(v.version)?);
    s.basic_codes.delete(&s.actor, kind, &code, version).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- Menu Maintenance 選單維護（p_zm）與側欄 -----

fn lang_of(q: &LangQuery) -> String {
    q.lang.clone().unwrap_or_else(|| DEFAULT_LANG.to_owned())
}

async fn list_menus(
    Ctx(s): Ctx,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::MenuSummaryDto>>> {
    Ok(Json(s.menus.menus(&s.actor, &lang_of(&q)).await?))
}

/// 目前使用者的側欄樹狀選單（MN9–MN11）
async fn menu_tree(
    Ctx(s): Ctx,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::MenuNodeDto>>> {
    Ok(Json(s.menus.tree(&s.actor, &lang_of(&q)).await?))
}

async fn menu_items(
    Ctx(s): Ctx,
    Path(menu): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::MenuItemDto>>> {
    Ok(Json(s.menus.items(&s.actor, &menu, &lang_of(&q)).await?))
}

async fn add_menu_item(
    Ctx(s): Ctx,
    Path(menu): Path<String>,
    Json(req): Json<ttrs_shared::MenuItemInput>,
) -> ApiResult<(StatusCode, Json<Vec<ttrs_shared::MenuItemDto>>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.menus.add(&s.actor, &menu, req).await?),
    ))
}

async fn remove_menu_item(
    Ctx(s): Ctx,
    Path((menu, program)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    s.menus.remove(&s.actor, &menu, &program).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// 上移（up）、下移（down）
async fn shift_menu_item(
    Ctx(s): Ctx,
    Path((menu, program, direction)): Path<(String, String, String)>,
) -> ApiResult<Response> {
    let up = match direction.as_str() {
        "up" => true,
        "down" => false,
        _ => return Ok(StatusCode::NOT_FOUND.into_response()),
    };
    Ok(Json(s.menus.shift(&s.actor, &menu, &program, up).await?).into_response())
}

async fn renumber_menu(
    Ctx(s): Ctx,
    Path(menu): Path<String>,
) -> ApiResult<Json<Vec<ttrs_shared::MenuItemDto>>> {
    Ok(Json(s.menus.renumber(&s.actor, &menu).await?))
}

async fn copy_menu(
    Ctx(s): Ctx,
    Path(menu): Path<String>,
    Json(req): Json<ttrs_shared::MenuCopyInput>,
) -> ApiResult<StatusCode> {
    s.menus.copy(&s.actor, &menu, &req.new_menu_no).await?;
    Ok(StatusCode::CREATED)
}

async fn delete_menu(Ctx(s): Ctx, Path(menu): Path<String>) -> ApiResult<StatusCode> {
    s.menus.delete(&s.actor, &menu).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn shared_history(
    Ctx(s): Ctx,
    Path((program, key)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.history
            .history(&s.actor, &program, &key, q.lang.as_deref())
            .await?,
    ))
}

async fn department_history(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.departments
            .history(&s.actor, &plant, &code, q.lang.as_deref())
            .await?,
    ))
}

async fn employee_history(
    Ctx(s): Ctx,
    Path((plant, id)): Path<(String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.employees
            .history(&s.actor, &plant, &id, q.lang.as_deref())
            .await?,
    ))
}

async fn code_history(
    Ctx(s): Ctx,
    Path((plant, kind, code)): Path<(String, String, String)>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ttrs_shared::ChangeDto>>> {
    Ok(Json(
        s.classification
            .history(&s.actor, &plant, &kind, &code, q.lang.as_deref())
            .await?,
    ))
}

/// 請求範圍的操作情境（ADR 0012 §2）：請求代號（X-Request-Id 或新產生）、來源位址（nginx 的 X-Forwarded-For／X-Real-IP）、瀏覽器
async fn change_request(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let h = req.headers();
    let text = |name: &str| {
        h.get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    let request_id = text("x-request-id")
        .filter(|v| v.len() == 36 && v.chars().all(|c| c.is_ascii_hexdigit() || c == '-'))
        .unwrap_or_else(ttrs_app::change_log::new_request_id);
    let client_ip = text("x-forwarded-for")
        .and_then(|v| v.split(',').next().map(|ip| ip.trim().to_owned()))
        .or_else(|| text("x-real-ip"))
        .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok());
    let info = ttrs_app::change_log::RequestInfo {
        request_id,
        client_ip,
        user_agent: text("user-agent"),
    };
    ttrs_app::change_log::with_request(info, next.run(req)).await
}
