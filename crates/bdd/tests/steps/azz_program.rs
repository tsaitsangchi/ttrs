//! p_zz 程式登錄 — specs/features/azz/program_registry.feature

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::{Actor, memory::InMemoryStore};
use ttrs_shared::{
    IdleActionDto, IdleControlDto, ProgramDto, ProgramInput, ReportExtModeDto, ReportTypeDto,
};

use crate::{Backend, ErpWorld};

fn input(no: &str, module: &str) -> ProgramInput {
    ProgramInput {
        program_no: no.into(),
        module_code: module.into(),
        ..Default::default()
    }
}

fn yes_no(label: &str) -> bool {
    match label {
        "是" => true,
        "否" => false,
        other => panic!("未知的是/否：{other}"),
    }
}

fn yes_no_label(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

/// 依中文標籤找列舉值（標籤定義在 ttrs_shared，與畫面一致）
fn by_label<T: Copy>(all: &[T], label: fn(T) -> &'static str, text: &str) -> T {
    all.iter()
        .copied()
        .find(|v| label(*v) == text)
        .unwrap_or_else(|| panic!("未知的選項：{text}"))
}

fn opt_label<T: Copy>(v: Option<T>, label: fn(T) -> &'static str) -> String {
    v.map(label).unwrap_or_default().to_string()
}

fn opt_num(v: Option<i32>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}

/// 讀取「| 欄位 | 值 |」表格（略過標題列）
fn field_rows(step: &Step) -> Vec<(String, String)> {
    let table = step.table.as_ref().expect("此步驟需要表格");
    table
        .rows
        .iter()
        .skip(1)
        .map(|r| (r[0].clone(), r[1].clone()))
        .collect()
}

async fn program(w: &ErpWorld, no: &str) -> ProgramDto {
    w.programs.get(no).await.expect("讀取程式")
}

/// 準備基本資料：記憶體後端呼叫 `memory`，PostgreSQL 後端執行 `sql`
async fn seed(w: &ErpWorld, memory: impl FnOnce(&InMemoryStore), sql: &'static str, args: &[&str]) {
    match &w.backend {
        Backend::Memory(s) => memory(s),
        Backend::Pg(pool) => {
            let mut q = sqlx::query(sql);
            for a in args {
                q = q.bind(*a);
            }
            q.execute(pool).await.unwrap();
        }
    }
}

// ----- 假設 -----

#[given(expr = "模組 {string} 已存在")]
async fn module_exists(w: &mut ErpWorld, module: String) {
    let sql = "INSERT INTO module (module_code, name) VALUES ($1, $1)";
    seed(w, |s| s.add_module(&module), sql, &[&module]).await;
}

#[given(expr = "權限類別 {string} 已存在")]
async fn permission_class_exists(w: &mut ErpWorld, class: String) {
    let sql = "INSERT INTO permission_class (class_code, name) VALUES ($1, $1)";
    seed(w, |s| s.add_permission_class(&class), sql, &[&class]).await;
}

#[given(expr = "選單 {string} 包含程式 {string}")]
async fn menu_contains(w: &mut ErpWorld, menu: String, no: String) {
    let sql = "INSERT INTO menu_item (menu_no, program_no) VALUES ($1, $2)";
    seed(w, |s| s.add_menu_item(&menu, &no), sql, &[&menu, &no]).await;
}

#[given(expr = "查詢定義 {string} 已存在")]
async fn query_definition_exists(w: &mut ErpWorld, query: String) {
    let sql = "INSERT INTO query_definition (query_no) VALUES ($1)";
    seed(w, |s| s.add_query_definition(&query), sql, &[&query]).await;
}

#[given(expr = "目前使用者為 {string}，所屬部門 {string}，權限類別 {string}")]
fn current_user(w: &mut ErpWorld, user: String, dept: String, class: String) {
    w.actor = Actor {
        user_id: user,
        dept,
        permission_class: class,
    };
}

#[given(expr = "已有程式 {string} 模組 {string}")]
async fn existing_program(w: &mut ErpWorld, no: String, module: String) {
    w.programs
        .create(&w.actor, input(&no, &module))
        .await
        .unwrap();
}

#[given(expr = "已有程式 {string} 模組 {string}，名稱 {string}")]
async fn existing_program_named(w: &mut ErpWorld, no: String, module: String, name: String) {
    let req = ProgramInput {
        name: Some(name),
        ..input(&no, &module)
    };
    w.programs.create(&w.actor, req).await.unwrap();
}

// ----- 當 -----

async fn create(w: &mut ErpWorld, req: ProgramInput) {
    w.last = Some(w.programs.create(&w.actor, req).await.map(|_| ()));
}

#[when(expr = "使用者新增程式 {string} 模組 {string}")]
async fn create_program(w: &mut ErpWorld, no: String, module: String) {
    create(w, input(&no, &module)).await;
}

#[when(expr = "使用者新增程式 {string} 模組 {string}，報表類型 {string}")]
async fn create_with_report_type(w: &mut ErpWorld, no: String, module: String, rt: String) {
    let req = ProgramInput {
        report_type: Some(by_label(&ReportTypeDto::ALL, ReportTypeDto::label, &rt)),
        ..input(&no, &module)
    };
    create(w, req).await;
}

#[when(
    expr = "使用者新增程式 {string} 模組 {string}，閒置控制 {string}，閒置秒數 {int}，逾時處理 {string}"
)]
async fn create_with_idle(
    w: &mut ErpWorld,
    no: String,
    module: String,
    control: String,
    seconds: i32,
    action: String,
) {
    let req = ProgramInput {
        idle_control: Some(by_label(
            &IdleControlDto::ALL,
            IdleControlDto::label,
            &control,
        )),
        idle_seconds: Some(seconds),
        idle_action: Some(by_label(&IdleActionDto::ALL, IdleActionDto::label, &action)),
        ..input(&no, &module)
    };
    create(w, req).await;
}

