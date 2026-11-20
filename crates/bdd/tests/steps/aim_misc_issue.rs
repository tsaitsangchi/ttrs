//! 雜項發料 — specs/features/aim/misc_issue.feature（MI-1～MI-19）

use cucumber::{gherkin::Step, given, then, when};
use rust_decimal::Decimal;
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{
    InventoryTxnCopyInput, InventoryTxnDto, InventoryTxnInput, InventoryTxnLineInput,
};

use crate::{ErpWorld, steps::common::rows};

fn date(s: &str) -> chrono::NaiveDate {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("日期格式 YYYY-MM-DD")
}

fn status_name(code: &str) -> &'static str {
    match code {
        "N" => "開立",
        "Y" => "已確認",
        "X" => "已作廢",
        _ => "?",
    }
}

/// 單頭的預設輸入：備註必填（MT14），測試一律帶「測試」
fn base_input(doc_no: &str) -> InventoryTxnInput {
    InventoryTxnInput {
        doc_no: doc_no.to_owned(),
        memo: Some("測試".into()),
        ..InventoryTxnInput::default()
    }
}

fn line_inputs(step: &Step) -> Vec<InventoryTxnLineInput> {
    let table = step.table.as_ref().expect("此步驟需要單身表格");
    let header = &table.rows[0];
    rows(step)
        .into_iter()
        .map(|r| {
            let get = |name: &str| header.iter().position(|h| h == name).map(|i| r[i].clone());
            InventoryTxnLineInput {
                line_no: get("項次").map_or(0, |v| v.parse().expect("項次")),
                item_no: get("料件").unwrap_or_default(),
                warehouse: get("倉庫"),
                location: get("儲位"),
                lot: get("批號"),
                qty: get("數量").map_or(Decimal::ZERO, |v| v.parse().expect("數量")),
                reason: get("理由碼").filter(|v| !v.is_empty()),
            }
        })
        .collect()
}

async fn create(w: &mut ErpWorld, plant: &str, input: InventoryTxnInput) {
    let r = w.misc_issues.create(&w.actor, plant, input).await;
    w.misc_no = r.as_ref().ok().map(|d| d.doc_no.clone());
    w.last = Some(r.map(drop));
}

async fn doc(w: &ErpWorld, plant: &str, no: &str) -> InventoryTxnDto {
    w.misc_issues
        .get(&w.actor, plant, no)
        .await
        .unwrap_or_else(|e| panic!("查詢雜項發料單 {no} 失敗：{e}"))
}

fn to_input(d: &InventoryTxnDto) -> InventoryTxnInput {
    InventoryTxnInput {
        doc_no: d.doc_no.clone(),
        input_date: Some(d.input_date),
        posting_date: Some(d.posting_date),
        dept: Some(d.dept.clone().unwrap_or_default()),
        applicant: Some(d.applicant.clone().unwrap_or_default()),
        memo: d.memo.clone(),
        lines: Some(
            d.lines
                .iter()
                .map(|l| InventoryTxnLineInput {
                    line_no: l.line_no,
                    item_no: l.item_no.clone(),
                    warehouse: Some(l.warehouse.clone()),
                    location: Some(l.location.clone()),
                    lot: Some(l.lot.clone()),
                    qty: l.qty,
                    reason: l.reason.clone(),
                })
                .collect(),
        ),
        version: Some(d.version),
    }
}

fn field_key(label: &str) -> &'static str {
    match label {
        "單號" => "doc_no",
        "料件" => "item_no",
        "倉庫" => "warehouse",
        "狀態" => "status",
        "部門" => "dept",
        "申請人" => "applicant",
        other => panic!("未知欄位：{other}"),
    }
}

// ----- 假設 -----

#[given(
    regex = r#"^營運中心 "([^"]*)" 已有雜項發料單 "([^"]*)"，單身料件 "([^"]*)" 倉庫 "([^"]*)" 數量 (\d+)$"#
)]
async fn has_doc(w: &mut ErpWorld, plant: String, no: String, item: String, wh: String, qty: i64) {
    let input = InventoryTxnInput {
        input_date: Some(date("2026-11-20")),
        lines: Some(vec![InventoryTxnLineInput {
            item_no: item,
            warehouse: Some(wh),
            qty: Decimal::from(qty),
            ..InventoryTxnLineInput::default()
        }]),
        ..base_input(&no)
    };
    if let Err(e) = w.misc_issues.create(&w.actor, &plant, input).await {
        panic!("建立雜項發料單 {no} 失敗：{e}");
    }
}

