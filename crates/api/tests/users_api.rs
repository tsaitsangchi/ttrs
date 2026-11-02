//! User Maintenance API 契約測試：HTTP 狀態碼與錯誤格式。業務規則由 BDD 涵蓋（user_maintenance.feature）。

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use ttrs_api::{AppState, actor_from_env, router};
use ttrs_app::{
    DataDictionaryService, ItemService, LegacyCatalogService, PermissionClassService, PlantService,
    ProgramPermissionService, ProgramService, UserPermissionService, UserService,
    memory::InMemoryStore,
};
use ttrs_shared::{ApiError, UserDto};

async fn app() -> Router {
    let store = InMemoryStore::seeded();
    store.add_module("MENU");
    let store = Arc::new(store);
    let app = router(AppState {
        items: ItemService::new(store.clone(), store.clone()),
        programs: ProgramService::new(store.clone()),
        dictionary: DataDictionaryService::new(store.clone(), store.clone(), store.clone()),
        legacy: LegacyCatalogService::new(store.clone()),
        users: UserService::new(
            store.clone(),
            store.clone(),
            store.clone(),
            store.clone(),
            store.clone(),
        ),
        classes: PermissionClassService::new(store.clone(), store.clone(), store.clone()),
        plants: PlantService::new(store.clone()),
        permissions: ProgramPermissionService::new(store.clone(), store.clone(), store.clone()),
        special: UserPermissionService::new(store.clone(), store.clone(), store.clone(), store),
        actor: actor_from_env(),
    });
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/programs",
            Some(serde_json::json!({ "program_no": "menu_main", "module_code": "MENU" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    app
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

fn new_user(id: &str) -> serde_json::Value {
    serde_json::json!({
        "user_id": id, "name": "王小明", "permission_class": "ADMIN",
        "menu": "menu_main", "default_plant": "TW01"
    })
}

#[tokio::test]
async fn create_get_duplicate_and_delete() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(request("POST", "/api/users", Some(new_user("u001"))))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let dto: UserDto = body(res).await;
    assert_eq!(dto.allowed_plants, ["TW01"]);
    assert!(dto.active);

    let res = app
        .clone()
        .oneshot(request("POST", "/api/users", Some(new_user("u001"))))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(body::<ApiError>(res).await.code, "USER_ALREADY_EXISTS");

    let res = app
        .clone()
        .oneshot(request("POST", "/api/users/u001/deactivate", None))
        .await
        .unwrap();
    assert!(!body::<UserDto>(res).await.active);

    let res = app
        .clone()
        .oneshot(request("DELETE", "/api/users/u001", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = app
        .oneshot(request("GET", "/api/users/u001", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(body::<ApiError>(res).await.code, "USER_NOT_FOUND");
}

#[tokio::test]
async fn invalid_menu_returns_422() {
    let mut u = new_user("u002");
    u["menu"] = "aimi100".into();
    let res = app()
        .await
        .oneshot(request("POST", "/api/users", Some(u)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body::<ApiError>(res).await.code, "USER_MENU_INVALID");
}
