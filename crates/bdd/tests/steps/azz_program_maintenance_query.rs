//! Program Maintenance QBE 查詢 — specs/features/azz/program_maintenance_query.feature

use cucumber::{gherkin::Step, given, when};
use ttrs_shared::{ProgramInput, QbeCondition, QueryRequest, ReportTypeDto};

use crate::{
    ErpWorld,
    steps::common::{query_request, rows, set_query_result},
};

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

async fn run_query(w: &mut ErpWorld, req: QueryRequest) {
    let result = w.programs.query(req).await;
    set_query_result(w, result, |p| p.program_no.clone());
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
    let req = query_request(step, field_key, None);
    run_query(w, req).await;
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
            lang: None,
        },
    )
    .await;
}
