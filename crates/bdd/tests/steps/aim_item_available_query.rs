//! 料件數量查詢 — specs/features/aim/item_available_query.feature（IA-1～IA-3）

use cucumber::{gherkin::Step, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::QueryRequest;

use crate::{ErpWorld, steps::common::rows};

fn field_key(label: &str) -> &'static str {
    match label {
        "料件編號" => "item_no",
        "品名" => "name",
        "規格" => "spec",
        other => panic!("未知欄位：{other}"),
    }
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以下列條件查詢料件數量:$"#)]
async fn query_by(w: &mut ErpWorld, plant: String, step: &Step) {
    let req = QueryRequest {
        page_size: Some(1000),
        ..crate::steps::common::query_request(step, field_key, None)
    };
    let r = w.item_available.query(&w.actor, &plant, req).await;
    w.item_available_result = r.as_ref().ok().map(|p| p.items.clone());
    crate::steps::common::set_query_result(w, r, |d| d.item_no.clone());
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以下列條件列印料件數量:$"#)]
async fn report(w: &mut ErpWorld, plant: String, step: &Step) {
    let conditions = crate::steps::common::query_request(step, field_key, None).conditions;
    let r = w
        .item_available
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

#[then(regex = r#"^料件數量清單應為:$"#)]
fn list_is(w: &mut ErpWorld, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let got = w.item_available_result.as_ref().expect("尚未查詢料件數量");
    let table = step.table.as_ref().expect("表格");
    let header = &table.rows[0];
    let expected = rows(step);
    assert_eq!(got.len(), expected.len(), "筆數");
    for (d, r) in got.iter().zip(expected) {
        for (i, h) in header.iter().enumerate() {
            let v = match h.as_str() {
                "料件" => d.item_no.clone(),
                "品名" => d.name.clone().unwrap_or_default(),
                "可用量" => d.available.normalize().to_string(),
                other => panic!("未知欄位：{other}"),
            };
            assert_eq!(v, r[i], "{h}");
        }
    }
}
