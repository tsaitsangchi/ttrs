//! 編碼性質維護 — specs/features/aoo/encoding_nature.feature（EN1–EN8）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{EncodingNatureDto, EncodingNatureInput, QueryRequest};

use crate::{ErpWorld, steps::common::rows};

const PROGRAM: &str = "encoding-nature-maintenance";

fn input(code: &str, name: &str, category: &str, version: Option<i32>) -> EncodingNatureInput {
    EncodingNatureInput {
        nature_code: code.to_owned(),
        name: Some(name.to_owned()),
        category: category.to_owned(),
        version,
    }
}

async fn nature(w: &ErpWorld, code: &str) -> EncodingNatureDto {
    w.encoding_natures
        .get(&w.actor, code)
        .await
        .unwrap_or_else(|e| panic!("查詢編碼性質 {code} 失敗：{e}"))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn field_key(label: &str) -> &'static str {
    match label {
        "性質" => "nature_code",
        "說明" => "name",
        "類別" => "category",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

async fn seed(w: &mut ErpWorld, code: &str, name: &str, category: &str) {
    w.encoding_natures
        .create(&w.actor, input(code, name, category, None))
        .await
        .unwrap_or_else(|e| panic!("建立編碼性質 {code} 失敗：{e}"));
}

#[given(regex = r#"^已有編碼性質 "([^"]+)"，類別 "(\d)"$"#)]
async fn has(w: &mut ErpWorld, code: String, category: String) {
    seed(w, &code, &code, &category).await;
}

#[given(regex = r#"^已有編碼性質 "([^"]+)"，說明 "([^"]*)"，類別 "(\d)"$"#)]
async fn has_named(w: &mut ErpWorld, code: String, name: String, category: String) {
    seed(w, &code, &name, &category).await;
}

#[given(regex = r#"^另一位使用者已修改編碼性質 "([^"]+)"$"#)]
async fn changed_by_other(w: &mut ErpWorld, code: String) {
    let n = nature(w, &code).await;
    w.nature_version = Some(n.version);
    w.encoding_natures
        .update(
            &w.actor,
            &code,
            input(&code, "他人修改", &n.category, Some(n.version)),
        )
        .await
        .expect("他人修改");
}

#[given(regex = r#"^使用者已確認編碼性質 "([^"]+)"$"#)]
async fn confirmed(w: &mut ErpWorld, code: String) {
    w.encoding_natures
        .confirm(&w.actor, &code, true, None)
        .await
        .expect("確認");
}

#[when(regex = r#"^使用者新增編碼性質 "([^"]*)"，說明 "([^"]*)"，類別 "([^"]*)"$"#)]
async fn add(w: &mut ErpWorld, code: String, name: String, category: String) {
    let r = w
        .encoding_natures
        .create(&w.actor, input(&code, &name, &category, None))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(以舊版本)?修改編碼性質 "([^"]+)"，說明 "([^"]*)"，類別 "([^"]*)"$"#)]
async fn modify(w: &mut ErpWorld, stale: String, code: String, name: String, category: String) {
    let version = if stale.is_empty() {
        w.encoding_natures
            .get(&w.actor, &code)
            .await
            .ok()
            .map(|n| n.version)
    } else {
        w.nature_version
    };
    let r = w
        .encoding_natures
        .update(&w.actor, &code, input(&code, &name, &category, version))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(停用|啟用)編碼性質 "([^"]+)"$"#)]
async fn set_active(w: &mut ErpWorld, what: String, code: String) {
    let r = w
        .encoding_natures
        .set_active(&w.actor, &code, what == "啟用", None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者刪除編碼性質 "([^"]+)"$"#)]
async fn delete(w: &mut ErpWorld, code: String) {
    let r = w.encoding_natures.delete(&w.actor, &code, None).await;
    w.last = Some(r);
}

#[when(regex = r#"^使用者(確認|取消確認)編碼性質 "([^"]+)"$"#)]
async fn confirm(w: &mut ErpWorld, what: String, code: String) {
    let r = w
        .encoding_natures
        .confirm(&w.actor, &code, what == "確認", None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者把編碼性質 "([^"]+)" 複製為 "([^"]+)"$"#)]
async fn copy(w: &mut ErpWorld, from: String, to: String) {
    let r = w.encoding_natures.copy(&w.actor, &from, &to).await;
    w.last = Some(r.map(drop));
}

#[when("使用者查詢編碼性質")]
async fn query_all(w: &mut ErpWorld) {
    let r = w
        .encoding_natures
        .query(&w.actor, QueryRequest::default())
        .await;
    crate::steps::common::set_query_result(w, r, |n| n.nature_code.clone());
}

#[when("使用者以下列條件查詢編碼性質:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.encoding_natures.query(&w.actor, req).await;
    crate::steps::common::set_query_result(w, r, |n| n.nature_code.clone());
}

#[when(regex = r"^使用者(列印編碼性質|匯出編碼性質為 Excel)$")]
async fn report(w: &mut ErpWorld, what: String) {
    let format = if what.starts_with("列印") {
        ReportFormat::Pdf
    } else {
        ReportFormat::Excel
    };
    let r = w
        .encoding_natures
        .report_in(&w.actor, Vec::new(), format, &Lang::default_lang())
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[then(regex = r#"^編碼性質 "([^"]+)" 應為:$"#)]
async fn is(w: &mut ErpWorld, code: String, step: &Step) {
    let n = nature(w, &code).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "說明" => n.name.clone().unwrap_or_default(),
            "類別" => n.category.clone(),
            "資料有效" => yes_no(n.active).to_owned(),
            "已確認" => yes_no(n.confirmed).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(regex = r#"^編碼性質 "([^"]+)" 的(資料有效|已確認)應為 "(是|否)"$"#)]
async fn flag_is(w: &mut ErpWorld, code: String, what: String, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let n = nature(w, &code).await;
    let v = if what == "資料有效" {
        n.active
    } else {
        n.confirmed
    };
    assert_eq!(yes_no(v), expected);
}

#[then(regex = r#"^編碼性質 "([^"]+)" 應不存在$"#)]
async fn gone(w: &mut ErpWorld, code: String) {
    let r = w.encoding_natures.get(&w.actor, &code).await;
    assert_eq!(r.err().map(|e| e.code()), Some("ENCODING_NATURE_NOT_FOUND"));
}

#[then(regex = r#"^編碼性質清單順序應為 "([^"]*)"$"#)]
async fn order_is(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^報表欄位應為 "([^"]+)"$"#)]
async fn report_columns(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("列印失敗：{e}");
    }
    let r = w.report.as_ref().expect("尚未產生報表");
    assert_eq!(r.columns.join("、"), expected);
}

#[then(regex = r"^報表應有 (\d+) 筆資料$")]
async fn report_rows(w: &mut ErpWorld, n: usize) {
    assert_eq!(w.report.as_ref().expect("尚未產生報表").rows.len(), n);
}

#[then(regex = r#"^編碼性質 "([^"]+)" 的異動記錄應有 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, code: String, expected: String) {
    let h = w
        .shared_history
        .history(&w.actor, PROGRAM, &code, None)
        .await
        .expect("異動記錄");
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}
