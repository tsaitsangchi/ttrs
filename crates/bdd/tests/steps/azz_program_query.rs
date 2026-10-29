//! p_zz QBE 查詢 — specs/features/azz/program_query.feature

use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{ProgramInput, QbeCondition, QueryRequest, ReportTypeDto};

use crate::ErpWorld;

/// 畫面欄位名稱 → 查詢欄位鍵值（與 ProgramDto 欄位名稱相同）
fn field_key(label: &str) -> &'static str {
    match label {
        "程式代號" => "program_no",
        "模組" => "module_code",
        "程式名稱" => "name",
        "報表類型" => "report_type",
        "報表寬度" => "report_width",
        "最後修改日期" => "updated_on",
        "可執行功能" => "default_actions",
        other => panic!("未知欄位：{other}"),
    }
}

/// 讀取表格（略過標題列）
fn rows(step: &Step) -> Vec<Vec<String>> {
    let table = step.table.as_ref().expect("此步驟需要表格");
    table.rows.iter().skip(1).cloned().collect()
}

async fn run_query(w: &mut ErpWorld, req: QueryRequest) {
    match w.programs.query(req).await {
        Ok(page) => {
            w.program_page = Some(page);
            w.last = Some(Ok(()));
        }
        Err(e) => {
            w.program_page = None;
            w.last = Some(Err(e));
        }
    }
}

fn condition(label: &str, expr: &str) -> QbeCondition {
    QbeCondition {
        field: field_key(label).to_owned(),
        expr: expr.to_owned(),
    }
}

#[given(expr = "已有下列程式:")]
async fn existing_programs(w: &mut ErpWorld, step: &Step) {
    for r in rows(step) {
        let report_type = ReportTypeDto::ALL
            .into_iter()
            .find(|t| t.label() == r[3])
            .unwrap_or_else(|| panic!("未知報表類型：{}", r[3]));
        let req = ProgramInput {
            program_no: r[0].clone(),
            module_code: r[1].clone(),
            name: Some(r[2].clone()),
            report_type: Some(report_type),
            report_width: (!r[4].is_empty()).then(|| r[4].parse().unwrap()),
            ..Default::default()
        };
        w.programs.create(&w.actor, req).await.unwrap();
    }
}

#[when(expr = "使用者以下列條件查詢程式:")]
async fn query_by_table(w: &mut ErpWorld, step: &Step) {
    let conditions = rows(step).iter().map(|r| condition(&r[0], &r[1])).collect();
    run_query(
        w,
        QueryRequest {
            conditions,
            ..Default::default()
        },
    )
    .await;
}

#[when(expr = "使用者以程式代號 {string} 查詢程式")]
async fn query_by_no(w: &mut ErpWorld, expr: String) {
    run_query(
        w,
        QueryRequest {
            conditions: vec![condition("程式代號", &expr)],
            ..Default::default()
        },
    )
    .await;
}

#[when(expr = "使用者以程式代號 {string} 查詢程式，每頁 {int} 筆，取第 {int} 頁")]
async fn query_by_no_paged(w: &mut ErpWorld, expr: String, size: u32, page: u32) {
    run_query(
        w,
        QueryRequest {
            conditions: vec![condition("程式代號", &expr)],
            page: Some(page),
            page_size: Some(size),
        },
    )
    .await;
}

fn result_nos(w: &ErpWorld) -> String {
    let page = w.program_page.as_ref().expect("沒有查詢結果（查詢失敗？）");
    page.items
        .iter()
        .map(|p| p.program_no.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

#[then(expr = "查詢結果應為 {string}")]
fn result_is(w: &mut ErpWorld, expected: String) {
    assert_eq!(result_nos(w), expected);
}

#[then(expr = "查詢結果應為 {string} 共 {int} 筆")]
fn result_is_with_total(w: &mut ErpWorld, expected: String, total: u64) {
    assert_eq!(result_nos(w), expected);
    assert_eq!(w.program_page.as_ref().unwrap().total, total, "總筆數");
}

#[then(expr = "查詢結果應有 {int} 筆")]
fn result_count(w: &mut ErpWorld, total: u64) {
    let page = w.program_page.as_ref().expect("沒有查詢結果（查詢失敗？）");
    assert_eq!(page.total, total);
}
