//! 料件預設倉儲維護 — specs/features/aim/item_storage.feature（IS1–IS12）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{
    ItemInput, ItemStorageBatchRequest, ItemStorageInput, ItemStorageItemDto, QbeCondition,
    QueryRequest,
};

use crate::{ErpWorld, steps::common::rows};

fn input(warehouse: &str, location: &str, max: &str) -> ItemStorageInput {
    ItemStorageInput {
        warehouse_code: warehouse.to_owned(),
        location_code: location.to_owned(),
        max_qty: if max.is_empty() {
            None
        } else {
            Some(max.parse().unwrap_or_else(|_| panic!("數字：{max}")))
        },
        version: None,
    }
}

async fn view(w: &ErpWorld, plant: &str, item: &str) -> ItemStorageItemDto {
    w.item_storages
        .get(&w.actor, plant, item)
        .await
        .unwrap_or_else(|e| panic!("查詢料件 {item} 的預設倉儲失敗：{e}"))
}

async fn version_of(w: &ErpWorld, plant: &str, item: &str, wh: &str, loc: &str) -> i32 {
    view(w, plant, item)
        .await
        .storages
        .iter()
        .find(|s| s.warehouse_code == wh && s.location_code == loc)
        .unwrap_or_else(|| panic!("料件 {item} 沒有預設倉儲 {wh}／{loc}"))
        .version
}

fn field_key(label: &str) -> &'static str {
    match label {
        "料號" => "item_no",
        "品名" => "name",
        "倉庫" => "warehouse_code",
        "儲位" => "location_code",
        "最高存量" => "max_qty",
        other => panic!("未知欄位：{other}"),
    }
}

// ----- 假設 -----

/// 不帶分群碼的料件（料件預設倉儲用；產品分類 P01、單位 PCS 由背景建立）
#[given(expr = "營運中心 {string} 已有料件 {string}，品名 {string}")]
async fn has_item(w: &mut ErpWorld, plant: String, no: String, name: String) {
    let req = ItemInput {
        item_no: no.clone(),
        name: Some(name),
        stock_unit: Some("PCS".into()),
        product_class: Some("P01".into()),
        ..ItemInput::default()
    };
    if let Err(e) = w.items.create(&w.actor, &plant, req).await {
        panic!("建立料件 {no} 失敗：{e}");
    }
}

#[given(
    regex = r#"^另一位使用者已把營運中心 "([^"]*)" 料件 "([^"]*)" 的預設倉儲 "([^"]*)"／"([^"]*)" 最高存量改為 "([^"]*)"$"#
)]
async fn other_user_updates(
    w: &mut ErpWorld,
    plant: String,
    item: String,
    wh: String,
    loc: String,
    max: String,
) {
    let v = version_of(w, &plant, &item, &wh, &loc).await;
    w.item_storage_version = Some(v);
    let mut i = input(&wh, &loc, &max);
    i.version = Some(v);
    w.item_storages
        .update(&w.actor, &plant, &item, (&wh, &loc), i)
        .await
        .unwrap_or_else(|e| panic!("修改預設倉儲失敗：{e}"));
}

// ----- 當 -----

