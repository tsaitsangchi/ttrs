//! 編碼類別維護 — specs/features/aoo/encoding_class.feature（EC1–EC11）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{EncodingClassDto, EncodingClassInput, EncodingClassSegmentInput, QueryRequest};

use crate::{ErpWorld, steps::common::rows};

const PROGRAM: &str = "encoding-class-maintenance";
const TYPES: [&str; 9] = [
    "固定值",
    "獨立段",
    "流水號",
    "欄位值",
    "年",
    "季",
    "月",
    "週",
    "日",
];

fn type_code(label: &str) -> String {
    TYPES
        .iter()
        .position(|t| *t == label)
        .map_or_else(|| label.to_owned(), |i| (i + 1).to_string())
}

fn type_label(code: &str) -> &'static str {
    code.parse::<usize>()
        .ok()
        .and_then(|i| TYPES.get(i.wrapping_sub(1)))
        .copied()
        .unwrap_or("?")
}

fn head(code: &str, name: &str, nature: &str, version: Option<i32>) -> EncodingClassInput {
    EncodingClassInput {
        class_code: code.to_owned(),
        name: Some(name.to_owned()),
        nature_code: nature.to_owned(),
        version,
    }
}

fn opt(s: &str) -> Option<String> {
    Some(s.to_owned()).filter(|v| !v.is_empty())
}

fn segment(kind: &str, length: &str, value: &str, description: &str) -> EncodingClassSegmentInput {
    EncodingClassSegmentInput {
        segment_type: type_code(kind),
        length: length.parse().ok(),
        value: opt(value),
        description: opt(description),
        ..Default::default()
    }
}

async fn class(w: &ErpWorld, code: &str) -> EncodingClassDto {
    w.encoding_classes
        .get(&w.actor, code)
        .await
        .unwrap_or_else(|e| panic!("查詢編碼類別 {code} 失敗：{e}"))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn field_key(label: &str) -> &'static str {
    match label {
        "編碼類別" => "class_code",
        "名稱" => "name",
        "性質" => "nature_code",
        "段次" => "seq_no",
        "型態" => "segment_type",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

fn ok(w: &ErpWorld) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
}

#[given(regex = r#"^編碼性質 "([^"]+)" 已停用$"#)]
async fn nature_inactive(w: &mut ErpWorld, code: String) {
    w.encoding_natures
        .set_active(&w.actor, &code, false, None)
        .await
        .expect("停用編碼性質");
}

#[given(regex = r#"^已有編碼類別 "([^"]+)"，性質 "([^"]+)"$"#)]
async fn has(w: &mut ErpWorld, code: String, nature: String) {
    w.encoding_classes
        .create(&w.actor, head(&code, &code, &nature, None))
        .await
        .unwrap_or_else(|e| panic!("建立編碼類別失敗：{e}"));
}

#[given(
    regex = r#"^編碼類別 "([^"]+)" 已有一段：型態 "([^"]+)"(?:，長度 (\d+))?(?:，(?:固定值|獨立段) "([^"]*)")?(?:，說明 "([^"]*)")?(，名稱顯示)?$"#
)]
async fn has_segment(
    w: &mut ErpWorld,
    code: String,
    kind: String,
    length: String,
    value: String,
    description: String,
    show: String,
) {
    let input = EncodingClassSegmentInput {
        show_name: !show.is_empty(),
        ..segment(&kind, &length, &value, &description)
    };
    w.encoding_classes
        .add_segment(&w.actor, &code, input)
        .await
        .unwrap_or_else(|e| panic!("新增一段失敗：{e}"));
}

// ----- 料件自動編號（item_auto_number.feature） -----

#[given(regex = r#"^整體管理系統參數的料件編號自動編碼為 "(是|否)"$"#)]
async fn set_auto(w: &mut ErpWorld, v: String) {
    let cur = w.general.get(&w.actor).await.expect("參數");
    let input = ttrs_shared::GeneralParametersInput {
        accounting_periods: cur.accounting_periods,
        base_currency: cur.base_currency.clone(),
        rate_basis: cur.rate_basis.clone(),
        auto_item_number: Some(v == "是"),
        version: Some(cur.version),
    };
    w.general.update(&w.actor, input).await.expect("修改參數");
}

