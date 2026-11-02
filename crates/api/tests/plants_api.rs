//! Plant Maintenance API 契約測試：HTTP 狀態碼與錯誤格式。業務規則由 BDD 涵蓋（plant_maintenance.feature）。

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
use ttrs_shared::{ApiError, PlantDto};

fn app() -> Router {
    let store = Arc::new(InMemoryStore::seeded());
    router(AppState {
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

#[tokio::test]
async fn create_duplicate_copy_deactivate_and_delete() {
    let app = app();
    let cn01 =
        serde_json::json!({ "plant_id": "cn01", "name": "昆山廠", "time_zone": "Asia/Shanghai" });
    let res = app
        .clone()
        .oneshot(request("POST", "/api/plants", Some(cn01.clone())))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let dto: PlantDto = body(res).await;
    assert_eq!(dto.plant_id, "CN01");
    assert!(dto.active);

    let res = app
        .clone()
        .oneshot(request("POST", "/api/plants", Some(cn01)))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(body::<ApiError>(res).await.code, "PLANT_ALREADY_EXISTS");

    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/plants/CN01/copy",
            Some(serde_json::json!({ "new_plant_id": "CN02" })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    assert_eq!(body::<PlantDto>(res).await.time_zone, "Asia/Shanghai");

    let res = app
        .clone()
        .oneshot(request("POST", "/api/plants/CN01/deactivate", None))
        .await
        .unwrap();
    assert!(!body::<PlantDto>(res).await.active);

    let res = app
        .oneshot(request("DELETE", "/api/plants/CN01", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn invalid_time_zone_422_and_plant_in_use_409() {
    let app = app();
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/plants",
            Some(
                serde_json::json!({ "plant_id": "CN03", "name": "測試廠", "time_zone": "Taipei" }),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body::<ApiError>(res).await.code, "PLANT_TIME_ZONE_INVALID");

    // 種子資料的 TW01 已有料件時不可刪除
    let res = app
        .clone()
        .oneshot(request(
            "POST",
            "/api/items",
            Some(serde_json::json!({
                "plant_id": "TW01", "item_no": "A001", "name": "螺絲", "stock_unit": "PCS"
            })),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let res = app
        .oneshot(request("DELETE", "/api/plants/TW01", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    assert_eq!(body::<ApiError>(res).await.code, "PLANT_IN_USE");
}
