//! 維護作業標準功能鈕的共用步驟（docs/adr/0013-standard-maintenance-actions.md）：
//! 確認／取消確認（SA9）、列印與匯出 Excel（SA3）。各作業的規格 `*_standard_actions.feature` 共用，
//! 依步驟中的作業資料名稱（營運中心、使用者…）分派到對應的 use case。
//! 報表內容的檢查沿用部門報表的步驟（aoo_department_report.rs）。

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::{AppError, report::ReportFormat};
use ttrs_domain::lang::Lang;
use ttrs_shared::QbeCondition;

use crate::{ErpWorld, steps::common::rows};

/// 確認狀態（已確認否、確認人、確認時間是否有值）
struct ConfirmState {
    confirmed: bool,
    by: Option<String>,
}

/// 確認（true）或取消確認（false）
async fn set_confirmed(
    w: &ErpWorld,
    entity: &str,
    key: &str,
    confirm: bool,
) -> Result<(), AppError> {
    let a = &w.actor;
    match entity {
        "營運中心" => w.plants.confirm(a, key, confirm, None).await.map(drop),
        "使用者" => w.users.confirm(a, key, confirm, None).await.map(drop),
        "權限類別" => w.classes.confirm(a, key, confirm, None).await.map(drop),
        other => panic!("尚未支援確認的作業資料：{other}"),
    }
}

async fn confirm_state(w: &ErpWorld, entity: &str, key: &str) -> ConfirmState {
    let a = &w.actor;
    match entity {
        "營運中心" => {
            let p = w.plants.get(a, key).await.unwrap();
            ConfirmState {
                confirmed: p.confirmed,
                by: p.confirmed_by,
            }
        }
        "使用者" => {
            let u = w.users.get(a, key).await.unwrap();
            ConfirmState {
                confirmed: u.confirmed,
                by: u.confirmed_by,
            }
        }
        "權限類別" => {
            let c = w.classes.get(a, key).await.unwrap();
            ConfirmState {
                confirmed: c.confirmed,
                by: c.confirmed_by,
            }
        }
        other => panic!("尚未支援確認的作業資料：{other}"),
    }
}

/// 查詢欄位名稱 → 鍵值（各作業報表條件用到的）
fn field_key(label: &str) -> &'static str {
    match label {
        "營運中心代碼" => "plant_id",
        "使用者代號" => "user_id",
        "權限類別代號" => "class_code",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

async fn report(
    w: &mut ErpWorld,
    entity: &str,
    conditions: Vec<QbeCondition>,
    format: ReportFormat,
) {
    let lang = Lang::default_lang();
    let a = w.actor.clone();
    let r = match entity {
        "營運中心" => w.plants.report_in(&a, conditions, format, &lang).await,
        "使用者" => w.users.report_in(&a, conditions, format, &lang).await,
        "權限類別" => w.classes.report_in(&a, conditions, format, &lang).await,
        other => panic!("尚未支援列印的作業資料：{other}"),
    };
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[given(regex = r#"^(營運中心|使用者|權限類別) "(\S+)" 已確認$"#)]
async fn given_confirmed(w: &mut ErpWorld, entity: String, key: String) {
    set_confirmed(w, &entity, &key, true)
        .await
        .unwrap_or_else(|e| panic!("確認{entity} {key} 失敗：{e}"));
}

#[when(regex = r#"^使用者(確認|取消確認)(營運中心|使用者|權限類別) "(\S+)"$"#)]
async fn when_confirm(w: &mut ErpWorld, op: String, entity: String, key: String) {
    let r = set_confirmed(w, &entity, &key, op == "確認").await;
    w.last = Some(r);
}

#[then(regex = r#"^(營運中心|使用者|權限類別) "(\S+)" 的確認狀態應為:$"#)]
async fn then_state(w: &mut ErpWorld, entity: String, key: String, step: &Step) {
    let s = confirm_state(w, &entity, &key).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "已確認" => if s.confirmed { "是" } else { "否" }.to_owned(),
            "確認人" => s.by.clone().unwrap_or_default(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[when(regex = r#"^使用者以條件「(\S+) = (\S+)」列印(營運中心|使用者|權限類別)$"#)]
async fn print_with(w: &mut ErpWorld, label: String, expr: String, entity: String) {
    let c = vec![QbeCondition {
        field: field_key(&label).to_owned(),
        expr,
    }];
    report(w, &entity, c, ReportFormat::Pdf).await;
}

#[when(regex = r#"^使用者不輸入條件匯出(營運中心|使用者|權限類別)為 Excel$"#)]
async fn export_all(w: &mut ErpWorld, entity: String) {
    report(w, &entity, Vec::new(), ReportFormat::Excel).await;
}

#[then(expr = "報表明細的第一欄應為 {string}")]
fn first_column(w: &mut ErpWorld, expected: String) {
    let r = w.report.as_ref().expect("尚未產生報表");
    let keys: Vec<&str> = r.rows.iter().map(|x| x[0].as_str()).collect();
    assert_eq!(keys.join(","), expected);
}

/// 複製（ADR 0013 SA5）
#[when(regex = r#"^使用者將(權限類別) "(\S+)" 複製為 "(\S+)"$"#)]
async fn copy(w: &mut ErpWorld, entity: String, from: String, to: String) {
    let a = w.actor.clone();
    let r = match entity.as_str() {
        "權限類別" => w.classes.copy(&a, &from, &to).await.map(drop),
        other => panic!("尚未支援複製的作業資料：{other}"),
    };
    w.last = Some(r);
}
