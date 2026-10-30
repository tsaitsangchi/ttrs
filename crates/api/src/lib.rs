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
    Actor, AppError, DataDictionaryService, ItemService, LegacyCatalogService, ProgramService,
    program_service::DEFAULT_LANG,
};
use ttrs_shared::{
    ApiError, ColumnInfoDto, ColumnTextDto, ConstraintDto, CreateItemRequest, GrantRequest,
    IndexDto, ItemDto, LegacyImportResultDto, LegacyTableDto, LegacyTableSummaryDto, MigrationDto,
    NewProgramNoRequest, Page, ProgramDto, ProgramInput, ProgramNameDto, QueryRequest,
    TableDictionaryDto, TableDictionaryInput, TableTextDto,
};

#[derive(Clone)]
pub struct AppState {
    pub items: ItemService,
    pub programs: ProgramService,
    pub dictionary: DataDictionaryService,
    pub legacy: LegacyCatalogService,
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
            | "LEGACY_TABLE_NOT_FOUND" => StatusCode::NOT_FOUND,
            "ITEM_ALREADY_EXISTS"
            | "ITEM_INVALID_TRANSITION"
            | "ITEM_HAS_TRANSACTIONS"
            | "PROGRAM_ALREADY_EXISTS"
            | "DICT_ALREADY_EXISTS" => StatusCode::CONFLICT,
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
    s.programs.grant(&no, &req.permission_class).await?;
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
