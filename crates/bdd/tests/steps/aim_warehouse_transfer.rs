//! 庫存直接調撥 — specs/features/aim/warehouse_transfer.feature（WT-1～WT-15）

use cucumber::{gherkin::Step, given, then, when};
use rust_decimal::Decimal;
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{
    WarehouseTransferCopyInput, WarehouseTransferDto, WarehouseTransferInput,
    WarehouseTransferLineInput,
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

fn base_input(doc_no: &str, d: &str) -> WarehouseTransferInput {
    WarehouseTransferInput {
        doc_no: doc_no.to_owned(),
        transfer_date: Some(date(d)),
        ..WarehouseTransferInput::default()
    }
}

fn line_inputs(step: &Step) -> Vec<WarehouseTransferLineInput> {
    let table = step.table.as_ref().expect("此步驟需要單身表格");
    let header = &table.rows[0];
    rows(step)
        .into_iter()
        .map(|r| {
            let get = |name: &str| header.iter().position(|h| h == name).map(|i| r[i].clone());
            WarehouseTransferLineInput {
                line_no: get("項次").map_or(0, |v| v.parse().expect("項次")),
                item_no: get("料件").unwrap_or_default(),
                from_warehouse: get("撥出倉庫").unwrap_or_default(),
                from_location: get("撥出儲位"),
                from_lot: get("撥出批號"),
                to_warehouse: get("撥入倉庫").unwrap_or_default(),
                to_location: get("撥入儲位"),
                to_lot: get("撥入批號"),
                qty: get("數量").map_or(Decimal::ZERO, |v| v.parse().expect("數量")),
                reason: get("理由碼").filter(|v| !v.is_empty()),
            }
        })
        .collect()
}

fn one_line(item: &str, from: &str, to: &str, qty: i64) -> Vec<WarehouseTransferLineInput> {
    vec![WarehouseTransferLineInput {
        item_no: item.to_owned(),
        from_warehouse: from.to_owned(),
        to_warehouse: to.to_owned(),
        qty: Decimal::from(qty),
        ..WarehouseTransferLineInput::default()
    }]
}

async fn create(w: &mut ErpWorld, plant: &str, input: WarehouseTransferInput) {
    let r = w.warehouse_transfers.create(&w.actor, plant, input).await;
    w.transfer_no = r.as_ref().ok().map(|d| d.doc_no.clone());
    w.last = Some(r.map(drop));
}

async fn doc(w: &ErpWorld, plant: &str, no: &str) -> WarehouseTransferDto {
    w.warehouse_transfers
        .get(&w.actor, plant, no)
        .await
        .unwrap_or_else(|e| panic!("查詢調撥單 {no} 失敗：{e}"))
}

fn field_key(label: &str) -> &'static str {
    match label {
        "單號" => "doc_no",
        "料件" => "item_no",
        "撥出倉庫" => "from_warehouse",
        "撥入倉庫" => "to_warehouse",
        "狀態" => "status",
        other => panic!("未知欄位：{other}"),
    }
}

// ----- 假設 -----

#[given(
    regex = r#"^營運中心 "([^"]*)" 已有(已確認的)?調撥單 "([^"]*)"，單身料件 "([^"]*)" 從倉庫 "([^"]*)" 到倉庫 "([^"]*)" 數量 (\d+)$"#
)]
#[allow(clippy::too_many_arguments)] // 步驟的每個擷取群組各一個參數
async fn has_doc(
    w: &mut ErpWorld,
    plant: String,
    confirmed: String,
    no: String,
    item: String,
    from: String,
    to: String,
    qty: i64,
) {
    let input = WarehouseTransferInput {
        lines: Some(one_line(&item, &from, &to, qty)),
        ..base_input(&no, "2026-11-20")
    };
    let d = w
        .warehouse_transfers
        .create(&w.actor, &plant, input)
        .await
        .unwrap_or_else(|e| panic!("建立調撥單 {no} 失敗：{e}"));
    if !confirmed.is_empty() {
        w.warehouse_transfers
            .confirm(&w.actor, &plant, &no, true, Some(d.version))
            .await
            .unwrap_or_else(|e| panic!("確認調撥單 {no} 失敗：{e}"));
    }
}