#[then(regex = r#"^整體管理系統參數的料件編號自動編碼應為 "(是|否)"$"#)]
async fn auto_is(w: &mut ErpWorld, v: String) {
    let cur = w.general.get(&w.actor).await.expect("參數");
    assert_eq!(yes_no(cur.auto_item_number), v);
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]+)" 以編碼類別 "([^"]+)" 取料號(?:，獨立段選 "([^"]*)")?$"#
)]
async fn next_item_no(w: &mut ErpWorld, plant: String, code: String, pick: String) {
    let picks: Vec<String> = if pick.is_empty() {
        Vec::new()
    } else {
        pick.split('、').map(str::to_owned).collect()
    };
    let r = w
        .encoding_classes
        .next_item_number(&w.actor, &plant, &code, &picks)
        .await;
    w.encoding_preview = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

#[then(regex = r#"^取得的料號應為 "([^"]+)"，品名 "([^"]*)"$"#)]
async fn item_no_is(w: &mut ErpWorld, no: String, name: String) {
    ok(w);
    let p = w.encoding_preview.as_ref().expect("尚未取號");
    assert_eq!(
        (p.number.as_str(), p.name.as_str()),
        (no.as_str(), name.as_str())
    );
}

#[when(regex = r#"^使用者在營運中心 "([^"]+)" 用取得的料號新增料件$"#)]
async fn create_with_number(w: &mut ErpWorld, plant: String) {
    let p = w.encoding_preview.clone().expect("尚未取號");
    let class = ttrs_shared::ProductClassInput {
        class_code: "P01".into(),
        name: Some("產品".into()),
        ..Default::default()
    };
    let _ = w.product_classes.create(&w.actor, &plant, class).await;
    let req = ttrs_shared::ItemInput {
        item_no: p.number.clone(),
        name: Some(p.name.clone()),
        stock_unit: Some("PCS".into()),
        product_class: Some("P01".into()),
        ..Default::default()
    };
    let r = w.items.create(&w.actor, &plant, req).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]+)" 查詢可取料號的編碼類別$"#)]