#[when(expr = "使用者新增程式 {string} 模組 {string}，傳遞帳別 {string}")]
async fn create_with_pass_book(w: &mut ErpWorld, no: String, module: String, flag: String) {
    let req = ProgramInput {
        pass_book_no: yes_no(&flag),
        ..input(&no, &module)
    };
    create(w, req).await;
}

#[when(expr = "使用者新增程式 {string} 模組 {string}，執行指令 {string}")]
async fn create_with_run_command(w: &mut ErpWorld, no: String, module: String, cmd: String) {
    let req = ProgramInput {
        run_command: Some(cmd),
        ..input(&no, &module)
    };
    create(w, req).await;
}

#[when(expr = "使用者新增程式 {string} 模組 {string}，執行環境設定為:")]
async fn create_with_runtime(w: &mut ErpWorld, step: &Step, no: String, module: String) {
    let mut req = input(&no, &module);
    for (field, value) in field_rows(step) {
        match field.as_str() {
            "執行指令" => req.run_command = Some(value),
            "視窗型態" => req.window_style = Some(value),
            "報表副檔名" => {
                req.report_ext_mode = Some(by_label(
                    &ReportExtModeDto::ALL,
                    ReportExtModeDto::label,
                    &value,
                ));
            }
            "說明開關" => req.help_enabled = yes_no(&value),
            "錯誤寫入記錄" => req.log_errors = yes_no(&value),
            other => panic!("未知欄位：{other}"),
        }
    }
    create(w, req).await;
}

#[when(expr = "使用者將程式 {string} 的代號改為 {string}")]
async fn change_program_no(w: &mut ErpWorld, old: String, new: String) {
    w.last = Some(w.programs.change_no(&w.actor, &old, &new).await);
}

#[when(expr = "使用者將程式 {string} 的模組改為 {string}")]
async fn change_module(w: &mut ErpWorld, no: String, module: String) {
    let result = match w.programs.get(&no).await {
        Ok(p) => {
            let req = ProgramInput {
                module_code: module,
                ..p.to_input()
            };
            w.programs.update(&w.actor, &no, req).await.map(|_| ())
        }
        Err(e) => Err(e),
    };
    w.last = Some(result);
}

#[when(expr = "使用者刪除程式 {string}")]
async fn delete_program(w: &mut ErpWorld, no: String) {
    w.last = Some(w.programs.delete(&w.actor, &no).await);
}

#[when(expr = "使用者將程式 {string} 複製為 {string}")]
async fn copy_program(w: &mut ErpWorld, from: String, to: String) {
    w.last = Some(w.programs.copy(&w.actor, &from, &to).await.map(|_| ()));
}

#[when(expr = "使用者為程式 {string} 設定權限類別 {string} 的權限")]
async fn grant_permission(w: &mut ErpWorld, no: String, class: String) {
    w.last = Some(w.programs.grant(&no, &class).await);
}

// ----- 那麼 -----

