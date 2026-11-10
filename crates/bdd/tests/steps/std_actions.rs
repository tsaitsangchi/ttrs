//! 維護作業標準功能鈕的共用步驟（docs/adr/0013-standard-maintenance-actions.md）：
//! 確認／取消確認（SA9）、列印與匯出 Excel（SA3）。各作業的規格 `*_standard_actions.feature` 共用，
//! 依步驟中的作業資料名稱（營運中心、使用者…）分派到對應的 use case。
//! 報表內容的檢查沿用部門報表的步驟（aoo_department_report.rs）。

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::{AppError, report::ReportFormat};
use ttrs_domain::{basic_code::BasicCodeKind, lang::Lang};
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
        "單位" => w.units.confirm(a, key, confirm, None).await.map(drop),
        "幣別" => w.currencies.confirm(a, key, confirm, None).await.map(drop),
        "專案" => w
            .projects
            .confirm(a, "TW01", key, confirm, None)
            .await
            .map(drop),
        // 工廠層作業：營運中心 TW01
        "產品分類" => w
            .product_classes
            .confirm(a, "TW01", key, confirm, None)
            .await
            .map(drop),
        "部門" => w
            .departments
            .confirm(a, "TW01", key, confirm, None)
            .await
            .map(drop),
        "員工" => w
            .employees
            .confirm(a, "TW01", key, confirm, None)
            .await
            .map(drop),
        other => {
            let k = basic_kind(other).unwrap_or_else(|| panic!("尚未支援確認的作業資料：{other}"));
            w.basic_codes
                .confirm(a, k, key, confirm, None)
                .await
                .map(drop)
        }
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
        "單位" => {
            let u = w.units.get(a, key).await.unwrap();
            ConfirmState {
                confirmed: u.confirmed,
                by: u.confirmed_by,
            }
        }
        "專案" => {
            let x = w.projects.get(a, "TW01", key).await.unwrap();
            ConfirmState {
                confirmed: x.confirmed,
                by: x.confirmed_by,
            }
        }
        "幣別" => {
            let c = w.currencies.get(a, key).await.unwrap();
            ConfirmState {
                confirmed: c.confirmed,
                by: c.confirmed_by,
            }
        }
        "產品分類" => {
            let c = w.product_classes.get(a, "TW01", key).await.unwrap();
            ConfirmState {
                confirmed: c.confirmed,
                by: c.confirmed_by,
            }
        }
        "部門" => {
            let d = w.departments.get(a, "TW01", key).await.unwrap();
            ConfirmState {
                confirmed: d.confirmed,
                by: d.confirmed_by,
            }
        }
        "員工" => {
            let e = w.employees.get(a, "TW01", key).await.unwrap();
            ConfirmState {
                confirmed: e.confirmed,
                by: e.confirmed_by,
            }
        }
        other => {
            let k = basic_kind(other).unwrap_or_else(|| panic!("尚未支援確認的作業資料：{other}"));
            let c = w.basic_codes.get(a, k, key, None).await.unwrap();
            ConfirmState {
                confirmed: c.confirmed,
                by: c.confirmed_by,
            }
        }
    }
}

/// 基本代碼的作業資料名稱（basic_code_service.rs）
pub fn basic_kind(entity: &str) -> Option<BasicCodeKind> {
    Some(match entity {
        "區域" => BasicCodeKind::Region,
        "國別" => BasicCodeKind::Country,
        "地區" => BasicCodeKind::Area,
        "交運方式" => BasicCodeKind::ShippingMethod,
        "報表抬頭" => BasicCodeKind::ReportTitle,
        _ => return None,
    })
}

