//! 伺服器端緊急重設密碼 — specs/features/azz/emergency_password_reset.feature（ADR 0010 §B、§C）

use std::sync::Arc;

use cucumber::{given, then, when};
use ttrs_app::{Actor, Services, memory::InMemoryStore};
use ttrs_shared::{ProgramInput, UserInput};

use crate::{
    ErpWorld,
    steps::{
        azz_authorization::{class_input, set_class_permission},
        azz_program_maintenance::seed,
        azz_user_maintenance::{modify, seed_user, standard},
    },
};

/// 使用者維護管理員用的權限類別
const ADMIN_CLASS: &str = "UADMIN";

/// 執行 ttrs-admin；作業系統使用者預設為 root
async fn run(w: &mut ErpWorld, command: &str, operator: &str) {
    let args: Vec<&str> = command.split_whitespace().collect();
    assert_eq!(args.first(), Some(&"ttrs-admin"), "只支援 ttrs-admin 指令");
    w.command = Some(ttrs_api::admin_cli::run(&w.emergency, &args[1..], operator).await);
}

fn temp_password(w: &ErpWorld) -> String {
    match w.command.as_ref().expect("尚未執行指令") {
        Ok(out) => out.temp_password.clone().expect("指令沒有顯示臨時密碼"),
        Err(e) => panic!("指令失敗 {}：{e}", e.code()),
    }
}

/// 使用者維護（user-maintenance）登錄為程式，權限類別 UADMIN 有 modify 功能
async fn ensure_admin_class(w: &ErpWorld) {
    let sql = "INSERT INTO module (module_code, name) VALUES ('AZZ', 'AZZ') ON CONFLICT DO NOTHING";
    seed(w, |s| s.add_module("AZZ"), sql, &[]).await;
    let req = ProgramInput {
        program_type: Some("I".into()),
        program_no: "user-maintenance".into(),
        module_code: "AZZ".into(),
        default_actions: Some(
            "insert, query, modify, delete, reproduce, invalid, user_unlock".into(),
        ),
        ..Default::default()
    };
    match w.programs.create(&Actor::default(), req).await {
        Ok(_) => {}
        Err(e) if e.code() == "PROGRAM_ALREADY_EXISTS" => {}
        Err(e) => panic!("登錄 user-maintenance 失敗：{e}"),
    }
    match w.classes.create(&w.actor, class_input(ADMIN_CLASS)).await {
        Ok(_) => {}
        Err(e) if e.code() == "PERMISSION_CLASS_ALREADY_EXISTS" => {}
        Err(e) => panic!("建立權限類別 {ADMIN_CLASS} 失敗：{e}"),
    }
    set_class_permission(
        w,
        ADMIN_CLASS,
        "user-maintenance",
        Some(vec!["modify".into()]),
        ["0", "0", "0"],
    )
    .await;
}

async fn make_admin(w: &mut ErpWorld, id: &str) {
    if w.users.get(&w.actor, id).await.is_err() {
        seed_user(w, &w.actor, standard(id)).await;
    }
    modify(w, id, |r| r.permission_class = ADMIN_CLASS.into()).await;
    w.last
        .take()
        .expect("設定權限類別")
        .unwrap_or_else(|e| panic!("設定 {id} 的權限類別失敗：{e}"));
}

// ----- 假設 -----

#[given(expr = "只有使用者 {string} 擁有使用者維護（user-maintenance）的 modify 權限")]
async fn only_admin(w: &mut ErpWorld, id: String) {
    ensure_admin_class(w).await;
    make_admin(w, &id).await;
}

#[given(expr = "使用者 {string} 也擁有使用者維護（user-maintenance）的 modify 權限")]
async fn another_admin(w: &mut ErpWorld, id: String) {
    ensure_admin_class(w).await;
    make_admin(w, &id).await;
}

// ----- 當 -----

#[when(expr = "在伺服器執行 {string}")]
async fn run_command(w: &mut ErpWorld, command: String) {
    run(w, &command, "root").await;
}

#[when(expr = "作業系統使用者 {string} 在伺服器執行 {string}")]
async fn run_command_as(w: &mut ErpWorld, operator: String, command: String) {
    run(w, &command, &operator).await;
}

