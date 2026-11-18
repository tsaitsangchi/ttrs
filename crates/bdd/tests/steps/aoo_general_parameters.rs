//! 整體管理系統參數 — specs/features/aoo/general_parameters.feature（GP，TipTop aoos010 / aza_file）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::Actor;
use ttrs_shared::{GeneralParametersDto, GeneralParametersInput};

use crate::{ErpWorld, steps::common::rows};

const PROGRAM: &str = "general-management-parameters";

/// 規格上的匯率採用 → 代碼
fn basis_code(label: &str) -> &'static str {
    match label {
        "每月" => "monthly",
        "每日" => "daily",
        other => panic!("未知的匯率採用：{other}"),
    }
}

fn basis_label(code: &str) -> &'static str {
    match code {
        "monthly" => "每月",
        "daily" => "每日",
        other => panic!("未知的匯率採用代碼：{other}"),
    }
}

/// 規格上的欄位名稱 → 異動記錄的欄位
fn column_of(label: &str) -> &'static str {
    match label {
        "會計期間" => "accounting_periods",
        "本國幣別" => "base_currency",
        "匯率採用" => "rate_basis",
        other => panic!("未知欄位：{other}"),
    }
}

async fn current(w: &ErpWorld) -> GeneralParametersDto {
    w.general
        .get(&w.actor)
        .await
        .unwrap_or_else(|e| panic!("查看整體管理系統參數失敗：{e}"))
}

/// 以表格（欄位、值）修改目前的參數；`version` 為 None 時用目前的版本
async fn update(w: &mut ErpWorld, actor: &Actor, step: &Step, version: Option<i32>) {
    let cur = current(w).await;
    let mut input = GeneralParametersInput {
        accounting_periods: cur.accounting_periods,
        base_currency: cur.base_currency.clone(),
        rate_basis: cur.rate_basis.clone(),
        auto_item_number: None,
        version: Some(version.unwrap_or(cur.version)),
    };
    for r in rows(step) {
        let (label, value) = (r[0].as_str(), r[1].as_str());
        match label {
            "會計期間" => {
                input.accounting_periods = value
                    .parse()
                    .unwrap_or_else(|_| panic!("會計期間須為數字：{value}"))
            }
            "本國幣別" => input.base_currency = value.to_owned(),
            "匯率採用" => input.rate_basis = basis_code(value).to_owned(),
            other => panic!("未知欄位：{other}"),
        }
    }
    let r = w.general.update(actor, input).await;
    w.last = Some(r.map(drop));
}

#[when("使用者查看整體管理系統參數")]
async fn view(w: &mut ErpWorld) {
    let r = w.general.get(&w.actor).await;
    w.last = Some(r.map(drop));
}

#[then(expr = "整體管理系統參數應為:")]
async fn params_are(w: &mut ErpWorld, step: &Step) {
    let p = current(w).await;
    for r in rows(step) {
        let (label, expected) = (r[0].as_str(), r[1].as_str());
        let actual = match label {
            "會計期間" => p.accounting_periods.to_string(),
            "本國幣別" => p.base_currency.clone(),
            "匯率採用" => basis_label(&p.rate_basis).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, expected, "{label}");
    }
}

#[when(expr = "使用者修改整體管理系統參數:")]
async fn modify(w: &mut ErpWorld, step: &Step) {
    let actor = w.actor.clone();
    update(w, &actor, step, None).await;
}

#[then(expr = "整體管理系統參數的最後修改者應為 {string}")]
async fn modified_by(w: &mut ErpWorld, user: String) {
    let p = current(w).await;
    assert_eq!(p.updated_by.as_deref(), Some(user.as_str()));
}

#[given("另一位使用者已修改整體管理系統參數")]
async fn other_modified(w: &mut ErpWorld) {
    let cur = current(w).await;
    w.param_version = Some(cur.version);
    let other = Actor {
        user_id: "other".into(),
        ..w.actor.clone()
    };
    let input = GeneralParametersInput {
        accounting_periods: cur.accounting_periods,
        base_currency: cur.base_currency.clone(),
        rate_basis: if cur.rate_basis == "daily" {
            "monthly".into()
        } else {
            "daily".into()
        },
        auto_item_number: None,
        version: Some(cur.version),
    };
    w.general
        .update(&other, input)
        .await
        .unwrap_or_else(|e| panic!("另一位使用者修改失敗：{e}"));
}

#[when(expr = "使用者以舊的版本修改整體管理系統參數:")]
async fn modify_stale(w: &mut ErpWorld, step: &Step) {
    let actor = w.actor.clone();
    let version = w.param_version.expect("尚未記錄舊的版本");
    update(w, &actor, step, Some(version)).await;
}

#[then(regex = r#"^整體管理系統參數的異動記錄應包含 "(\S+)" 由 "(\S+)" 改為 "(\S+)"$"#)]
async fn history_has(w: &mut ErpWorld, label: String, before: String, after: String) {
    let column = column_of(&label);
    let code = |v: &str| {
        if column == "rate_basis" {
            basis_code(v).to_owned()
        } else {
            v.to_owned()
        }
    };
    let (before, after) = (code(&before), code(&after));
    let changes = w
        .shared_history
        .history(&w.actor, PROGRAM, "0", None)
        .await
        .unwrap_or_else(|e| panic!("查詢異動記錄失敗：{e}"));
    let found = changes.iter().any(|c| {
        c.fields.iter().any(|f| {
            f.column == column
                && f.before.as_deref() == Some(before.as_str())
                && f.after.as_deref() == Some(after.as_str())
        })
    });
    assert!(found, "異動記錄：{changes:?}");
}

#[given(expr = "整體管理系統參數的匯率採用為 {string}")]
async fn basis_is(w: &mut ErpWorld, label: String) {
    let cur = current(w).await;
    let input = GeneralParametersInput {
        accounting_periods: cur.accounting_periods,
        base_currency: cur.base_currency.clone(),
        rate_basis: basis_code(&label).to_owned(),
        auto_item_number: None,
        version: Some(cur.version),
    };
    let actor = w.actor.clone();
    w.general
        .update(&actor, input)
        .await
        .unwrap_or_else(|e| panic!("設定匯率採用失敗：{e}"));
}

// ----- 匯率維護的提示（GP7） -----

#[when(expr = "使用者查看 {string} 的匯率設定")]
async fn rate_settings(w: &mut ErpWorld, program: String) {
    let r = w.rates.settings(&w.actor, &program).await;
    w.rate_settings = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

#[then(expr = "匯率設定的提示應為 {string}")]
async fn rate_hint(w: &mut ErpWorld, expected: String) {
    let s = w.rate_settings.clone().expect("尚未查看匯率設定");
    let actual = match s.hint.as_deref() {
        None => "無",
        Some("monthly-unused") => "每月匯率不會被單據取用",
        Some("daily-unused") => "每日匯率不會被單據取用",
        Some(other) => panic!("未知的提示：{other}"),
    };
    assert_eq!(actual, expected);
}
