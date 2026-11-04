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

/// 固定身分（TTRS_AUTH=fixed）且該身分未在使用者維護註冊：401 USER_NOT_REGISTERED
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

fn json(method: &str, uri: &str, body: &str, cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(c) = cookie {
        b = b.header("cookie", c);
    }
    b.body(Body::from(body.to_owned())).unwrap()
}

/// 登入模式：未登入 401；密碼錯誤 401 SIGN_IN_FAILED；初始密碼須先改密碼 403；
/// 改密碼後登入取得 cookie，帶 cookie 才能呼叫其他 API；登出後失效
#[tokio::test]
async fn password_sign_in_flow() {
    // 以固定身分（ADMIN）建立選單與使用者（初始密碼＝使用者代號）
    let services = Services::new(Arc::new(InMemoryStore::seeded()));
    let admin = router(AppState::new(services.clone(), actor_from_env()));
    let res = admin
        .clone()
        .oneshot(json(
            "POST",
            "/api/programs",
            r#"{"program_no":"menu-main","module_code":"MENU"}"#,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let res = admin
        .oneshot(json(
            "POST",
            "/api/users",
            r#"{"user_id":"u001","permission_class":"ADMIN","menu":"menu-main","default_plant":"TW01"}"#,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let app = router(AppState::with_sign_in(services, false));

    let res = app
        .clone()
        .oneshot(Request::get("/api/programs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(code(res).await, "NOT_SIGNED_IN");

    let res = app
        .clone()
        .oneshot(json(
            "POST",
            "/api/session",
            r#"{"user_id":"u001","password":"wrong"}"#,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(code(res).await, "SIGN_IN_FAILED");

    let res = app
        .clone()
        .oneshot(json(
            "POST",
            "/api/session",
            r#"{"user_id":"u001","password":"u001"}"#,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(code(res).await, "PASSWORD_CHANGE_REQUIRED");

    let res = app
        .clone()
        .oneshot(json(
            "POST",
            "/api/password",
            r#"{"user_id":"u001","old_password":"u001","new_password":"Secret123","confirm_password":"Secret123"}"#,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = app
        .clone()
        .oneshot(json(
            "POST",
            "/api/session",
            r#"{"user_id":"u001","password":"Secret123"}"#,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cookie = res.headers()["set-cookie"].to_str().unwrap().to_owned();
    assert!(cookie.contains("HttpOnly"));
    let cookie = cookie.split(';').next().unwrap().to_owned();

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/session")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::delete("/api/session")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = app
        .oneshot(
            Request::get("/api/session")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
