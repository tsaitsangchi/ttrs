//! 路由與錯誤對應。handler 只做轉譯，業務邏輯一律在 ttrs-app。

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
    Actor, ActorKind, AppError, Authorizer, DataDictionaryService, DepartmentService,
    EmployeeService, ItemService, LegacyCatalogService, PermissionClassService, PlantService,
    ProgramPermissionService, ProgramService, Services, SignInService, UserPermissionService,
    UserService, program_service::DEFAULT_LANG,
};
use ttrs_shared::{
    ApiError, ChangePasswordInput, ColumnInfoDto, ColumnTextDto, ConstraintDto,
    CopyPermissionsRequest, CopyUserPermissionsRequest, CreateItemRequest,
    DepartmentAccountingInput, DepartmentDto, DepartmentInput, EmployeeDto, EmployeeInput,
    GeneratePermissionsRequest, GrantRequest, IndexDto, ItemDto, LegacyImportResultDto,
    LegacyTableDto, LegacyTableSummaryDto, MenuPermissionsRequest, MigrationDto, MyPermissionDto,
    NewPlantIdRequest, NewProgramNoRequest, NewUserIdRequest, Page, PermissionBatchResultDto,
    PermissionClassDto, PermissionClassInput, PlantDto, PlantInput, ProgramDto, ProgramInput,
    ProgramNameDto, ProgramPermissionDto, ProgramPermissionInput, QueryRequest, SessionDto,
    SignInInput, SwitchPlantInput, TableDictionaryDto, TableDictionaryInput, TableTextDto,
    UpdatedCountDto, UserDto, UserInput, UserPermissionDto, UserPermissionInput,
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
    pub sign_in: SignInService,
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
            sign_in,
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
            sign_in,
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
        .route("/api/session/plant", axum::routing::put(switch_plant))
        // 目前使用者的有效權限（authorization.feature AQ13）
        .route("/api/me/permissions/{program}", get(my_permission))
        .route("/api/items", get(list_items).post(create_item))
        .route("/api/items/{plant}/{no}", get(get_item).delete(delete_item))
        .route("/api/items/{plant}/{no}/confirm", post(confirm_item))
        .route("/api/items/{plant}/{no}/unconfirm", post(unconfirm_item))
        .route("/api/programs", get(list_programs).post(create_program))
        .route("/api/programs/query", post(query_programs))
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
        // Department Maintenance 部門維護（TipTop aooi030；工廠層）
        .route("/api/plants/{plant}/departments", post(create_department))
        .route(
            "/api/plants/{plant}/departments/query",
            post(query_departments),
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
    Ok(Json(s.users.update(&s.actor, &id, req).await?))
}

async fn delete_user(Ctx(s): Ctx, Path(id): Path<String>) -> ApiResult<StatusCode> {
    s.users.delete(&s.actor, &id).await?;
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

async fn deactivate_user(Ctx(s): Ctx, Path(id): Path<String>) -> ApiResult<Json<UserDto>> {
    Ok(Json(s.users.deactivate(&s.actor, &id).await?))
}

/// 解除鎖定（p_zx user_unlock）
async fn unlock_user(Ctx(s): Ctx, Path(id): Path<String>) -> ApiResult<StatusCode> {
    s.users.unlock(&s.actor, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn activate_user(Ctx(s): Ctx, Path(id): Path<String>) -> ApiResult<Json<UserDto>> {
    Ok(Json(s.users.activate(&s.actor, &id).await?))
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
    Ok(Json(s.plants.update(&s.actor, &id, req).await?))
}

async fn delete_plant(Ctx(s): Ctx, Path(id): Path<String>) -> ApiResult<StatusCode> {
    s.plants.delete(&s.actor, &id).await?;
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

async fn deactivate_plant(Ctx(s): Ctx, Path(id): Path<String>) -> ApiResult<Json<PlantDto>> {
    Ok(Json(s.plants.deactivate(&s.actor, &id).await?))
}

async fn activate_plant(Ctx(s): Ctx, Path(id): Path<String>) -> ApiResult<Json<PlantDto>> {
    Ok(Json(s.plants.activate(&s.actor, &id).await?))
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
    Ok(Json(s.classes.update(&s.actor, &code, req).await?))
}

async fn delete_class(Ctx(s): Ctx, Path(code): Path<String>) -> ApiResult<StatusCode> {
    s.classes.delete(&s.actor, &code).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn deactivate_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
) -> ApiResult<Json<PermissionClassDto>> {
    Ok(Json(s.classes.deactivate(&s.actor, &code).await?))
}

async fn activate_class(
    Ctx(s): Ctx,
    Path(code): Path<String>,
) -> ApiResult<Json<PermissionClassDto>> {
    Ok(Json(s.classes.activate(&s.actor, &code).await?))
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
            | "EMPLOYEE_NOT_FOUND" => StatusCode::NOT_FOUND,
            "NOT_SIGNED_IN"
            | "SIGN_IN_FAILED"
            | "USER_LOCKED"
            | "USER_NOT_REGISTERED"
            | "USER_INACTIVE"
            | "USER_EXPIRED" => StatusCode::UNAUTHORIZED,
            "PASSWORD_CHANGE_REQUIRED" => StatusCode::FORBIDDEN,
            "PLANT_NOT_ALLOWED" => StatusCode::FORBIDDEN,
            "ITEM_ALREADY_EXISTS"
            | "ITEM_INVALID_TRANSITION"
            | "ITEM_HAS_TRANSACTIONS"
            | "PROGRAM_ALREADY_EXISTS"
            | "DICT_ALREADY_EXISTS"
            | "USER_ALREADY_EXISTS"
            | "IDP_ACCOUNT_IN_USE"
            | "PERMISSION_CLASS_ALREADY_EXISTS"
            | "PERMISSION_CLASS_IN_USE"
            | "PERMISSION_CLASS_HAS_PERMISSIONS"
            | "PLANT_ALREADY_EXISTS"
            | "PLANT_IN_USE"
            | "PLANT_SCHEMA_DUPLICATE"
            | "DEPARTMENT_ALREADY_EXISTS"
            | "DEPARTMENT_IN_USE"
            | "EMPLOYEE_ALREADY_EXISTS"
            | "PERMISSION_ALREADY_EXISTS"
            | "USER_PERMISSION_DUPLICATE"
            | "USER_HAS_SPECIAL_PERMISSIONS" => StatusCode::CONFLICT,
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
    let result = s.sign_in.sign_in(&input.user_id, &input.password).await;
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

#[derive(Deserialize)]
struct PlantQuery {
    plant: String,
}

async fn list_items(Ctx(s): Ctx, Query(q): Query<PlantQuery>) -> ApiResult<Json<Vec<ItemDto>>> {
    Ok(Json(s.items.list(&s.actor, &q.plant).await?))
}

async fn create_item(
    Ctx(s): Ctx,
    Json(req): Json<CreateItemRequest>,
) -> ApiResult<(StatusCode, Json<ItemDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.items.create(&s.actor, req).await?),
    ))
}

async fn get_item(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<ItemDto>> {
    Ok(Json(s.items.get(&s.actor, &plant, &no).await?))
}

async fn confirm_item(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<ItemDto>> {
    Ok(Json(s.items.confirm(&s.actor, &plant, &no).await?))
}

async fn unconfirm_item(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<ItemDto>> {
    Ok(Json(s.items.unconfirm(&s.actor, &plant, &no).await?))
}

async fn delete_item(
    Ctx(s): Ctx,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    s.items.delete(&s.actor, &plant, &no).await?;
    Ok(StatusCode::NO_CONTENT)
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
    Ok(Json(s.programs.update(&s.actor, &no, req).await?))
}

async fn delete_program(Ctx(s): Ctx, Path(no): Path<String>) -> ApiResult<StatusCode> {
    s.programs.delete(&s.actor, &no).await?;
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
    Ok(Json(s.dictionary.update(&s.actor, &table, req).await?))
}

async fn delete_dictionary(Ctx(s): Ctx, Path(table): Path<String>) -> ApiResult<StatusCode> {
    s.dictionary.delete(&s.actor, &table).await?;
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
    Ok(Json(
        s.departments.update(&s.actor, &plant, &code, req).await?,
    ))
}

async fn set_department_accounting(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
    Json(req): Json<DepartmentAccountingInput>,
) -> ApiResult<Json<DepartmentDto>> {
    Ok(Json(
        s.departments
            .set_accounting(&s.actor, &plant, &code, req)
            .await?,
    ))
}

async fn deactivate_department(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
) -> ApiResult<Json<DepartmentDto>> {
    Ok(Json(
        s.departments.deactivate(&s.actor, &plant, &code).await?,
    ))
}

async fn activate_department(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
) -> ApiResult<Json<DepartmentDto>> {
    Ok(Json(s.departments.activate(&s.actor, &plant, &code).await?))
}

async fn delete_department(
    Ctx(s): Ctx,
    Path((plant, code)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    s.departments.delete(&s.actor, &plant, &code).await?;
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
    Ok(Json(s.employees.update(&s.actor, &plant, &id, req).await?))
}

async fn deactivate_employee(
    Ctx(s): Ctx,
    Path((plant, id)): Path<(String, String)>,
) -> ApiResult<Json<EmployeeDto>> {
    Ok(Json(s.employees.deactivate(&s.actor, &plant, &id).await?))
}

async fn activate_employee(
    Ctx(s): Ctx,
    Path((plant, id)): Path<(String, String)>,
) -> ApiResult<Json<EmployeeDto>> {
    Ok(Json(s.employees.activate(&s.actor, &plant, &id).await?))
}

async fn delete_employee(
    Ctx(s): Ctx,
    Path((plant, id)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    s.employees.delete(&s.actor, &plant, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