/// 查詢欄位名稱 → 鍵值（各作業報表條件用到的）
fn field_key(label: &str) -> &'static str {
    match label {
        "營運中心代碼" => "plant_id",
        "使用者代號" => "user_id",
        "權限類別代號" => "class_code",
        "員工編號" => "employee_id",
        "單位代碼" | "單位" => "unit_code",
        "幣別" => "currency_code",
        "專案編號" => "project_code",
        "產品分類" => "class_code",
        "程式代號" => "program_no",
        "區域" | "國別" | "地區" | "交運方式" | "抬頭代碼" => "code",
        "隸屬區域" | "隸屬國別" => "parent",
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
        "程式" => w.programs.report_in(&a, conditions, format, &lang).await,
        "程式執行權限" => w.permissions.report_in(&a, conditions, format, &lang).await,
        "使用者特殊權限" => w.special.report_in(&a, conditions, format, &lang).await,
        "單位" => w.units.report_in(&a, conditions, format, &lang).await,
        "幣別" => w.currencies.report_in(&a, conditions, format, &lang).await,
        "專案" => {
            w.projects
                .report_in(&a, "TW01", conditions, format, &lang)
                .await
        }
        "產品分類" => {
            w.product_classes
                .report_in(&a, "TW01", conditions, format, &lang)
                .await
        }
        "員工" => {
            w.employees
                .report_in(&a, "TW01", conditions, format, &lang)
                .await
        }
        other => {
            let k = basic_kind(other).unwrap_or_else(|| panic!("尚未支援列印的作業資料：{other}"));
            w.basic_codes
                .report_in(&a, k, conditions, format, &lang)
                .await
        }
    };
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[given(
    regex = r#"^(營運中心|使用者|權限類別|部門|員工|單位|幣別|專案|產品分類|區域|國別|地區|交運方式|報表抬頭) "(\S+)" 已確認$"#
)]
async fn given_confirmed(w: &mut ErpWorld, entity: String, key: String) {
    set_confirmed(w, &entity, &key, true)
        .await
        .unwrap_or_else(|e| panic!("確認{entity} {key} 失敗：{e}"));
}

#[when(
    regex = r#"^使用者(確認|取消確認)(營運中心|使用者|權限類別|部門|員工|單位|幣別|專案|產品分類|區域|國別|地區|交運方式|報表抬頭) "(\S+)"$"#
)]
async fn when_confirm(w: &mut ErpWorld, op: String, entity: String, key: String) {
    let r = set_confirmed(w, &entity, &key, op == "確認").await;
    w.last = Some(r);
}

#[then(
    regex = r#"^(營運中心|使用者|權限類別|部門|員工|單位|幣別|專案|產品分類|區域|國別|地區|交運方式|報表抬頭) "(\S+)" 的確認狀態應為:$"#
)]
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

#[when(
    regex = r#"^使用者以條件「(\S+) = (\S+)」列印(營運中心|使用者|權限類別|部門|員工|單位|幣別|專案|產品分類|程式|程式執行權限|使用者特殊權限|區域|國別|地區|交運方式|報表抬頭)$"#
)]
async fn print_with(w: &mut ErpWorld, label: String, expr: String, entity: String) {
    let c = vec![QbeCondition {
        field: field_key(&label).to_owned(),
        expr,
    }];
    report(w, &entity, c, ReportFormat::Pdf).await;
}

#[when(
    regex = r#"^使用者不輸入條件匯出(營運中心|使用者|權限類別|部門|員工|單位|幣別|專案|產品分類|區域|國別|地區|交運方式|報表抬頭)為 Excel$"#
)]
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
#[when(
    regex = r#"^使用者將(權限類別|部門|員工|單位|幣別|專案|產品分類|區域|國別|地區|交運方式|報表抬頭) "(\S+)" 複製為 "(\S+)"$"#
)]
async fn copy(w: &mut ErpWorld, entity: String, from: String, to: String) {
    let a = w.actor.clone();
    let r = match entity.as_str() {
        "權限類別" => w.classes.copy(&a, &from, &to).await.map(drop),
        "部門" => w.departments.copy(&a, "TW01", &from, &to).await.map(drop),
        "員工" => w.employees.copy(&a, "TW01", &from, &to).await.map(drop),
        "單位" => w.units.copy(&a, &from, &to).await.map(drop),
        "幣別" => w.currencies.copy(&a, &from, &to).await.map(drop),
        "專案" => w.projects.copy(&a, "TW01", &from, &to).await.map(drop),
        "產品分類" => w
            .product_classes
            .copy(&a, "TW01", &from, &to)
            .await
            .map(drop),
        other => {
            let k = basic_kind(other).unwrap_or_else(|| panic!("尚未支援複製的作業資料：{other}"));
            w.basic_codes.copy(&a, k, &from, &to).await.map(drop)
        }
    };
    w.last = Some(r);
}

