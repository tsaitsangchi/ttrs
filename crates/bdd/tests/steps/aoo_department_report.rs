//! aooi030 部門維護的列印與匯出 Excel — specs/features/aoo/department_report.feature（DR1–DR12，ADR 0011）

use std::io::Cursor;

use calamine::{Reader, Xlsx};
use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::{ReportData, ReportFormat};
use ttrs_shared::{DepartmentInput, ProgramInput, QbeCondition};

use crate::{
    ErpWorld,
    steps::{azz_program_maintenance::seed, common::rows},
};

/// 畫面欄位名稱 → 查詢欄位鍵值（department_report.feature 用到的）
fn field_key(label: &str) -> &'static str {
    match label {
        "部門編號" => "dept_code",
        "簡稱" => "name",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

async fn run_report(
    w: &mut ErpWorld,
    plant: &str,
    conditions: Vec<QbeCondition>,
    format: ReportFormat,
) {
    let result = w
        .departments
        .report(&w.actor, plant, conditions, format)
        .await;
    w.last = Some(result.map(|r| {
        w.report = Some(r);
    }));
}

fn condition(label: &str, expr: &str) -> Vec<QbeCondition> {
    vec![QbeCondition {
        field: field_key(label).to_owned(),
        expr: expr.to_owned(),
    }]
}

fn report(w: &ErpWorld) -> &ReportData {
    w.report.as_ref().expect("尚未產生報表")
}

// ----- 假設 -----

#[given(expr = "營運中心 {string} 已有部門:")]
async fn plant_has_departments(w: &mut ErpWorld, plant: String, step: &Step) {
    for r in rows(step) {
        let req = DepartmentInput {
            version: None,
            dept_code: r[0].clone(),
            name: Some(r[1].clone()),
            full_name: Some(r[2].clone()).filter(|s| !s.is_empty()),
        };
        w.departments
            .create(&w.actor, &plant, req)
            .await
            .unwrap_or_else(|e| panic!("建立部門 {} 失敗：{e}", r[0]));
    }
}

#[given(expr = "部門 {string} 已停用")]
async fn department_inactive(w: &mut ErpWorld, code: String) {
    w.departments
        .deactivate(&w.actor, "TW01", &code, None)
        .await
        .unwrap_or_else(|e| panic!("停用部門 {code} 失敗：{e}"));
}

/// 程式資料的 zz05 列印選擇條件；程式尚未登錄時先登錄
#[given(expr = "程式 {string} 的列印選擇條件為 {string}")]
async fn print_conditions(w: &mut ErpWorld, program: String, flag: String) {
    let input = match w.programs.get(&w.actor, &program).await {
        Ok(p) => p.to_input(),
        Err(_) => {
            let sql = "INSERT INTO module (module_code, name) VALUES ('AOO', 'AOO') ON CONFLICT DO NOTHING";
            seed(w, |s| s.add_module("AOO"), sql, &[]).await;
            let req = ProgramInput {
                program_type: Some("I".into()),
                program_no: program.clone(),
                module_code: "AOO".into(),
                ..Default::default()
            };
            w.programs
                .create(&w.actor, req)
                .await
                .unwrap_or_else(|e| panic!("登錄程式 {program} 失敗：{e}"))
                .to_input()
        }
    };
    let req = ProgramInput {
        print_conditions: flag == "Y",
        ..input
    };
    w.programs
        .update(&w.actor, &program, req)
        .await
        .unwrap_or_else(|e| panic!("設定 {program} 列印選擇條件失敗：{e}"));
}

#[given(expr = "報表同步產生的上限為 {int} 筆")]
fn report_limit(w: &mut ErpWorld, limit: u64) {
    w.departments = w.departments.clone().with_report_limit(limit);
}

// ----- 當 -----

#[when(regex = r#"^使用者以條件「(\S+) = (\S+)」列印營運中心 "(\S+)" 的部門$"#)]
async fn print_with(w: &mut ErpWorld, label: String, expr: String, plant: String) {
    run_report(w, &plant, condition(&label, &expr), ReportFormat::Pdf).await;
}

#[when(regex = r#"^使用者以條件「(\S+) = (\S+)」列印營運中心 "(\S+)" 的部門並輸出為 PDF$"#)]
async fn print_pdf_with(w: &mut ErpWorld, label: String, expr: String, plant: String) {
    run_report(w, &plant, condition(&label, &expr), ReportFormat::Pdf).await;
}

#[when(expr = "使用者不輸入條件列印營運中心 {string} 的部門")]
async fn print_all(w: &mut ErpWorld, plant: String) {
    run_report(w, &plant, Vec::new(), ReportFormat::Pdf).await;
}

#[when(regex = r#"^使用者以條件「(\S+) = (\S+)」匯出營運中心 "(\S+)" 的部門為 Excel$"#)]
async fn export_with(w: &mut ErpWorld, label: String, expr: String, plant: String) {
    run_report(w, &plant, condition(&label, &expr), ReportFormat::Excel).await;
}

// ----- 那麼 -----

/// 依表格標題取報表對應欄位比對
#[then("報表明細應為:")]
fn report_rows(w: &mut ErpWorld, step: &Step) {
    let r = report(w);
    let table = step.table.as_ref().expect("需要表格");
    let headers = &table.rows[0];
    let idx: Vec<usize> = headers
        .iter()
        .map(|h| {
            r.columns
                .iter()
                .position(|c| c == h)
                .unwrap_or_else(|| panic!("報表沒有欄位 {h}：{:?}", r.columns))
        })
        .collect();
    let actual: Vec<Vec<String>> = r
        .rows
        .iter()
        .map(|row| idx.iter().map(|i| row[*i].clone()).collect())
        .collect();
    assert_eq!(actual, rows(step));
}

#[then(expr = "報表明細的部門編號應為 {string}")]
fn report_codes(w: &mut ErpWorld, expected: String) {
    let codes: Vec<&str> = report(w).rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(codes.join(","), expected);
}

#[then("報表應可輸出為 PDF")]
fn pdf_ok(w: &mut ErpWorld) {
    let pdf = ttrs_infra::report::render_pdf(report(w)).expect("產生 PDF");
    assert!(pdf.starts_with(b"%PDF"), "應為 PDF");
}

#[then(expr = "報表的查詢條件說明應為 {string}")]
fn condition_text(w: &mut ErpWorld, expected: String) {
    assert_eq!(report(w).condition.clone().unwrap_or_default(), expected);
}

#[then(regex = r#"^報表表頭應有營運中心 "(\S+)"、報表名稱「(\S+)」、列印人 "(\S+)" 與列印時間$"#)]
fn header(w: &mut ErpWorld, plant: String, title: String, user: String) {
    let r = report(w);
    assert_eq!(r.plant, plant);
    assert_eq!(r.title, title);
    assert_eq!(r.printed_by, user);
    let src = ttrs_infra::report::typst_source(r);
    assert!(src.contains(&r.printed_at), "表頭應有列印時間");
}

#[then("報表應印出第幾頁／共幾頁")]
fn page_numbers(w: &mut ErpWorld) {
    let src = ttrs_infra::report::typst_source(report(w));
    assert!(
        src.contains("counter(page).display") && src.contains("counter(page).final"),
        "頁尾應有第幾頁／共幾頁"
    );
}

#[then(expr = "Excel 應有 {int} 筆明細，欄位與報表相同")]
fn excel_rows(w: &mut ErpWorld, n: usize) {
    let r = report(w);
    let bytes = ttrs_infra::report::render_xlsx(r).expect("產生 Excel");
    let mut book: Xlsx<_> = Xlsx::new(Cursor::new(bytes)).expect("讀取 Excel");
    let sheet = book.worksheet_range_at(0).expect("工作表").expect("工作表");
    let mut it = sheet.rows();
    let header: Vec<String> = it
        .next()
        .expect("標題列")
        .iter()
        .map(|c| c.to_string())
        .collect();
    assert_eq!(header, r.columns);
    assert_eq!(it.count(), n);
}

#[then(regex = r#"^稽核紀錄應有一筆程式 "(\S+)"、動作 "(\S+)"、筆數 (\d+)$"#)]
async fn audit(w: &mut ErpWorld, program: String, action: String, count: u64) {
    let entries = w.programs.audit_entries(&w.actor, "TW01").await.unwrap();
    let want = format!("筆數={count}");
    assert!(
        entries.iter().any(|e| e.program == program
            && e.action == action
            && e.detail.as_deref().is_some_and(|d| d.contains(&want))),
        "應有稽核紀錄：{entries:?}"
    );
}