#[then(expr = "程式 {string} 的資料應為:")]
async fn program_fields(w: &mut ErpWorld, step: &Step, no: String) {
    let p = program(w, &no).await;
    for (field, expected) in field_rows(step) {
        let actual = match field.as_str() {
            "報表類型" => p.report_type.label().to_string(),
            "閒置控制" => p.idle_control.label().to_string(),
            "建立者" => p.created_by.clone(),
            "建立部門" => p.created_dept.clone(),
            "最後修改者" => p.updated_by.clone().unwrap_or_default(),
            "延後列印次數" => p.deferred_print_count.to_string(),
            "目前列印次數" => p.current_print_count.to_string(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, expected, "欄位「{field}」");
    }
}

#[then(expr = "程式 {string} 的執行環境設定應為:")]
async fn runtime_fields(w: &mut ErpWorld, step: &Step, no: String) {
    let p = program(w, &no).await;
    for (field, expected) in field_rows(step) {
        let actual = match field.as_str() {
            "執行指令" => p.run_command.clone().unwrap_or_default(),
            "視窗型態" => p.window_style.clone().unwrap_or_default(),
            "報表副檔名" => opt_label(p.report_ext_mode, ReportExtModeDto::label),
            "說明開關" => yes_no_label(p.help_enabled).to_string(),
            "錯誤寫入記錄" => yes_no_label(p.log_errors).to_string(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, expected, "欄位「{field}」");
    }
}

#[then(expr = "程式 {string} 的程式類別應為 {string}")]
async fn program_type(w: &mut ErpWorld, no: String, expected: String) {
    assert_eq!(
        program(w, &no).await.program_type.unwrap_or_default(),
        expected
    );
}

#[then(expr = "程式 {string} 的閒置秒數應為 {string}")]
async fn idle_seconds(w: &mut ErpWorld, no: String, expected: String) {
    assert_eq!(opt_num(program(w, &no).await.idle_seconds), expected);
}

#[then(expr = "程式 {string} 的逾時處理應為 {string}")]
async fn idle_action(w: &mut ErpWorld, no: String, expected: String) {
    let p = program(w, &no).await;
    assert_eq!(opt_label(p.idle_action, IdleActionDto::label), expected);
}

#[then(expr = "程式 {string} 的傳遞帳別應為 {string}")]
async fn pass_book_no(w: &mut ErpWorld, no: String, expected: String) {
    assert_eq!(yes_no_label(program(w, &no).await.pass_book_no), expected);
}

#[then(expr = "程式 {string} 的最後修改者應為 {string}")]
async fn updated_by(w: &mut ErpWorld, no: String, expected: String) {
    assert_eq!(
        program(w, &no).await.updated_by.unwrap_or_default(),
        expected
    );
}

#[then(expr = "程式 {string} 的建立者應為 {string}")]
async fn created_by(w: &mut ErpWorld, no: String, expected: String) {
    assert_eq!(program(w, &no).await.created_by, expected);
}

#[then(expr = "程式 {string} 的名稱應為 {string}")]
async fn program_name(w: &mut ErpWorld, no: String, expected: String) {
    assert_eq!(program(w, &no).await.name.unwrap_or_default(), expected);
}

#[then(expr = "程式 {string} 的執行指令應為 {string}")]
async fn run_command(w: &mut ErpWorld, no: String, expected: String) {
    assert_eq!(
        program(w, &no).await.run_command.unwrap_or_default(),
        expected
    );
}

#[then(expr = "程式 {string} 應不存在")]
async fn program_absent(w: &mut ErpWorld, no: String) {
    match w.programs.get(&no).await {
        Err(e) if e.code() == "PROGRAM_NOT_FOUND" => {}
        other => panic!("預期程式不存在，實際：{other:?}"),
    }
}

#[then(expr = "程式 {string} 應無任何語系名稱")]
async fn no_names(w: &mut ErpWorld, no: String) {
    let names = w.programs.names(&no).await.unwrap();
    assert!(names.is_empty(), "仍有名稱：{names:?}");
}

#[then(expr = "權限類別 {string} 應擁有程式 {string} 的權限")]
async fn has_permission(w: &mut ErpWorld, class: String, no: String) {
    let classes = w.programs.permission_classes(&no).await.unwrap();
    assert!(classes.contains(&class), "權限類別：{classes:?}");
}

#[then(expr = "權限類別 {string} 應不擁有程式 {string} 的權限")]
async fn lacks_permission(w: &mut ErpWorld, class: String, no: String) {
    let classes = w.programs.permission_classes(&no).await.unwrap();
    assert!(!classes.contains(&class), "權限類別：{classes:?}");
}

#[then(expr = "選單 {string} 應包含程式 {string}")]
async fn menu_has(w: &mut ErpWorld, menu: String, no: String) {
    let menus = w.programs.menus_containing(&no).await.unwrap();
    assert!(menus.contains(&menu), "所屬選單：{menus:?}");
}

#[then(expr = "選單 {string} 應不包含程式 {string}")]
async fn menu_lacks(w: &mut ErpWorld, menu: String, no: String) {
    let menus = w.programs.menus_containing(&no).await.unwrap();
    assert!(!menus.contains(&menu), "所屬選單：{menus:?}");
}

#[then(expr = "應有一筆 {string} 刪除程式 {string} 的異動紀錄")]
async fn delete_audited(w: &mut ErpWorld, user: String, no: String) {
    let entries = w.programs.audit_entries(&no).await.unwrap();
    let hits = entries
        .iter()
        .filter(|e| e.program == "p_zz" && e.user_id == user && e.action == "delete")
        .count();
    assert_eq!(hits, 1, "異動紀錄：{entries:?}");
}
