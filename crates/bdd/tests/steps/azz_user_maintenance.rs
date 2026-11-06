//! User Maintenance 使用者維護 — specs/features/azz/user_maintenance.feature

use chrono::{Days, Local, NaiveDate};
use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::Actor;
use ttrs_shared::{DepartmentInput, ProgramInput, UserDto, UserInput};

use super::azz_program_maintenance::seed;
use crate::ErpWorld;

fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// 昨天／今天／明天／空白 → 日期
fn day(word: &str) -> Option<NaiveDate> {
    match word {
        "" => None,
        "昨天" => today().checked_sub_days(Days::new(1)),
        "今天" => Some(today()),
        "明天" => today().checked_add_days(Days::new(1)),
        other => Some(NaiveDate::parse_from_str(other, "%Y-%m-%d").expect("日期格式 YYYY-MM-DD")),
    }
}

fn day_label(d: Option<NaiveDate>) -> String {
    match d {
        None => String::new(),
        Some(d) if d == today() => "今天".into(),
        Some(d) => d.to_string(),
    }
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn input(id: &str, name: &str, dept: &str, class: &str, menu: &str, plant: &str) -> UserInput {
    UserInput {
        user_id: id.into(),
        name: Some(name.into()).filter(|s: &String| !s.is_empty()),
        dept_code: Some(dept.into()),
        permission_class: class.into(),
        menu: menu.into(),
        default_plant: plant.into(),
        ..Default::default()
    }
}

/// 背景已備妥的標準使用者
pub(crate) fn standard(id: &str) -> UserInput {
    input(id, "王小明", "SAL", "CLASS1", "menu-main", "TW01")
}

/// 前置資料：部門尚未建立時先建立（使用者的部門須存在於預設營運中心，user_maintenance_department.feature）
pub(crate) async fn ensure_dept(w: &ErpWorld, plant: &str, code: Option<&str>) {
    let Some(code) = code.filter(|c| !c.trim().is_empty()) else {
        return;
    };
    if w.departments.get(&w.actor, plant, code).await.is_err() {
        let dept = DepartmentInput {
            version: None,
            dept_code: code.into(),
            name: Some(code.into()),
            full_name: None,
        };
        w.departments
            .create(&w.actor, plant, dept)
            .await
            .unwrap_or_else(|e| panic!("建立部門 {plant}/{code} 失敗：{e}"));
    }
}

/// 「假設已有使用者」：先備妥部門，再以 actor 建立使用者
pub(crate) async fn seed_user(w: &ErpWorld, actor: &Actor, req: UserInput) {
    ensure_dept(w, &req.default_plant, req.dept_code.as_deref()).await;
    let id = req.user_id.clone();
    w.users
        .create(actor, req)
        .await
        .unwrap_or_else(|e| panic!("建立使用者 {id} 失敗：{e}"));
}

pub(crate) async fn user(w: &ErpWorld, id: &str) -> UserDto {
    w.users
        .get(&w.actor, id)
        .await
        .unwrap_or_else(|e| panic!("讀取使用者 {id} 失敗：{e}"))
}

/// 讀出使用者、修改輸入後存檔，結果記在 w.last
pub(crate) async fn modify(w: &mut ErpWorld, id: &str, change: impl FnOnce(&mut UserInput)) {
    let mut req = user(w, id).await.to_input();
    change(&mut req);
    let result = w.users.update(&w.actor, id, req).await;
    w.last = Some(result.map(|_| ()));
}

// ----- 假設 -----

#[given(expr = "選單程式 {string} 已存在")]
async fn menu_program_exists(w: &mut ErpWorld, no: String) {
    let sql =
        "INSERT INTO module (module_code, name) VALUES ('MENU', 'MENU') ON CONFLICT DO NOTHING";
    seed(w, |s| s.add_module("MENU"), sql, &[]).await;
    let req = ProgramInput {
        program_type: Some("I".into()),
        program_no: no,
        module_code: "MENU".into(),
        ..Default::default()
    };
    w.programs.create(&w.actor, req).await.unwrap();
}

#[given(expr = "已有使用者 {string}")]
async fn existing_user(w: &mut ErpWorld, id: String) {
    seed_user(w, &w.actor, standard(&id)).await;
}

#[given(expr = "已有使用者 {string}，公司帳號 {string}")]
async fn existing_user_with_account(w: &mut ErpWorld, id: String, account: String) {
    let req = UserInput {
        idp_username: Some(account),
        ..standard(&id)
    };
    seed_user(w, &w.actor, req).await;
}

#[given(expr = "已有使用者 {string}，權限類別 {string}，選單 {string}")]
async fn existing_user_with_class(w: &mut ErpWorld, id: String, class: String, menu: String) {
    let req = UserInput {
        permission_class: class,
        menu,
        ..standard(&id)
    };
    seed_user(w, &w.actor, req).await;
}

/// user_maintenance_department.feature
#[given(expr = "已有使用者 {string}，部門 {string}，預設營運中心 {string}")]
async fn existing_user_in_dept(w: &mut ErpWorld, id: String, dept: String, plant: String) {
    let req = UserInput {
        dept_code: Some(dept),
        default_plant: plant,
        ..standard(&id)
    };
    seed_user(w, &w.actor, req).await;
}

async fn existing_multi_plant_user(w: &mut ErpWorld, id: &str, plants: &[&str]) {
    let req = UserInput {
        multi_plant: true,
        default_plant: plants[0].into(),
        ..standard(id)
    };
    seed_user(w, &w.actor, req).await;
    // 其餘營運中心依「變更預設營運中心時加入清單」的規則加入，再改回第一個為預設
    for p in &plants[1..] {
        ensure_dept(w, p, Some("SAL")).await;
        let mut req = user(w, id).await.to_input();
        req.default_plant = (*p).into();
        w.users.update(&w.actor, id, req).await.unwrap();
    }
    let mut req = user(w, id).await.to_input();
    req.default_plant = plants[0].into();
    w.users.update(&w.actor, id, req).await.unwrap();
}

#[given(expr = "已有多營運中心使用者 {string}，可使用營運中心 {string}")]
async fn existing_multi_one(w: &mut ErpWorld, id: String, plant: String) {
    existing_multi_plant_user(w, &id, &[&plant]).await;
}

#[given(expr = "已有多營運中心使用者 {string}，可使用營運中心 {string}、{string}")]
async fn existing_multi_two(w: &mut ErpWorld, id: String, a: String, b: String) {
    existing_multi_plant_user(w, &id, &[&a, &b]).await;
}

// ----- 當 -----

#[when(
    expr = "使用者新增使用者 {string}，名稱 {string}，部門 {string}，權限類別 {string}，選單 {string}，預設營運中心 {string}"
)]
async fn create_user(
    w: &mut ErpWorld,
    id: String,
    name: String,
    dept: String,
    class: String,
    menu: String,
    plant: String,
) {
    let req = input(&id, &name, &dept, &class, &menu, &plant);
    let result = w.users.create(&w.actor, req).await;
    w.last = Some(result.map(|_| ()));
}

