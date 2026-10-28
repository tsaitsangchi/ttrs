//! 路由與錯誤對應。handler 只做轉譯，業務邏輯一律在 ttrs-app。

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use ttrs_app::{AppError, ItemService};
use ttrs_shared::{ApiError, CreateItemRequest, ItemDto};

#[derive(Clone)]
pub struct AppState {
    pub items: ItemService,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/api/items", get(list_items).post(create_item))
        .route("/api/items/{plant}/{no}", get(get_item).delete(delete_item))
        .route("/api/items/{plant}/{no}/confirm", post(confirm_item))
        .layer(CorsLayer::permissive()) // TODO: 上線前改為白名單
        .layer(TraceLayer::new_for_http())
        .with_state(state)
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
            "ITEM_NOT_FOUND" => StatusCode::NOT_FOUND,
            "ITEM_ALREADY_EXISTS" | "ITEM_INVALID_TRANSITION" | "ITEM_HAS_TRANSACTIONS" => {
                StatusCode::CONFLICT
            }
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

async fn delete_item(
    State(s): State<AppState>,
    Path((plant, no)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    s.items.delete(&plant, &no).await?;
    Ok(StatusCode::NO_CONTENT)
}
