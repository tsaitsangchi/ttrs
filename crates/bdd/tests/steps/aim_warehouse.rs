//! 倉庫儲位維護 — specs/features/aim/warehouse.feature（WH1–WH13）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{ItemInput, QueryRequest, WarehouseDto, WarehouseInput, WarehouseLocationInput};

use crate::{ErpWorld, steps::common::rows};

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn flag(v: &str) -> bool {
    v == "是"
}

fn opt(v: &str) -> Option<String> {
    Some(v.to_owned()).filter(|s| !s.is_empty())
}

fn new_input(code: &str, name: &str, class: &str) -> WarehouseInput {
    WarehouseInput {
        warehouse_code: code.to_owned(),
        name: opt(name),
        storage_class: class.to_owned(),
        ..WarehouseInput::default()
    }
}

async fn wh(w: &ErpWorld, plant: &str, code: &str) -> WarehouseDto {
    w.warehouses
        .get(&w.actor, plant, code)
        .await
        .unwrap_or_else(|e| panic!("查詢倉庫 {code} 失敗：{e}"))
}

/// 以目前資料為底修改倉庫
async fn modify(
    w: &mut ErpWorld,
    plant: &str,
    code: &str,
    stale: bool,
    f: impl FnOnce(&mut WarehouseInput),
) {
    let d = wh(w, plant, code).await;
    let mut input = d.to_input();
    if stale {
        input.version = w.warehouse_version;
    }
    f(&mut input);
    let r = w.warehouses.update(&w.actor, plant, code, input).await;
    w.last = Some(r.map(drop));
}

fn field_key(label: &str) -> &'static str {
    match label {
        "倉庫別" => "warehouse_code",
        "名稱" => "name",
        "倉儲類別" => "storage_class",
        "儲位" => "location_code",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

// ----- 假設 -----

#[given(regex = r#"^營運中心 "([^"]*)" 已有倉庫 "([^"]*)"$"#)]
async fn has(w: &mut ErpWorld, plant: String, code: String) {
    let r = w
        .warehouses
        .create(&w.actor, &plant, new_input(&code, &code, "S"))
        .await;
    r.unwrap_or_else(|e| panic!("建立倉庫 {code} 失敗：{e}"));
}

#[given(regex = r#"^營運中心 "([^"]*)" 的倉庫 "([^"]*)" 已有儲位 "([^"]*)"$"#)]
async fn has_location(w: &mut ErpWorld, plant: String, code: String, loc: String) {
    let v = wh(w, &plant, &code).await.version;
    let input = WarehouseLocationInput {
        location_code: loc.clone(),
        name: Some(loc.clone()),
        version: Some(v),
        ..Default::default()
    };
    w.warehouses
        .add_location(&w.actor, &plant, &code, input)
        .await
        .unwrap_or_else(|e| panic!("建立儲位 {loc} 失敗：{e}"));
}

#[given(regex = r#"^使用者已(停用|確認)營運中心 "([^"]*)" 的倉庫 "([^"]*)"$"#)]
async fn already(w: &mut ErpWorld, what: String, plant: String, code: String) {
    let v = Some(wh(w, &plant, &code).await.version);
    let r = if what == "停用" {
        w.warehouses
            .set_active(&w.actor, &plant, &code, false, v)
            .await
    } else {
        w.warehouses.confirm(&w.actor, &plant, &code, true, v).await
    };
    r.unwrap_or_else(|e| panic!("{what}倉庫失敗：{e}"));
}

#[given(regex = r#"^另一位使用者已修改營運中心 "([^"]*)" 的倉庫 "([^"]*)"$"#)]
async fn changed_by_other(w: &mut ErpWorld, plant: String, code: String) {
    let d = wh(w, &plant, &code).await;
    w.warehouse_version = Some(d.version);
    let mut input = d.to_input();
    input.name = Some("他人修改".into());
    w.warehouses
        .update(&w.actor, &plant, &code, input)
        .await
        .unwrap_or_else(|e| panic!("他人修改失敗：{e}"));
}

// ----- 當 -----

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 新增倉庫 "([^"]*)"，名稱 "([^"]*)"，倉儲類別 "([^"]*)"$"#
)]
async fn add(w: &mut ErpWorld, plant: String, code: String, name: String, class: String) {
    let r = w
        .warehouses
        .create(&w.actor, &plant, new_input(&code, &name, &class))
        .await;
    w.last = Some(r.map(drop));
}

