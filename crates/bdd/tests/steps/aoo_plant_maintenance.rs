//! Plant Maintenance 營運中心維護 — specs/features/aoo/plant_maintenance.feature

use chrono::{Local, NaiveDate};
use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{PlantDto, PlantInput};

use crate::{Backend, ErpWorld};

fn date_label(d: Option<NaiveDate>) -> String {
    match d {
        None => String::new(),
        Some(d) if d == Local::now().date_naive() => "今天".into(),
        Some(d) => d.to_string(),
    }
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

async fn plant(w: &ErpWorld, id: &str) -> PlantDto {
    w.plants
        .get(&w.actor, id)
        .await
        .unwrap_or_else(|e| panic!("讀取營運中心 {id} 失敗：{e}"))
}

/// 讀出營運中心、修改輸入後存檔，結果記在 w.last
async fn modify(w: &mut ErpWorld, id: &str, change: impl FnOnce(&mut PlantInput)) {
    let mut req = plant(w, id).await.to_input();
    change(&mut req);
    let result = w.plants.update(&w.actor, id, req).await;
    w.last = Some(result.map(|_| ()));
}

// ----- 假設 -----

#[given(expr = "使用者停用營運中心 {string}")]
async fn plant_deactivated(w: &mut ErpWorld, id: String) {
    w.plants.deactivate(&w.actor, &id, None).await.unwrap();
}

#[given(expr = "使用者將營運中心 {string} 的地址改為 {string}，電話改為 {string}")]
async fn plant_contact_set(w: &mut ErpWorld, id: String, address: String, phone: String) {
    let mut req = plant(w, &id).await.to_input();
    req.address = Some(address);
    req.phone = Some(phone);
    w.plants.update(&w.actor, &id, req).await.unwrap();
}

#[given(expr = "已新增營運中心 {string}，資料庫代碼 {string}")]
async fn plant_with_schema(w: &mut ErpWorld, id: String, schema: String) {
    let req = PlantInput {
        plant_id: id.clone(),
        name: Some(id),
        schema_name: Some(schema),
        ..Default::default()
    };
    w.plants.create(&w.actor, req).await.unwrap();
}

/// 前提：背景建立的營運中心依建立順序取得資料庫代碼
#[given(expr = "營運中心 {string} 的資料庫代碼為 {string}")]
async fn schema_is(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(plant(w, &id).await.schema_name, expected);
}

// ----- 當 -----

#[when(expr = "使用者新增營運中心 {string}，名稱 {string}，時區 {string}，資料庫代碼 {string}")]
async fn create_plant_with_schema(
    w: &mut ErpWorld,
    id: String,
    name: String,
    time_zone: String,
    schema: String,
) {
    let req = PlantInput {
        plant_id: id,
        name: Some(name),
        time_zone: Some(time_zone),
        schema_name: Some(schema),
        ..Default::default()
    };
    let result = w.plants.create(&w.actor, req).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者將營運中心 {string} 的資料庫代碼改為 {string}")]
async fn change_schema(w: &mut ErpWorld, id: String, schema: String) {
    modify(w, &id, |r| r.schema_name = Some(schema)).await;
}

#[when(expr = "使用者新增營運中心 {string}，名稱 {string}，時區 {string}")]
async fn create_plant(w: &mut ErpWorld, id: String, name: String, time_zone: String) {
    let req = PlantInput {
        plant_id: id,
        name: Some(name),
        time_zone: Some(time_zone),
        ..Default::default()
    };
    let result = w.plants.create(&w.actor, req).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者將營運中心 {string} 的代號改為 {string}")]
async fn change_id(w: &mut ErpWorld, id: String, new_id: String) {
    modify(w, &id, |r| r.plant_id = new_id).await;
}

#[when(expr = "使用者將營運中心 {string} 的地址改為 {string}，電話改為 {string}")]
async fn change_contact(w: &mut ErpWorld, id: String, address: String, phone: String) {
    modify(w, &id, |r| {
        r.address = Some(address);
        r.phone = Some(phone);
    })
    .await;
}

#[when(expr = "使用者停用營運中心 {string}")]
async fn deactivate(w: &mut ErpWorld, id: String) {
    let result = w.plants.deactivate(&w.actor, &id, None).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者啟用營運中心 {string}")]
async fn activate(w: &mut ErpWorld, id: String) {
    let result = w.plants.activate(&w.actor, &id, None).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者刪除營運中心 {string}")]
async fn delete(w: &mut ErpWorld, id: String) {
    let result = w.plants.delete(&w.actor, &id, None).await;
    w.last = Some(result);
}

#[when(expr = "使用者將營運中心 {string} 複製為 {string}")]
async fn copy(w: &mut ErpWorld, id: String, new_id: String) {
    let result = w.plants.copy(&w.actor, &id, &new_id).await;
    w.last = Some(result.map(|_| ()));
}

// ----- 那麼 -----

#[then(expr = "營運中心 {string} 應為:")]
async fn plant_should_be(w: &mut ErpWorld, id: String, step: &Step) {
    let p = plant(w, &id).await;
    for row in step.table.as_ref().expect("需要表格").rows.iter().skip(1) {
        let actual = match row[0].as_str() {
            "名稱" => p.name.clone(),
            "時區" => p.time_zone.clone(),
            "資料有效" => yes_no(p.active).into(),
            "資料所有者" => p.created_by.clone(),
            "建立日期" => date_label(Some(p.created_on)),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, row[1], "{}", row[0]);
    }
}

#[then(expr = "營運中心 {string} 的時區應為 {string}")]
async fn time_zone_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(plant(w, &id).await.time_zone, expected);
}

#[then(expr = "營運中心 {string} 的地址應為 {string}")]
async fn address_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(plant(w, &id).await.address.unwrap_or_default(), expected);
}

#[then(expr = "營運中心 {string} 的電話應為 {string}")]
async fn phone_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(plant(w, &id).await.phone.unwrap_or_default(), expected);
}

#[then(expr = "營運中心 {string} 的最後修改者應為 {string}")]
async fn updated_by_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(plant(w, &id).await.updated_by.unwrap_or_default(), expected);
}

#[then(expr = "營運中心 {string} 的資料有效應為 {string}")]
async fn active_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(yes_no(plant(w, &id).await.active), expected);
}

