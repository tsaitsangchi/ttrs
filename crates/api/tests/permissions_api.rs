//! Program Permission Maintenance API 契約測試：HTTP 狀態碼與錯誤格式。
//! 業務規則由 BDD 涵蓋（program_permission_maintenance.feature）。

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
    ProgramPermissionService, ProgramService, UserService, memory::InMemoryStore,
};
use ttrs_shared::{ApiError, PermissionBatchResultDto, ProgramPermissionDto};

async fn app() -> Router {
    let store = InMemoryStore::seeded();
    store.add_permission_class("SALES");
    let store = Arc::new(store);
    let app = router(AppState {
        items: ItemService::new(store.clone(), store.clone()),
        programs: ProgramService::new(store.clone()),
        dictionary: DataDictionaryService::new(store.clone(), store.clone(), store.clone()),
        legacy: LegacyCatalogService::new(store.clone()),
        users: UserService::new(store.clone(), store.clone(), store.clone(), store.clone()),
        classes: PermissionClassService::new(store.clone(), store.clone(), store.clone()),
        plants: PlantService::new(store.clone()),
        permissions: ProgramPermissionService::new(store.clone(), store.clone(), store),
        actor: actor_from_env(),
    });
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/programs",
            Some(serde_json::json!({
                "program_no": "aimi100", "module_code": "AIM", "name": "料件基本資料",
                "default_actions": "insert, query, modify, delete"
            })),
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

#[tokio::test]
async fn grant_list_update_and_revoke() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/permission-classes/SALES/permissions",
            Some(serde_json::json!({ "program_no": "aimi100" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let dto: ProgramPermissionDto = body(res).await;
    assert_eq!(dto.actions, ["insert", "query", "modify", "delete"]);
    assert_eq!(dto.program_name.as_deref(), Some("料件基本資料"));

    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/permission-classes/SALES/permissions",
            Some(serde_json::json!({ "program_no": "aimi100" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(
        body::<ApiError>(res).await.code,
        "PERMISSION_ALREADY_EXISTS"
    );

    let res = app
        .clone()
        .oneshot(request(
            "PUT",
            "/api/permission-classes/SALES/permissions/aimi100",
            Some(serde_json::json!({ "actions": ["query"], "other_dept_data": "7" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let dto: ProgramPermissionDto = body(res).await;
    assert_eq!(
        (dto.actions.as_slice(), dto.other_dept_data.as_str()),
        (&["query".to_owned()][..], "7")
    );

    let res = app
        .clone()
        .oneshot(request(
            "GET",
            "/api/permission-classes/SALES/permissions?lang=en",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(body::<Vec<ProgramPermissionDto>>(res).await.len(), 1);

    let res = app
        .clone()
        .oneshot(request(
            "DELETE",
            "/api/permission-classes/SALES/permissions/aimi100",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let res = app
        .oneshot(request(
            "DELETE",
            "/api/permission-classes/SALES/permissions/aimi100",
            None,
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_eq!(body::<ApiError>(res).await.code, "PERMISSION_NOT_FOUND");
}

#[tokio::test]
async fn invalid_action_422_and_copy() {
    let app = app().await;
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/permission-classes/SALES/permissions",
            Some(serde_json::json!({ "program_no": "aimi100", "actions": ["confirm"] })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body::<ApiError>(res).await.code,
        "PERMISSION_ACTION_NOT_ALLOWED"
    );

    // 新增程式時已自動授權給建立者的權限類別 ADMIN；複製到尚無權限的 SALES
    let res = app
        .oneshot(request(
            "POST",
            "/api/permission-classes/ADMIN/copy-permissions",
            Some(serde_json::json!({ "target_class": "SALES" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body::<PermissionBatchResultDto>(res).await.added, 1);
}
