//! 庫存參數 — specs/features/aim/inventory_parameters.feature（IP1–IP10）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{InventoryParametersDto, InventoryParametersInput};

use crate::{ErpWorld, steps::common::rows};

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn date_basis(code: &str) -> &'static str {
    match code {
        "1" => "單據日期",
        "2" => "產生日期",
        other => panic!("未知帳務基礎日期：{other}"),
    }
}

/// 畫面欄位名稱 → 目前的值（與規格表格的寫法相同）
fn value_of(d: &InventoryParametersDto, label: &str) -> String {
    let v = &d.settings;
    match label {
        "料件存放位置管制" => v.storage_control.clone(),
        "快速移動週轉率" => v.fast_turnover.normalize().to_string(),
        "平速移動週轉率" => v.average_turnover.normalize().to_string(),
        "帳務基礎日期" => date_basis(&v.posting_date_basis).to_owned(),
        "雜收時可新增儲位" => yes_no(v.misc_receipt_new_location).to_owned(),
        "開窗查詢庫存明細" => yes_no(v.stock_query_window).to_owned(),
        "兩階段營運中心調撥單號相同" => yes_no(v.plant_transfer_same_no).to_owned(),
        "兩階段倉庫調撥單號相同" => yes_no(v.warehouse_transfer_same_no).to_owned(),
        "雜發單輸入部門" => yes_no(v.misc_issue_dept).to_owned(),
        "雜收單輸入部門" => yes_no(v.misc_receipt_dept).to_owned(),
        "可不經倉儲維護新增倉儲" => yes_no(v.add_storage_freely).to_owned(),
        other => panic!("未知欄位：{other}"),
    }
}

fn set(i: &mut InventoryParametersInput, label: &str, value: &str) {
    let flag = value == "是";
    let num = || value.parse().unwrap_or_else(|_| panic!("數字：{value}"));
    match label {
        "料件存放位置管制" => i.storage_control = value.to_owned(),
        "快速移動週轉率" => i.fast_turnover = num(),
        "平速移動週轉率" => i.average_turnover = num(),
        "帳務基礎日期" => {
            i.posting_date_basis = match value {
                "單據日期" => "1",
                "產生日期" => "2",
                other => other,
            }
            .to_owned()
        }
        "雜收時可新增儲位" => i.misc_receipt_new_location = flag,
        "開窗查詢庫存明細" => i.stock_query_window = flag,
        "兩階段營運中心調撥單號相同" => i.plant_transfer_same_no = flag,
        "兩階段倉庫調撥單號相同" => i.warehouse_transfer_same_no = flag,
        "雜發單輸入部門" => i.misc_issue_dept = flag,
        "雜收單輸入部門" => i.misc_receipt_dept = flag,
        "可不經倉儲維護新增倉儲" => i.add_storage_freely = flag,
        other => panic!("未知欄位：{other}"),
    }
}

async fn current(w: &ErpWorld, plant: &str) -> InventoryParametersDto {
    w.inventory_parameters
        .get(&w.actor, plant)
        .await
        .unwrap_or_else(|e| panic!("查看庫存參數失敗：{e}"))
}

async fn modify(w: &mut ErpWorld, plant: &str, stale: bool, step: &Step) {
    let mut input = current(w, plant).await.to_input();
    if stale {
        input.version = w.inventory_param_version;
    }
    for r in rows(step) {
        set(&mut input, &r[0], &r[1]);
    }
    let r = w.inventory_parameters.update(&w.actor, plant, input).await;
    w.last = Some(r.map(drop));
}

#[given(regex = r#"^另一位使用者已修改營運中心 "([^"]*)" 的庫存參數$"#)]
async fn other_user_modified(w: &mut ErpWorld, plant: String) {
    let mut input = current(w, &plant).await.to_input();
    w.inventory_param_version = input.version;
    input.misc_issue_dept = !input.misc_issue_dept;
    w.inventory_parameters
        .update(&w.actor, &plant, input)
        .await
        .unwrap_or_else(|e| panic!("修改庫存參數失敗：{e}"));
}

#[when(regex = r#"^使用者查看營運中心 "([^"]*)" 的庫存參數$"#)]
async fn view(w: &mut ErpWorld, plant: String) {
    let r = w.inventory_parameters.get(&w.actor, &plant).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(以舊版本)?修改營運中心 "([^"]*)" 的庫存參數:$"#)]
async fn update(w: &mut ErpWorld, stale: String, plant: String, step: &Step) {
    modify(w, &plant, !stale.is_empty(), step).await;
}

#[when(regex = r#"^使用者列印營運中心 "([^"]*)" 的庫存參數$"#)]
async fn report(w: &mut ErpWorld, plant: String) {
    let r = w
        .inventory_parameters
        .report(&w.actor, &plant, ReportFormat::Pdf, &Lang::default_lang())
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[then(regex = r#"^營運中心 "([^"]*)" 的庫存參數應為:$"#)]
async fn is(w: &mut ErpWorld, plant: String, step: &Step) {
    let d = current(w, &plant).await;
    for r in rows(step) {
        assert_eq!(value_of(&d, &r[0]), r[1], "欄位 {}", r[0]);
    }
}

#[then(regex = r#"^營運中心 "([^"]*)" 庫存參數的最後修改者應為 "([^"]*)"$"#)]
async fn updated_by(w: &mut ErpWorld, plant: String, user: String) {
    assert_eq!(
        current(w, &plant).await.updated_by.as_deref(),
        Some(user.as_str())
    );
}

#[then(regex = r#"^營運中心 "([^"]*)" 庫存參數的異動記錄應有 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, plant: String, expected: String) {
    let h = w
        .inventory_parameters
        .history(&w.actor, &plant, None)
        .await
        .expect("異動記錄");
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}
