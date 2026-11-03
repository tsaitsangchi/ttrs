//! Permission Class Maintenance API 契約測試：HTTP 狀態碼與錯誤格式。
//! 業務規則由 BDD 涵蓋（permission_class_maintenance.feature）。

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{Services, memory::InMemoryStore};
use ttrs_shared::{ApiError, ClassIdleControlDto, PermissionClassDto, UpdatedCountDto};

async fn app() -> Router {
    let store = InMemoryStore::seeded();
    store.add_module("MENU");
    let store = Arc::new(store);
    let app = router(AppState::new(Services::new(store), actor_from_env()));
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/programs",
            Some(serde_json::json!({ "program_no": "menu-sales", "module_code": "MENU" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    app
}

fn request(method: &str, uri: &str, body: Option<serde_json::Value>) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
        .unwrap()
}

async fn body<T: serde::de::DeserializeOwned>(res: axum::response::Response) -> T {
    serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn create_duplicate_deactivate_and_delete() {
    let app = app().await;
    let sales = serde_json::json!({
        "class_code": "SALES", "name": "業務人員", "default_menu": "menu-sales"
    });
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/permission-classes",
            Some(sales.clone()),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let dto: PermissionClassDto = body(res).await;
    assert_eq!(dto.idle_control, ClassIdleControlDto::Off);
    assert!(dto.active);

    let res = app
        .clone()
        .oneshot(request("POST", "/api/permission-classes", Some(sales)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(
        body::<ApiError>(res).await.code,
        "PERMISSION_CLASS_ALREADY_EXISTS"
    );

    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/permission-classes/SALES/deactivate",
            None,
        ))
        .await
        .unwrap();
    assert!(!body::<PermissionClassDto>(res).await.active);

    let res = app
        .clone()
        .oneshot(request("DELETE", "/api/permission-classes/SALES", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn class_in_use_returns_409_and_menus_update() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/permission-classes/ADMIN",
            Some(serde_json::json!({ "name": "系統管理員", "default_menu": "menu-sales" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let user = serde_json::json!({
        "user_id": "u001", "name": "王小明", "permission_class": "ADMIN", "default_plant": "TW01"
    });
    let res = app
        .clone()
        .oneshot(request("POST", "/api/users", Some(user)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let res = app
        .clone()
        .oneshot(request("DELETE", "/api/permission-classes/ADMIN", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(body::<ApiError>(res).await.code, "PERMISSION_CLASS_IN_USE");

    // 使用者選單已帶入預設選單，沒有需要更新的使用者
    let res = app
        .oneshot(request(
            "POST",
            "/api/permission-classes/update-user-menus",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body::<UpdatedCountDto>(res).await.updated, 0);
}
