//! 料件分群確認與取消確認 — specs/features/aim/item_group_confirm.feature（ADR 0013 SA9）

use cucumber::{gherkin::Step, given, then, when};

use crate::{ErpWorld, steps::common::rows};

const PLANT: &str = "TW01";

async fn group(w: &ErpWorld, code: &str) -> ttrs_shared::ItemGroupDto {
    w.item_groups
        .get(&w.actor, PLANT, code)
        .await
        .unwrap_or_else(|e| panic!("查詢料件分群 {code} 失敗：{e}"))
}

#[given(expr = "營運中心 {string} 料件分群 {string} 已確認")]
async fn confirmed(w: &mut ErpWorld, plant: String, code: String) {
    w.item_groups
        .confirm(&w.actor, &plant, &code, true, None)
        .await
        .unwrap_or_else(|e| panic!("確認料件分群 {code} 失敗：{e}"));
}

#[when(expr = "使用者確認營運中心 {string} 料件分群 {string}")]
async fn confirm(w: &mut ErpWorld, plant: String, code: String) {
    let r = w
        .item_groups
        .confirm(&w.actor, &plant, &code, true, None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者取消確認營運中心 {string} 料件分群 {string}")]
async fn unconfirm(w: &mut ErpWorld, plant: String, code: String) {
    let r = w
        .item_groups
        .confirm(&w.actor, &plant, &code, false, None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者刪除營運中心 {string} 料件分群 {string}")]
async fn delete(w: &mut ErpWorld, plant: String, code: String) {
    let r = w.item_groups.delete(&w.actor, &plant, &code, None).await;
    w.last = Some(r);
}

#[when(expr = "使用者將營運中心 {string} 料件分群 {string} 的說明改為 {string}")]
async fn rename(w: &mut ErpWorld, plant: String, code: String, desc: String) {
    let g = group(w, &code).await;
    let input = ttrs_shared::ItemGroupInput {
        description: Some(desc),
        version: None,
        ..g.fields
    };
    let r = w.item_groups.update(&w.actor, &plant, &code, input).await;
    w.last = Some(r.map(drop));
}

#[then(expr = "料件分群 {string} 的確認狀態應為:")]
async fn state_is(w: &mut ErpWorld, code: String, step: &Step) {
    let g = group(w, &code).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "已確認" => if g.confirmed { "是" } else { "否" }.to_owned(),
            "確認人" => g.confirmed_by.clone().unwrap_or_default(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(expr = "料件分群 {string} 應有確認時間")]
async fn has_time(w: &mut ErpWorld, code: String) {
    assert!(group(w, &code).await.confirmed_at.is_some(), "應有確認時間");
}
