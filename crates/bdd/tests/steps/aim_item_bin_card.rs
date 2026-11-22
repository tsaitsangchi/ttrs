//! 料件 BIN 卡查詢 — specs/features/aim/item_bin_card.feature（BN-1～BN-4）

use cucumber::{gherkin::Step, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{BinCardDto, BinCardQuery, QueryRequest};

use crate::{ErpWorld, steps::common::rows};

fn field_key(label: &str) -> &'static str {
    match label {
        "料件編號" => "item_no",
        "品名" => "name",
        "規格" => "spec",
        "來源碼" => "source_code",
        "倉庫" => "warehouse",
        "儲位" => "location",
        "批號" => "lot",
        "單位" => "unit",
        "庫存量" => "qty",
        other => panic!("未知欄位：{other}"),
    }
}

/// BIN 卡的鍵：料件/倉庫/儲位/批號
fn card_key(c: &BinCardDto) -> String {
    format!("{}/{}/{}/{}", c.item_no, c.warehouse, c.location, c.lot)
}

fn card<'a>(w: &'a ErpWorld, key: &str) -> &'a BinCardDto {
    w.bin_card_result
        .as_ref()
        .expect("尚未查詢 BIN 卡")
        .iter()
        .find(|c| card_key(c) == key)
        .unwrap_or_else(|| panic!("查詢結果沒有 BIN 卡 {key}"))
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 查詢 (\d+) 年 (\d+) 期的 BIN 卡，條件:$"#)]
async fn query_by(w: &mut ErpWorld, plant: String, year: i32, period: i16, step: &Step) {
    let q = BinCardQuery {
        year,
        period,
        request: QueryRequest {
            page_size: Some(1000),
            ..crate::steps::common::query_request(step, field_key, None)
        },
    };
    let r = w.bin_cards.query(&w.actor, &plant, q).await;
    w.bin_card_result = r.as_ref().ok().map(|p| p.items.clone());
    crate::steps::common::set_query_result(w, r, card_key);
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 列印 (\d+) 年 (\d+) 期的 BIN 卡，條件:$"#)]
async fn report(w: &mut ErpWorld, plant: String, year: i32, period: i16, step: &Step) {
    let conditions = crate::steps::common::query_request(step, field_key, None).conditions;
    let r = w
        .bin_cards
        .report_in(
            &w.actor,
            &plant,
            year,
            period,
            conditions,
            ReportFormat::Pdf,
            &Lang::default_lang(),
        )
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[then(regex = r#"^BIN 卡清單應為 "([^"]*)"$"#)]
fn list_is(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^BIN 卡 "([^"]*)" 的期初庫存應為 (-?\d+)，目前庫存應為 (-?\d+)$"#)]
fn opening_is(w: &mut ErpWorld, key: String, opening: i64, current: i64) {
    let c = card(w, &key);
    assert_eq!(c.opening, rust_decimal::Decimal::from(opening), "期初");
    assert_eq!(c.current, rust_decimal::Decimal::from(current), "目前");
}

#[then(regex = r#"^BIN 卡 "([^"]*)" 的異動應為:$"#)]
fn lines_are(w: &mut ErpWorld, key: String, step: &Step) {
    let c = card(w, &key);
    let table = step.table.as_ref().expect("表格");
    let header = &table.rows[0];
    let expected = rows(step);
    assert_eq!(c.lines.len(), expected.len(), "異動筆數");
    for (l, r) in c.lines.iter().zip(expected) {
        for (i, h) in header.iter().enumerate() {
            let got = match h.as_str() {
                "單據日期" => l.txn_date.to_string(),
                "異動狀況" => l.program.clone(),
                "單號" => l.doc_no.clone(),
                "項次" => l.doc_line.to_string(),
                "數量" => l.qty.normalize().to_string(),
                "單位" => l.unit.clone(),
                "結存" => l.balance.normalize().to_string(),
                other => panic!("未知欄位：{other}"),
            };
            assert_eq!(got, r[i], "{h}");
        }
    }
}

#[then(regex = r#"^BIN 卡 "([^"]*)" 的異動應有 (\d+) 筆$"#)]
fn line_count(w: &mut ErpWorld, key: String, n: usize) {
    assert_eq!(card(w, &key).lines.len(), n);
}
