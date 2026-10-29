//! 路由與錯誤對應。handler 只做轉譯，業務邏輯一律在 ttrs-app。

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use ttrs_app::{Actor, AppError, ItemService, ProgramService};
use ttrs_shared::{
    ApiError, CreateItemRequest, GrantRequest, ItemDto, NewProgramNoRequest, ProgramDto,
    ProgramInput,
};

#[derive(Clone)]
pub struct AppState {
    pub items: ItemService,
    pub programs: ProgramService,
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
        .route(
            "/api/programs/{no}",
            get(get_program).put(update_program).delete(delete_program),
        )
        .route("/api/programs/{no}/rename", post(rename_program))
        .route("/api/programs/{no}/copy", post(copy_program))
        .route(
            "/api/programs/{no}/permissions",
            get(list_permissions).post(grant_permission),
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

pub struct HttpError(AppError);

impl From<AppError> for HttpError {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let status = match self.0.code() {
            "ITEM_NOT_FOUND" | "PROGRAM_NOT_FOUND" => StatusCode::NOT_FOUND,
            "ITEM_ALREADY_EXISTS"
            | "ITEM_INVALID_TRANSITION"
            | "ITEM_HAS_TRANSACTIONS"
            | "PROGRAM_ALREADY_EXISTS" => StatusCode::CONFLICT,
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

// ----- p_zz 程式登錄 -----

async fn list_programs(State(s): State<AppState>) -> ApiResult<Json<Vec<ProgramDto>>> {
    Ok(Json(s.programs.list().await?))
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

async fn get_program(
    State(s): State<AppState>,
    Path(no): Path<String>,
) -> ApiResult<Json<ProgramDto>> {
    Ok(Json(s.programs.get(&no).await?))
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

async fn rename_program(
    State(s): State<AppState>,
    Path(no): Path<String>,
    Json(req): Json<NewProgramNoRequest>,
) -> ApiResult<Json<ProgramDto>> {
    s.programs.change_no(&s.actor, &no, &req.new_no).await?;
    Ok(Json(s.programs.get(&req.new_no).await?))
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