#[given(
    regex = r#"^使用者在營運中心 "([^"]*)" 把倉庫 "([^"]*)" 改為可用倉儲 "(是|否)"，MRP 可用 "(是|否)"$"#
)]
#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 把倉庫 "([^"]*)" 改為可用倉儲 "(是|否)"，MRP 可用 "(是|否)"$"#
)]
async fn set_usable(w: &mut ErpWorld, plant: String, code: String, usable: String, mrp: String) {
    modify(w, &plant, &code, false, |i| {
        i.usable = Some(flag(&usable));
        i.mrp_usable = Some(flag(&mrp));
    })
    .await;
}

#[when(
    regex = r#"^使用者(以舊版本)?在營運中心 "([^"]*)" 把倉庫 "([^"]*)" 的發料優先順序改為 (-?\d+)$"#
)]
async fn set_priority(w: &mut ErpWorld, stale: String, plant: String, code: String, n: i32) {
    modify(w, &plant, &code, !stale.is_empty(), |i| {
        i.issue_priority = Some(n)
    })
    .await;
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 把倉庫 "([^"]*)" 的成本中心改為 "([^"]*)"$"#)]
async fn set_cost_center(w: &mut ErpWorld, plant: String, code: String, cc: String) {
    modify(w, &plant, &code, false, |i| i.cost_center = opt(&cc)).await;
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 把倉庫 "([^"]*)" 改為倉儲類別 "([^"]*)"，保稅 "(是|否)"$"#
)]
async fn set_class(w: &mut ErpWorld, plant: String, code: String, class: String, bonded: String) {
    modify(w, &plant, &code, false, |i| {
        i.storage_class = class.clone();
        i.bonded = Some(flag(&bonded));
    })
    .await;
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 的倉庫 "([^"]*)" 新增儲位 "([^"]*)"，名稱 "([^"]*)"$"#
)]
async fn add_location(w: &mut ErpWorld, plant: String, code: String, loc: String, name: String) {
    let v = wh(w, &plant, &code).await.version;
    let input = WarehouseLocationInput {
        location_code: loc,
        name: opt(&name),
        version: Some(v),
        ..Default::default()
    };
    let r = w
        .warehouses
        .add_location(&w.actor, &plant, &code, input)
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 把倉庫 "([^"]*)" 儲位 "([^"]*)" 的科目方式改為 "([^"]*)"，科目 "([^"]*)"$"#
)]
async fn set_account(
    w: &mut ErpWorld,
    plant: String,
    code: String,
    loc: String,
    method: String,
    account: String,
) {
    let d = wh(w, &plant, &code).await;
    let l = d
        .locations
        .iter()
        .find(|l| l.location_code == loc)
        .expect("儲位")
        .clone();
    let mut input = l.to_input(d.version);
    input.account_method = Some(method);
    input.account = opt(&account);
    let r = w
        .warehouses
        .update_location(&w.actor, &plant, &code, input)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 刪除倉庫 "([^"]*)"$"#)]
