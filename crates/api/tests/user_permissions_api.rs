//! User Special Permission Maintenance API 契約測試：HTTP 狀態碼與錯誤格式。
//! 業務規則由 BDD 涵蓋（user_permission_maintenance.feature）。

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{Services, memory::InMemoryStore};
use ttrs_shared::{ApiError, PermissionBatchResultDto, UserPermissionDto};

async fn app() -> Router {
    let store = InMemoryStore::seeded();
    store.add_module("MENU");
    store.add_permission_class("SALES");
    let store = Arc::new(store);
    let app = router(AppState::new(Services::new(store), actor_from_env()));
    for body in [
        serde_json::json!({ "program_no": "menu_main", "module_code": "MENU" }),
        serde_json::json!({ "program_no": "aimi109", "module_code": "AIM", "default_actions": "insert, query" }),
    ] {
        let res = app
            .clone()
            .oneshot(request("POST", "/api/programs", Some(body)))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }
    for id in ["u001", "u002"] {
        let user = serde_json::json!({
            "user_id": id, "name": "王小明", "permission_class": "ADMIN",
            "menu": "menu_main", "default_plant": "TW01"
        });
        let res = app
            .clone()
            .oneshot(request("POST", "/api/users", Some(user)))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }
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
async fn add_with_own_class_update_and_remove() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/users/u001/permissions",
            Some(serde_json::json!({ "kind": "class", "target": "SALES", "include_own_class": true })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let rows: Vec<UserPermissionDto> = body(res).await;
    let targets: Vec<&str> = rows.iter().map(|r| r.target.as_str()).collect();
    assert_eq!(targets, ["ADMIN", "SALES"]);

    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/users/u001/permissions",
            Some(serde_json::json!({ "kind": "class", "target": "SALES" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(
        body::<ApiError>(res).await.code,
        "USER_PERMISSION_DUPLICATE"
    );

    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/users/u001/permissions",
            Some(serde_json::json!({ "kind": "program", "target": "aimi109" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let res = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/users/u001/permissions/programs/aimi109",
            Some(serde_json::json!({ "actions": ["query"], "detail_rows": "3" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let row: UserPermissionDto = body(res).await;
    assert_eq!(
        (row.actions, row.detail_rows),
        (vec!["query".to_owned()], Some("3".to_owned()))
    );

    let res = app
        .clone()
        .oneshot(request("DELETE", "/api/users/u001/permissions/3", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = app
        .oneshot(request("DELETE", "/api/users/u001/permissions/3", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn copy_and_target_with_rows_409() {
    let app = app().await;
    for (user, target) in [("u001", "SALES"), ("u002", "ADMIN")] {
        let res = app
            .clone()
            .oneshot(request(
                "POST",
                &format!("/api/users/{user}/permissions"),
                Some(serde_json::json!({ "kind": "class", "target": target })),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
    }
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/users/u001/copy-permissions",
            Some(serde_json::json!({ "target_user": "u002" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(
        body::<ApiError>(res).await.code,
        "USER_HAS_SPECIAL_PERMISSIONS"
    );

    let res = app
        .clone()
        .oneshot(request("DELETE", "/api/users/u002/permissions", None))
        .await
        .unwrap();
    assert_eq!(body::<PermissionBatchResultDto>(res).await.removed, 1);
    let res = app
        .oneshot(request(
            "POST",
            "/api/users/u001/copy-permissions",
            Some(serde_json::json!({ "target_user": "u002" })),
        ))
        .await
        .unwrap();
    assert_eq!(body::<PermissionBatchResultDto>(res).await.added, 1);
}