// ----- 當 -----

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 以單別 "([^"]*)" 新增調撥單，調撥日 "([^"]*)"，單身:$"#
)]
async fn create_with_lines(w: &mut ErpWorld, plant: String, code: String, d: String, step: &Step) {
    let input = WarehouseTransferInput {
        lines: Some(line_inputs(step)),
        ..base_input(&code, &d)
    };
    create(w, &plant, input).await;
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以單別 "([^"]*)" 新增調撥單，調撥日 "([^"]*)"$"#)]
async fn create_empty(w: &mut ErpWorld, plant: String, code: String, d: String) {
    create(w, &plant, base_input(&code, &d)).await;
}

#[given(
    regex = r#"^使用者在營運中心 "([^"]*)" (確認|取消確認|作廢|取消作廢|刪除|過帳|過帳還原)調撥單 "([^"]*)"$"#
)]
#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" (確認|取消確認|作廢|取消作廢|刪除|過帳|過帳還原)調撥單 "([^"]*)"$"#
)]
async fn act(w: &mut ErpWorld, plant: String, op: String, no: String) {
    let v = w
        .warehouse_transfers
        .get(&w.actor, &plant, &no)
        .await
        .ok()
        .map(|d| d.version);
    let s = &w.warehouse_transfers;
    let r = match op.as_str() {
        "確認" => s.confirm(&w.actor, &plant, &no, true, v).await.map(drop),
        "取消確認" => s.confirm(&w.actor, &plant, &no, false, v).await.map(drop),
        "作廢" => s.set_void(&w.actor, &plant, &no, true, v).await.map(drop),
        "取消作廢" => s.set_void(&w.actor, &plant, &no, false, v).await.map(drop),
        "過帳" => s.post(&w.actor, &plant, &no, v).await.map(drop),
        "過帳還原" => s.undo_post(&w.actor, &plant, &no, v).await.map(drop),
        _ => s.delete(&w.actor, &plant, &no, v).await,
    };
    w.last = Some(r);
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 以單別 "([^"]*)" 複製調撥單 "([^"]*)"，調撥日 "([^"]*)"$"#
)]
async fn copy(w: &mut ErpWorld, plant: String, code: String, no: String, d: String) {
    let input = WarehouseTransferCopyInput {
        doc_no: code,
        transfer_date: Some(date(&d)),
    };
    let r = w
        .warehouse_transfers
        .copy(&w.actor, &plant, &no, input)
        .await;
    w.transfer_no = r.as_ref().ok().map(|d| d.doc_no.clone());
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以下列條件查詢調撥單:$"#)]
async fn query_by(w: &mut ErpWorld, plant: String, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.warehouse_transfers.query(&w.actor, &plant, req).await;
    crate::steps::common::set_query_result(w, r, |d| d.doc_no.clone());
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 列印調撥單$"#)]
async fn report(w: &mut ErpWorld, plant: String) {
    let r = w
        .warehouse_transfers
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

#[then(regex = r#"^應產生調撥單號 "([^"]*)"$"#)]
fn created_no(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("新增失敗 {}：{e}", e.code());
    }
    assert_eq!(w.transfer_no.as_deref(), Some(expected.as_str()));
}

#[then(regex = r#"^營運中心 "([^"]*)" 的調撥單 "([^"]*)" 應為:$"#)]
async fn is(w: &mut ErpWorld, plant: String, no: String, step: &Step) {
    let d = doc(w, &plant, &no).await;
    for r in rows(step) {
        let got = match r[0].as_str() {
            "調撥日" => d.transfer_date.to_string(),
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

#[then(regex = r#"^營運中心 "([^"]*)" 的調撥單 "([^"]*)" 單身應為:$"#)]
async fn lines_are(w: &mut ErpWorld, plant: String, no: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗 {}：{e}", e.code());
    }
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
                "撥出倉庫" => l.from_warehouse.clone(),
                "撥出儲位" => l.from_location.clone(),
                "撥出批號" => l.from_lot.clone(),
                "撥入倉庫" => l.to_warehouse.clone(),
                "撥入儲位" => l.to_location.clone(),
                "撥入批號" => l.to_lot.clone(),
                "單位" => l.unit.clone(),
                "數量" => l.qty.normalize().to_string(),
                "理由碼" => l.reason.clone().unwrap_or_default(),
                other => panic!("未知欄位：{other}"),
            };
            assert_eq!(got, r[i], "第 {} 項 {h}", l.line_no);
        }
    }
}

#[then(regex = r#"^營運中心 "([^"]*)" 的調撥單 "([^"]*)" 單身應有 (\d+) 筆$"#)]
async fn line_count(w: &mut ErpWorld, plant: String, no: String, n: usize) {
    assert_eq!(doc(w, &plant, &no).await.lines.len(), n);
}

#[then(regex = r#"^營運中心 "([^"]*)" 的調撥單 "([^"]*)" 狀態應為 "([^"]*)"$"#)]
async fn status_is(w: &mut ErpWorld, plant: String, no: String, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗 {}：{e}", e.code());
    }
    assert_eq!(status_name(&doc(w, &plant, &no).await.status), expected);
}

#[then(regex = r#"^營運中心 "([^"]*)" 的調撥單 "([^"]*)" 應為(已過帳|未過帳)$"#)]
async fn posted_is(w: &mut ErpWorld, plant: String, no: String, state: String) {
    if state == "已過帳"
        && let Some(Err(e)) = &w.last
    {
        panic!("過帳失敗 {}：{e}", e.code());
    }
    assert_eq!(doc(w, &plant, &no).await.posted, state == "已過帳");
}

#[then(regex = r#"^營運中心 "([^"]*)" 的調撥單 "([^"]*)" 應不存在$"#)]
async fn not_exists(w: &mut ErpWorld, plant: String, no: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗 {}：{e}", e.code());
    }
    let r = w.warehouse_transfers.get(&w.actor, &plant, &no).await;
    assert_eq!(
        r.err().map(|e| e.code()),
        Some("INVENTORY_TRANSACTION_NOT_FOUND")
    );
}

#[then(regex = r#"^調撥單清單順序應為 "([^"]*)"$"#)]
fn order_is(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^營運中心 "([^"]*)" 調撥單 "([^"]*)" 的異動記錄應有 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, plant: String, no: String, expected: String) {
    let h = w
        .warehouse_transfers
        .history(&w.actor, &plant, &no, None)
        .await
        .expect("異動記錄");
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}
