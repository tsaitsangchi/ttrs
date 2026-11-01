//! Data Dictionary Maintenance API 契約測試：HTTP 狀態碼、錯誤格式與 CSV 下載。業務規則由 BDD 涵蓋。

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{
    DataDictionaryService, ItemService, LegacyCatalogService, ProgramService, UserService,
    memory::InMemoryStore,
    ports::{ColumnSchema, TableSchema},
};
use ttrs_shared::ApiError;

fn app() -> Router {
    let store = InMemoryStore::seeded();
    store.add_table_schema(TableSchema {
        name: "module".into(),
        columns: vec![ColumnSchema {
            name: "module_code".into(),
            data_type: "text".into(),
            not_null: true,
            default: None,
        }],
        indexes: vec![],
        constraints: vec![],
    });
    let store = Arc::new(store);
    router(AppState {
        items: ItemService::new(store.clone(), store.clone()),
        programs: ProgramService::new(store.clone()),
        dictionary: DataDictionaryService::new(store.clone(), store.clone(), store.clone()),
        legacy: LegacyCatalogService::new(store.clone()),
        users: UserService::new(store.clone(), store.clone(), store),
        actor: actor_from_env(),
    })
}

fn json(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn create(table: &str) -> Request<Body> {
    json(
        "POST",
        "/api/dictionary/tables",
        serde_json::json!({ "table_name": table, "module_code": "AZZ", "table_type": "master" }),
    )
}

async fn error_code(res: axum::response::Response) -> String {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice::<ApiError>(&bytes).unwrap().code
}

#[tokio::test]
async fn create_201_duplicate_409_missing_404() {
    let app = app();
    let res = app.clone().oneshot(create("module")).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let res = app.clone().oneshot(create("module")).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(error_code(res).await, "DICT_ALREADY_EXISTS");

    let res = app
        .oneshot(
            Request::get("/api/dictionary/tables/unit")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn table_not_in_schema_returns_422() {
    let res = app().oneshot(create("no_such_table")).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error_code(res).await, "DICT_TABLE_NOT_IN_SCHEMA");
}

#[tokio::test]
async fn export_is_csv_with_bom() {
    let app = app();
    app.clone().oneshot(create("module")).await.unwrap();
    let res = app
        .oneshot(
            Request::get("/api/dictionary/tables/module/export?lang=en")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/csv")
    );
    let body = String::from_utf8(
        to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.starts_with('\u{feff}'), "缺少 UTF-8 BOM");
    assert!(
        body.contains("Column,Type,Name,Extra\nmodule_code,text,,\n"),
        "{body}"
    );
}