#[given(regex = r#"^營運中心 "([^"]*)" 已有雜項發料單 "([^"]*)"，沒有單身$"#)]
async fn has_empty_doc(w: &mut ErpWorld, plant: String, no: String) {
    let input = InventoryTxnInput {
        input_date: Some(date("2026-11-20")),
        ..base_input(&no)
    };
    if let Err(e) = w.misc_issues.create(&w.actor, &plant, input).await {
        panic!("建立雜項發料單 {no} 失敗：{e}");
    }
}

// ----- 當 -----

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 以單別 "([^"]*)" 新增雜項發料單，輸入日 "([^"]*)"，單身:$"#
)]
async fn create_with_date_lines(
    w: &mut ErpWorld,
    plant: String,
    code: String,
    d: String,
    step: &Step,
) {
    let input = InventoryTxnInput {
        input_date: Some(date(&d)),
        lines: Some(line_inputs(step)),
        ..base_input(&code)
    };
    create(w, &plant, input).await;
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以單別 "([^"]*)" 新增雜項發料單，單身:$"#)]
async fn create_with_lines(w: &mut ErpWorld, plant: String, code: String, step: &Step) {
    let input = InventoryTxnInput {
        input_date: Some(date("2026-11-20")),
        lines: Some(line_inputs(step)),
        ..base_input(&code)
    };
    create(w, &plant, input).await;
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以單別 "([^"]*)" 新增雜項發料單，輸入日 "([^"]*)"$"#)]
async fn create_with_date(w: &mut ErpWorld, plant: String, code: String, d: String) {
    let input = InventoryTxnInput {
        input_date: Some(date(&d)),
        ..base_input(&code)
    };
    create(w, &plant, input).await;
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 以單別 "([^"]*)" 新增雜項發料單，(申請人|部門|備註) "([^"]*)"$"#
)]
async fn create_with_field(
    w: &mut ErpWorld,
    plant: String,
    code: String,
    field: String,
    value: String,
) {
    let mut input = InventoryTxnInput {
        input_date: Some(date("2026-11-20")),
        ..base_input(&code)
    };
    match field.as_str() {
        "申請人" => input.applicant = Some(value),
        "部門" => input.dept = Some(value),
        _ => input.memo = Some(value),
    }
    create(w, &plant, input).await;
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 新增雜項發料單 "([^"]*)"，輸入日 "([^"]*)"$"#)]
async fn create_manual(w: &mut ErpWorld, plant: String, no: String, d: String) {
    let input = InventoryTxnInput {
        input_date: Some(date(&d)),
        ..base_input(&no)
    };
    create(w, &plant, input).await;
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 把雜項發料單 "([^"]*)" 的備註改為 "([^"]*)"$"#)]
async fn change_memo(w: &mut ErpWorld, plant: String, no: String, memo: String) {
    let d = doc(w, &plant, &no).await;
    let mut input = to_input(&d);
    input.memo = Some(memo);
    let r = w.misc_issues.update(&w.actor, &plant, &no, input).await;
    w.last = Some(r.map(drop));
}

#[given(
    regex = r#"^使用者在營運中心 "([^"]*)" (確認|取消確認|作廢|取消作廢|刪除)雜項發料單 "([^"]*)"$"#
)]
#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" (確認|取消確認|作廢|取消作廢|刪除)雜項發料單 "([^"]*)"$"#
)]
async fn act(w: &mut ErpWorld, plant: String, op: String, no: String) {
    let v = w
        .misc_issues
        .get(&w.actor, &plant, &no)
        .await
        .ok()
        .map(|d| d.version);
    let s = &w.misc_issues;
    let r = match op.as_str() {
        "確認" => s.confirm(&w.actor, &plant, &no, true, v).await.map(drop),
        "取消確認" => s.confirm(&w.actor, &plant, &no, false, v).await.map(drop),
        "作廢" => s.set_void(&w.actor, &plant, &no, true, v).await.map(drop),
        "取消作廢" => s.set_void(&w.actor, &plant, &no, false, v).await.map(drop),
        _ => s.delete(&w.actor, &plant, &no, v).await,
    };
    w.last = Some(r);
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 以單別 "([^"]*)" 複製雜項發料單 "([^"]*)"，輸入日 "([^"]*)"，扣帳日 "([^"]*)"$"#
)]
async fn copy(
    w: &mut ErpWorld,
    plant: String,
    code: String,
    no: String,
    input_date: String,
    posting_date: String,
) {
    let input = InventoryTxnCopyInput {
        doc_no: code,
        input_date: Some(date(&input_date)),
        posting_date: Some(date(&posting_date)),
    };
    let r = w.misc_issues.copy(&w.actor, &plant, &no, input).await;
    w.misc_no = r.as_ref().ok().map(|d| d.doc_no.clone());
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以下列條件查詢雜項發料單:$"#)]
async fn query_by(w: &mut ErpWorld, plant: String, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.misc_issues.query(&w.actor, &plant, req).await;
    crate::steps::common::set_query_result(w, r, |d| d.doc_no.clone());
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 列印雜項發料單$"#)]
async fn report(w: &mut ErpWorld, plant: String) {
    let r = w
        .misc_issues
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

#[then(regex = r#"^應產生雜項發料單號 "([^"]*)"$"#)]
fn created_no(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("新增失敗 {}：{e}", e.code());
    }
    assert_eq!(w.misc_no.as_deref(), Some(expected.as_str()));
}

#[then(regex = r#"^營運中心 "([^"]*)" 的雜項發料單 "([^"]*)" 應為:$"#)]
async fn is(w: &mut ErpWorld, plant: String, no: String, step: &Step) {
    let d = doc(w, &plant, &no).await;
    for r in rows(step) {
        let got = match r[0].as_str() {
            "輸入日" => d.input_date.to_string(),
            "扣帳日" => d.posting_date.to_string(),
            "申請人" => d.applicant.clone().unwrap_or_default(),
            "部門" => d.dept.clone().unwrap_or_default(),
            "備註" => d.memo.clone().unwrap_or_default(),
            "狀態" => status_name(&d.status).into(),
            "過帳" => if d.posted { "是" } else { "否" }.into(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(got, r[1], "欄位 {}", r[0]);
    }
}

#[then(regex = r#"^營運中心 "([^"]*)" 的雜項發料單 "([^"]*)" 單身應為:$"#)]
async fn lines_are(w: &mut ErpWorld, plant: String, no: String, step: &Step) {
    let d = doc(w, &plant, &no).await;
    let table = step.table.as_ref().expect("表格");
    let header = &table.rows[0];
    let expected = rows(step);
    assert_eq!(d.lines.len(), expected.len(), "單身筆數");
    for (l, r) in d.lines.iter().zip(expected) {
        for (i, h) in header.iter().enumerate() {
            let got = match h.as_str() {
                "項次" => l.line_no.to_string(),
                "料件" => l.item_no.clone(),
                "品名" => l.item_name.clone().unwrap_or_default(),
                "倉庫" => l.warehouse.clone(),
                "儲位" => l.location.clone(),
                "批號" => l.lot.clone(),
                "單位" => l.unit.clone(),
                "數量" => l.qty.normalize().to_string(),
                "理由碼" => l.reason.clone().unwrap_or_default(),
                other => panic!("未知欄位：{other}"),
            };
            assert_eq!(got, r[i], "第 {} 項 {h}", l.line_no);
        }
    }
}

#[then(regex = r#"^營運中心 "([^"]*)" 的雜項發料單 "([^"]*)" 單身應有 (\d+) 筆$"#)]
async fn line_count(w: &mut ErpWorld, plant: String, no: String, n: usize) {
    assert_eq!(doc(w, &plant, &no).await.lines.len(), n);
}

#[then(regex = r#"^營運中心 "([^"]*)" 的雜項發料單 "([^"]*)" 狀態應為 "([^"]*)"$"#)]
async fn status_is(w: &mut ErpWorld, plant: String, no: String, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗 {}：{e}", e.code());
    }
    assert_eq!(status_name(&doc(w, &plant, &no).await.status), expected);
}

#[then(regex = r#"^營運中心 "([^"]*)" 的雜項發料單 "([^"]*)" 應不存在$"#)]
async fn not_exists(w: &mut ErpWorld, plant: String, no: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗 {}：{e}", e.code());
    }
    let r = w.misc_issues.get(&w.actor, &plant, &no).await;
    assert_eq!(
        r.err().map(|e| e.code()),
        Some("INVENTORY_TRANSACTION_NOT_FOUND")
    );
}

#[then(regex = r#"^雜項發料單清單順序應為 "([^"]*)"$"#)]
fn order_is(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^營運中心 "([^"]*)" 雜項發料單 "([^"]*)" 的異動記錄應有 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, plant: String, no: String, expected: String) {
    let h = w
        .misc_issues
        .history(&w.actor, &plant, &no, None)
        .await
        .expect("異動記錄");
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}

// ----- 過帳與過帳還原（misc_issue_posting.feature） -----

#[given(
    regex = r#"^營運中心 "([^"]*)" 已有已確認的雜項發料單 "([^"]*)"，單身料件 "([^"]*)" 倉庫 "([^"]*)"(?: 批號 "([^"]*)")? 數量 (\d+)$"#
)]
async fn has_confirmed_doc(
    w: &mut ErpWorld,
    plant: String,
    no: String,
    item: String,
    wh: String,
    lot: String,
    qty: i64,
) {
    let input = InventoryTxnInput {
        input_date: Some(date("2026-11-20")),
        lines: Some(vec![InventoryTxnLineInput {
            item_no: item,
            warehouse: Some(wh),
            lot: Some(lot),
            qty: Decimal::from(qty),
            ..InventoryTxnLineInput::default()
        }]),
        ..base_input(&no)
    };
    let d = w
        .misc_issues
        .create(&w.actor, &plant, input)
        .await
        .unwrap_or_else(|e| panic!("建立雜項發料單 {no} 失敗：{e}"));
    w.misc_issues
        .confirm(&w.actor, &plant, &no, true, Some(d.version))
        .await
        .unwrap_or_else(|e| panic!("確認雜項發料單 {no} 失敗：{e}"));
}

