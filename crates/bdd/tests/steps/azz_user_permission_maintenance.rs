//! User Special Permission Maintenance 使用者特殊權限維護 — specs/features/azz/user_permission_maintenance.feature

use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{SpecialPermissionKindDto, UserPermissionDto, UserPermissionInput};

use crate::ErpWorld;

fn yes_no(text: &str) -> bool {
    match text {
        "是" => true,
        "否" => false,
        other => panic!("應為 是／否：{other}"),
    }
}

fn class_input(class: &str, include_own_class: bool) -> UserPermissionInput {
    UserPermissionInput {
        kind: SpecialPermissionKindDto::Class,
        target: class.into(),
        include_own_class,
        ..Default::default()
    }
}

fn program_input(program: &str) -> UserPermissionInput {
    UserPermissionInput {
        kind: SpecialPermissionKindDto::Program,
        target: program.into(),
        ..Default::default()
    }
}

async fn rows(w: &ErpWorld, user: &str) -> Vec<UserPermissionDto> {
    w.special
        .list(&w.actor, user, "zh-TW")
        .await
        .unwrap_or_else(|e| panic!("讀取使用者 {user} 的特殊權限失敗：{e}"))
}

async fn program_row(w: &ErpWorld, user: &str, program: &str) -> UserPermissionDto {
    rows(w, user)
        .await
        .into_iter()
        .find(|r| r.kind == SpecialPermissionKindDto::Program && r.target == program)
        .unwrap_or_else(|| panic!("使用者 {user} 沒有程式 {program} 的特殊權限"))
}

// ----- 假設 -----

#[given(expr = "使用者 {string} 有特殊權限類別 {string}")]
async fn has_class(w: &mut ErpWorld, user: String, class: String) {
    w.special
        .add(&w.actor, &user, class_input(&class, false))
        .await
        .unwrap();
}

#[given(expr = "使用者 {string} 有特殊權限程式 {string}")]
async fn has_program(w: &mut ErpWorld, user: String, program: String) {
    w.special
        .add(&w.actor, &user, program_input(&program))
        .await
        .unwrap();
}

// ----- 當 -----

#[when(expr = "使用者為使用者 {string} 新增特殊權限類別 {string}，加入原權限類別 {string}")]
async fn add_class(w: &mut ErpWorld, user: String, class: String, include: String) {
    let result = w
        .special
        .add(&w.actor, &user, class_input(&class, yes_no(&include)))
        .await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者為使用者 {string} 新增特殊權限程式 {string}")]
async fn add_program(w: &mut ErpWorld, user: String, program: String) {
    let result = w
        .special
        .add(&w.actor, &user, program_input(&program))
        .await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者為使用者 {string} 新增特殊權限程式 {string}，允許執行功能 {string}")]
async fn add_program_with_actions(w: &mut ErpWorld, user: String, program: String, text: String) {
    let req = UserPermissionInput {
        actions: Some(
            text.split(',')
                .map(str::to_owned)
                .filter(|s| !s.trim().is_empty())
                .collect(),
        ),
        ..program_input(&program)
    };
    let result = w.special.add(&w.actor, &user, req).await;
    w.last = Some(result.map(|_| ()));
}

#[when(
    regex = r#"^使用者將使用者 "([^"]*)" 的特殊權限程式 "([^"]*)" 的(他人資料權限|他部門資料權限|單身權限)設為 "([^"]*)"$"#
)]
async fn set_level(w: &mut ErpWorld, user: String, program: String, field: String, code: String) {
    let mut req = program_row(w, &user, &program).await.to_input();
    match field.as_str() {
        "他人資料權限" => req.other_user_data = Some(code),
        "他部門資料權限" => req.other_dept_data = Some(code),
        "單身權限" => req.detail_rows = Some(code),
        other => panic!("未知欄位：{other}"),
    }
    let result = w
        .special
        .update_program(&w.actor, &user, &program, req)
        .await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者移除使用者 {string} 的第 {int} 項特殊權限")]
async fn remove(w: &mut ErpWorld, user: String, line_no: i32) {
    let result = w.special.remove(&w.actor, &user, line_no).await;
    w.last = Some(result);
}

#[when(expr = "使用者刪除使用者 {string} 的全部特殊權限")]
async fn remove_all(w: &mut ErpWorld, user: String) {
    let result = w.special.remove_all(&w.actor, &user).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者將使用者 {string} 的特殊權限複製到 {string}")]
async fn copy(w: &mut ErpWorld, from: String, to: String) {
    let result = w.special.copy(&w.actor, &from, &to).await;
    w.last = Some(result.map(|_| ()));
}

// ----- 那麼 -----

/// 「1 類別 CLASS1、2 程式 aimi101」
#[then(expr = "使用者 {string} 的特殊權限應為 {string}")]
async fn rows_should_be(w: &mut ErpWorld, user: String, expected: String) {
    let actual: Vec<String> = rows(w, &user)
        .await
        .iter()
        .map(|r| {
            let kind = match r.kind {
                SpecialPermissionKindDto::Class => "類別",
                SpecialPermissionKindDto::Program => "程式",
            };
            format!("{} {kind} {}", r.line_no, r.target)
        })
        .collect();
    assert_eq!(actual.join("、"), expected);
}

#[then(expr = "使用者 {string} 的特殊權限程式 {string} 應為:")]
async fn program_row_should_be(w: &mut ErpWorld, user: String, program: String, step: &Step) {
    let r = program_row(w, &user, &program).await;
    for row in step.table.as_ref().expect("需要表格").rows.iter().skip(1) {
        let actual = match row[0].as_str() {
            "允許執行功能" => r.actions.join(", "),
            "他人資料權限" => r.other_user_data.clone().unwrap_or_default(),
            "他部門資料權限" => r.other_dept_data.clone().unwrap_or_default(),
            "單身權限" => r.detail_rows.clone().unwrap_or_default(),
            "最後修改者" => r.updated_by.clone().unwrap_or_default(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, row[1], "{}", row[0]);
    }
}

#[then(expr = "使用者 {string} 應沒有特殊權限")]
async fn no_rows(w: &mut ErpWorld, user: String) {
    let n = match &w.backend {
        crate::Backend::Memory(s) => s.user_permission_count(&user),
        crate::Backend::Pg(pool) => {
            let (n,): (i64,) =
                sqlx::query_as("SELECT count(*) FROM user_permission WHERE user_id = $1")
                    .bind(&user)
                    .fetch_one(pool)
                    .await
                    .unwrap();
            usize::try_from(n).unwrap()
        }
    };
    assert_eq!(n, 0);
}
