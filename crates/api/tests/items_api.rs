//! API 契約測試：確認 HTTP 狀態碼與錯誤格式。業務規則本身由 BDD 涵蓋。

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{Services, memory::InMemoryStore};
use ttrs_shared::ApiError;

fn app() -> axum::Router {
    let store = Arc::new(InMemoryStore::seeded());
    router(AppState::new(Services::new(store), actor_from_env()))
}

fn post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn create(no: &str) -> Request<Body> {
    post(
        "/api/plants/TW01/items",
        serde_json::json!({
            "item_no": no, "name": "螺絲", "stock_unit": "PCS", "product_class": "P01"
        }),
    )
}

/// 料件必填產品分類（IM26）：先建產品分類
async fn with_product_class(app: &axum::Router) {
    let res = app
        .clone()
        .oneshot(post(
            "/api/plants/TW01/product-classes",
            serde_json::json!({ "class_code": "P01", "name": "產品" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn create_then_duplicate_returns_409() {
    let app = app();
    with_product_class(&app).await;
    let res = app.clone().oneshot(create("A001")).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let res = app.oneshot(create("A001")).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let err: ApiError =
        serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(err.code, "ITEM_ALREADY_EXISTS");
}

#[tokio::test]
async fn missing_item_returns_404() {
    let res = app()
        .oneshot(
            Request::get("/api/plants/TW01/items/NOPE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn state_change_requires_version() {
    let app = app();
    with_product_class(&app).await;
    let res = app.clone().oneshot(create("A001")).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/plants/TW01/items/A001/confirm")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let res = app
        .oneshot(
            Request::post("/api/plants/TW01/items/A001/confirm?version=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
