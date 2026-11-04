//! 各維護作業 QBE 查詢的 API 契約測試：HTTP 狀態碼與回傳格式。
//! 業務規則由 BDD 涵蓋（*_maintenance_query.feature）。

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{Services, memory::InMemoryStore};
use ttrs_shared::{ApiError, Page, PermissionClassDto, PlantDto, ProgramPermissionDto, UserDto};

async fn app() -> Router {
    let store = InMemoryStore::seeded();
    store.add_module("MENU");
    let store = Arc::new(store);
    let app = router(AppState::new(Services::new(store), actor_from_env()));
    // 新增程式時自動授權給目前使用者的權限類別 ADMIN（ADMIN 另有起始資料的已移植作業權限）
    for (uri, body) in [
        (
            "/api/programs",
            serde_json::json!({ "program_no": "menu-main", "module_code": "MENU" }),
        ),
        (
            "/api/programs",
            serde_json::json!({ "program_no": "item-price", "module_code": "AIM", "program_type": "I", "default_actions": "query" }),
        ),
        // 使用者的部門須存在於預設營運中心（user_maintenance_department.feature）
        (
            "/api/plants/TW01/departments",
            serde_json::json!({ "dept_code": "SAL", "name": "業務部" }),
        ),
        (
            "/api/users",
            serde_json::json!({
                "user_id": "u001", "name": "王小明", "dept_code": "sal", "permission_class": "ADMIN",
                "menu": "menu-main", "default_plant": "TW01"
            }),
        ),
    ] {
        let res = app.clone().oneshot(post(uri, body)).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED, "{uri}");
    }
    app
}

fn post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn condition(field: &str, expr: &str) -> serde_json::Value {
    serde_json::json!({ "conditions": [{ "field": field, "expr": expr }] })
}

async fn body<T: serde::de::DeserializeOwned>(res: axum::response::Response) -> T {
    serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn users_query_and_field_not_allowed_422() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(post("/api/users/query", condition("dept_code", "SAL")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let page: Page<UserDto> = body(res).await;
    assert_eq!((page.total, page.items[0].user_id.as_str()), (1, "u001"));

    let res = app
        .oneshot(post(
            "/api/users/query",
            condition("allowed_plants", "TW01"),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body::<ApiError>(res).await.code, "QBE_FIELD_NOT_ALLOWED");
}

#[tokio::test]
async fn plants_and_classes_query() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(post("/api/plants/query", condition("plant_id", "tw*")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let page: Page<PlantDto> = body(res).await;
    assert_eq!(page.items[0].plant_id, "TW01");

    let res = app
        .oneshot(post(
            "/api/permission-classes/query",
            condition("idle_control", "1"),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let page: Page<PermissionClassDto> = body(res).await;
    assert_eq!(page.items[0].class_code, "ADMIN");
}

#[tokio::test]
async fn program_permissions_query_and_class_detail() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(post(
            "/api/program-permissions/query",
            condition("actions", "query"),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let page: Page<PermissionClassDto> = body(res).await;
    assert_eq!(page.total, 1);

    let res = app
        .oneshot(post(
            "/api/permission-classes/ADMIN/permission-query",
            condition("program_no", "item-price"),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rows: Vec<ProgramPermissionDto> = body(res).await;
    assert_eq!(rows.len(), 1);
}