#[given(
    regex = r#"^使用者在營運中心 "([^"]*)" 為料件 "([^"]*)" 新增預設倉儲，倉庫 "([^"]*)"，儲位 "([^"]*)"，最高存量 "([^"]*)"$"#
)]
async fn given_create(
    w: &mut ErpWorld,
    plant: String,
    item: String,
    wh: String,
    loc: String,
    max: String,
) {
    w.item_storages
        .create(&w.actor, &plant, &item, input(&wh, &loc, &max))
        .await
        .unwrap_or_else(|e| panic!("新增預設倉儲失敗：{e}"));
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 為料件 "([^"]*)" 新增預設倉儲，倉庫 "([^"]*)"，儲位 "([^"]*)"，最高存量 "([^"]*)"$"#
)]
async fn create(
    w: &mut ErpWorld,
    plant: String,
    item: String,
    wh: String,
    loc: String,
    max: String,
) {
    let r = w
        .item_storages
        .create(&w.actor, &plant, &item, input(&wh, &loc, &max))
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者(以舊版本)?在營運中心 "([^"]*)" 把料件 "([^"]*)" 的預設倉儲 "([^"]*)"／"([^"]*)" 改為倉庫 "([^"]*)"，儲位 "([^"]*)"，最高存量 "([^"]*)"$"#
)]
#[allow(clippy::too_many_arguments)]
async fn update(
    w: &mut ErpWorld,
    stale: String,
    plant: String,
    item: String,
    wh: String,
    loc: String,
    new_wh: String,
    new_loc: String,
    max: String,
) {
    let version = if stale.is_empty() {
        Some(version_of(w, &plant, &item, &wh, &loc).await)
    } else {
        w.item_storage_version
    };
    let mut i = input(&new_wh, &new_loc, &max);
    i.version = version;
    let r = w
        .item_storages
        .update(&w.actor, &plant, &item, (&wh, &loc), i)
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 刪除料件 "([^"]*)" 的預設倉儲 "([^"]*)"／"([^"]*)"$"#
)]
async fn delete(w: &mut ErpWorld, plant: String, item: String, wh: String, loc: String) {
    let v = version_of(w, &plant, &item, &wh, &loc).await;
    let r = w
        .item_storages
        .delete(&w.actor, &plant, &item, (&wh, &loc), Some(v))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 把料件 "([^"]*)" 的預設倉儲複製到料件 "([^"]*)"$"#)]
async fn copy(w: &mut ErpWorld, plant: String, item: String, target: String) {
    let r = w.item_storages.copy(&w.actor, &plant, &item, &target).await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 以料號 "([^"]*)" 整批產生預設倉儲，倉庫 "([^"]*)"，儲位 "([^"]*)"$"#
)]
async fn batch(w: &mut ErpWorld, plant: String, pattern: String, wh: String, loc: String) {
    let req = ItemStorageBatchRequest {
        conditions: vec![QbeCondition {
            field: "item_no".into(),
            expr: pattern,
        }],
        warehouse_code: wh,
        location_code: loc,
    };
    let r = w.item_storages.batch_generate(&w.actor, &plant, req).await;
    w.last = Some(r.map(|res| {
        w.item_storage_result = Some(res);
    }));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 查詢料件預設倉儲$"#)]
async fn query_all(w: &mut ErpWorld, plant: String) {
    let r = w
        .item_storages
        .query(&w.actor, &plant, QueryRequest::default())
        .await;
    crate::steps::common::set_query_result(w, r, |d| d.item_no.clone());
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以下列條件查詢料件預設倉儲:$"#)]
async fn query_by(w: &mut ErpWorld, plant: String, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.item_storages.query(&w.actor, &plant, req).await;
    crate::steps::common::set_query_result(w, r, |d| d.item_no.clone());
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 列印料件預設倉儲$"#)]
async fn report(w: &mut ErpWorld, plant: String) {
    let r = w
        .item_storages
        .report_in(
            &w.actor,
            &plant,
            Vec::new(),
            ReportFormat::Pdf,
            &Lang::default_lang(),
        )
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

// ----- 那麼 -----

#[then(regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 的預設倉儲應為:$"#)]
async fn is(w: &mut ErpWorld, plant: String, item: String, step: &Step) {
    let got: Vec<Vec<String>> = view(w, &plant, &item)
        .await
        .storages
        .iter()
        .map(|s| {
            vec![
                s.warehouse_code.clone(),
                s.location_code.clone(),
                s.max_qty
                    .map(|q| q.normalize().to_string())
                    .unwrap_or_default(),
            ]
        })
        .collect();
    assert_eq!(got, rows(step));
}

#[then(regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 的預設倉儲應有 (\d+) 筆$"#)]
async fn count(w: &mut ErpWorld, plant: String, item: String, n: usize) {
    assert_eq!(view(w, &plant, &item).await.storages.len(), n);
}

#[then(regex = r#"^整批產生應新增 (\d+) 筆、略過 (\d+) 筆$"#)]
async fn batch_result(w: &mut ErpWorld, inserted: u64, skipped: u64) {
    let r = w.item_storage_result.as_ref().expect("整批產生結果");
    assert_eq!((r.inserted, r.skipped), (inserted, skipped));
}

#[then(regex = r#"^料件預設倉儲清單順序應為 "([^"]*)"$"#)]
async fn list_order(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^營運中心 "([^"]*)" 的倉庫 "([^"]*)" 應可刪除$"#)]
async fn warehouse_deletable(w: &mut ErpWorld, plant: String, code: String) {
    let d = w
        .warehouses
        .get(&w.actor, &plant, &code)
        .await
        .expect("倉庫");
    w.warehouses
        .delete(&w.actor, &plant, &code, Some(d.version))
        .await
        .unwrap_or_else(|e| panic!("刪除倉庫 {code} 失敗：{e}"));
}

#[then(regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 的預設倉儲異動記錄應有 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, plant: String, item: String, expected: String) {
    let h = w
        .item_storages
        .history(&w.actor, &plant, &item, None)
        .await
        .expect("異動記錄");
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}
