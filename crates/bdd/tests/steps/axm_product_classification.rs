//! 產品分類維護 — specs/features/axm/product_classification.feature（PC1–PC9）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{DepartmentAccountingInput, DepartmentInput, ProductClassDto, ProductClassInput};

use crate::{ErpWorld, steps::common::rows};

const PLANT: &str = "TW01";

async fn create(w: &mut ErpWorld, plant: &str, input: ProductClassInput) {
    let r = w.product_classes.create(&w.actor, plant, input).await;
    w.last = Some(r.map(drop));
}

fn input(code: &str, name: &str) -> ProductClassInput {
    ProductClassInput {
        class_code: code.to_owned(),
        name: Some(name.to_owned()),
        ..ProductClassInput::default()
    }
}

async fn class(w: &ErpWorld, code: &str) -> ProductClassDto {
    w.product_classes
        .get(&w.actor, PLANT, code)
        .await
        .unwrap_or_else(|e| panic!("查詢產品分類 {code} 失敗：{e}"))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

/// 成本中心部門：管理類別 1、對應成本中心為自己（PC4）
#[given(expr = "{string} 已有成本中心部門 {string}")]
async fn cost_center_dept(w: &mut ErpWorld, plant: String, code: String) {
    let dept = DepartmentInput {
        dept_code: code.clone(),
        name: Some(code.clone()),
        full_name: None,
        version: None,
    };
    w.departments
        .create(&w.actor, &plant, dept)
        .await
        .expect("建立部門");
    let acct = DepartmentAccountingInput {
        accounting_dept: true,
        expense_type: None,
        control_type: "1".into(),
        cost_center: Some(code.clone()),
        version: None,
    };
    w.departments
        .set_accounting(&w.actor, &plant, &code, acct)
        .await
        .expect("設定成本中心");
}

#[given(expr = "營運中心 {string} 已有產品分類 {string}")]
async fn has_class(w: &mut ErpWorld, plant: String, code: String) {
    create(w, &plant, input(&code, &code)).await;
    if let Some(Err(e)) = &w.last {
        panic!("建立產品分類 {code} 失敗：{e}");
    }
}

#[when(expr = "使用者在營運中心 {string} 新增產品分類 {string}，名稱 {string}，成本中心 {string}")]
async fn add(w: &mut ErpWorld, plant: String, code: String, name: String, cc: String) {
    let req = ProductClassInput {
        cost_center: Some(cc).filter(|c| !c.is_empty()),
        ..input(&code, &name)
    };
    create(w, &plant, req).await;
}

#[when(expr = "使用者在營運中心 {string} 新增產品分類 {string}，期數 {int}")]
async fn add_periods(w: &mut ErpWorld, plant: String, code: String, periods: i32) {
    let req = ProductClassInput {
        periods: Some(periods),
        ..input(&code, &code)
    };
    create(w, &plant, req).await;
}

#[when(expr = "使用者將營運中心 {string} 產品分類 {string} 的名稱改為 {string}")]
async fn rename(w: &mut ErpWorld, plant: String, code: String, name: String) {
    let c = class(w, &code).await;
    let req = ProductClassInput {
        name: Some(name),
        ..c.to_input()
    };
    let r = w.product_classes.update(&w.actor, &plant, &code, req).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者停用營運中心 {string} 產品分類 {string}")]
async fn deactivate(w: &mut ErpWorld, plant: String, code: String) {
    let r = w
        .product_classes
        .set_active(&w.actor, &plant, &code, false, None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者刪除營運中心 {string} 產品分類 {string}")]
async fn delete(w: &mut ErpWorld, plant: String, code: String) {
    let r = w
        .product_classes
        .delete(&w.actor, &plant, &code, None)
        .await;
    w.last = Some(r);
}

#[then(expr = "產品分類 {string} 應為:")]
async fn class_is(w: &mut ErpWorld, code: String, step: &Step) {
    let c = class(w, &code).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "名稱" => c.name.clone().unwrap_or_default(),
            "成本中心" => c.cost_center.clone().unwrap_or_default(),
            "資料有效" => yes_no(c.active).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(expr = "產品分類 {string} 的名稱應為 {string}")]
async fn name_is(w: &mut ErpWorld, code: String, name: String) {
    assert_eq!(class(w, &code).await.name, Some(name));
}

#[then(expr = "產品分類 {string} 的資料有效應為 {string}")]
async fn active_is(w: &mut ErpWorld, code: String, expected: String) {
    assert_eq!(yes_no(class(w, &code).await.active), expected);
}

#[then(expr = "營運中心 {string} 應沒有產品分類 {string}")]
async fn absent(w: &mut ErpWorld, plant: String, code: String) {
    assert!(
        w.product_classes
            .get(&w.actor, &plant, &code)
            .await
            .is_err()
    );
}
