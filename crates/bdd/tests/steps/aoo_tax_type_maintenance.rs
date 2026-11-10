//! 稅別維護 — specs/features/aoo/tax_type_maintenance.feature（TX1–TX9）
//! 鍵值為稅別碼＋進／銷項（如 "T5" 銷項），標準功能的步驟也寫在這裡。

use cucumber::{gherkin::Step, given, then, when};
use rust_decimal::Decimal;
use ttrs_shared::{TaxTypeDto, TaxTypeInput};

use crate::{ErpWorld, steps::common::rows};

/// 進項 → 1、銷項 → 2
fn direction(label: &str) -> &'static str {
    match label {
        "進項" | "1" => "1",
        "銷項" | "2" => "2",
        other => panic!("未知的進銷項：{other}"),
    }
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

/// 設定一個欄位（欄位名稱、值皆為規格上的文字）
fn set_field(req: &mut TaxTypeInput, label: &str, value: &str) {
    let opt = || Some(value.to_owned()).filter(|v| !v.is_empty());
    match label {
        "稅別碼" => req.tax_code = value.to_owned(),
        "進銷項" => req.direction = direction(value).to_owned(),
        "名稱" => req.name = opt(),
        "會計科目" => req.account = opt(),
        "會計科目二" => req.account2 = opt(),
        "發票聯數" => req.invoice_copies = opt(),
        "課稅別" => {
            req.tax_class = Some(
                match value {
                    "應稅" => "1",
                    "零稅率" => "2",
                    "免稅" => "3",
                    other => other,
                }
                .to_owned(),
            )
        }
        "稅率" => req.rate = Some(value.parse::<Decimal>().expect("稅率")),
        "單價含稅否" => req.include_tax = Some(value == "是"),
        "申報格式" => req.filing_format = opt(),
        other => panic!("未知欄位：{other}"),
    }
}

/// 「名稱 "應稅"，會計科目 "2194"，稅率 5」→ 各欄位
fn parse_fields(req: &mut TaxTypeInput, text: &str) {
    for part in text.split('，') {
        let (label, value) = part
            .split_once(' ')
            .unwrap_or_else(|| panic!("無法解析：{part}"));
        set_field(req, label, value.trim().trim_matches('"'));
    }
}

async fn create(w: &mut ErpWorld, req: TaxTypeInput) {
    let r = w.taxes.create(&w.actor, req).await;
    w.last = Some(r.map(drop));
}

async fn tax(w: &ErpWorld, code: &str, dir: &str) -> TaxTypeDto {
    w.taxes
        .get(&w.actor, code, direction(dir))
        .await
        .unwrap_or_else(|e| panic!("查詢稅別 {code} {dir} 失敗：{e}"))
}

#[given(regex = r#"^已有稅別 "(\S+)" (進項|銷項)$"#)]
async fn has(w: &mut ErpWorld, code: String, dir: String) {
    let d = direction(&dir);
    let req = TaxTypeInput {
        tax_code: code.clone(),
        direction: d.to_owned(),
        name: Some(code.clone()),
        account: Some(if d == "1" { "1144" } else { "2194" }.to_owned()),
        rate: Some(Decimal::new(5, 0)),
        filing_format: Some(if d == "1" { "21" } else { "31" }.to_owned()),
        ..Default::default()
    };
    create(w, req).await;
    if let Some(Err(e)) = &w.last {
        panic!("建立稅別 {code} {dir} 失敗：{e}");
    }
}

#[when(expr = "使用者新增稅別:")]
async fn add_table(w: &mut ErpWorld, step: &Step) {
    let mut req = TaxTypeInput::default();
    for r in rows(step) {
        set_field(&mut req, &r[0], &r[1]);
    }
    create(w, req).await;
}