#[when(
    expr = "使用者新增使用者 {string}，名稱 {string}，部門 {string}，權限類別 {string}，選單 {string}，預設營運中心 {string}，公司帳號 {string}"
)]
#[allow(clippy::too_many_arguments)]
async fn create_user_with_account(
    w: &mut ErpWorld,
    id: String,
    name: String,
    dept: String,
    class: String,
    menu: String,
    plant: String,
    account: String,
) {
    let req = UserInput {
        idp_username: Some(account),
        ..input(&id, &name, &dept, &class, &menu, &plant)
    };
    let result = w.users.create(&w.actor, req).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者將使用者 {string} 改為非多營運中心")]
async fn set_single_plant(w: &mut ErpWorld, id: String) {
    modify(w, &id, |r| r.multi_plant = false).await;
}

#[when(expr = "使用者將使用者 {string} 的預設營運中心改為 {string}")]
async fn change_default_plant(w: &mut ErpWorld, id: String, plant: String) {
    modify(w, &id, |r| r.default_plant = plant).await;
}

#[when(expr = "使用者將使用者 {string} 的失效日期設為 {string}")]
async fn set_valid_until(w: &mut ErpWorld, id: String, word: String) {
    modify(w, &id, |r| r.valid_until = day(&word)).await;
}

#[when(expr = "使用者將使用者 {string} 的名稱改為 {string}")]
async fn rename(w: &mut ErpWorld, id: String, name: String) {
    modify(w, &id, |r| r.name = Some(name)).await;
}

#[when(expr = "使用者將使用者 {string} 的部門改為 {string}")]
async fn change_dept(w: &mut ErpWorld, id: String, dept: String) {
    modify(w, &id, |r| r.dept_code = Some(dept)).await;
}

#[when(expr = "使用者將使用者 {string} 的代號改為 {string}")]
async fn change_id(w: &mut ErpWorld, id: String, new_id: String) {
    modify(w, &id, |r| r.user_id = new_id).await;
}

#[when(expr = "使用者將使用者 {string} 的權限類別改為 {string}")]
async fn change_class(w: &mut ErpWorld, id: String, class: String) {
    modify(w, &id, |r| r.permission_class = class).await;
}

#[when(expr = "使用者將使用者 {string} 的偏好語言改為 {string}")]
async fn change_lang(w: &mut ErpWorld, id: String, lang: String) {
    modify(w, &id, |r| r.lang = Some(lang)).await;
}

#[when(expr = "使用者將使用者 {string} 的電子郵件改為 {string}")]
async fn change_email(w: &mut ErpWorld, id: String, email: String) {
    modify(w, &id, |r| r.email = Some(email)).await;
}

#[when(expr = "使用者停用使用者 {string}")]
async fn deactivate(w: &mut ErpWorld, id: String) {
    let result = w.users.deactivate(&w.actor, &id).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者啟用使用者 {string}")]
async fn activate(w: &mut ErpWorld, id: String) {
    let result = w.users.activate(&w.actor, &id).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者刪除使用者 {string}")]
async fn delete(w: &mut ErpWorld, id: String) {
    let result = w.users.delete(&w.actor, &id).await;
    w.last = Some(result);
}

#[when(expr = "使用者將使用者 {string} 複製為 {string}")]
async fn copy(w: &mut ErpWorld, id: String, new_id: String) {
    let result = w.users.copy(&w.actor, &id, &new_id).await;
    w.last = Some(result.map(|_| ()));
}

// ----- 那麼 -----

#[then(expr = "使用者 {string} 應為:")]
async fn user_should_be(w: &mut ErpWorld, id: String, step: &Step) {
    let u = user(w, &id).await;
    for row in step.table.as_ref().expect("需要表格").rows.iter().skip(1) {
        let actual = match row[0].as_str() {
            "部門" => u.dept_code.clone().unwrap_or_default(),
            "偏好語言" => u.lang.clone(),
            "多營運中心" => yes_no(u.multi_plant).into(),
            "資料有效" => yes_no(u.active).into(),
            "資料所有者" => u.created_by.clone(),
            "資料所有部門" => u.created_dept.clone(),
            "建立日期" => day_label(Some(u.created_on)),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, row[1], "{}", row[0]);
    }
}

#[then(expr = "使用者 {string} 可使用的營運中心應為 {string}")]
async fn allowed_plants(w: &mut ErpWorld, id: String, expected: String) {
    let u = user(w, &id).await;
    let mut actual = u.allowed_plants.clone();
    let mut want: Vec<String> = expected.split('、').map(str::to_owned).collect();
    actual.sort();
    want.sort();
    assert_eq!(actual, want);
}

#[then(expr = "使用者 {string} 的資料有效應為 {string}")]
async fn active_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(yes_no(user(w, &id).await.active), expected);
}

#[then(expr = "使用者 {string} 的失效日期應為 {string}")]
async fn valid_until_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(day_label(user(w, &id).await.valid_until), expected);
}

#[then(expr = "使用者 {string} 的最後修改者應為 {string}")]
async fn updated_by_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(user(w, &id).await.updated_by.unwrap_or_default(), expected);
}