async fn item_options(w: &mut ErpWorld, _plant: String) {
    let r = w.encoding_classes.item_number_options(&w.actor).await;
    w.item_number_options = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

#[then(regex = r#"^可取料號的編碼類別應為 "([^"]*)"$"#)]
async fn options_are(w: &mut ErpWorld, expected: String) {
    ok(w);
    let o = w.item_number_options.as_ref().expect("尚未查詢");
    let got: Vec<&str> = o.classes.iter().map(|c| c.class_code.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}

#[then(regex = r#"^編碼類別 "([^"]+)" 的獨立段 "([^"]+)" 可選 "([^"]*)"$"#)]
async fn options_values(w: &mut ErpWorld, code: String, seg: String, expected: String) {
    let o = w.item_number_options.as_ref().expect("尚未查詢");
    let c = o
        .classes
        .iter()
        .find(|c| c.class_code == code)
        .expect("沒有這個類別");
    let s = c
        .segments
        .iter()
        .find(|s| s.segment_code == seg)
        .expect("沒有這個獨立段");
    let got: Vec<String> = s
        .values
        .iter()
        .map(|(v, n)| format!("{v} {}", n.clone().unwrap_or_default()))
        .collect();
    assert_eq!(got.join("、"), expected);
}

// ----- 編碼試算（encoding_rule.feature） -----

#[when(
    regex = r#"^使用者在營運中心 "([^"]+)" 以編碼類別 "([^"]+)" 試算(?:，獨立段選 "([^"]*)")?$"#
)]
async fn preview(w: &mut ErpWorld, plant: String, code: String, pick: String) {
    let picks: Vec<String> = if pick.is_empty() {
        Vec::new()
    } else {
        pick.split('、').map(str::to_owned).collect()
    };
    let r = w
        .encoding_classes
        .preview(&w.actor, &plant, &code, &picks)
        .await;
    w.encoding_preview = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

#[then(regex = r#"^試算的(號碼|名稱)應為 "([^"]*)"$"#)]
async fn preview_is(w: &mut ErpWorld, what: String, expected: String) {
    ok(w);
    let p = w.encoding_preview.as_ref().expect("尚未試算");
    let got = if what == "號碼" { &p.number } else { &p.name };
    assert_eq!(got, &expected);
}

#[then(regex = r#"^試算應提示 "([A-Z_]+)"$"#)]
async fn preview_notice(w: &mut ErpWorld, code: String) {
    ok(w);
    let p = w.encoding_preview.as_ref().expect("尚未試算");
    assert!(p.notices.contains(&code), "{:?}", p.notices);
}

#[given(regex = r#"^使用者已(停用|確認)編碼類別 "([^"]+)"$"#)]
async fn already(w: &mut ErpWorld, what: String, code: String) {
    let r = if what == "停用" {
        w.encoding_classes
            .set_active(&w.actor, &code, false, None)
            .await
    } else {
        w.encoding_classes
            .confirm(&w.actor, &code, true, None)
            .await
    };
    r.unwrap_or_else(|e| panic!("{what}失敗：{e}"));
}

#[given(regex = r#"^另一位使用者已修改編碼類別 "([^"]+)"$"#)]
async fn changed_by_other(w: &mut ErpWorld, code: String) {
    let c = class(w, &code).await;
    w.class_version = Some(c.version);
    w.encoding_classes
        .update(
            &w.actor,
            &code,
            head(&code, "他人修改", &c.nature_code, Some(c.version)),
        )
        .await
        .expect("他人修改");
}

#[when(regex = r#"^使用者新增編碼類別 "([^"]*)"，名稱 "([^"]*)"，性質 "([^"]*)"$"#)]
async fn add(w: &mut ErpWorld, code: String, name: String, nature: String) {
    let r = w
        .encoding_classes
        .create(&w.actor, head(&code, &name, &nature, None))
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者在編碼類別 "([^"]+)" (?:的段次 (\d+) 插入|新增)一段：型態 "([^"]+)"(?:，長度 (\d+))?(?:，(?:固定值|獨立段) "([^"]*)")?(?:，說明 "([^"]*)")?$"#
)]
async fn add_segment(
    w: &mut ErpWorld,
    code: String,
    at: String,
    kind: String,
    length: String,
    value: String,
    description: String,
) {
    let input = segment(&kind, &length, &value, &description);
    let r = match at.parse::<i32>() {
        Ok(at) => {
            w.encoding_classes
                .insert_segment(&w.actor, &code, at, input)
                .await
        }
        Err(_) => w.encoding_classes.add_segment(&w.actor, &code, input).await,
    };
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者刪除編碼類別 "([^"]+)" 的段次 (\d+)$"#)]
async fn remove_segment(w: &mut ErpWorld, code: String, no: i32) {
    let r = w
        .encoding_classes
        .remove_segment(&w.actor, &code, no, None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者重排編碼類別 "([^"]+)" 的段次$"#)]
async fn renumber(w: &mut ErpWorld, code: String) {
    let r = w.encoding_classes.renumber(&w.actor, &code, None).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(以舊版本)?修改編碼類別 "([^"]+)"，名稱 "([^"]*)"，性質 "([^"]*)"$"#)]
async fn modify(w: &mut ErpWorld, stale: String, code: String, name: String, nature: String) {
    let version = if stale.is_empty() {
        w.encoding_classes
            .get(&w.actor, &code)
            .await
            .ok()
            .map(|c| c.version)
    } else {
        w.class_version
    };
    let r = w
        .encoding_classes
        .update(&w.actor, &code, head(&code, &name, &nature, version))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(確認|取消確認|停用|啟用)編碼類別 "([^"]+)"$"#)]
async fn state(w: &mut ErpWorld, what: String, code: String) {
    let c = &w.encoding_classes;
    let a = &w.actor;
    let r = match what.as_str() {
        "確認" => c.confirm(a, &code, true, None).await,
        "取消確認" => c.confirm(a, &code, false, None).await,
        "停用" => c.set_active(a, &code, false, None).await,
        _ => c.set_active(a, &code, true, None).await,
    };
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者刪除編碼類別 "([^"]+)"$"#)]
async fn delete(w: &mut ErpWorld, code: String) {
    let r = w.encoding_classes.delete(&w.actor, &code, None).await;
    w.last = Some(r);
}

#[when(regex = r#"^使用者把編碼類別 "([^"]+)" 複製為 "([^"]+)"$"#)]
async fn copy(w: &mut ErpWorld, from: String, to: String) {
    let r = w.encoding_classes.copy(&w.actor, &from, &to).await;
    w.last = Some(r.map(drop));
}

