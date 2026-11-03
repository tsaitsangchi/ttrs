//! 登入連線的 API 契約測試：HTTP 狀態碼與回傳格式。業務規則由 BDD 涵蓋（sign_in.feature）。

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{Services, memory::InMemoryStore};
use ttrs_shared::ApiError;

async fn code(res: axum::response::Response) -> String {
    let e: ApiError =
        serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap();
    e.code
}

/// 開發用身分未在使用者維護註冊：401 USER_NOT_REGISTERED
#[tokio::test]
async fn unregistered_user_gets_401() {
    let app = router(AppState::new(
        Services::new(Arc::new(InMemoryStore::seeded())),
        actor_from_env(),
    ));
    let res = app
        .clone()
        .oneshot(Request::get("/api/session").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(code(res).await, "USER_NOT_REGISTERED");

    let res = app
        .oneshot(
            Request::put("/api/session/plant")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"plant_id":"TW01"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
