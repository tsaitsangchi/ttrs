//! 編碼獨立段維護 — specs/features/aoo/encoding_segment.feature（IS1–IS8）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{
    EncodingSegmentDto, EncodingSegmentInput, EncodingSegmentValueInput, QueryRequest,
};

use crate::{ErpWorld, steps::common::rows};

const PROGRAM: &str = "encoding-segment-maintenance";

fn head(code: &str, name: &str, length: i16, version: Option<i32>) -> EncodingSegmentInput {
    EncodingSegmentInput {
        segment_code: code.to_owned(),
        name: Some(name.to_owned()),
        length: Some(length),
        version,
    }
}

fn value(line: Option<i32>, v: &str, name: &str) -> EncodingSegmentValueInput {
    EncodingSegmentValueInput {
        line,
        value: v.to_owned(),
        name: Some(name.to_owned()),
        version: None,
    }
}

async fn segment(w: &ErpWorld, code: &str) -> EncodingSegmentDto {
    w.encoding_segments
        .get(&w.actor, code)
        .await
        .unwrap_or_else(|e| panic!("查詢編碼獨立段 {code} 失敗：{e}"))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn field_key(label: &str) -> &'static str {
    match label {
        "獨立段編號" => "segment_code",
        "名稱" => "name",
        "長度" => "length",
        "項次" => "line",
        "編碼編號" => "value",
        "編碼名稱" => "value_name",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

fn ok<T>(w: &ErpWorld, r: Result<T, ttrs_app::AppError>, what: &str) -> T {
    let _ = w;
    r.unwrap_or_else(|e| panic!("{what} 失敗：{e}"))
}

#[given(regex = r#"^已有編碼獨立段 "([^"]+)"，長度 (\d+)$"#)]
async fn has(w: &mut ErpWorld, code: String, length: i16) {
    let r = w
        .encoding_segments
        .create(&w.actor, head(&code, &code, length, None))
        .await;
    ok(w, r, "建立編碼獨立段");
}

#[given(regex = r#"^已有編碼獨立段 "([^"]+)"，名稱 "([^"]*)"，長度 (\d+)$"#)]
async fn has_named(w: &mut ErpWorld, code: String, name: String, length: i16) {
    let r = w
        .encoding_segments
        .create(&w.actor, head(&code, &name, length, None))
        .await;
    ok(w, r, "建立編碼獨立段");
}

#[given(regex = r#"^編碼獨立段 "([^"]+)" 已有編碼編號 "([^"]*)"，名稱 "([^"]*)"$"#)]
async fn has_value(w: &mut ErpWorld, code: String, v: String, name: String) {
    let r = w
        .encoding_segments
        .add_value(&w.actor, &code, value(None, &v, &name))
        .await;
    ok(w, r, "新增編碼編號");
}

#[given(regex = r#"^使用者已(停用|確認)編碼獨立段 "([^"]+)"$"#)]
async fn already(w: &mut ErpWorld, what: String, code: String) {
    let r = if what == "停用" {
        w.encoding_segments
            .set_active(&w.actor, &code, false, None)
            .await
    } else {
        w.encoding_segments
            .confirm(&w.actor, &code, true, None)
            .await
    };
    ok(w, r, &what);
}

#[given(regex = r#"^另一位使用者已修改編碼獨立段 "([^"]+)"$"#)]
async fn changed_by_other(w: &mut ErpWorld, code: String) {
    let s = segment(w, &code).await;
    w.segment_version = Some(s.version);
    let r = w
        .encoding_segments
        .update(
            &w.actor,
            &code,
            head(&code, "他人修改", s.length, Some(s.version)),
        )
        .await;
    ok(w, r, "他人修改");
}

#[when(regex = r#"^使用者新增編碼獨立段 "([^"]*)"，名稱 "([^"]*)"，長度 (\d+)$"#)]
async fn add(w: &mut ErpWorld, code: String, name: String, length: i16) {
    let r = w
        .encoding_segments
        .create(&w.actor, head(&code, &name, length, None))
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者在編碼獨立段 "([^"]+)" 新增(?:項次 (\d+) )?編碼編號 "([^"]*)"，名稱 "([^"]*)"$"#
)]
async fn add_value(w: &mut ErpWorld, code: String, line: String, v: String, name: String) {
    let line = line.parse().ok();
    let r = w
        .encoding_segments
        .add_value(&w.actor, &code, value(line, &v, &name))
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者把編碼獨立段 "([^"]+)" 項次 (\d+) 改為編碼編號 "([^"]*)"，名稱 "([^"]*)"$"#
)]
async fn update_value(w: &mut ErpWorld, code: String, line: i32, v: String, name: String) {
    let r = w
        .encoding_segments
        .update_value(&w.actor, &code, line, value(None, &v, &name))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者刪除編碼獨立段 "([^"]+)" 項次 (\d+)$"#)]