/// 刪除（系統層作業）
#[when(regex = r#"^使用者刪除(單位|幣別|專案|區域|國別|地區|交運方式|報表抬頭) "(\S+)"$"#)]
async fn delete(w: &mut ErpWorld, entity: String, key: String) {
    let a = w.actor.clone();
    let r = match entity.as_str() {
        "單位" => w.units.delete(&a, &key, None).await,
        "幣別" => w.currencies.delete(&a, &key, None).await,
        "專案" => w.projects.delete(&a, "TW01", &key, None).await,
        other => {
            let k = basic_kind(other).unwrap_or_else(|| panic!("尚未支援刪除的作業資料：{other}"));
            w.basic_codes.delete(&a, k, &key, None).await
        }
    };
    w.last = Some(r);
}

/// QBE 查詢（ADR 0013 SA1：原本沒有查詢的作業）
#[when(
    regex = r#"^使用者以下列條件查詢(單位|幣別|專案|產品分類|區域|國別|地區|交運方式|報表抬頭):$"#
)]
async fn query(w: &mut ErpWorld, entity: String, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let a = w.actor.clone();
    match entity.as_str() {
        "單位" => {
            let r = w.units.query(&a, req).await;
            crate::steps::common::set_query_result(w, r, |u| u.unit_code.clone());
        }
        "專案" => {
            let r = w.projects.query(&a, "TW01", req).await;
            crate::steps::common::set_query_result(w, r, |x| x.project_code.clone());
        }
        "幣別" => {
            let r = w.currencies.query(&a, req).await;
            crate::steps::common::set_query_result(w, r, |c| c.currency_code.clone());
        }
        "產品分類" => {
            let r = w.product_classes.query(&a, "TW01", req).await;
            crate::steps::common::set_query_result(w, r, |c| c.class_code.clone());
        }
        other => {
            let k = basic_kind(other).unwrap_or_else(|| panic!("尚未支援查詢的作業資料：{other}"));
            let r = w.basic_codes.query(&a, k, req).await;
            crate::steps::common::set_query_result(w, r, |c| c.code.clone());
        }
    }
}

// ----- 分類碼（aooi300）：以類別區分，營運中心 TW01 -----

