//! 路由與錯誤對應。handler 只做轉譯，業務邏輯一律在 ttrs-app。

use axum::{
    Json, Router,
    extract::{Path, Query, State},
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
    Actor, AppError, DataDictionaryService, ItemService, LegacyCatalogService,
    PermissionClassService, PlantService, ProgramPermissionService, ProgramService,
    UserPermissionService, UserService, program_service::DEFAULT_LANG,
};
use ttrs_shared::{
    ApiError, ColumnInfoDto, ColumnTextDto, ConstraintDto, CopyPermissionsRequest,
    CopyUserPermissionsRequest, CreateItemRequest, GeneratePermissionsRequest, GrantRequest,
    IndexDto, ItemDto, LegacyImportResultDto, LegacyTableDto, LegacyTableSummaryDto,
    MenuPermissionsRequest, MigrationDto, NewPlantIdRequest, NewProgramNoRequest, NewUserIdRequest,
    Page, PermissionBatchResultDto, PermissionClassDto, PermissionClassInput, PlantDto, PlantInput,
    ProgramDto, ProgramInput, ProgramNameDto, ProgramPermissionDto, ProgramPermissionInput,
    QueryRequest, TableDictionaryDto, TableDictionaryInput, TableTextDto, UpdatedCountDto, UserDto,
    UserInput, UserPermissionDto, UserPermissionInput,
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
    /// 尚未有登入機制前，所有請求都以此使用者身分執行（見 `actor_from_env`）
    pub actor: Actor,
}

/// 開發期的操作者身分：`TTRS_USER`、`TTRS_DEPT`、`TTRS_PERMISSION_CLASS`。
/// 導入登入後改由認證資訊取得。
pub fn actor_from_env() -> Actor {
    let var = |k: &str, default: &str| std::env::var(k).unwrap_or_else(|_| default.to_owned());
    Actor {
        user_id: var("TTRS_USER", "tiptop"),
        dept: var("TTRS_DEPT", "MIS"),
        permission_class: var("TTRS_PERMISSION_CLASS", "ADMIN"),
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
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
        .route(
            "/api/users/{id}",
            get(get_user).put(update_user).delete(delete_user),
        )
        .route("/api/users/{id}/copy", post(copy_user))
        .route("/api/users/{id}/deactivate", post(deactivate_user))
        .route("/api/users/{id}/activate", post(activate_user))
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
        // Plant Maintenance 營運中心維護（TipTop aooi901）
        .route("/api/plants", get(list_plants).post(create_plant))
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
    State(s): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<Vec<LegacyTableSummaryDto>>> {
    Ok(Json(s.legacy.search(&q.q).await?))
}

async fn get_legacy(
    State(s): State<AppState>,
    Path(legacy): Path<String>,
) -> ApiResult<Json<LegacyTableDto>> {
    Ok(Json(s.legacy.get(&legacy).await?))
}

/// 整批取代（C2）；名稱不合法或重複時 422 且不寫入（C6）
async fn import_legacy(
    State(s): State<AppState>,
    Json(req): Json<Vec<LegacyTableDto>>,
) -> ApiResult<Json<LegacyImportResultDto>> {
    Ok(Json(s.legacy.import(req).await?))
}

/// 已移植資料表對應的 TipTop 舊表；沒有對照時回傳 null（C1）
async fn table_legacy(
    State(s): State<AppState>,
    Path(table): Path<String>,
) -> ApiResult<Json<Option<LegacyTableDto>>> {
    Ok(Json(s.legacy.for_table(&table).await?))
}

// ----- User Maintenance 使用者維護（TipTop p_zx） -----

async fn list_users(State(s): State<AppState>) -> ApiResult<Json<Vec<UserDto>>> {
    Ok(Json(s.users.list().await?))
}

async fn create_user(
    State(s): State<AppState>,
    Json(req): Json<UserInput>,
) -> ApiResult<(StatusCode, Json<UserDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.users.create(&s.actor, req).await?),
    ))
}

async fn get_user(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<UserDto>> {
    Ok(Json(s.users.get(&id).await?))
}

async fn update_user(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UserInput>,
) -> ApiResult<Json<UserDto>> {
    Ok(Json(s.users.update(&s.actor, &id, req).await?))
}

async fn delete_user(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    s.users.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_user(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<NewUserIdRequest>,
) -> ApiResult<(StatusCode, Json<UserDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.users.copy(&s.actor, &id, &req.new_user_id).await?),
    ))
}

