//! 料件分群維護 — specs/features/aim/item_group.feature（IG1–IG12）

use cucumber::{gherkin::Step, given, then, when};
use rust_decimal::Decimal;
use ttrs_shared::{ItemGroupDto, ItemGroupInput};

use crate::{ErpWorld, steps::common::rows};

const PLANT: &str = "TW01";

fn input(code: &str) -> ItemGroupInput {
    ItemGroupInput {
        group_code: code.to_owned(),
        ..ItemGroupInput::default()
    }
}

async fn create(w: &mut ErpWorld, plant: &str, req: ItemGroupInput) {
    let r = w.item_groups.create(&w.actor, plant, req).await;
    w.last = Some(r.map(drop));
}

async fn group(w: &ErpWorld, code: &str) -> ItemGroupDto {
    w.item_groups
        .get(&w.actor, PLANT, code)
        .await
        .unwrap_or_else(|e| panic!("查詢料件分群 {code} 失敗：{e}"))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

#[given(expr = "營運中心 {string} 已有料件分群 {string}，說明 {string}")]
async fn has_group(w: &mut ErpWorld, plant: String, code: String, desc: String) {
    let req = ItemGroupInput {
        description: Some(desc),
        ..input(&code)
    };
    create(w, &plant, req).await;
    if let Some(Err(e)) = &w.last {
        panic!("建立料件分群 {code} 失敗：{e}");
    }
}

#[when(
    expr = "使用者在營運中心 {string} 新增料件分群 {string}，說明 {string}，來源碼 {string}，庫存單位 {string}"
)]
async fn add(
    w: &mut ErpWorld,
    plant: String,
    code: String,
    desc: String,
    source: String,
    unit: String,
) {
    let req = ItemGroupInput {
        description: Some(desc),
        source_code: Some(source),
        stock_unit: Some(unit),
        ..input(&code)
    };
    create(w, &plant, req).await;
}

#[when(expr = "使用者在營運中心 {string} 新增料件分群 {string}，其他分群碼一 {string}")]
async fn add_other_group(w: &mut ErpWorld, plant: String, code: String, other: String) {
    let req = ItemGroupInput {
        other_group1: Some(other),
        ..input(&code)
    };
    create(w, &plant, req).await;
}

#[when(expr = "使用者在營運中心 {string} 新增料件分群 {string}，安全庫存量 {int}")]
async fn add_safety(w: &mut ErpWorld, plant: String, code: String, qty: i64) {
    let req = ItemGroupInput {
        safety_stock: Some(Decimal::from(qty)),
        ..input(&code)
    };
    create(w, &plant, req).await;
}

#[when(expr = "使用者將營運中心 {string} 料件分群 {string} 複製為 {string}")]
async fn copy(w: &mut ErpWorld, plant: String, from: String, to: String) {
    let r = w.item_groups.copy(&w.actor, &plant, &from, &to).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者停用營運中心 {string} 料件分群 {string}")]
async fn deactivate(w: &mut ErpWorld, plant: String, code: String) {
    let r = w
        .item_groups
        .set_active(&w.actor, &plant, &code, false, None)
        .await;
    w.last = Some(r.map(drop));
}

#[then(expr = "料件分群 {string} 應為:")]
async fn group_is(w: &mut ErpWorld, code: String, step: &Step) {
    let g = group(w, &code).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "說明" => g.fields.description.clone().unwrap_or_default(),
            "來源碼" => g.fields.source_code.clone().unwrap_or_default(),
            "庫存單位" => g.fields.stock_unit.clone().unwrap_or_default(),
            "資料有效" => yes_no(g.active).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(expr = "料件分群 {string} 的說明應為 {string}")]
async fn desc_is(w: &mut ErpWorld, code: String, desc: String) {
    assert_eq!(group(w, &code).await.fields.description, Some(desc));
}

#[then(expr = "料件分群 {string} 的資料有效應為 {string}")]
async fn active_is(w: &mut ErpWorld, code: String, expected: String) {
    assert_eq!(yes_no(group(w, &code).await.active), expected);
}
