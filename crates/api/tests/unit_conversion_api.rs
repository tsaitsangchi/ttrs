//! 單位換算 API 契約測試（aooi102、aooi103）：路由與 HTTP 狀態。業務規則由 BDD 涵蓋（specs/features/aoo/unit_conversion.feature）。

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
    let store = InMemoryStore::seeded();
    store.add_unit("BOX");
    router(AppState::new(
        Services::new(Arc::new(store)),
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
async fn general_conversion_and_factor() {
    let app = app();
    let body =
        json!({ "from_unit": "BOX", "to_unit": "PCS", "from_quantity": "1", "to_quantity": "12" });
    let (status, c) = send(&app, "POST", "/api/unit-conversions", body.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{c}");
    let reverse =
        json!({ "from_unit": "PCS", "to_unit": "BOX", "from_quantity": "12", "to_quantity": "1" });
    let (status, e) = send(&app, "POST", "/api/unit-conversions", reverse).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(e["code"], "UNIT_CONVERSION_ALREADY_EXISTS");

    let (status, f) = send(
        &app,
        "GET",
        "/api/plants/TW01/unit-conversion-factor?item=A001&from=PCS&to=BOX",
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(f.as_str().map(|v| v.parse::<f64>().is_ok()), Some(true));

    let v = c["version"].as_i64().unwrap();
    let (status, _) = send(
        &app,
        "POST",
        &format!("/api/unit-conversions/BOX/PCS/deactivate?version={v}"),
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, e) = send(
        &app,
        "GET",
        "/api/plants/TW01/unit-conversion-factor?item=A001&from=BOX&to=PCS",
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(e["code"], "UNIT_CONVERSION_NOT_FOUND");

    let (status, _) = send(
        &app,
        "GET",
        "/api/unit-conversions/report.pdf?lang=zh-TW",
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, e) = send(
        &app,
        "GET",
        "/api/plants/TW01/items/NOPE/unit-conversions",
        json!(null),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{e}");
}
