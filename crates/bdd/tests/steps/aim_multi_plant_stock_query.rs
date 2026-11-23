//! 料件多工廠庫存數量查詢 — specs/features/aim/multi_plant_stock_query.feature（MP-1～MP-4）

use cucumber::{gherkin::Step, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{MultiPlantStockDto, QueryRequest};

use crate::{ErpWorld, steps::common::rows};

fn field_key(label: &str) -> &'static str {
    match label {
        "料件編號" => "item_no",
        "品名" => "name",
        "規格" => "spec",
        "倉庫" => "warehouse",
        "儲位" => "location",
        "批號" => "lot",
        other => panic!("未知欄位：{other}"),
    }
}

fn stock<'a>(w: &'a ErpWorld, item: &str) -> &'a MultiPlantStockDto {
    w.multi_plant_stock_result
        .as_ref()
        .expect("尚未查詢多工廠庫存")
        .iter()
        .find(|s| s.item_no == item)
        .unwrap_or_else(|| panic!("查詢結果沒有料件 {item}"))
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以下列條件查詢多工廠庫存:$"#)]
async fn query_by(w: &mut ErpWorld, plant: String, step: &Step) {
    let req = QueryRequest {
        page_size: Some(1000),
        ..crate::steps::common::query_request(step, field_key, None)
    };
    let r = w.multi_plant_stock.query(&w.actor, &plant, req).await;
    w.multi_plant_stock_result = r.as_ref().ok().map(|p| p.items.clone());
    crate::steps::common::set_query_result(w, r, |d| d.item_no.clone());
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以下列條件列印多工廠庫存:$"#)]
async fn report(w: &mut ErpWorld, plant: String, step: &Step) {
    let conditions = crate::steps::common::query_request(step, field_key, None).conditions;
    let r = w
        .multi_plant_stock
        .report_in(
            &w.actor,
            &plant,
            conditions,
            ReportFormat::Pdf,
            &Lang::default_lang(),
        )
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[then(regex = r#"^多工廠庫存清單順序應為 "([^"]*)"$"#)]
fn order_is(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^料件 "([^"]*)" 的多工廠庫存合計應為 (\S+)$"#)]
fn total_is(w: &mut ErpWorld, item: String, expected: String) {
    assert_eq!(stock(w, &item).total.normalize().to_string(), expected);
}

#[then(regex = r#"^料件 "([^"]*)" 的多工廠庫存明細應為:$"#)]
fn lines_are(w: &mut ErpWorld, item: String, step: &Step) {
    let s = stock(w, &item);
    let table = step.table.as_ref().expect("表格");
    let header = &table.rows[0];
    let expected = rows(step);
    assert_eq!(s.lines.len(), expected.len(), "明細筆數");
    for (l, r) in s.lines.iter().zip(expected) {
        for (i, h) in header.iter().enumerate() {
            let got = match h.as_str() {
                "營運中心" => l.plant.clone(),
                "倉庫" => l.warehouse.clone(),
                "儲位" => l.location.clone(),
                "批號" => l.lot.clone(),
                "單位" => l.unit.clone(),
                "庫存量" => l.qty.normalize().to_string(),
                other => panic!("未知欄位：{other}"),
            };
            assert_eq!(got, r[i], "{h}");
        }
    }
}

#[then(regex = r#"^料件 "([^"]*)" 的多工廠庫存明細應有 (\d+) 筆$"#)]
fn line_count(w: &mut ErpWorld, item: String, n: usize) {
    assert_eq!(stock(w, &item).lines.len(), n);
}