#[then(expr = "使用者 {string} 的最後修改日期應為 {string}")]
async fn updated_on_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(day_label(user(w, &id).await.updated_on), expected);
}

#[then(expr = "使用者 {string} 應不存在")]
async fn should_not_exist(w: &mut ErpWorld, id: String) {
    match w.users.get(&w.actor, &id).await {
        Err(e) => assert_eq!(e.code(), "USER_NOT_FOUND"),
        Ok(u) => panic!("使用者仍存在：{u:?}"),
    }
}

#[then(expr = "使用者 {string} 應沒有可使用的營運中心")]
async fn no_plants(w: &mut ErpWorld, id: String) {
    let user_id = ttrs_domain::user::UserId::parse(&id).unwrap();
    let plants = match &w.backend {
        crate::Backend::Memory(s) => s.user_plant_count(&user_id),
        crate::Backend::Pg(pool) => {
            let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM user_plant WHERE user_id = $1")
                .bind(&id)
                .fetch_one(pool)
                .await
                .unwrap();
            usize::try_from(n).unwrap()
        }
    };
    assert_eq!(plants, 0);
}

#[then(expr = "使用者 {string} 的名稱應為 {string}")]
async fn name_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(user(w, &id).await.name.unwrap_or_default(), expected);
}

#[then(expr = "使用者 {string} 的公司帳號應為 {string}")]
async fn account_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(
        user(w, &id).await.idp_username.unwrap_or_default(),
        expected
    );
}

#[then(expr = "使用者 {string} 的選單應為 {string}")]
async fn menu_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(user(w, &id).await.menu, expected);
}

#[then(expr = "使用者 {string} 的權限類別應為 {string}")]
async fn class_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(user(w, &id).await.permission_class, expected);
}

#[then(expr = "使用者 {string} 的資料所有者應為 {string}")]
async fn owner_should_be(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(user(w, &id).await.created_by, expected);
}
