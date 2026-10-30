//! API 契約測試：確認 HTTP 狀態碼與錯誤格式。業務規則本身由 BDD 涵蓋。

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{
    DataDictionaryService, ItemService, LegacyCatalogService, ProgramService, memory::InMemoryStore,
};
use ttrs_shared::ApiError;

fn app() -> axum::Router {
    let store = Arc::new(InMemoryStore::seeded());
    router(AppState {
        items: ItemService::new(store.clone(), store.clone()),
        programs: ProgramService::new(store.clone()),
        dictionary: DataDictionaryService::new(store.clone(), store.clone(), store.clone()),
        legacy: LegacyCatalogService::new(store),
        actor: actor_from_env(),
    })
}

fn create(no: &str) -> Request<Body> {
    let body = serde_json::json!({
        "plant_id": "TW01", "item_no": no, "name": "螺絲", "spec": null, "stock_unit": "PCS"
    });
    Request::post("/api/items")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn create_then_duplicate_returns_409() {
    let app = app();
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
            Request::get("/api/items/TW01/NOPE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
