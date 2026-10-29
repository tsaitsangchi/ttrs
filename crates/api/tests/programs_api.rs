//! Program Maintenance API 契約測試：HTTP 狀態碼與錯誤格式。業務規則由 BDD 涵蓋。

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{DataDictionaryService, ItemService, ProgramService, memory::InMemoryStore};
use ttrs_shared::{ApiError, ProgramDto};

fn app() -> Router {
    let store = Arc::new(InMemoryStore::seeded());
    router(AppState {
        items: ItemService::new(store.clone(), store.clone()),
        programs: ProgramService::new(store.clone()),
        dictionary: DataDictionaryService::new(store.clone(), store),
        actor: actor_from_env(),
    })
}

fn json(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn create(no: &str) -> Request<Body> {
    json(
        "POST",
        "/api/programs",
        serde_json::json!({ "program_no": no, "module_code": "AIM", "name": "料件基本資料維護作業" }),
    )
}

async fn body<T: serde::de::DeserializeOwned>(res: axum::response::Response) -> T {
    serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn create_returns_201_with_defaults_then_duplicate_409() {
    let app = app();
    let res = app.clone().oneshot(create("aimi100")).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let dto: ProgramDto = body(res).await;
    assert_eq!(dto.program_type.as_deref(), Some("I"));
    assert_eq!(dto.name.as_deref(), Some("料件基本資料維護作業"));

    let res = app.oneshot(create("aimi100")).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(body::<ApiError>(res).await.code, "PROGRAM_ALREADY_EXISTS");
}

#[tokio::test]
async fn rule_violation_returns_422() {
    let res = app().oneshot(create("aimr400")).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body::<ApiError>(res).await.code, "REPORT_TYPE_REQUIRED");
}

#[tokio::test]
/// 改代號：PUT 原代號，內容帶新的 program_no
async fn change_no_via_update_then_old_no_returns_404() {
    let app = app();
    app.clone().oneshot(create("aimi100")).await.unwrap();
    let res = app
        .clone()
        .oneshot(json(
            "PUT",
            "/api/programs/aimi100",
            serde_json::json!({ "program_no": "aimi101", "module_code": "AIM" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/programs/aimi100")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .oneshot(
            Request::get("/api/programs/aimi101/permissions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body::<Vec<String>>(res).await, vec!["ADMIN".to_string()]);
}

#[tokio::test]
async fn delete_returns_204() {
    let app = app();
    app.clone().oneshot(create("aimi100")).await.unwrap();
    let res = app
        .oneshot(
            Request::delete("/api/programs/aimi100")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn query_returns_page_and_bad_condition_422() {
    let app = app();
    app.clone().oneshot(create("aimi100")).await.unwrap();
    app.clone().oneshot(create("aimi101")).await.unwrap();

    let res = app
        .clone()
        .oneshot(json(
            "POST",
            "/api/programs/query",
            serde_json::json!({ "conditions": [{ "field": "program_no", "expr": "aimi10?" }], "page_size": 1 }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let page: ttrs_shared::Page<ProgramDto> = body(res).await;
    assert_eq!((page.total, page.items.len()), (2, 1));

    let res = app
        .oneshot(json(
            "POST",
            "/api/programs/query",
            serde_json::json!({ "conditions": [{ "field": "report_width", "expr": "abc" }] }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body::<ApiError>(res).await.code, "QBE_INVALID_NUMBER");
}
