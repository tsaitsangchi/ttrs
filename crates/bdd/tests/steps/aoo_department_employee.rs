//! aooi030 部門維護、aooi040 員工維護 — specs/features/aoo/department_maintenance.feature、employee_maintenance.feature

use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{DepartmentDto, DepartmentInput, EmployeeDto, EmployeeInput};

use crate::{
    ErpWorld,
    steps::common::{query_request, set_query_result},
};

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn text(v: &str) -> Option<String> {
    (!v.is_empty()).then(|| v.to_owned())
}

async fn dept(w: &ErpWorld, plant: &str, code: &str) -> DepartmentDto {
    w.departments
        .get(&w.actor, plant, code)
        .await
        .unwrap_or_else(|e| panic!("讀取部門 {plant}/{code} 失敗：{e}"))
}

async fn employee(w: &ErpWorld, plant: &str, id: &str) -> EmployeeDto {
    w.employees
        .get(&w.actor, plant, id)
        .await
        .unwrap_or_else(|e| panic!("讀取員工 {plant}/{id} 失敗：{e}"))
}

fn dept_input(code: &str, name: &str, full_name: &str) -> DepartmentInput {
    DepartmentInput {
        version: None,
        dept_code: code.into(),
        name: text(name),
        full_name: text(full_name),
    }
}

fn employee_input(id: &str, name: &str, dept: &str, email: Option<String>) -> EmployeeInput {
    EmployeeInput {
        employee_id: id.into(),
        name: text(name),
        dept_code: text(dept),
        email,
        ..Default::default()
    }
}

/// 會計資料維護：讀出目前的會計欄位、修改後存檔
async fn set_accounting(
    w: &mut ErpWorld,
    plant: &str,
    code: &str,
    change: impl FnOnce(&mut ttrs_shared::DepartmentAccountingInput),
) {
    let mut req = dept(w, plant, code).await.to_accounting();
    change(&mut req);
    let result = w
        .departments
        .set_accounting(&w.actor, plant, code, req)
        .await;
    w.last = Some(result.map(|_| ()));
}

// ----- 部門：假設 -----

#[given(expr = "{string} 已有部門 {string}")]
async fn existing_dept(w: &mut ErpWorld, plant: String, code: String) {
    w.departments
        .create(&w.actor, &plant, dept_input(&code, &code, ""))
        .await
        .unwrap();
}

#[given(expr = "使用者停用 {string} 部門 {string}")]
async fn dept_deactivated(w: &mut ErpWorld, plant: String, code: String) {
    w.departments
        .deactivate(&w.actor, &plant, &code, None)
        .await
        .unwrap();
}

// ----- 部門：當 -----

