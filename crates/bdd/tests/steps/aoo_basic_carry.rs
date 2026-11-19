//! 基本資料拋轉與拋轉歷史查詢 — specs/features/aoo/basic_data_carry.feature（BC1–BC12）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::{ActorKind, report::ReportFormat};
use ttrs_domain::lang::Lang;
use ttrs_shared::{BasicCarryRequest, QueryRequest};

use crate::{ErpWorld, steps::common::rows};

/// 畫面上的項目名稱 ↔ 代號（BC3）
const ITEMS: [(&str, &str); 9] = [
    ("部門", "department"),
    ("員工", "employee"),
    ("材料類別", "material_category"),
    ("其他分群碼一", "other_group_1"),
    ("其他分群碼二", "other_group_2"),
    ("其他分群碼三", "other_group_3"),
    ("其他分群碼四", "other_group_4"),
    ("產品分類", "product_class"),
    ("料件分群", "item_group"),
];

fn item_code(label: &str) -> String {
    ITEMS
        .iter()
        .find(|(l, _)| *l == label)
        .map(|(_, c)| (*c).to_owned())
        .unwrap_or_else(|| label.to_owned())
}

fn item_label(code: &str) -> String {
    ITEMS
        .iter()
        .find(|(_, c)| *c == code)
        .map(|(l, _)| (*l).to_owned())
        .unwrap_or_else(|| code.to_owned())
}

fn split(list: &str) -> Vec<String> {
    list.split('、')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn action_label(a: &str) -> &'static str {
    if a == "insert" { "新增" } else { "覆蓋" }
}

fn field_key(label: &str) -> &'static str {
    match label {
        "資料中心" => "center_code",
        "資料類別" => "data_type",
        "拋轉營運中心" => "plant",
        "程式代號" => "program",
        "主鍵值" => "record_key",
        "拋轉人員" => "carried_by",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

async fn carry(w: &mut ErpWorld, plant: &str, items: &str, targets: Option<Vec<String>>) {
    let req = BasicCarryRequest {
        items: split(items).iter().map(|l| item_code(l)).collect(),
        targets,
    };
    let r = w.basic_carry.carry(&w.actor, plant, req).await;
    w.last = Some(r.map(|r| {
        w.carry_result = Some(r);
    }));
}

// ----- 假設 -----

#[given(regex = r#"^目前操作者為 AI，代表使用者 "([^"]*)"$"#)]
fn acting_as_ai(w: &mut ErpWorld, user: String) {
    w.actor.user_id = user;
    w.actor.kind = ActorKind::Ai;
}

// ----- 當 -----

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 開啟基本資料拋轉$"#)]
async fn open(w: &mut ErpWorld, plant: String) {
    let r = w.basic_carry.info(&w.actor, &plant).await;
    w.last = Some(r.map(|i| {
        w.carry_info = Some(i);
    }));
}

#[given(regex = r#"^使用者在營運中心 "([^"]*)" 拋轉基本資料 "([^"]*)" 到 "([^"]*)"$"#)]
#[when(regex = r#"^使用者在營運中心 "([^"]*)" 拋轉基本資料 "([^"]*)" 到 "([^"]*)"$"#)]
async fn carry_to(w: &mut ErpWorld, plant: String, items: String, targets: String) {
    carry(w, &plant, &items, Some(split(&targets))).await;
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 拋轉基本資料 "([^"]*)" 到全部拋轉營運中心$"#)]
async fn carry_all(w: &mut ErpWorld, plant: String, items: String) {
    carry(w, &plant, &items, None).await;
}

#[when("使用者以下列條件查詢拋轉歷史:")]
async fn query_history(w: &mut ErpWorld, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.basic_carry.history(&w.actor, req).await;
    w.last = Some(r.map(|p| {
        w.carry_history = Some(p.items);
    }));
}

#[when(regex = r"^使用者(列印拋轉歷史|匯出拋轉歷史為 Excel)$")]
async fn report(w: &mut ErpWorld, what: String) {
    let format = if what.starts_with("列印") {
        ReportFormat::Pdf
    } else {
        ReportFormat::Excel
    };
    let r = w
        .basic_carry
        .history_report(&w.actor, Vec::new(), format, &Lang::default_lang())
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

// ----- 那麼 -----

fn info(w: &ErpWorld) -> ttrs_shared::BasicCarryInfoDto {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    w.carry_info.clone().expect("尚未開啟基本資料拋轉")
}

#[then(regex = r#"^基本資料拋轉的資料中心應為 "([^"]*)"$"#)]
fn center_is(w: &mut ErpWorld, expected: String) {
    assert_eq!(info(w).center_code, expected);
}

#[then(regex = r#"^可拋轉的基本資料應為 "([^"]*)"$"#)]
fn items_are(w: &mut ErpWorld, expected: String) {
    let got: Vec<String> = info(w).items.iter().map(|c| item_label(c)).collect();
    assert_eq!(got.join("、"), expected);
}

#[then(regex = r#"^基本資料拋轉的拋轉營運中心應為 "([^"]*)"$"#)]
fn targets_are(w: &mut ErpWorld, expected: String) {
    assert_eq!(info(w).targets.join("、"), expected);
}

#[then("拋轉結果應為:")]
fn result_is(w: &mut ErpWorld, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("拋轉失敗：{e}");
    }
    let r = w.carry_result.clone().expect("尚未拋轉");
    let got: Vec<Vec<String>> = r
        .rows
        .iter()
        .map(|x| {
            vec![
                x.plant.clone(),
                item_label(&x.item),
                x.inserted.to_string(),
                x.updated.to_string(),
                x.skipped.to_string(),
                if x.ok { "成功" } else { "失敗" }.to_owned(),
            ]
        })
        .collect();
    assert_eq!(got, rows(step), "{:?}", r.rows);
}

#[then(regex = r#"^拋轉紀錄應有 (\d+) 筆，營運中心 "([^"]*)"，動作 "([^"]*)"$"#)]
async fn log_count(w: &mut ErpWorld, n: usize, plant: String, action: String) {
    let page = w
        .basic_carry
        .history(&w.actor, QueryRequest::default())
        .await
        .expect("拋轉歷史");
    let got = page
        .items
        .iter()
        .filter(|l| l.plant == plant && action_label(&l.action) == action)
        .count();
    assert_eq!(got, n, "{:?}", page.items);
}

#[then(regex = r#"^"([^"]*)" 類別 "([^"]*)" 分類碼 "([^"]*)" 應不存在$"#)]
async fn code_gone(w: &mut ErpWorld, plant: String, kind: String, code: String) {
    let list = w
        .classification
        .list(&w.actor, &plant, &kind)
        .await
        .expect("分類碼清單");
    assert!(!list.iter().any(|c| c.code == code), "{list:?}");
}

#[then("拋轉歷史應為:")]
fn history_is(w: &mut ErpWorld, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let got: Vec<Vec<String>> = w
        .carry_history
        .clone()
        .expect("尚未查詢")
        .into_iter()
        .map(|l| {
            vec![
                l.center_code,
                l.data_type,
                l.plant,
                l.program,
                l.record_key,
                action_label(&l.action).to_owned(),
                l.carried_by,
            ]
        })
        .collect();
    assert_eq!(got, rows(step));
}

#[then(regex = r"^拋轉歷史應有 (\d+) 筆$")]
fn history_count(w: &mut ErpWorld, n: usize) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    assert_eq!(w.carry_history.clone().expect("尚未查詢").len(), n);
}
