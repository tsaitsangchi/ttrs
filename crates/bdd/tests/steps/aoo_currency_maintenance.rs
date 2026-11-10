//! 幣別維護 — specs/features/aoo/currency_maintenance.feature（CY1–CY8）
//! 查詢、複製、確認、刪除等標準功能的步驟在 std_actions.rs。

use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{CurrencyDto, CurrencyInput};

use crate::{ErpWorld, steps::common::rows};

async fn create(w: &mut ErpWorld, req: CurrencyInput) {
    let r = w.currencies.create(&w.actor, req).await;
    w.last = Some(r.map(drop));
}

fn input(code: &str, name: &str) -> CurrencyInput {
    CurrencyInput {
        currency_code: code.to_owned(),
        name: Some(name.to_owned()),
        ..Default::default()
    }
}

async fn currency(w: &ErpWorld, code: &str) -> CurrencyDto {
    w.currencies
        .get(&w.actor, code)
        .await
        .unwrap_or_else(|e| panic!("查詢幣別 {code} 失敗：{e}"))
}

/// 小數位數欄位名稱 → 輸入欄位
fn decimals_field<'a>(req: &'a mut CurrencyInput, field: &str) -> &'a mut Option<i16> {
    match field {
        "單價小數位數" => &mut req.price_decimals,
        "金額小數位數" => &mut req.amount_decimals,
        "小計總計小數位數" => &mut req.total_decimals,
        "匯率小數位數" => &mut req.rate_decimals,
        other => panic!("未知欄位：{other}"),
    }
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

#[given(expr = "已有幣別 {string}")]
async fn has(w: &mut ErpWorld, code: String) {
    create(w, input(&code, &code)).await;
    if let Some(Err(e)) = &w.last {
        panic!("建立幣別 {code} 失敗：{e}");
    }
}

#[when(expr = "使用者新增幣別 {string}，說明 {string}")]
async fn add(w: &mut ErpWorld, code: String, name: String) {
    create(w, input(&code, &name)).await;
}

#[when(
    regex = r#"^使用者新增幣別 "(\S+)"，說明 "([^"]*)"，(單價小數位數|金額小數位數|小計總計小數位數|匯率小數位數) 為 (-?\d+)$"#
)]
async fn add_with_decimals(w: &mut ErpWorld, code: String, name: String, field: String, n: i16) {
    let mut req = input(&code, &name);
    *decimals_field(&mut req, &field) = Some(n);
    create(w, req).await;
}

#[when(expr = "使用者新增幣別 {string}，說明 {string}，計算方式 為 {string}")]
async fn add_with_method(w: &mut ErpWorld, code: String, name: String, method: String) {
    let mut req = input(&code, &name);
    req.rate_method = Some(method);
    create(w, req).await;
}

#[when(
    regex = r#"^使用者將幣別 "(\S+)" 的(單價小數位數|金額小數位數|小計總計小數位數|匯率小數位數)改為 (-?\d+)$"#
)]
async fn change_decimals(w: &mut ErpWorld, code: String, field: String, n: i16) {
    let mut req = currency(w, &code).await.to_input();
    *decimals_field(&mut req, &field) = Some(n);
    let r = w.currencies.update(&w.actor, &code, req).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(停用|啟用)幣別 "(\S+)"$"#)]
async fn toggle(w: &mut ErpWorld, op: String, code: String) {
    let r = w
        .currencies
        .set_active(&w.actor, &code, op == "啟用", None)
        .await;
    w.last = Some(r.map(drop));
}

#[then(expr = "幣別 {string} 應為:")]
async fn currency_is(w: &mut ErpWorld, code: String, step: &Step) {
    let c = currency(w, &code).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "說明" => c.name.clone().unwrap_or_default(),
            "單價小數位數" => c.price_decimals.to_string(),
            "金額小數位數" => c.amount_decimals.to_string(),
            "小計總計小數位數" => c.total_decimals.to_string(),
            "匯率小數位數" => c.rate_decimals.to_string(),
            "計算方式" => match c.rate_method.as_str() {
                "1" => "乘".to_owned(),
                "2" => "除".to_owned(),
                other => other.to_owned(),
            },
            "資料有效" => yes_no(c.active).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(regex = r#"^幣別 "(\S+)" 的金額小數位數應為 (\d+)$"#)]
async fn amount_decimals_are(w: &mut ErpWorld, code: String, n: i16) {
    assert_eq!(currency(w, &code).await.amount_decimals, n);
}

#[then(expr = "幣別 {string} 的資料有效應為 {string}")]
async fn active_is(w: &mut ErpWorld, code: String, expected: String) {
    assert_eq!(yes_no(currency(w, &code).await.active), expected);
}

#[then(expr = "幣別 {string} 應不存在")]
async fn absent(w: &mut ErpWorld, code: String) {
    assert!(
        w.currencies.get(&w.actor, &code).await.is_err(),
        "幣別 {code} 應不存在"
    );
}
