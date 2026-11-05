//! 報表 API 契約測試：檔案類型與內嵌／下載。業務規則由 BDD 涵蓋（department_report.feature）。

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{Services, memory::InMemoryStore};

fn app() -> axum::Router {
    router(AppState::new(
        Services::new(Arc::new(InMemoryStore::seeded())),
        actor_from_env(),
    ))
}

async fn get(app: &axum::Router, uri: &str) -> axum::response::Response {
    app.clone()
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn department_report_formats() {
    let app = app();
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/plants/TW01/departments")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"dept_code":"MIS","name":"資訊部"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let res = get(&app, "/api/plants/TW01/departments/report.pdf?lang=zh-TW").await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/pdf");
    assert!(
        res.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("inline")
    );
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    assert!(body.starts_with(b"%PDF"));

    let q = "%5B%7B%22field%22%3A%22dept_code%22%2C%22expr%22%3A%22M*%22%7D%5D";
    let res = get(
        &app,
        &format!("/api/plants/TW01/departments/report.xlsx?q={q}"),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment")
    );

    let res = get(&app, "/api/plants/TW01/departments/report.doc").await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