async fn deactivate_user(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<UserDto>> {
    Ok(Json(s.users.deactivate(&s.actor, &id).await?))
}

async fn activate_user(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<UserDto>> {
    Ok(Json(s.users.activate(&s.actor, &id).await?))
}

// ----- Plant Maintenance 營運中心維護（TipTop aooi901） -----

async fn list_plants(State(s): State<AppState>) -> ApiResult<Json<Vec<PlantDto>>> {
    Ok(Json(s.plants.list().await?))
}

async fn create_plant(
    State(s): State<AppState>,
    Json(req): Json<PlantInput>,
) -> ApiResult<(StatusCode, Json<PlantDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.plants.create(&s.actor, req).await?),
    ))
}

async fn get_plant(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<PlantDto>> {
    Ok(Json(s.plants.get(&id).await?))
}

async fn update_plant(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PlantInput>,
) -> ApiResult<Json<PlantDto>> {
    Ok(Json(s.plants.update(&s.actor, &id, req).await?))
}

async fn delete_plant(State(s): State<AppState>, Path(id): Path<String>) -> ApiResult<StatusCode> {
    s.plants.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_plant(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<NewPlantIdRequest>,
) -> ApiResult<(StatusCode, Json<PlantDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.plants.copy(&s.actor, &id, &req.new_plant_id).await?),
    ))
}

async fn deactivate_plant(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<PlantDto>> {
    Ok(Json(s.plants.deactivate(&s.actor, &id).await?))
}

async fn activate_plant(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<PlantDto>> {
    Ok(Json(s.plants.activate(&s.actor, &id).await?))
}

// ----- Permission Class Maintenance 權限類別維護（TipTop p_zw） -----

async fn list_classes(State(s): State<AppState>) -> ApiResult<Json<Vec<PermissionClassDto>>> {
    Ok(Json(s.classes.list().await?))
}

async fn create_class(
    State(s): State<AppState>,
    Json(req): Json<PermissionClassInput>,
) -> ApiResult<(StatusCode, Json<PermissionClassDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.classes.create(&s.actor, req).await?),
    ))
}