#[when(expr = "使用者將使用者 {string} 停用")]
async fn deactivate(w: &mut ErpWorld, id: String) {
    w.last = Some(w.users.deactivate(&w.actor, &id).await.map(drop));
}

#[when(expr = "使用者將使用者 {string} 的權限類別改為無權限")]
async fn class_without_permission(w: &mut ErpWorld, id: String) {
    modify(w, &id, |r: &mut UserInput| {
        r.permission_class = "CLASS1".into()
    })
    .await;
}

// ----- 那麼 -----

#[then("指令應成功並顯示一組臨時密碼")]
fn shows_temp_password(w: &mut ErpWorld) {
    let p = temp_password(w);
    assert!(p.chars().count() >= 12, "臨時密碼太短：{p}");
    let out = w.command.as_ref().unwrap().as_ref().unwrap();
    assert!(out.message.contains(&p), "應在輸出顯示臨時密碼");
}

#[then("指令應成功")]
fn command_ok(w: &mut ErpWorld) {
    if let Err(e) = w.command.as_ref().expect("尚未執行指令") {
        panic!("指令失敗 {}：{e}", e.code());
    }
}

#[then(expr = "指令應失敗並顯示錯誤 {string}")]
fn command_fails(w: &mut ErpWorld, code: String) {
    match w.command.as_ref().expect("尚未執行指令") {
        Ok(out) => panic!("預期錯誤 {code}，實際成功：{}", out.message),
        Err(e) => assert_eq!(e.code(), code),
    }
}

#[then(expr = "以使用者代號 {string}、顯示的臨時密碼登入應出現錯誤 {string}")]
async fn sign_in_with_temp(w: &mut ErpWorld, id: String, code: String) {
    let p = temp_password(w);
    match w.sign_in.sign_in(&id, &p).await {
        Ok(_) => panic!("預期錯誤 {code}，實際成功"),
        Err(e) => assert_eq!(e.code(), code),
    }
}

#[then(regex = r"^稽核紀錄應有一筆「伺服器指令 (\S+) 重設使用者 (\S+) 的密碼」$")]
async fn audit_recorded(w: &mut ErpWorld, operator: String, id: String) {
    let entries = w.programs.audit_entries(&w.actor, &id).await.unwrap();
    assert!(
        entries.iter().any(|e| e.program == "ttrs-admin"
            && e.user_id == format!("os:{operator}")
            && e.action == "reset-password"),
        "應有稽核紀錄：{entries:?}"
    );
}

/// 臨時密碼只顯示在終端機：稽核紀錄不含臨時密碼（ttrs-admin 也不寫入程式紀錄）
#[then("稽核紀錄與程式紀錄都不應含有臨時密碼")]
async fn audit_has_no_password(w: &mut ErpWorld) {
    let p = temp_password(w);
    let entries = w.programs.audit_entries(&w.actor, "admin").await.unwrap();
    assert!(!entries.is_empty(), "應有稽核紀錄");
    for e in &entries {
        let text = format!("{} {} {} {}", e.program, e.user_id, e.key, e.action);
        assert!(!text.contains(&p), "稽核紀錄不應含臨時密碼：{text}");
    }
}

/// HTTP API 沒有緊急重設的路由（緊急重設只在 ttrs-admin，R11）
#[then("API 不應提供緊急重設密碼的端點")]
async fn no_http_endpoint(_w: &mut ErpWorld) {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    let services = Services::new(Arc::new(InMemoryStore::new()));
    let app = ttrs_api::router(ttrs_api::AppState::with_sign_in(services, false));
    // 使用者維護原有的 POST /api/users/{id}/unlock 需權限，不屬於緊急重設
    let mut uris: Vec<String> = ttrs_api::admin_cli::COMMANDS
        .iter()
        .flat_map(|c| [format!("/api/{c}"), format!("/api/admin/{c}")])
        .collect();
    uris.push("/api/users/admin/reset-password".into());
    for uri in uris {
        let req = Request::post(&uri).body(Body::empty()).unwrap();
        let status = app.clone().oneshot(req).await.unwrap().status();
        assert_eq!(status.as_u16(), 404, "{uri} 不應存在");
    }
}
