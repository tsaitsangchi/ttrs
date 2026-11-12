//! 整體管理系統參數 API 契約測試（aoos010）：路由、版本與 HTTP 狀態。
//! 業務規則由 BDD 涵蓋（specs/features/aoo/general_parameters.feature）。

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
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
async fn view_and_modify_with_version() {
    let app = app();
    let (status, _) = send(
        &app,
        "POST",
        "/api/currencies",
        json!({ "currency_code": "TWD" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, p) = send(&app, "GET", "/api/general-parameters", json!(null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["accounting_periods"], 12);
    assert_eq!(p["base_currency"], "TWD");
    assert_eq!(p["rate_basis"], "daily");
    let version = p["version"].as_i64().unwrap();

    let input = |v: Option<i64>| json!({ "accounting_periods": 13, "base_currency": "TWD", "rate_basis": "monthly", "version": v });
    let (status, e) = send(&app, "PUT", "/api/general-parameters", input(None)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(e["code"], "VERSION_REQUIRED");

    let (status, p) = send(&app, "PUT", "/api/general-parameters", input(Some(version))).await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["accounting_periods"], 13);
    assert_eq!(p["rate_basis"], "monthly");

    // 舊版本（ADR 0012 §4）
    let (status, e) = send(&app, "PUT", "/api/general-parameters", input(Some(version))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(e["code"], "RECORD_CHANGED");

    // 匯率採用每月：每日匯率維護顯示提示（GP7）
    let (status, s) = send(&app, "GET", "/api/daily-rates/settings", json!(null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s["hint"], "daily-unused");
    let (_, s) = send(&app, "GET", "/api/monthly-rates/settings", json!(null)).await;
    assert_eq!(s["hint"], Value::Null);
}

#[tokio::test]
async fn invalid_values_and_report() {
    let app = app();
    let (_, p) = send(&app, "GET", "/api/general-parameters", json!(null)).await;
    let (status, e) = send(
        &app,
        "PUT",
        "/api/general-parameters",
        json!({ "accounting_periods": 14, "base_currency": "TWD", "rate_basis": "daily", "version": p["version"] }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(e["code"], "ACCOUNTING_PERIODS_INVALID");

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/general-parameters/report.pdf?lang=zh-TW")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()[header::CONTENT_TYPE], "application/pdf");
}