#[given(regex = r#"^使用者在營運中心 "([^"]*)" (過帳|過帳還原)雜項發料單 "([^"]*)"$"#)]
#[when(regex = r#"^使用者在營運中心 "([^"]*)" (過帳|過帳還原)雜項發料單 "([^"]*)"$"#)]
async fn post(w: &mut ErpWorld, plant: String, op: String, no: String) {
    let v = w
        .misc_issues
        .get(&w.actor, &plant, &no)
        .await
        .ok()
        .map(|d| d.version);
    let r = if op == "過帳" {
        w.misc_issues.post(&w.actor, &plant, &no, None, v).await
    } else {
        w.misc_issues.undo_post(&w.actor, &plant, &no, v).await
    };
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 過帳雜項發料單 "([^"]*)"，扣帳日 "([^"]*)"$"#)]
async fn post_on(w: &mut ErpWorld, plant: String, no: String, d: String) {
    let v = w
        .misc_issues
        .get(&w.actor, &plant, &no)
        .await
        .ok()
        .map(|d| d.version);
    let r = w
        .misc_issues
        .post(&w.actor, &plant, &no, Some(date(&d)), v)
        .await;
    w.last = Some(r.map(drop));
}

#[then(regex = r#"^營運中心 "([^"]*)" 的雜項發料單 "([^"]*)" 應為(已過帳|未過帳)$"#)]
async fn posted_is(w: &mut ErpWorld, plant: String, no: String, state: String) {
    if state == "已過帳"
        && let Some(Err(e)) = &w.last
    {
        panic!("過帳失敗 {}：{e}", e.code());
    }
    assert_eq!(doc(w, &plant, &no).await.posted, state == "已過帳");
}
