//! 編碼年對照維護 — specs/features/aoo/encoding_year.feature（EY1–EY8）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{EncodingYearDto, EncodingYearInput, QueryRequest};

use crate::{ErpWorld, steps::common::rows};

const PROGRAM: &str = "encoding-year-maintenance";

fn input(year: i32, code: &str, version: Option<i32>) -> EncodingYearInput {
    EncodingYearInput {
        year: Some(year),
        code_value: code.to_owned(),
        version,
    }
}

async fn get(w: &ErpWorld, year: i32, length: i16) -> EncodingYearDto {
    w.encoding_years
        .get(&w.actor, year, length)
        .await
        .unwrap_or_else(|e| panic!("查詢編碼年對照 {year}/{length} 失敗：{e}"))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn field_key(label: &str) -> &'static str {
    match label {
        "年" => "year",
        "長度" => "length",
        "編碼編號" => "code_value",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

#[given(regex = r#"^已有編碼年對照：年 (\d+)，編碼編號 "([^"]*)"$"#)]
async fn has(w: &mut ErpWorld, year: i32, code: String) {
    w.encoding_years
        .create(&w.actor, input(year, &code, None))
        .await
        .unwrap_or_else(|e| panic!("建立編碼年對照失敗：{e}"));
}

#[given(regex = r"^另一位使用者已修改編碼年對照 (\d+) 長度 (\d+)$")]
async fn changed_by_other(w: &mut ErpWorld, year: i32, length: i16) {
    let y = get(w, year, length).await;
    w.year_version = Some(y.version);
    w.encoding_years
        .update(
            &w.actor,
            year,
            length,
            input(year, &y.code_value, Some(y.version)),
        )
        .await
        .expect("他人修改");
}

#[given(regex = r"^使用者已確認編碼年對照 (\d+) 長度 (\d+)$")]
async fn confirmed(w: &mut ErpWorld, year: i32, length: i16) {
    w.encoding_years
        .confirm(&w.actor, year, length, true, None)
        .await
        .expect("確認");
}

#[when(regex = r#"^使用者新增編碼年對照：年 (\d+)，編碼編號 "([^"]*)"$"#)]
async fn add(w: &mut ErpWorld, year: i32, code: String) {
    let r = w
        .encoding_years
        .create(&w.actor, input(year, &code, None))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(以舊版本)?把編碼年對照 (\d+) 長度 (\d+) 的編碼編號改為 "([^"]*)"$"#)]
async fn modify(w: &mut ErpWorld, stale: String, year: i32, length: i16, code: String) {
    let version = if stale.is_empty() {
        w.encoding_years
            .get(&w.actor, year, length)
            .await
            .ok()
            .map(|y| y.version)
    } else {
        w.year_version
    };
    let r = w
        .encoding_years
        .update(&w.actor, year, length, input(year, &code, version))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r"^使用者(停用|啟用)編碼年對照 (\d+) 長度 (\d+)$")]
async fn set_active(w: &mut ErpWorld, what: String, year: i32, length: i16) {
    let r = w
        .encoding_years
        .set_active(&w.actor, year, length, what == "啟用", None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r"^使用者刪除編碼年對照 (\d+) 長度 (\d+)$")]
async fn delete(w: &mut ErpWorld, year: i32, length: i16) {
    let r = w.encoding_years.delete(&w.actor, year, length, None).await;
    w.last = Some(r);
}

#[when(regex = r"^使用者(確認|取消確認)編碼年對照 (\d+) 長度 (\d+)$")]
async fn confirm(w: &mut ErpWorld, what: String, year: i32, length: i16) {
    let r = w
        .encoding_years
        .confirm(&w.actor, year, length, what == "確認", None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r"^使用者把編碼年對照 (\d+) 長度 (\d+) 複製到 (\d+) 年$")]
async fn copy(w: &mut ErpWorld, year: i32, length: i16, to: i32) {
    let r = w
        .encoding_years
        .copy(&w.actor, year, length, Some(to))
        .await;
    w.last = Some(r.map(drop));
}

fn key(y: &EncodingYearDto) -> String {
    format!("{}/{}/{}", y.year, y.length, y.code_value)
}

#[when("使用者查詢編碼年對照")]
async fn query_all(w: &mut ErpWorld) {
    let r = w
        .encoding_years
        .query(&w.actor, QueryRequest::default())
        .await;
    crate::steps::common::set_query_result(w, r, key);
}

#[when("使用者以下列條件查詢編碼年對照:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.encoding_years.query(&w.actor, req).await;
    crate::steps::common::set_query_result(w, r, key);
}

#[when(regex = r"^使用者(列印編碼年對照|匯出編碼年對照為 Excel)$")]
async fn report(w: &mut ErpWorld, what: String) {
    let format = if what.starts_with("列印") {
        ReportFormat::Pdf
    } else {
        ReportFormat::Excel
    };
    let r = w
        .encoding_years
        .report_in(&w.actor, Vec::new(), format, &Lang::default_lang())
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[then(regex = r"^編碼年對照 (\d+) 長度 (\d+) 應為:$")]
async fn is(w: &mut ErpWorld, year: i32, length: i16, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let y = get(w, year, length).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "編碼編號" => y.code_value.clone(),
            "資料有效" => yes_no(y.active).to_owned(),
            "已確認" => yes_no(y.confirmed).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(regex = r#"^編碼年對照 (\d+) 長度 (\d+) 的(資料有效|已確認)應為 "(是|否)"$"#)]
async fn flag_is(w: &mut ErpWorld, year: i32, length: i16, what: String, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let y = get(w, year, length).await;
    let v = if what == "資料有效" {
        y.active
    } else {
        y.confirmed
    };
    assert_eq!(yes_no(v), expected);
}

#[then(regex = r"^編碼年對照 (\d+) 長度 (\d+) 應不存在$")]
async fn gone(w: &mut ErpWorld, year: i32, length: i16) {
    let r = w.encoding_years.get(&w.actor, year, length).await;
    assert_eq!(r.err().map(|e| e.code()), Some("ENCODING_YEAR_NOT_FOUND"));
}

#[then(regex = r#"^編碼年對照清單應為 "([^"]*)"$"#)]
async fn list_is(w: &mut ErpWorld, expected: String) {
    if w.query_result.is_none() {
        let r = w
            .encoding_years
            .query(&w.actor, QueryRequest::default())
            .await;
        crate::steps::common::set_query_result(w, r, key);
    }
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^編碼年對照 (\d+) 長度 (\d+) 的異動記錄應有 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, year: i32, length: i16, expected: String) {
    let h = w
        .shared_history
        .history(&w.actor, PROGRAM, &year.to_string(), None)
        .await
        .expect("異動記錄");
    // 異動記錄以年查詢（同一年各長度一併列出）；情境中該年只有這一個長度
    let _ = length;
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}
