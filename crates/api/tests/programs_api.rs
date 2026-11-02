//! Program Maintenance API 契約測試：HTTP 狀態碼與錯誤格式。業務規則由 BDD 涵蓋。

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{
    DataDictionaryService, ItemService, LegacyCatalogService, PermissionClassService,
    ProgramService, UserService, memory::InMemoryStore,
};
use ttrs_shared::{ApiError, ProgramDto};

fn app() -> Router {
    let store = Arc::new(InMemoryStore::seeded());
    router(AppState {
        items: ItemService::new(store.clone(), store.clone()),
        programs: ProgramService::new(store.clone()),
        dictionary: DataDictionaryService::new(store.clone(), store.clone(), store.clone()),
        legacy: LegacyCatalogService::new(store.clone()),
        users: UserService::new(store.clone(), store.clone(), store.clone(), store.clone()),
        classes: PermissionClassService::new(store.clone(), store.clone(), store),
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

fn create(no: &str) -> Request<Body> {
    json(
        "POST",
        "/api/programs",
        serde_json::json!({ "program_no": no, "module_code": "AIM", "name": "料件基本資料維護作業" }),
    )
}

async fn body<T: serde::de::DeserializeOwned>(res: axum::response::Response) -> T {
    serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn create_returns_201_with_defaults_then_duplicate_409() {
    let app = app();
    let res = app.clone().oneshot(create("aimi100")).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let dto: ProgramDto = body(res).await;
    assert_eq!(dto.program_type.as_deref(), Some("I"));
    assert_eq!(dto.name.as_deref(), Some("料件基本資料維護作業"));

    let res = app.oneshot(create("aimi100")).await.unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(body::<ApiError>(res).await.code, "PROGRAM_ALREADY_EXISTS");
}

#[tokio::test]
async fn rule_violation_returns_422() {
    let res = app().oneshot(create("aimr400")).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body::<ApiError>(res).await.code, "REPORT_TYPE_REQUIRED");
}

#[tokio::test]
/// 改代號：PUT 原代號，內容帶新的 program_no
async fn change_no_via_update_then_old_no_returns_404() {
    let app = app();
    app.clone().oneshot(create("aimi100")).await.unwrap();
    let res = app
        .clone()
        .oneshot(json(
            "PUT",
            "/api/programs/aimi100",
            serde_json::json!({ "program_no": "aimi101", "module_code": "AIM" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/programs/aimi100")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let res = app
        .oneshot(
            Request::get("/api/programs/aimi101/permissions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(body::<Vec<String>>(res).await, vec!["ADMIN".to_string()]);
}

#[tokio::test]
async fn delete_returns_204() {
    let app = app();
    app.clone().oneshot(create("aimi100")).await.unwrap();
    let res = app
        .oneshot(
            Request::delete("/api/programs/aimi100")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn query_returns_page_and_bad_condition_422() {
    let app = app();
    app.clone().oneshot(create("aimi100")).await.unwrap();
    app.clone().oneshot(create("aimi101")).await.unwrap();

    let res = app
        .clone()
        .oneshot(json(
            "POST",
            "/api/programs/query",
            serde_json::json!({ "conditions": [{ "field": "program_no", "expr": "aimi10?" }], "page_size": 1 }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let page: ttrs_shared::Page<ProgramDto> = body(res).await;
    assert_eq!((page.total, page.items.len()), (2, 1));

    let res = app
        .oneshot(json(
            "POST",
            "/api/programs/query",
            serde_json::json!({ "conditions": [{ "field": "report_width", "expr": "abc" }] }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body::<ApiError>(res).await.code, "QBE_INVALID_NUMBER");
}

#[tokio::test]
async fn names_per_language_and_localized_get() {
    let app = app();
    app.clone().oneshot(create("aimi100")).await.unwrap();
    let res = app
        .clone()
        .oneshot(json(
            "PUT",
            "/api/programs/aimi100/names",
            serde_json::json!({ "lang": "en", "name": "Item Master Maintenance" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/programs/aimi100?lang=en")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let dto: ProgramDto = body(res).await;
    assert_eq!(dto.name.as_deref(), Some("Item Master Maintenance"));
    assert_eq!(dto.names.len(), 2);

    let res = app
        .oneshot(json(
            "PUT",
            "/api/programs/aimi100/names",
            serde_json::json!({ "lang": "ja", "name": "x" }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body::<ApiError>(res).await.code, "LANGUAGE_NOT_SUPPORTED");
}

/// MultilingualField：新增/修改時以 `names` 一次儲存各語系；任一語系不合法則整筆不寫入
#[tokio::test]
async fn names_saved_together_on_create_and_update() {
    let app = app();
    let res = app
        .clone()
        .oneshot(json(
            "POST",
            "/api/programs",
            serde_json::json!({ "program_no": "aimi100", "module_code": "AIM", "names": [
                { "lang": "zh-TW", "name": "料件基本資料維護作業", "report_title": null },
                { "lang": "en", "name": "Item Master", "report_title": null },
                { "lang": "zh-CN", "name": "", "report_title": null },
            ] }),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let dto: ProgramDto = body(res).await;
    assert_eq!(dto.name.as_deref(), Some("料件基本資料維護作業"));
    let langs: Vec<&str> = dto.names.iter().map(|n| n.lang.as_str()).collect();
    assert_eq!(langs, ["en", "zh-TW"], "新增時空白語系不建立");

    let mut input = dto.to_input();
    input.names = dto.names.clone();
    input.names[0].name = "Item Master Maintenance".into();
    input.names.push(ttrs_shared::ProgramNameDto {
        lang: "ja".into(),
        name: "x".into(),
        report_title: None,
    });
    let put = |input: &ttrs_shared::ProgramInput| {
        json(
            "PUT",
            "/api/programs/aimi100",
            serde_json::to_value(input).unwrap(),
        )
    };
    let res = app.clone().oneshot(put(&input)).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body::<ApiError>(res).await.code, "LANGUAGE_NOT_SUPPORTED");

    input.names.pop();
    let res = app.oneshot(put(&input)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let dto: ProgramDto = body(res).await;
    let en = dto.names.iter().find(|n| n.lang == "en").unwrap();
    assert_eq!(en.name, "Item Master Maintenance");
}
