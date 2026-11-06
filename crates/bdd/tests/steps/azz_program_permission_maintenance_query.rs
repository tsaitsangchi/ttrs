//! Program Permission Maintenance QBE 查詢 — specs/features/azz/program_permission_maintenance_query.feature

use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{ProgramPermissionInput, QueryRequest};

use crate::{
    ErpWorld,
    steps::common::{query_request, rows, set_query_result},
};

/// 畫面欄位名稱 → 查詢欄位鍵值（單頭 class_code；單身與 ProgramPermissionDto 欄位名稱相同）
fn field_key(label: &str) -> &'static str {
    match label {
        "權限類別" => "class_code",
        "程式代號" => "program_no",
        "允許執行功能" => "actions",
        "他人資料權限" => "other_user_data",
        "他部門資料權限" => "other_dept_data",
        "單身權限" => "detail_rows",
        "程式名稱" => "program_name",
        other => panic!("未知欄位：{other}"),
    }
}

async fn run(w: &mut ErpWorld, req: QueryRequest) {
    w.query_conditions = req.conditions.clone();
    let result = w.permissions.query(&w.actor, req).await;
    set_query_result(w, result, |c| c.class_code.clone());
}

/// 權限類別 | 程式代號 | 允許執行功能 | 他人資料權限 | 他部門資料權限 | 單身權限；
/// 已有的權限（新增程式時自動授權）改為表中的值
#[given(expr = "已有下列程式執行權限:")]
async fn existing_permissions(w: &mut ErpWorld, step: &Step) {
    for r in rows(step) {
        let req = ProgramPermissionInput {
            version: None,
            program_no: r[1].clone(),
            actions: Some(
                r[2].split(',')
                    .map(|a| a.trim().to_owned())
                    .filter(|a| !a.is_empty())
                    .collect(),
            ),
            other_user_data: Some(r[3].clone()),
            other_dept_data: Some(r[4].clone()),
            detail_rows: Some(r[5].clone()),
        };
        if w.permissions
            .get(&w.actor, &r[0], &r[1], "zh-TW")
            .await
            .is_ok()
        {
            w.permissions
                .update(&w.actor, &r[0], &r[1], req)
                .await
                .unwrap();
        } else {
            w.permissions.grant(&w.actor, &r[0], req).await.unwrap();
        }
    }
}

#[when(expr = "使用者以下列條件查詢程式執行權限:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    run(w, query_request(step, field_key, None)).await;
}

#[when(expr = "使用者以下列條件查詢程式執行權限，每頁 {int} 筆，取第 {int} 頁:")]
async fn query_paged(w: &mut ErpWorld, size: u32, page: u32, step: &Step) {
    run(w, query_request(step, field_key, Some((size, page)))).await;
}

/// YQ3：以最近一次查詢的條件取該權限類別的程式
#[then(expr = "權限類別 {string} 符合條件的程式應為 {string}")]
async fn matching_programs(w: &mut ErpWorld, class: String, expected: String) {
    let req = QueryRequest {
        conditions: w.query_conditions.clone(),
        ..Default::default()
    };
    let rows = w
        .permissions
        .query_class(&w.actor, &class, req)
        .await
        .unwrap_or_else(|e| panic!("查詢權限類別 {class} 的程式失敗：{e}"));
    let actual: Vec<&str> = rows.iter().map(|p| p.program_no.as_str()).collect();
    assert_eq!(actual.join(","), expected);
}