async fn get_class(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<Json<PermissionClassDto>> {
    Ok(Json(s.classes.get(&code).await?))
}

async fn update_class(
    State(s): State<AppState>,
    Path(code): Path<String>,
    Json(req): Json<PermissionClassInput>,
) -> ApiResult<Json<PermissionClassDto>> {
    Ok(Json(s.classes.update(&s.actor, &code, req).await?))
}

async fn delete_class(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<StatusCode> {
    s.classes.delete(&code).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn deactivate_class(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<Json<PermissionClassDto>> {
    Ok(Json(s.classes.deactivate(&s.actor, &code).await?))
}

async fn activate_class(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<Json<PermissionClassDto>> {
    Ok(Json(s.classes.activate(&s.actor, &code).await?))
}

async fn update_user_menus(State(s): State<AppState>) -> ApiResult<Json<UpdatedCountDto>> {
    let updated = s.classes.update_user_menus(&s.actor).await?;
    Ok(Json(UpdatedCountDto { updated }))
}

// ----- Program Permission Maintenance 程式執行權限維護（TipTop p_zy） -----

async fn list_class_permissions(
    State(s): State<AppState>,
    Path(code): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<ProgramPermissionDto>>> {
    let lang = q.lang.unwrap_or_else(|| DEFAULT_LANG.to_owned());
    Ok(Json(s.permissions.list(&code, &lang).await?))
}

async fn grant_class_permission(
    State(s): State<AppState>,
    Path(code): Path<String>,
    Json(req): Json<ProgramPermissionInput>,
) -> ApiResult<(StatusCode, Json<ProgramPermissionDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.permissions.grant(&s.actor, &code, req).await?),
    ))
}

async fn update_class_permission(
    State(s): State<AppState>,
    Path((code, program)): Path<(String, String)>,
    Json(req): Json<ProgramPermissionInput>,
) -> ApiResult<Json<ProgramPermissionDto>> {
    Ok(Json(
        s.permissions.update(&s.actor, &code, &program, req).await?,
    ))
}

async fn revoke_class_permission(
    State(s): State<AppState>,
    Path((code, program)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    s.permissions.revoke(&code, &program).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn revoke_all_permissions(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> ApiResult<Json<PermissionBatchResultDto>> {
    Ok(Json(s.permissions.revoke_all(&code).await?))
}

async fn copy_permissions(
    State(s): State<AppState>,
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
    State(s): State<AppState>,
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
    State(s): State<AppState>,
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
    State(s): State<AppState>,
    Path(code): Path<String>,
    Json(req): Json<MenuPermissionsRequest>,
) -> ApiResult<Json<PermissionBatchResultDto>> {
    Ok(Json(s.permissions.delete_by_menu(&code, &req.menu).await?))
}

// ----- User Special Permission Maintenance 使用者特殊權限維護（TipTop p_zxw） -----

async fn list_user_permissions(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<UserPermissionDto>>> {
    let lang = q.lang.unwrap_or_else(|| DEFAULT_LANG.to_owned());
    Ok(Json(s.special.list(&id, &lang).await?))
}

async fn add_user_permission(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UserPermissionInput>,
) -> ApiResult<(StatusCode, Json<Vec<UserPermissionDto>>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.special.add(&s.actor, &id, req).await?),
    ))
}

async fn update_user_program_permission(
    State(s): State<AppState>,
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
    State(s): State<AppState>,
    Path((id, line)): Path<(String, i32)>,
) -> ApiResult<StatusCode> {
    s.special.remove(&id, line).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_all_user_permissions(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<PermissionBatchResultDto>> {
    Ok(Json(s.special.remove_all(&id).await?))
}

async fn copy_user_permissions(
    State(s): State<AppState>,
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
            | "PERMISSION_NOT_FOUND" => StatusCode::NOT_FOUND,
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
            | "PERMISSION_ALREADY_EXISTS"
            | "USER_PERMISSION_DUPLICATE"
            | "USER_HAS_SPECIAL_PERMISSIONS" => StatusCode::CONFLICT,
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

#[derive(Deserialize)]
struct PlantQuery {
    plant: String,
}

async fn list_items(
    State(s): State<AppState>,
    Query(q): Query<PlantQuery>,
) -> ApiResult<Json<Vec<ItemDto>>> {
    Ok(Json(s.items.list(&q.plant).await?))
}

async fn create_item(
    State(s): State<AppState>,
    Json(req): Json<CreateItemRequest>,
) -> ApiResult<(StatusCode, Json<ItemDto>)> {
    Ok((StatusCode::CREATED, Json(s.items.create(req).await?)))
}

async fn get_item(
    State(s): State<AppState>,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<ItemDto>> {
    Ok(Json(s.items.get(&plant, &no).await?))
}

async fn confirm_item(
    State(s): State<AppState>,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<ItemDto>> {
    Ok(Json(s.items.confirm(&plant, &no).await?))
}

async fn unconfirm_item(
    State(s): State<AppState>,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<Json<ItemDto>> {
    Ok(Json(s.items.unconfirm(&plant, &no).await?))
}

async fn delete_item(
    State(s): State<AppState>,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    s.items.delete(&plant, &no).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- Program Maintenance 程式維護（TipTop p_zz） -----

async fn list_programs(State(s): State<AppState>) -> ApiResult<Json<Vec<ProgramDto>>> {
    Ok(Json(s.programs.list().await?))
}

/// QBE 查詢（比照 TipTop p_zz 的 Q.查詢）；條件格式見 ttrs_domain::qbe
async fn query_programs(
    State(s): State<AppState>,
    Json(req): Json<QueryRequest>,
) -> ApiResult<Json<Page<ProgramDto>>> {
    Ok(Json(s.programs.query(req).await?))
}

async fn create_program(
    State(s): State<AppState>,
    Json(req): Json<ProgramInput>,
) -> ApiResult<(StatusCode, Json<ProgramDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.programs.create(&s.actor, req).await?),
    ))
}

/// `?lang=en` 依語系顯示名稱（沒有該語系時退回繁中）
async fn get_program(
    State(s): State<AppState>,
    Path(no): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<ProgramDto>> {
    Ok(Json(match q.lang {
        Some(lang) => s.programs.get_localized(&no, &lang).await?,
        None => s.programs.get(&no).await?,
    }))
}

/// 維護某語系的程式名稱與報表抬頭（gaz_file）
async fn set_program_name(
    State(s): State<AppState>,
    Path(no): Path<String>,
    Json(req): Json<ProgramNameDto>,
) -> ApiResult<StatusCode> {
    s.programs.set_name(&s.actor, &no, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_program(
    State(s): State<AppState>,
    Path(no): Path<String>,
    Json(req): Json<ProgramInput>,
) -> ApiResult<Json<ProgramDto>> {
    Ok(Json(s.programs.update(&s.actor, &no, req).await?))
}

async fn delete_program(
    State(s): State<AppState>,
    Path(no): Path<String>,
) -> ApiResult<StatusCode> {
    s.programs.delete(&s.actor, &no).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_program(
    State(s): State<AppState>,
    Path(no): Path<String>,
    Json(req): Json<NewProgramNoRequest>,
) -> ApiResult<(StatusCode, Json<ProgramDto>)> {
    let dto = s.programs.copy(&s.actor, &no, &req.new_no).await?;
    Ok((StatusCode::CREATED, Json(dto)))
}

async fn list_permissions(
    State(s): State<AppState>,
    Path(no): Path<String>,
) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(s.programs.permission_classes(&no).await?))
}

async fn grant_permission(
    State(s): State<AppState>,
    Path(no): Path<String>,
    Json(req): Json<GrantRequest>,
) -> ApiResult<StatusCode> {
    s.programs
        .grant(&s.actor, &no, &req.permission_class)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ----- Data Dictionary Maintenance 資料字典維護（TipTop p_zta） -----

async fn schema_tables(State(s): State<AppState>) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(s.dictionary.schema_tables().await?))
}

/// `?lang=` 決定顯示語系（ADR 0005）；回應的 `texts` 含所有語系
async fn list_dictionaries(
    State(s): State<AppState>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<Vec<TableDictionaryDto>>> {
    let lang = q.lang.as_deref().unwrap_or(DEFAULT_LANG);
    Ok(Json(s.dictionary.list_localized(lang).await?))
}

async fn create_dictionary(
    State(s): State<AppState>,
    Json(req): Json<TableDictionaryInput>,
) -> ApiResult<(StatusCode, Json<TableDictionaryDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(s.dictionary.create(&s.actor, req).await?),
    ))
}

async fn get_dictionary(
    State(s): State<AppState>,
    Path(table): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Json<TableDictionaryDto>> {
    let lang = q.lang.as_deref().unwrap_or(DEFAULT_LANG);
    Ok(Json(s.dictionary.get_localized(&table, lang).await?))
}

async fn update_dictionary(
    State(s): State<AppState>,
    Path(table): Path<String>,
    Json(req): Json<TableDictionaryInput>,
) -> ApiResult<Json<TableDictionaryDto>> {
    Ok(Json(s.dictionary.update(&table, req).await?))
}

async fn delete_dictionary(
    State(s): State<AppState>,
    Path(table): Path<String>,
) -> ApiResult<StatusCode> {
    s.dictionary.delete(&table).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn table_texts(
    State(s): State<AppState>,
    Path(table): Path<String>,
) -> ApiResult<Json<Vec<TableTextDto>>> {
    Ok(Json(s.dictionary.table_texts(&table).await?))
}

async fn set_table_text(
    State(s): State<AppState>,
    Path(table): Path<String>,
    Json(req): Json<TableTextDto>,
) -> ApiResult<StatusCode> {
    s.dictionary.set_table_text(&table, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn table_columns(
    State(s): State<AppState>,
    Path(table): Path<String>,
) -> ApiResult<Json<Vec<ColumnInfoDto>>> {
    Ok(Json(s.dictionary.columns(&table).await?))
}

async fn column_texts(
    State(s): State<AppState>,
    Path(table): Path<String>,
) -> ApiResult<Json<Vec<ColumnTextDto>>> {
    Ok(Json(s.dictionary.column_texts(&table).await?))
}

async fn set_column_text(
    State(s): State<AppState>,
    Path(table): Path<String>,
    Json(req): Json<ColumnTextDto>,
) -> ApiResult<StatusCode> {
    s.dictionary.set_column_text(&table, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn table_indexes(
    State(s): State<AppState>,
    Path(table): Path<String>,
) -> ApiResult<Json<Vec<IndexDto>>> {
    Ok(Json(s.dictionary.indexes(&table).await?))
}

async fn table_constraints(
    State(s): State<AppState>,
    Path(table): Path<String>,
) -> ApiResult<Json<Vec<ConstraintDto>>> {
    Ok(Json(s.dictionary.constraints(&table).await?))
}

async fn table_history(
    State(s): State<AppState>,
    Path(table): Path<String>,
) -> ApiResult<Json<Vec<MigrationDto>>> {
    Ok(Json(s.dictionary.history(&table).await?))
}

#[derive(Deserialize)]
struct LangQuery {
    lang: Option<String>,
}

/// 匯出 CSV（D6）；開頭加 UTF-8 BOM，Excel 才會以 UTF-8 開啟中文
async fn export_dictionary(
    State(s): State<AppState>,
    Path(table): Path<String>,
    Query(q): Query<LangQuery>,
) -> ApiResult<Response> {
    let lang = q.lang.unwrap_or_else(|| DEFAULT_LANG.to_owned());
    let csv = s.dictionary.export_csv(&table, &lang).await?;
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
