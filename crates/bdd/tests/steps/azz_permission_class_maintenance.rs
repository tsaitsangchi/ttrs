//! Permission Class Maintenance 權限類別維護 — specs/features/azz/permission_class_maintenance.feature

use chrono::{Local, NaiveDate};
use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{ClassIdleControlDto, IdleActionDto, PermissionClassDto, PermissionClassInput};

use crate::ErpWorld;

fn date_label(d: Option<NaiveDate>) -> String {
    match d {
        None => String::new(),
        Some(d) if d == Local::now().date_naive() => "今天".into(),
        Some(d) => d.to_string(),
    }
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

pub fn by_label<T: Copy>(all: &[T], label: fn(T) -> &'static str, text: &str) -> Option<T> {
    if text.is_empty() {
        return None;
    }
    let found = all.iter().copied().find(|v| label(*v) == text);
    Some(found.unwrap_or_else(|| panic!("未知的選項：{text}")))
}

fn input(code: &str, name: &str, menu: &str) -> PermissionClassInput {
    PermissionClassInput {
        class_code: code.into(),
        name: Some(name.into()),
        default_menu: Some(menu.into()),
        ..Default::default()
    }
}

async fn class(w: &ErpWorld, code: &str) -> PermissionClassDto {
    w.classes
        .get(code)
        .await
        .unwrap_or_else(|e| panic!("讀取權限類別 {code} 失敗：{e}"))
}

/// 讀出權限類別、修改輸入後存檔，結果記在 w.last
async fn modify(w: &mut ErpWorld, code: &str, change: impl FnOnce(&mut PermissionClassInput)) {
    let mut req = class(w, code).await.to_input();
    change(&mut req);
    let result = w.classes.update(&w.actor, code, req).await;
    w.last = Some(result.map(|_| ()));
}

// ----- 假設 -----

#[given(expr = "權限類別 {string} 已存在，預設選單 {string}")]
async fn class_with_menu(w: &mut ErpWorld, code: String, menu: String) {
    w.classes
        .create(&w.actor, input(&code, &code, &menu))
        .await
        .unwrap();
}

#[given(expr = "權限類別 {string} 可執行程式 {string}")]
async fn class_can_run(w: &mut ErpWorld, code: String, program: String) {
    w.programs.grant(&w.actor, &program, &code).await.unwrap();
}

#[given(expr = "使用者停用權限類別 {string}")]
async fn class_deactivated(w: &mut ErpWorld, code: String) {
    w.classes.deactivate(&w.actor, &code).await.unwrap();
}

// ----- 當 -----

#[when(expr = "使用者新增權限類別 {string}，說明 {string}，預設選單 {string}")]
async fn create_class(w: &mut ErpWorld, code: String, name: String, menu: String) {
    let result = w.classes.create(&w.actor, input(&code, &name, &menu)).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者將權限類別 {string} 的代號改為 {string}")]
async fn change_code(w: &mut ErpWorld, code: String, new_code: String) {
    modify(w, &code, |r| r.class_code = new_code).await;
}

#[when(expr = "使用者將權限類別 {string} 的說明改為 {string}")]
async fn rename(w: &mut ErpWorld, code: String, name: String) {
    modify(w, &code, |r| r.name = Some(name)).await;
}

#[when(expr = "使用者將權限類別 {string} 設為閒置控制 {string}，閒置處理 {string}，閒置秒數 {int}")]
async fn set_idle(w: &mut ErpWorld, code: String, control: String, action: String, seconds: i32) {
    modify(w, &code, |r| {
        r.idle_control = by_label(
            &ClassIdleControlDto::ALL,
            ClassIdleControlDto::label,
            &control,
        );
        r.idle_action = by_label(&IdleActionDto::ALL, IdleActionDto::label, &action);
        r.idle_seconds = Some(seconds);
    })
    .await;
}

#[when(expr = "使用者停用權限類別 {string}")]
async fn deactivate(w: &mut ErpWorld, code: String) {
    let result = w.classes.deactivate(&w.actor, &code).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者啟用權限類別 {string}")]
async fn activate(w: &mut ErpWorld, code: String) {
    let result = w.classes.activate(&w.actor, &code).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者刪除權限類別 {string}")]
async fn delete(w: &mut ErpWorld, code: String) {
    let result = w.classes.delete(&code).await;
    w.last = Some(result);
}

#[when(expr = "使用者執行更新所有使用者目錄編號")]
async fn update_user_menus(w: &mut ErpWorld) {
    let result = w.classes.update_user_menus(&w.actor).await;
    w.updated_count = result.as_ref().ok().copied();
    w.last = Some(result.map(|_| ()));
}

// ----- 那麼 -----

#[then(expr = "權限類別 {string} 應為:")]
async fn class_should_be(w: &mut ErpWorld, code: String, step: &Step) {
    let c = class(w, &code).await;
    for row in step.table.as_ref().expect("需要表格").rows.iter().skip(1) {
        let actual = match row[0].as_str() {
            "說明" => c.name.clone(),
            "預設選單" => c.default_menu.clone().unwrap_or_default(),
            "閒置控制" => c.idle_control.label().into(),
            "閒置處理" => c
                .idle_action
                .map(IdleActionDto::label)
                .unwrap_or_default()
                .into(),
            "閒置秒數" => c.idle_seconds.map(|s| s.to_string()).unwrap_or_default(),
            "資料有效" => yes_no(c.active).into(),
            "資料所有者" => c.created_by.clone(),
            "資料所有部門" => c.created_dept.clone(),
            "建立日期" => date_label(Some(c.created_on)),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, row[1], "{}", row[0]);
    }
}

#[then(expr = "權限類別 {string} 應沒有程式權限")]
async fn no_permissions(w: &mut ErpWorld, code: String) {
    let n = match &w.backend {
        crate::Backend::Memory(s) => s.class_permission_count(&code),
        crate::Backend::Pg(pool) => {
            let (n,): (i64,) =
                sqlx::query_as("SELECT count(*) FROM program_permission WHERE class_code = $1")
                    .bind(&code)
                    .fetch_one(pool)
                    .await
                    .unwrap();
            usize::try_from(n).unwrap()
        }
    };
    assert_eq!(n, 0);
}

#[then(expr = "權限類別 {string} 的說明應為 {string}")]
async fn name_should_be(w: &mut ErpWorld, code: String, expected: String) {
    assert_eq!(class(w, &code).await.name, expected);
}

#[then(expr = "權限類別 {string} 的閒置處理應為 {string}")]
async fn idle_action_should_be(w: &mut ErpWorld, code: String, expected: String) {
    let c = class(w, &code).await;
    assert_eq!(
        c.idle_action.map(IdleActionDto::label).unwrap_or_default(),
        expected
    );
}

#[then(expr = "權限類別 {string} 的閒置秒數應為 {string}")]
async fn idle_seconds_should_be(w: &mut ErpWorld, code: String, expected: String) {
    let c = class(w, &code).await;
    assert_eq!(
        c.idle_seconds.map(|s| s.to_string()).unwrap_or_default(),
        expected
    );
}

#[then(expr = "權限類別 {string} 的資料有效應為 {string}")]
async fn active_should_be(w: &mut ErpWorld, code: String, expected: String) {
    assert_eq!(yes_no(class(w, &code).await.active), expected);
}

#[then(expr = "權限類別 {string} 的最後修改者應為 {string}")]
async fn updated_by_should_be(w: &mut ErpWorld, code: String, expected: String) {
    assert_eq!(
        class(w, &code).await.updated_by.unwrap_or_default(),
        expected
    );
}

#[then(expr = "權限類別 {string} 的最後修改日期應為 {string}")]
async fn updated_on_should_be(w: &mut ErpWorld, code: String, expected: String) {
    assert_eq!(date_label(class(w, &code).await.updated_on), expected);
}

#[then(expr = "權限類別 {string} 應不存在")]
async fn should_not_exist(w: &mut ErpWorld, code: String) {
    match w.classes.get(&code).await {
        Err(e) => assert_eq!(e.code(), "PERMISSION_CLASS_NOT_FOUND"),
        Ok(c) => panic!("權限類別仍存在：{c:?}"),
    }
}

#[then(expr = "更新的使用者數應為 {int}")]
async fn updated_count_should_be(w: &mut ErpWorld, expected: u64) {
    assert_eq!(w.updated_count, Some(expected));
}