#[when("使用者查詢編碼類別")]
async fn query_all(w: &mut ErpWorld) {
    let r = w
        .encoding_classes
        .query(&w.actor, QueryRequest::default())
        .await;
    crate::steps::common::set_query_result(w, r, |c| c.class_code.clone());
}

#[when("使用者以下列條件查詢編碼類別:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.encoding_classes.query(&w.actor, req).await;
    crate::steps::common::set_query_result(w, r, |c| c.class_code.clone());
}

#[when(regex = r"^使用者(列印編碼類別|匯出編碼類別為 Excel)$")]
async fn report(w: &mut ErpWorld, what: String) {
    let format = if what.starts_with("列印") {
        ReportFormat::Pdf
    } else {
        ReportFormat::Excel
    };
    let r = w
        .encoding_classes
        .report_in(&w.actor, Vec::new(), format, &Lang::default_lang())
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[then(regex = r#"^編碼類別 "([^"]+)" 應為:$"#)]
async fn is(w: &mut ErpWorld, code: String, step: &Step) {
    ok(w);
    let c = class(w, &code).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "名稱" => c.name.clone(),
            "性質" => c.nature_code.clone(),
            "總段數" => c.segment_count.to_string(),
            "總長度" => c.total_length.to_string(),
            "資料有效" => yes_no(c.active).to_owned(),
            "已確認" => yes_no(c.confirmed).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(regex = r#"^編碼類別 "([^"]+)" 的各段應為:$"#)]
async fn segments_are(w: &mut ErpWorld, code: String, step: &Step) {
    ok(w);
    let got: Vec<Vec<String>> = class(w, &code)
        .await
        .segments
        .into_iter()
        .map(|s| {
            vec![
                s.seq_no.to_string(),
                s.description.unwrap_or_default(),
                s.length.to_string(),
                type_label(&s.segment_type).to_owned(),
                s.value.unwrap_or_default(),
                yes_no(s.show_name).to_owned(),
            ]
        })
        .collect();
    assert_eq!(got, rows(step));
}

#[then(regex = r#"^編碼類別 "([^"]+)" 各段的段次與型態應為 "([^"]*)"$"#)]
async fn nos_are(w: &mut ErpWorld, code: String, expected: String) {
    ok(w);
    let got: Vec<String> = class(w, &code)
        .await
        .segments
        .into_iter()
        .map(|s| format!("{} {}", s.seq_no, type_label(&s.segment_type)))
        .collect();
    assert_eq!(got.join("、"), expected);
}

#[then(regex = r#"^編碼類別 "([^"]+)" 的最後修改者應為 "([^"]+)"$"#)]
async fn updated_by(w: &mut ErpWorld, code: String, user: String) {
    ok(w);
    assert_eq!(class(w, &code).await.updated_by, Some(user));
}

#[then(regex = r#"^編碼類別 "([^"]+)" 的(資料有效|已確認)應為 "(是|否)"$"#)]
async fn flag_is(w: &mut ErpWorld, code: String, what: String, expected: String) {
    ok(w);
    let c = class(w, &code).await;
    let v = if what == "資料有效" {
        c.active
    } else {
        c.confirmed
    };
    assert_eq!(yes_no(v), expected);
}

#[then(regex = r#"^編碼類別 "([^"]+)" 應不存在$"#)]
async fn gone(w: &mut ErpWorld, code: String) {
    let r = w.encoding_classes.get(&w.actor, &code).await;
    assert_eq!(r.err().map(|e| e.code()), Some("ENCODING_CLASS_NOT_FOUND"));
}

#[then(regex = r#"^編碼類別清單順序應為 "([^"]*)"$"#)]
async fn order_is(w: &mut ErpWorld, expected: String) {
    ok(w);
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^編碼類別 "([^"]+)" 的異動記錄應包含單頭的 "([^"]+)" 與單身的 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, code: String, head_op: String, line_op: String) {
    let h = w
        .shared_history
        .history(&w.actor, PROGRAM, &code, None)
        .await
        .expect("異動記錄");
    let has = |table: &str, op: &str| h.iter().any(|c| c.table_name == table && c.operation == op);
    assert!(has("encoding_class", &head_op), "{h:?}");
    assert!(has("encoding_class_segment", &line_op), "{h:?}");
}
