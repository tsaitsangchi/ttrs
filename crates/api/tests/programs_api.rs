//! p_zz 程式登錄 API 契約測試：HTTP 狀態碼與錯誤格式。業務規則由 BDD 涵蓋。

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{ItemService, ProgramService, memory::InMemoryStore};
use ttrs_shared::{ApiError, ProgramDto};

fn app() -> Router {
    let store = Arc::new(InMemoryStore::seeded());
    router(AppState {
        items: ItemService::new(store.clone(), store.clone()),
        programs: ProgramService::new(store),
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
async fn rename_then_old_no_returns_404() {
    let app = app();
    app.clone().oneshot(create("aimi100")).await.unwrap();
    let res = app
        .clone()
        .oneshot(json(
            "POST",
            "/api/programs/aimi100/rename",
            serde_json::json!({ "new_no": "aimi101" }),
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
