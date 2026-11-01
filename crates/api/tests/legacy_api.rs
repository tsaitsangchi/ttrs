//! TipTop 對照目錄 API 契約測試：HTTP 狀態碼與錯誤格式。業務規則由 BDD 涵蓋（legacy_catalog.feature）。

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{
    DataDictionaryService, ItemService, LegacyCatalogService, ProgramService, UserService,
    memory::InMemoryStore,
};
use ttrs_shared::{ApiError, LegacyImportResultDto, LegacyTableDto, LegacyTableSummaryDto};

fn app() -> Router {
    let store = Arc::new(InMemoryStore::seeded());
    router(AppState {
        items: ItemService::new(store.clone(), store.clone()),
        programs: ProgramService::new(store.clone()),
        dictionary: DataDictionaryService::new(store.clone(), store.clone(), store.clone()),
        legacy: LegacyCatalogService::new(store.clone()),
        users: UserService::new(store.clone(), store.clone(), store),
        actor: actor_from_env(),
    })
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

fn catalog(table_name: &str) -> serde_json::Value {
    serde_json::json!([{
        "legacy_name": "zx_file", "name": table_name, "title": "部門使用者檔", "module": "azz",
        "columns": [
            { "legacy_name": "zx01", "name": "user_id", "data_type": "varchar2(10)", "primary_key": true, "description": "使用者代號" },
            { "legacy_name": "zx13", "name": "unused_zx13", "data_type": "varchar2(1)", "description": "No Use", "unused": true }
        ]
    }])
}

#[tokio::test]
async fn import_then_search_and_get() {
    let app = app();
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/legacy-catalog/import",
            Some(catalog("user_account")),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let result: LegacyImportResultDto = body(res).await;
    assert_eq!((result.tables, result.columns), (1, 2));

    let res = app
        .clone()
        .oneshot(request("GET", "/api/legacy-catalog?q=user*", None))
        .await
        .unwrap();
    let list: Vec<LegacyTableSummaryDto> = body(res).await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].column_count, 2);

    let res = app
        .clone()
        .oneshot(request("GET", "/api/legacy-catalog/zx_file", None))
        .await
        .unwrap();
    let t: LegacyTableDto = body(res).await;
    assert!(t.columns[1].unused);

    let res = app
        .clone()
        .oneshot(request("GET", "/api/legacy-catalog/zz_file", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(body::<ApiError>(res).await.code, "LEGACY_TABLE_NOT_FOUND");

    // 資料字典查對應的舊表：沒有對照時回傳 null
    let res = app
        .oneshot(request(
            "GET",
            "/api/dictionary/tables/program/legacy",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body::<Option<LegacyTableDto>>(res).await, None);
}

#[tokio::test]
async fn invalid_name_returns_422() {
    let res = app()
        .oneshot(request(
            "POST",
            "/api/legacy-catalog/import",
            Some(catalog("User-Account")),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body::<ApiError>(res).await.code, "LEGACY_NAME_INVALID");
}