#[when(regex = r#"^使用者新增稅別 "(\S+)" (進項|銷項)，(.+)$"#)]
async fn add(w: &mut ErpWorld, code: String, dir: String, rest: String) {
    let mut req = TaxTypeInput {
        tax_code: code,
        direction: direction(&dir).to_owned(),
        ..Default::default()
    };
    parse_fields(&mut req, &rest);
    create(w, req).await;
}

#[then(regex = r#"^稅別 "(\S+)" (進項|銷項) 應為:$"#)]
async fn tax_is(w: &mut ErpWorld, code: String, dir: String, step: &Step) {
    let t = tax(w, &code, &dir).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "名稱" => t.name.clone().unwrap_or_default(),
            "稅率" => t.rate.normalize().to_string(),
            "單價含稅否" => yes_no(t.include_tax).to_owned(),
            "申報格式" => t.filing_format.clone(),
            "資料有效" => yes_no(t.active).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[when(regex = r#"^使用者(停用|啟用)稅別 "(\S+)" (進項|銷項)$"#)]
async fn toggle(w: &mut ErpWorld, op: String, code: String, dir: String) {
    let r = w
        .taxes
        .set_active(&w.actor, &code, direction(&dir), op == "啟用", None)
        .await;
    w.last = Some(r.map(drop));
}

#[then(regex = r#"^稅別 "(\S+)" (進項|銷項) 的資料有效應為 "(是|否)"$"#)]
async fn active_is(w: &mut ErpWorld, code: String, dir: String, expected: String) {
    assert_eq!(yes_no(tax(w, &code, &dir).await.active), expected);
}

#[when(regex = r#"^使用者刪除稅別 "(\S+)" (進項|銷項)$"#)]
async fn delete(w: &mut ErpWorld, code: String, dir: String) {
    let r = w.taxes.delete(&w.actor, &code, direction(&dir), None).await;
    w.last = Some(r);
}

#[then(regex = r#"^稅別 "(\S+)" (進項|銷項) 應不存在$"#)]
async fn absent(w: &mut ErpWorld, code: String, dir: String) {
    let r = w.taxes.get(&w.actor, &code, direction(&dir)).await;
    assert!(r.is_err(), "稅別 {code} {dir} 應不存在");
}

#[when(expr = "使用者以下列條件查詢稅別:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    let key = |label: &str| -> &'static str {
        match label {
            "稅別碼" => "tax_code",
            "進銷項" => "direction",
            "名稱" => "name",
            other => panic!("未支援的查詢欄位：{other}"),
        }
    };
    let req = crate::steps::common::query_request(step, key, None);
    let r = w.taxes.query(&w.actor, req).await;
    crate::steps::common::set_query_result(w, r, |t| {
        let dir = if t.direction == "1" {
            "進項"
        } else {
            "銷項"
        };
        format!("{} {dir}", t.tax_code)
    });
}

#[when(regex = r#"^使用者將稅別 "(\S+)" (進項|銷項) 複製為 "(\S+)" (進項|銷項)$"#)]
async fn copy(w: &mut ErpWorld, code: String, dir: String, to: String, to_dir: String) {
    let r = w
        .taxes
        .copy(&w.actor, &code, direction(&dir), &to, direction(&to_dir))
        .await;
    w.last = Some(r.map(drop));
}

#[given(regex = r#"^稅別 "(\S+)" (進項|銷項) 已確認$"#)]
async fn confirmed(w: &mut ErpWorld, code: String, dir: String) {
    w.taxes
        .confirm(&w.actor, &code, direction(&dir), true, None)
        .await
        .unwrap_or_else(|e| panic!("確認稅別 {code} 失敗：{e}"));
}

#[then(regex = r#"^稅別 "(\S+)" (進項|銷項) 的確認狀態應為:$"#)]
async fn confirm_state(w: &mut ErpWorld, code: String, dir: String, step: &Step) {
    let t = tax(w, &code, &dir).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "已確認" => yes_no(t.confirmed).to_owned(),
            "確認人" => t.confirmed_by.clone().unwrap_or_default(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}
