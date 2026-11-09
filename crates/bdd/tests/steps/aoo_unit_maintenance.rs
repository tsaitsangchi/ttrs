//! 單位維護 — specs/features/aoo/unit_maintenance.feature（UN1–UN7）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{UnitDto, UnitInput};

use crate::{ErpWorld, steps::common::rows};

async fn create(w: &mut ErpWorld, code: &str, name: &str, decimals: Option<i16>) {
    let req = UnitInput {
        unit_code: code.to_owned(),
        name: Some(name.to_owned()),
        decimals,
        version: None,
    };
    let r = w.units.create(&w.actor, req).await;
    w.last = Some(r.map(drop));
}

async fn unit(w: &ErpWorld, code: &str) -> UnitDto {
    w.units
        .get(&w.actor, code)
        .await
        .unwrap_or_else(|e| panic!("查詢單位 {code} 失敗：{e}"))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

#[given(expr = "已有單位 {string}")]
async fn has_unit(w: &mut ErpWorld, code: String) {
    create(w, &code, &code, None).await;
    if let Some(Err(e)) = &w.last {
        panic!("建立單位 {code} 失敗：{e}");
    }
}

#[when(expr = "使用者新增單位 {string}，說明 {string}")]
async fn add(w: &mut ErpWorld, code: String, name: String) {
    create(w, &code, &name, None).await;
}

#[when(expr = "使用者新增單位 {string}，說明 {string}，小數位數 {int}")]
async fn add_with_decimals(w: &mut ErpWorld, code: String, name: String, decimals: i16) {
    create(w, &code, &name, Some(decimals)).await;
}

#[when(expr = "使用者停用單位 {string}")]
async fn deactivate(w: &mut ErpWorld, code: String) {
    let r = w.units.set_active(&w.actor, &code, false, None).await;
    w.last = Some(r.map(drop));
}

#[then(expr = "單位 {string} 應為:")]
async fn unit_is(w: &mut ErpWorld, code: String, step: &Step) {
    let u = unit(w, &code).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "說明" => u.name.clone().unwrap_or_default(),
            "小數位數" => u.decimals.to_string(),
            "資料有效" => yes_no(u.active).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(expr = "單位 {string} 的小數位數應為 {int}")]
async fn decimals_are(w: &mut ErpWorld, code: String, decimals: i16) {
    assert_eq!(unit(w, &code).await.decimals, decimals);
}

#[then(expr = "單位 {string} 的資料有效應為 {string}")]
async fn active_is(w: &mut ErpWorld, code: String, expected: String) {
    assert_eq!(yes_no(unit(w, &code).await.active), expected);
}
