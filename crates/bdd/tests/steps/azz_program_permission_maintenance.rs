//! Program Permission Maintenance 程式執行權限維護 — specs/features/azz/program_permission_maintenance.feature

use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{ProgramDto, ProgramInput, ProgramPermissionDto, ProgramPermissionInput};

use crate::ErpWorld;

/// 「insert, query」→ 動作清單；空字串為空清單
fn actions(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::to_owned)
        .filter(|s| !s.trim().is_empty())
        .collect()
}

async fn permission(w: &ErpWorld, class: &str, program: &str) -> ProgramPermissionDto {
    w.permissions
        .get(class, program, "zh-TW")
        .await
        .unwrap_or_else(|e| panic!("讀取權限類別 {class} 的程式 {program} 權限失敗：{e}"))
}

/// 讀出權限、修改某個資料權限欄位後存檔
async fn set_level(w: &mut ErpWorld, class: &str, program: &str, field: &str, code: String) {
    let mut req = permission(w, class, program).await.to_input();
    match field {
        "他人資料權限" => req.other_user_data = Some(code),
        "他部門資料權限" => req.other_dept_data = Some(code),
        "單身權限" => req.detail_rows = Some(code),
        other => panic!("未知欄位：{other}"),
    }
    let result = w.permissions.update(&w.actor, class, program, req).await;
    w.last = Some(result.map(|_| ()));
}

// ----- 假設 -----

#[given(expr = "已有程式 {string} 模組 {string}，可執行功能 {string}")]
async fn program_with_actions(
    w: &mut ErpWorld,
    no: String,
    module: String,
    default_actions: String,
) {
    let req = ProgramInput {
        program_no: no,
        module_code: module,
        default_actions: Some(default_actions),
        ..Default::default()
    };
    let _: ProgramDto = w.programs.create(&w.actor, req).await.unwrap();
}

#[given(
    regex = r#"^使用者將權限類別 "([^"]*)" 的程式 "([^"]*)" 的(他人資料權限|他部門資料權限|單身權限)設為 "([^"]*)"$"#
)]
async fn level_set(w: &mut ErpWorld, class: String, program: String, field: String, code: String) {
    set_level(w, &class, &program, &field, code).await;
    if let Some(Err(e)) = &w.last {
        panic!("設定 {field} 失敗：{e}");
    }
}

// ----- 當 -----

#[when(expr = "使用者為權限類別 {string} 新增程式 {string} 的權限")]
async fn grant(w: &mut ErpWorld, class: String, program: String) {
    let req = ProgramPermissionInput {
        program_no: program,
        ..Default::default()
    };
    let result = w.permissions.grant(&w.actor, &class, req).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者為權限類別 {string} 新增程式 {string} 的權限，允許執行功能 {string}")]
async fn grant_with_actions(w: &mut ErpWorld, class: String, program: String, text: String) {
    let req = ProgramPermissionInput {
        program_no: program,
        actions: Some(actions(&text)),
        ..Default::default()
    };
    let result = w.permissions.grant(&w.actor, &class, req).await;
    w.last = Some(result.map(|_| ()));
}

#[when(
    regex = r#"^使用者將權限類別 "([^"]*)" 的程式 "([^"]*)" 的(他人資料權限|他部門資料權限|單身權限)設為 "([^"]*)"$"#
)]
async fn level_change(
    w: &mut ErpWorld,
    class: String,
    program: String,
    field: String,
    code: String,
) {
    set_level(w, &class, &program, &field, code).await;
}

#[when(expr = "使用者移除權限類別 {string} 的程式 {string} 的權限")]
async fn revoke(w: &mut ErpWorld, class: String, program: String) {
    let result = w.permissions.revoke(&class, &program).await;
    w.last = Some(result);
}

#[when(expr = "使用者刪除權限類別 {string} 的全部權限")]
async fn revoke_all(w: &mut ErpWorld, class: String) {
    let result = w.permissions.revoke_all(&class).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者將權限類別 {string} 的權限複製到 {string}")]
async fn copy(w: &mut ErpWorld, from: String, to: String) {
    let result = w.permissions.copy(&w.actor, &from, &to).await;
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者為權限類別 {string} 依程式整批產生 {string} 的權限")]
async fn generate(w: &mut ErpWorld, class: String, programs: String) {
    let list: Vec<String> = programs.split('、').map(str::to_owned).collect();
    let result = w.permissions.generate(&w.actor, &class, &list).await;
    w.updated_count = result.as_ref().ok().map(|r| r.added);
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者為權限類別 {string} 依選單 {string} 整批產生權限")]
async fn generate_by_menu(w: &mut ErpWorld, class: String, menu: String) {
    let result = w
        .permissions
        .generate_by_menu(&w.actor, &class, &menu, false)
        .await;
    w.updated_count = result.as_ref().ok().map(|r| r.added);
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者為權限類別 {string} 依選單 {string} 整批產生權限，重設已有權限 {string}")]
async fn generate_by_menu_reset(w: &mut ErpWorld, class: String, menu: String, reset: String) {
    let reset = match reset.as_str() {
        "是" => true,
        "否" => false,
        other => panic!("重設已有權限應為 是／否：{other}"),
    };
    let result = w
        .permissions
        .generate_by_menu(&w.actor, &class, &menu, reset)
        .await;
    w.updated_count = result.as_ref().ok().map(|r| r.added);
    w.last = Some(result.map(|_| ()));
}

#[when(expr = "使用者為權限類別 {string} 依選單 {string} 整批刪除權限")]
async fn delete_by_menu(w: &mut ErpWorld, class: String, menu: String) {
    let result = w.permissions.delete_by_menu(&class, &menu).await;
    w.last = Some(result.map(|_| ()));
}

// ----- 那麼 -----

#[then(expr = "權限類別 {string} 的程式 {string} 權限應為:")]
async fn permission_should_be(w: &mut ErpWorld, class: String, program: String, step: &Step) {
    let p = permission(w, &class, &program).await;
    for row in step.table.as_ref().expect("需要表格").rows.iter().skip(1) {
        let actual = match row[0].as_str() {
            "允許執行功能" => p.actions.join(", "),
            "他人資料權限" => p.other_user_data.clone(),
            "他部門資料權限" => p.other_dept_data.clone(),
            "單身權限" => p.detail_rows.clone(),
            "資料所有者" => p.created_by.clone(),
            "最後修改者" => p.updated_by.clone().unwrap_or_default(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, row[1].trim(), "{}", row[0]);
    }
}

#[then(expr = "權限類別 {string} 的程式 {string} 允許執行功能應為 {string}")]
async fn actions_should_be(w: &mut ErpWorld, class: String, program: String, expected: String) {
    assert_eq!(
        permission(w, &class, &program).await.actions.join(", "),
        expected
    );
}

#[then(expr = "新增的權限筆數應為 {int}")]
async fn added_should_be(w: &mut ErpWorld, expected: u64) {
    assert_eq!(w.updated_count, Some(expected));
}