#[when(expr = "使用者在 {string} 新增部門 {string}，簡稱 {string}，全稱 {string}")]
async fn create_dept(w: &mut ErpWorld, plant: String, code: String, name: String, full: String) {
    let result = w
        .departments
        .create(&w.actor, &plant, dept_input(&code, &name, &full))
        .await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者將 {string} 部門 {string} 的編號改為 {string}")]
async fn change_dept_code(w: &mut ErpWorld, plant: String, code: String, new_code: String) {
    let mut req = dept(w, &plant, &code).await.to_input();
    req.dept_code = new_code;
    let result = w.departments.update(&w.actor, &plant, &code, req).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者將 {string} 部門 {string} 的費用類別改為 {string}")]
async fn change_expense(w: &mut ErpWorld, plant: String, code: String, v: String) {
    set_accounting(w, &plant, &code, |r| r.expense_type = Some(v)).await;
}

#[when(expr = "使用者將 {string} 部門 {string} 的管理類別改為 {string}")]
async fn change_control(w: &mut ErpWorld, plant: String, code: String, v: String) {
    set_accounting(w, &plant, &code, |r| {
        r.control_type = v;
        r.cost_center = None;
    })
    .await;
}

#[when(expr = "使用者將 {string} 部門 {string} 的管理類別改為 {string}，對應成本中心改為 {string}")]
async fn change_control_cc(w: &mut ErpWorld, plant: String, code: String, v: String, cc: String) {
    set_accounting(w, &plant, &code, |r| {
        r.control_type = v;
        r.cost_center = Some(cc);
    })
    .await;
}

#[when(expr = "使用者刪除 {string} 部門 {string}")]
async fn delete_dept(w: &mut ErpWorld, plant: String, code: String) {
    let result = w.departments.delete(&w.actor, &plant, &code, None).await;
    w.last = Some(result);
}

fn dept_key(label: &str) -> &'static str {
    match label {
        "部門編號" => "dept_code",
        "簡稱" => "name",
        "全稱" => "full_name",
        "會計部門" => "accounting_dept",
        "費用類別" => "expense_type",
        "管理類別" => "control_type",
        "對應成本中心" => "cost_center",
        "資料有效" => "active",
        other => panic!("未知欄位：{other}"),
    }
}

#[when(expr = "使用者在 {string} 以下列條件查詢部門:")]
async fn query_depts(w: &mut ErpWorld, plant: String, step: &Step) {
    let req = query_request(step, dept_key, None);
    let result = w.departments.query(&w.actor, &plant, req).await;
    set_query_result(w, result, |d| d.dept_code.clone());
}

// ----- 部門：那麼 -----

#[then(expr = "{string} 部門 {string} 應為:")]
async fn dept_should_be(w: &mut ErpWorld, plant: String, code: String, step: &Step) {
    let d = dept(w, &plant, &code).await;
    for row in step.table.as_ref().expect("需要表格").rows.iter().skip(1) {
        let actual = match row[0].as_str() {
            "簡稱" => d.name.clone(),
            "全稱" => d.full_name.clone().unwrap_or_default(),
            "會計部門" => yes_no(d.accounting_dept).into(),
            "管理類別" => d.control_type.clone(),
            "資料有效" => yes_no(d.active).into(),
            "資料所有者" => d.created_by.clone(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, row[1], "{}", row[0]);
    }
}

#[then(expr = "{string} 部門 {string} 的對應成本中心應為 {string}")]
async fn cost_center_should_be(w: &mut ErpWorld, plant: String, code: String, expected: String) {
    assert_eq!(
        dept(w, &plant, &code).await.cost_center.unwrap_or_default(),
        expected
    );
}

#[then(expr = "{string} 部門 {string} 應不存在")]
async fn dept_gone(w: &mut ErpWorld, plant: String, code: String) {
    match w.departments.get(&w.actor, &plant, &code).await {
        Err(e) => assert_eq!(e.code(), "DEPARTMENT_NOT_FOUND"),
        Ok(d) => panic!("部門仍存在：{d:?}"),
    }
}

// ----- 員工 -----

#[given(expr = "{string} 已有員工 {string}，部門 {string}")]
async fn existing_employee(w: &mut ErpWorld, plant: String, id: String, dept: String) {
    w.employees
        .create(&w.actor, &plant, employee_input(&id, &id, &dept, None))
        .await
        .unwrap();
}

#[when(expr = "使用者在 {string} 新增員工 {string}，姓名 {string}，部門 {string}")]
async fn create_employee(w: &mut ErpWorld, plant: String, id: String, name: String, dept: String) {
    let result = w
        .employees
        .create(&w.actor, &plant, employee_input(&id, &name, &dept, None))
        .await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者在 {string} 新增員工 {string}，姓名 {string}，部門 {string}，E-mail {string}")]
async fn create_employee_email(
    w: &mut ErpWorld,
    plant: String,
    id: String,
    name: String,
    dept: String,
    email: String,
) {
    let result = w
        .employees
        .create(
            &w.actor,
            &plant,
            employee_input(&id, &name, &dept, Some(email)),
        )
        .await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者將 {string} 員工 {string} 的編號改為 {string}")]
async fn change_employee_id(w: &mut ErpWorld, plant: String, id: String, new_id: String) {
    let mut req = employee(w, &plant, &id).await.to_input();
    req.employee_id = new_id;
    let result = w.employees.update(&w.actor, &plant, &id, req).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者刪除 {string} 員工 {string}")]
async fn delete_employee(w: &mut ErpWorld, plant: String, id: String) {
    let result = w.employees.delete(&w.actor, &plant, &id, None).await;
    w.last = Some(result);
}

fn employee_key(label: &str) -> &'static str {
    match label {
        "員工編號" => "employee_id",
        "姓名" => "name",
        "部門" => "dept_code",
        "職稱" => "title",
        "分機" => "extension",
        "E-mail" => "email",
        "資料有效" => "active",
        other => panic!("未知欄位：{other}"),
    }
}

#[when(expr = "使用者在 {string} 以下列條件查詢員工:")]
async fn query_employees(w: &mut ErpWorld, plant: String, step: &Step) {
    let req = query_request(step, employee_key, None);
    let result = w.employees.query(&w.actor, &plant, req).await;
    set_query_result(w, result, |e| e.employee_id.clone());
}

#[then(expr = "{string} 員工 {string} 應為:")]
async fn employee_should_be(w: &mut ErpWorld, plant: String, id: String, step: &Step) {
    let e = employee(w, &plant, &id).await;
    for row in step.table.as_ref().expect("需要表格").rows.iter().skip(1) {
        let actual = match row[0].as_str() {
            "姓名" => e.name.clone(),
            "部門" => e.dept_code.clone().unwrap_or_default(),
            "部門名稱" => e.dept_name.clone().unwrap_or_default(),
            "資料有效" => yes_no(e.active).into(),
            "資料所有者" => e.created_by.clone(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, row[1], "{}", row[0]);
    }
}

#[then(expr = "{string} 員工 {string} 應不存在")]
async fn employee_gone(w: &mut ErpWorld, plant: String, id: String) {
    match w.employees.get(&w.actor, &plant, &id).await {
        Err(e) => assert_eq!(e.code(), "EMPLOYEE_NOT_FOUND"),
        Ok(e) => panic!("員工仍存在：{e:?}"),
    }
}