async fn remove_value(w: &mut ErpWorld, code: String, line: i32) {
    let r = w
        .encoding_segments
        .remove_value(&w.actor, &code, line, None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(以舊版本)?修改編碼獨立段 "([^"]+)"，名稱 "([^"]*)"，長度 (\d+)$"#)]
async fn modify(w: &mut ErpWorld, stale: String, code: String, name: String, length: i16) {
    let version = if stale.is_empty() {
        w.encoding_segments
            .get(&w.actor, &code)
            .await
            .ok()
            .map(|s| s.version)
    } else {
        w.segment_version
    };
    let r = w
        .encoding_segments
        .update(&w.actor, &code, head(&code, &name, length, version))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(停用|啟用)編碼獨立段 "([^"]+)"$"#)]
async fn set_active(w: &mut ErpWorld, what: String, code: String) {
    let r = w
        .encoding_segments
        .set_active(&w.actor, &code, what == "啟用", None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者刪除編碼獨立段 "([^"]+)"$"#)]
async fn delete(w: &mut ErpWorld, code: String) {
    let r = w.encoding_segments.delete(&w.actor, &code, None).await;
    w.last = Some(r);
}

#[when(regex = r#"^使用者(確認|取消確認)編碼獨立段 "([^"]+)"$"#)]
async fn confirm(w: &mut ErpWorld, what: String, code: String) {
    let r = w
        .encoding_segments
        .confirm(&w.actor, &code, what == "確認", None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者把編碼獨立段 "([^"]+)" 複製為 "([^"]+)"$"#)]
async fn copy(w: &mut ErpWorld, from: String, to: String) {
    let r = w.encoding_segments.copy(&w.actor, &from, &to).await;
    w.last = Some(r.map(drop));
}

#[when("使用者查詢編碼獨立段")]
async fn query_all(w: &mut ErpWorld) {
    let r = w
        .encoding_segments
        .query(&w.actor, QueryRequest::default())
        .await;
    crate::steps::common::set_query_result(w, r, |s| s.segment_code.clone());
}

#[when("使用者以下列條件查詢編碼獨立段:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.encoding_segments.query(&w.actor, req).await;
    crate::steps::common::set_query_result(w, r, |s| s.segment_code.clone());
}

#[when(regex = r"^使用者(列印編碼獨立段|匯出編碼獨立段為 Excel)$")]
async fn report(w: &mut ErpWorld, what: String) {
    let format = if what.starts_with("列印") {
        ReportFormat::Pdf
    } else {
        ReportFormat::Excel
    };
    let r = w
        .encoding_segments
        .report_in(&w.actor, Vec::new(), format, &Lang::default_lang())
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[then(regex = r#"^編碼獨立段 "([^"]+)" 應為:$"#)]
async fn is(w: &mut ErpWorld, code: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let s = segment(w, &code).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "名稱" => s.name.clone().unwrap_or_default(),
            "長度" => s.length.to_string(),
            "資料有效" => yes_no(s.active).to_owned(),
            "已確認" => yes_no(s.confirmed).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(regex = r#"^編碼獨立段 "([^"]+)" 的單身應為:$"#)]
async fn values_are(w: &mut ErpWorld, code: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let got: Vec<Vec<String>> = segment(w, &code)
        .await
        .values
        .into_iter()
        .map(|v| vec![v.line.to_string(), v.value, v.name.unwrap_or_default()])
        .collect();
    assert_eq!(got, rows(step));
}

#[then(regex = r#"^編碼獨立段 "([^"]+)" 的最後修改者應為 "([^"]+)"$"#)]
async fn updated_by(w: &mut ErpWorld, code: String, user: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    assert_eq!(segment(w, &code).await.updated_by, Some(user));
}

#[then(regex = r#"^編碼獨立段 "([^"]+)" 的(資料有效|已確認)應為 "(是|否)"$"#)]
async fn flag_is(w: &mut ErpWorld, code: String, what: String, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let s = segment(w, &code).await;
    let v = if what == "資料有效" {
        s.active
    } else {
        s.confirmed
    };
    assert_eq!(yes_no(v), expected);
}

#[then(regex = r#"^編碼獨立段 "([^"]+)" 應不存在$"#)]
async fn gone(w: &mut ErpWorld, code: String) {
    let r = w.encoding_segments.get(&w.actor, &code).await;
    assert_eq!(
        r.err().map(|e| e.code()),
        Some("ENCODING_SEGMENT_NOT_FOUND")
    );
}

#[then(regex = r#"^編碼獨立段清單順序應為 "([^"]*)"$"#)]
async fn order_is(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^編碼獨立段 "([^"]+)" 的異動記錄應包含單頭的 "([^"]+)" 與單身的 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, code: String, head_op: String, line_op: String) {
    let h = w
        .shared_history
        .history(&w.actor, PROGRAM, &code, None)
        .await
        .expect("異動記錄");
    let has = |table: &str, op: &str| h.iter().any(|c| c.table_name == table && c.operation == op);
    assert!(has("encoding_segment", &head_op), "{h:?}");
    assert!(has("encoding_segment_code", &line_op), "{h:?}");
}