fn code_key(label: &str) -> &'static str {
    match label {
        "代號" => "code",
        "說明" => "description",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

#[given(expr = "類別 {string} 的分類碼 {string} 已確認")]
async fn code_confirmed(w: &mut ErpWorld, kind: String, code: String) {
    let a = w.actor.clone();
    w.classification
        .confirm(&a, "TW01", &kind, &code, true, None)
        .await
        .unwrap_or_else(|e| panic!("確認分類碼 {code} 失敗：{e}"));
}

#[when(regex = r#"^使用者(確認|取消確認)類別 "(\S+)" 的分類碼 "(\S+)"$"#)]
async fn code_confirm(w: &mut ErpWorld, op: String, kind: String, code: String) {
    let a = w.actor.clone();
    let r = w
        .classification
        .confirm(&a, "TW01", &kind, &code, op == "確認", None)
        .await;
    w.last = Some(r.map(drop));
}

#[then(expr = "類別 {string} 的分類碼 {string} 的確認狀態應為:")]
async fn code_state(w: &mut ErpWorld, kind: String, code: String, step: &Step) {
    let a = w.actor.clone();
    let list = w.classification.list(&a, "TW01", &kind).await.unwrap();
    let c = list.iter().find(|c| c.code == code).expect("分類碼");
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "已確認" => if c.confirmed { "是" } else { "否" }.to_owned(),
            "確認人" => c.confirmed_by.clone().unwrap_or_default(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[when(expr = "使用者將類別 {string} 的分類碼 {string} 複製為 {string}")]
async fn code_copy(w: &mut ErpWorld, kind: String, from: String, to: String) {
    let a = w.actor.clone();
    let r = w.classification.copy(&a, "TW01", &kind, &from, &to).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者以下列條件查詢類別 {string} 的分類碼:")]
async fn code_query(w: &mut ErpWorld, kind: String, step: &Step) {
    let req = crate::steps::common::query_request(step, code_key, None);
    let a = w.actor.clone();
    let r = w.classification.query(&a, "TW01", &kind, req).await;
    crate::steps::common::set_query_result(w, r, |c| c.code.clone());
}

#[when(expr = "使用者查詢類別 {string} 的分類碼")]
async fn code_query_all(w: &mut ErpWorld, kind: String) {
    let a = w.actor.clone();
    let r = w
        .classification
        .query(&a, "TW01", &kind, ttrs_shared::QueryRequest::default())
        .await;
    crate::steps::common::set_query_result(w, r, |c| c.code.clone());
}

async fn code_report(w: &mut ErpWorld, kind: &str, c: Vec<QbeCondition>, f: ReportFormat) {
    let a = w.actor.clone();
    let lang = Lang::default_lang();
    let r = w
        .classification
        .report_in(&a, "TW01", kind, c, f, &lang)
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[when(regex = r#"^使用者以條件「(\S+) = (\S+)」列印類別 "(\S+)" 的分類碼$"#)]
async fn code_print(w: &mut ErpWorld, label: String, expr: String, kind: String) {
    let c = vec![QbeCondition {
        field: code_key(&label).to_owned(),
        expr,
    }];
    code_report(w, &kind, c, ReportFormat::Pdf).await;
}

#[when(expr = "使用者不輸入條件匯出類別 {string} 的分類碼為 Excel")]
async fn code_export(w: &mut ErpWorld, kind: String) {
    code_report(w, &kind, Vec::new(), ReportFormat::Excel).await;
}

#[when(regex = r#"^使用者以條件「(\S+) = (\S+)」匯出(程式|程式執行權限|使用者特殊權限)為 Excel$"#)]
async fn export_with(w: &mut ErpWorld, label: String, expr: String, entity: String) {
    let c = vec![QbeCondition {
        field: field_key(&label).to_owned(),
        expr,
    }];
    report(w, &entity, c, ReportFormat::Excel).await;
}

/// 使用者特殊權限查詢（ADR 0013 SA1）：條件為使用者維護的查詢欄位
#[when(expr = "使用者以下列條件查詢使用者特殊權限:")]
async fn query_special(w: &mut ErpWorld, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let a = w.actor.clone();
    let r = w.special.query(&a, req).await;
    crate::steps::common::set_query_result(w, r, |u| u.user_id.clone());
}

// ----- 資料字典（p_zta）：一個資料表的欄位字典 -----

async fn dict_report(w: &mut ErpWorld, table: &str, f: ReportFormat) {
    let a = w.actor.clone();
    let lang = Lang::default_lang();
    let r = w.dictionary.report_in(&a, table, f, &lang).await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[when(expr = "使用者列印資料表 {string} 的字典")]
async fn dict_print(w: &mut ErpWorld, table: String) {
    dict_report(w, &table, ReportFormat::Pdf).await;
}

#[when(expr = "使用者匯出資料表 {string} 的字典為 Excel")]
async fn dict_export(w: &mut ErpWorld, table: String) {
    dict_report(w, &table, ReportFormat::Excel).await;
}

#[then("Excel 的明細筆數應與報表相同")]
fn excel_same_rows(w: &mut ErpWorld) {
    use calamine::{Reader, Xlsx};
    let r = w.report.as_ref().expect("尚未產生報表");
    let bytes = ttrs_infra::report::render_xlsx(r).expect("產生 Excel");
    let mut book: Xlsx<_> = Xlsx::new(std::io::Cursor::new(bytes)).expect("讀取 Excel");
    let sheet = book.worksheet_range_at(0).expect("工作表").expect("工作表");
    assert!(!r.rows.is_empty(), "報表應有明細");
    assert_eq!(sheet.rows().count() - 1, r.rows.len());
}
