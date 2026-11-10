//! 料件、料件分群的列印與匯出 Excel（ADR 0013 SA3 通用清單報表）—
//! specs/features/aim/item_maintenance_report.feature、item_group_report.feature
//! 報表內容的檢查步驟沿用部門報表（aoo_department_report.rs）。

use cucumber::{then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::QbeCondition;

use crate::ErpWorld;

fn key(label: &str) -> &'static str {
    match label {
        "料件編號" => "item_no",
        "資料有效碼" => "active_code",
        "分群碼" => "group_code",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

fn cond(label: &str, expr: &str) -> QbeCondition {
    QbeCondition {
        field: key(label).to_owned(),
        expr: expr.to_owned(),
    }
}

async fn items(w: &mut ErpWorld, plant: &str, c: Vec<QbeCondition>, f: ReportFormat, lang: &str) {
    let lang = Lang::parse(lang).unwrap();
    let r = w.items.report_in(&w.actor, plant, c, f, &lang).await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

async fn groups(w: &mut ErpWorld, plant: &str, c: Vec<QbeCondition>, f: ReportFormat) {
    let lang = Lang::default_lang();
    let r = w.item_groups.report_in(&w.actor, plant, c, f, &lang).await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[when(regex = r#"^使用者以條件「(\S+) = (\S+)」列印營運中心 "(\S+)" 的料件$"#)]
async fn print_items(w: &mut ErpWorld, label: String, expr: String, plant: String) {
    items(
        w,
        &plant,
        vec![cond(&label, &expr)],
        ReportFormat::Pdf,
        "zh-TW",
    )
    .await;
}

#[when(regex = r#"^使用者以條件「(\S+) = (\S+)」與「(\S+) (\S+)」列印營運中心 "(\S+)" 的料件$"#)]
async fn print_items2(
    w: &mut ErpWorld,
    l1: String,
    e1: String,
    l2: String,
    e2: String,
    plant: String,
) {
    let c = vec![cond(&l1, &e1), cond(&l2, &e2)];
    items(w, &plant, c, ReportFormat::Pdf, "zh-TW").await;
}

#[when(expr = "使用者不輸入條件列印營運中心 {string} 的料件")]
async fn print_all_items(w: &mut ErpWorld, plant: String) {
    items(w, &plant, Vec::new(), ReportFormat::Pdf, "zh-TW").await;
}

#[when(expr = "使用者以語系 {string} 列印營運中心 {string} 的料件")]
async fn print_items_lang(w: &mut ErpWorld, lang: String, plant: String) {
    items(w, &plant, Vec::new(), ReportFormat::Pdf, &lang).await;
}

#[when(regex = r#"^使用者以條件「(\S+) = (\S+)」匯出營運中心 "(\S+)" 的料件為 Excel$"#)]
async fn export_items(w: &mut ErpWorld, label: String, expr: String, plant: String) {
    items(
        w,
        &plant,
        vec![cond(&label, &expr)],
        ReportFormat::Excel,
        "zh-TW",
    )
    .await;
}

#[when(regex = r#"^使用者以條件「(\S+) = (\S+)」列印營運中心 "(\S+)" 的料件分群$"#)]
async fn print_groups(w: &mut ErpWorld, label: String, expr: String, plant: String) {
    groups(w, &plant, vec![cond(&label, &expr)], ReportFormat::Pdf).await;
}

#[when(expr = "使用者不輸入條件匯出營運中心 {string} 的料件分群為 Excel")]
async fn export_groups(w: &mut ErpWorld, plant: String) {
    groups(w, &plant, Vec::new(), ReportFormat::Excel).await;
}

#[then(expr = "報表明細的料件編號應為 {string}")]
fn item_nos(w: &mut ErpWorld, expected: String) {
    let r = w.report.as_ref().expect("尚未產生報表");
    let nos: Vec<&str> = r.rows.iter().map(|x| x[0].as_str()).collect();
    assert_eq!(nos.join(","), expected);
}

#[then(expr = "報表第 {int} 筆的第 {int} 欄應為 {string}")]
fn cell(w: &mut ErpWorld, row: usize, col: usize, expected: String) {
    let r = w.report.as_ref().expect("尚未產生報表");
    assert_eq!(r.rows[row - 1][col - 1], expected);
}
