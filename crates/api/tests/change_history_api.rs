//! 資料異動記錄 API 契約測試（ADR 0012）：版本號必填（CH4）、版本衝突 409、異動記錄端點與請求代號。
//! 業務規則由 BDD 涵蓋（specs/features/azz/change_history.feature）。

use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{Services, memory::InMemoryStore};
use ttrs_shared::{ChangeDto, DepartmentDto};

fn app() -> axum::Router {
    router(AppState::new(
        Services::new(Arc::new(InMemoryStore::seeded())),
        actor_from_env(),
    ))
}

async fn call(
    app: &axum::Router,
    method: Method,
    uri: &str,
    body: Option<&str>,
) -> (StatusCode, Vec<u8>) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-request-id", "0f8fad5b-d9cb-469f-a165-70867728950e")
        .header("x-forwarded-for", "10.1.2.3, 10.0.0.1")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_owned())))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    (
        status,
        to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
}

#[tokio::test]
async fn version_is_required_and_checked() {
    let app = app();
    let (s, body) = call(
        &app,
        Method::POST,
        "/api/plants/TW01/departments",
        Some(r#"{"dept_code":"MIS","name":"資訊部"}"#),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let d: DepartmentDto = serde_json::from_slice(&body).unwrap();
    assert_eq!(d.version, 1);
    assert!(d.created_at.is_some());

    // 沒帶版本 → 400（CH4）
    let (s, _) = call(
        &app,
        Method::PUT,
        "/api/plants/TW01/departments/MIS",
        Some(r#"{"name":"資訊處"}"#),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = call(
        &app,
        Method::DELETE,
        "/api/plants/TW01/departments/MIS",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);

    // 第 1 版修改成功，版本變 2；再以第 1 版修改 → 409
    let (s, body) = call(
        &app,
        Method::PUT,
        "/api/plants/TW01/departments/MIS",
        Some(r#"{"name":"資訊處","version":1}"#),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let d: DepartmentDto = serde_json::from_slice(&body).unwrap();
    assert_eq!(d.version, 2);
    assert!(d.updated_at.is_some());
    let (s, _) = call(
        &app,
        Method::POST,
        "/api/plants/TW01/departments/MIS/deactivate?version=1",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);

    // 異動記錄：新增、修改各一筆，帶請求代號與來源位址
    let (s, body) = call(
        &app,
        Method::GET,
        "/api/plants/TW01/departments/MIS/history?lang=zh-TW",
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let h: Vec<ChangeDto> = serde_json::from_slice(&body).unwrap();
    let ops: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(ops, ["insert", "update"]);
    let last = &h[1];
    assert_eq!(
        last.request_id.as_deref(),
        Some("0f8fad5b-d9cb-469f-a165-70867728950e")
    );
    assert_eq!(last.client_ip.as_deref(), Some("10.1.2.3"));
    assert_eq!(last.program.as_deref(), Some("department-maintenance"));
    assert_eq!(last.fields.len(), 1);
    assert_eq!(last.fields[0].column, "name");
}
