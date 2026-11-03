//! 權限檢查的 API 契約測試：拒絕時的 HTTP 狀態碼與回傳格式、目前使用者的有效權限。
//! 業務規則由 BDD 涵蓋（authorization.feature）。

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{Actor, ActorKind, Services, memory::InMemoryStore};
use ttrs_shared::{ApiError, MyPermissionDto};

fn app(actor: Actor) -> Router {
    let store = InMemoryStore::seeded();
    store.add_permission_class("SALES");
    router(AppState::new(Services::new(Arc::new(store)), actor))
}

/// 權限類別 SALES 沒有任何程式權限
fn sales_user() -> Actor {
    Actor {
        user_id: "u001".into(),
        dept: "SAL".into(),
        permission_class: "SALES".into(),
        kind: ActorKind::Person,
    }
}

async fn body<T: serde::de::DeserializeOwned>(res: axum::response::Response) -> T {
    serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn denied_returns_403_with_program_and_action() {
    let res = app(sales_user())
        .oneshot(
            Request::post("/api/users/query")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"conditions":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let err: ApiError = body(res).await;
    assert_eq!(err.code, "PERMISSION_DENIED");
    assert!(err.message.contains("user-maintenance"), "{}", err.message);
}

#[tokio::test]
async fn reference_lists_only_need_login() {
    let res = app(sales_user())
        .oneshot(get("/api/permission-classes"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn my_permission_for_admin_and_unknown_program() {
    // 開發用身分的權限類別 ADMIN 有起始資料的全部功能
    let app = app(actor_from_env());
    let res = app
        .clone()
        .oneshot(get("/api/me/permissions/user-maintenance"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let p: MyPermissionDto = body(res).await;
    assert!(p.allowed);
    assert!(p.allows("invalid"));
    assert_eq!(
        (
            p.other_user_data.as_str(),
            p.other_dept_data.as_str(),
            p.detail_rows.as_str()
        ),
        ("0", "0", "0")
    );

    let res = app
        .oneshot(get("/api/me/permissions/zzz999"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let p: MyPermissionDto = body(res).await;
    assert!(!p.allowed);
    assert!(p.actions.is_empty());
}
