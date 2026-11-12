//! 會計期間 API 契約測試（aoos020）：路由、HTTP 狀態。業務規則由 BDD 涵蓋（specs/features/aoo/accounting_periods.feature）。

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{Services, memory::InMemoryStore};

fn app() -> axum::Router {
    router(AppState::new(
        Services::new(Arc::new(InMemoryStore::seeded())),
        actor_from_env(),
    ))
}

async fn send(app: &axum::Router, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn create_locate_and_lifecycle() {
    let app = app();
    let (status, periods) = send(
        &app,
        "GET",
        "/api/accounting-years/defaults?year=2026&start=2026-01-01",
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(periods.as_array().unwrap().len(), 12);

    let body = json!({ "year": 2026, "periods": periods, "version": null });
    let (status, y) = send(&app, "POST", "/api/accounting-years", body.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{y}");
    let (status, e) = send(&app, "POST", "/api/accounting-years", body).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(e["code"], "ACCOUNTING_YEAR_ALREADY_EXISTS");

    let (status, d) = send(
        &app,
        "GET",
        "/api/accounting-days?from=2026-03-15",
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(d[0]["period"], 3);
    assert_eq!(d[0]["week"], 12);
    let (_, d) = send(
        &app,
        "GET",
        "/api/accounting-days?from=2026-01-30&to=2026-02-02",
        json!(null),
    )
    .await;
    assert_eq!(d.as_array().unwrap().len(), 4);
    let (status, e) = send(
        &app,
        "GET",
        "/api/accounting-days?from=2030-01-01",
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(e["code"], "ACCOUNTING_PERIOD_NOT_FOUND");

    let (status, p) = send(
        &app,
        "GET",
        "/api/accounting-years/2026/periods/2",
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["end_date"], "2026-02-28");

    let (status, _) = send(
        &app,
        "POST",
        "/api/accounting-years/2026/copy",
        json!({ "to": 2027 }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let v = y["version"].as_i64().unwrap();
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/accounting-years/2026/confirm?version={v}"),
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, e) = send(
        &app,
        "DELETE",
        &format!("/api/accounting-years/2026?version={}", v + 1),
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(e["code"], "RECORD_CONFIRMED_NOT_DELETABLE");

    let (status, _) = send(
        &app,
        "GET",
        "/api/accounting-years/report.pdf?lang=zh-TW",
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