#[then(expr = "營運中心 {string} 應不存在")]
async fn should_not_exist(w: &mut ErpWorld, id: String) {
    match w.plants.get(&w.actor, &id).await {
        Err(e) => assert_eq!(e.code(), "PLANT_NOT_FOUND"),
        Ok(p) => panic!("營運中心仍存在：{p:?}"),
    }
}

#[then(expr = "營運中心 {string} 的資料庫代碼應為 {string}")]
async fn schema_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(plant(w, &id).await.schema_name, expected);
}

#[then(expr = "工廠 schema {string} 應已建立並套用全部工廠 migration")]
async fn plant_schema_ready(w: &mut ErpWorld, schema: String) {
    match &w.backend {
        Backend::Memory(s) => assert!(
            s.plant_schema_exists(&schema),
            "工廠 schema {schema} 不存在"
        ),
        Backend::Pg(pool) => {
            let applied = ttrs_infra::plant_schema_version(pool, &schema)
                .await
                .unwrap()
                .unwrap_or_else(|| panic!("工廠 schema {schema} 不存在"));
            assert_eq!(
                applied,
                ttrs_infra::PLANT_MIGRATOR.iter().count(),
                "已套用的工廠 migration 數"
            );
        }
    }
}

#[then(expr = "工廠 schema {string} 應不存在")]
async fn plant_schema_gone(w: &mut ErpWorld, schema: String) {
    let exists = match &w.backend {
        Backend::Memory(s) => s.plant_schema_exists(&schema),
        Backend::Pg(pool) => ttrs_infra::plant_schema_version(pool, &schema)
            .await
            .unwrap()
            .is_some(),
    };
    assert!(!exists, "工廠 schema {schema} 仍存在");
}

/// 記憶體版沒有資料庫角色，以工廠 schema 代表
async fn plant_role_exists(w: &ErpWorld, schema: &str) -> bool {
    match &w.backend {
        Backend::Memory(s) => s.plant_schema_exists(schema),
        Backend::Pg(pool) => ttrs_infra::plant_role_exists(pool, schema).await.unwrap(),
    }
}

#[then(expr = "工廠 schema {string} 應有自己的資料庫角色")]
async fn plant_role_created(w: &mut ErpWorld, schema: String) {
    assert!(
        plant_role_exists(w, &schema).await,
        "工廠 schema {schema} 沒有資料庫角色"
    );
}

#[then(expr = "工廠 schema {string} 的資料庫角色應不存在")]
async fn plant_role_dropped(w: &mut ErpWorld, schema: String) {
    assert!(
        !plant_role_exists(w, &schema).await,
        "工廠 schema {schema} 的資料庫角色仍存在"
    );
}