async fn delete(w: &mut ErpWorld, plant: String, code: String) {
    let v = Some(wh(w, &plant, &code).await.version);
    let r = w.warehouses.delete(&w.actor, &plant, &code, v).await;
    w.last = Some(r);
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 刪除倉庫 "([^"]*)" 的儲位 "([^"]*)"$"#)]
async fn delete_location(w: &mut ErpWorld, plant: String, code: String, loc: String) {
    let v = Some(wh(w, &plant, &code).await.version);
    let r = w
        .warehouses
        .remove_location(&w.actor, &plant, &code, &loc, v)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 把倉庫 "([^"]*)" 複製為 "([^"]*)"$"#)]
async fn copy(w: &mut ErpWorld, plant: String, from: String, to: String) {
    let r = w.warehouses.copy(&w.actor, &plant, &from, &to).await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 新增料件 "([^"]*)"，主要倉庫 "([^"]*)"，主要儲位 "([^"]*)"$"#
)]
async fn add_item(w: &mut ErpWorld, plant: String, no: String, wh_code: String, loc: String) {
    let input = ItemInput {
        item_no: no,
        name: Some("測試".into()),
        stock_unit: Some("PCS".into()),
        product_class: Some("P01".into()),
        warehouse: opt(&wh_code),
        location: opt(&loc),
        ..ItemInput::default()
    };
    let r = w.items.create(&w.actor, &plant, input).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 查詢倉庫$"#)]
async fn query_all(w: &mut ErpWorld, plant: String) {
    let r = w
        .warehouses
        .query(&w.actor, &plant, QueryRequest::default())
        .await;
    crate::steps::common::set_query_result(w, r, |d| d.warehouse_code.clone());
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以下列條件查詢倉庫:$"#)]
async fn query(w: &mut ErpWorld, plant: String, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.warehouses.query(&w.actor, &plant, req).await;
    crate::steps::common::set_query_result(w, r, |d| d.warehouse_code.clone());
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 列印倉庫$"#)]
async fn report(w: &mut ErpWorld, plant: String) {
    let r = w
        .warehouses
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

#[then(regex = r#"^營運中心 "([^"]*)" 的倉庫 "([^"]*)" 應為:$"#)]
async fn is(w: &mut ErpWorld, plant: String, code: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let d = wh(w, &plant, &code).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "名稱" => d.name.clone().unwrap_or_default(),
            "倉儲類別" => d.storage_class.clone(),
            "可用倉儲" => yes_no(d.usable).to_owned(),
            "MRP 可用" => yes_no(d.mrp_usable).to_owned(),
            "保稅" => yes_no(d.bonded).to_owned(),
            "發料優先順序" => d.issue_priority.to_string(),
            "出貨優先順序" => d.ship_priority.to_string(),
            "資料有效" => yes_no(d.active).to_owned(),
            "已確認" => yes_no(d.confirmed).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(regex = r#"^營運中心 "([^"]*)" 的倉庫 "([^"]*)" 應沒有儲位$"#)]
async fn no_locations(w: &mut ErpWorld, plant: String, code: String) {
    assert!(wh(w, &plant, &code).await.locations.is_empty());
}

#[then(regex = r#"^營運中心 "([^"]*)" 倉庫 "([^"]*)" 的儲位應為:$"#)]
async fn locations_are(w: &mut ErpWorld, plant: String, code: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let got: Vec<Vec<String>> = wh(w, &plant, &code)
        .await
        .locations
        .into_iter()
        .map(|l| {
            vec![
                l.location_code,
                l.name.unwrap_or_default(),
                l.storage_class,
                yes_no(l.usable).to_owned(),
                yes_no(l.mrp_usable).to_owned(),
                yes_no(l.bonded).to_owned(),
                l.account_method,
            ]
        })
        .collect();
    assert_eq!(got, rows(step));
}

#[then(regex = r#"^營運中心 "([^"]*)" 的倉庫 "([^"]*)" 應不存在$"#)]
async fn gone(w: &mut ErpWorld, plant: String, code: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let r = w.warehouses.get(&w.actor, &plant, &code).await;
    assert_eq!(r.err().map(|e| e.code()), Some("WAREHOUSE_NOT_FOUND"));
}

#[then(regex = r#"^倉庫清單順序應為 "([^"]*)"$"#)]
async fn order_is(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^營運中心 "([^"]*)" 倉庫 "([^"]*)" 的異動記錄應有 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, plant: String, code: String, expected: String) {
    let h = w
        .warehouses
        .history(&w.actor, &plant, &code, None)
        .await
        .expect("異動記錄");
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}
