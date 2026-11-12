//! 匯率自動匯入 API 契約測試（aooi070 ★ auto_import）：路由、HTTP 狀態與錯誤代碼。
//! 匯入規則由 BDD 涵蓋（specs/features/aoo/exchange_rate_import.feature）。

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{Services, memory::InMemoryStore};

/// 台銀牌告匯率 CSV（rate.bot.com.tw/xrt/flcsv/0/day 的格式）
const BOT_CSV: &str = "幣別,匯率,現金,即期,遠期10天,遠期30天,遠期60天,遠期90天,遠期120天,遠期150天,遠期180天,匯率,現金,即期,遠期10天,遠期30天,遠期60天,遠期90天,遠期120天,遠期150天,遠期180天\n\
USD,本行買入,31.70000,32.02500,32.00000,31.95000,31.90000,31.85000,31.80000,31.75000,31.70000,本行賣出,32.37000,32.12500,32.10000,32.05000,32.00000,31.95000,31.90000,31.85000,31.80000\n";

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

async fn app_with_usd() -> axum::Router {
    let app = app();
    let (status, _) = send(
        &app,
        "POST",
        "/api/currencies",
        json!({ "currency_code": "USD", "name": "美元" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    app
}

#[tokio::test]
async fn upload_bank_csv_then_overwrite_needs_consent() {
    let app = app_with_usd().await;
    let body = json!({ "kind": "cash", "csv": BOT_CSV });

    let (status, r) = send(&app, "POST", "/api/daily-rates/import/bank", body.clone()).await;
    assert_eq!(status, StatusCode::OK, "{r}");
    assert_eq!(r["imported"], json!(["USD"]));
    let today = chrono::Local::now().date_naive();
    let (status, rate) = send(
        &app,
        "GET",
        &format!("/api/daily-rates/USD/{today}"),
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // 現金：（31.70＋32.37）/ 2（IM3）
    assert_eq!(rate["rates"]["bank_mid"], json!("32.035"));

    // 今天已有每日匯率：未同意覆寫時回 409，同意後成功（IM5）
    let (status, e) = send(&app, "POST", "/api/daily-rates/import/bank", body).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(e["code"], "EXCHANGE_RATE_IMPORT_OVERWRITE_REQUIRED");
    let (status, _) = send(
        &app,
        "POST",
        "/api/daily-rates/import/bank",
        json!({ "kind": "spot", "overwrite": true, "csv": BOT_CSV }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn invalid_source_or_kind_is_rejected() {
    let app = app_with_usd().await;
    // 收到網站驗證頁等非 CSV 內容（IM1）
    let (status, e) = send(
        &app,
        "POST",
        "/api/daily-rates/import/bank",
        json!({ "csv": "<!DOCTYPE html><html></html>" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(e["code"], "EXCHANGE_RATE_SOURCE_INVALID");

    let (status, e) = send(
        &app,
        "POST",
        "/api/daily-rates/import/bank",
        json!({ "kind": "Cash", "csv": BOT_CSV }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(e["code"], "EXCHANGE_RATE_KIND_INVALID");
}

#[tokio::test]
async fn unreachable_source_is_bad_gateway() {
    // 記憶體來源沒有報價：視同網站無法連線
    let app = app_with_usd().await;
    let (status, e) = send(&app, "POST", "/api/daily-rates/import/bank", json!({})).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(e["code"], "EXCHANGE_RATE_SOURCE_UNAVAILABLE");

    let (status, e) = send(&app, "POST", "/api/daily-rates/import/customs", json!({})).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(e["code"], "EXCHANGE_RATE_SOURCE_UNAVAILABLE");
}
