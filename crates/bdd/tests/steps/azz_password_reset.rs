//! 忘記密碼自助重設 — specs/features/azz/password_reset.feature（ADR 0010 §A）

use std::sync::Arc;

use chrono::{Local, NaiveDate};
use cucumber::{given, then, when};
use ttrs_app::mail::{Mail, MailSettings, MailTemplate};

use crate::{
    Backend, ErpWorld,
    steps::{
        azz_program_maintenance::seed, azz_user_maintenance::modify,
        azz_web_password::user_with_password,
    },
};

/// 測試用的對外網址（R4：TTRS_PUBLIC_URL）
const PUBLIC_URL: &str = "https://ttrs.example.com";

/// 信件中的重設連結 → 權杖
fn token_of(mail: &Mail) -> String {
    let (_, rest) = mail
        .body
        .split_once("token=")
        .unwrap_or_else(|| panic!("信件沒有重設連結：{}", mail.body));
    rest.chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect()
}

fn sent(w: &ErpWorld, to: &str, template: MailTemplate) -> Vec<Mail> {
    w.outbox
        .sent()
        .into_iter()
        .filter(|m| m.to == to && m.template == template)
        .collect()
}

/// 申請重設；有寄出新的重設信時記下其中的權杖
async fn request(w: &mut ErpWorld, id: &str) {
    let before = w.outbox.sent().len();
    let result = w.password_reset.request(id).await;
    if let Some(m) = w.outbox.sent().get(before..).and_then(|new| {
        new.iter()
            .rev()
            .find(|m| m.template == MailTemplate::PasswordReset)
            .cloned()
    }) {
        w.reset_token = Some(token_of(&m));
    }
    w.last = Some(result);
}

fn token(w: &ErpWorld) -> String {
    w.reset_token.clone().expect("尚未收到重設連結")
}

async fn reset(w: &mut ErpWorld, token: &str, new: &str, confirm: &str) {
    w.last = Some(w.password_reset.reset(token, new, confirm).await);
}

/// 密碼更新日與須改密碼（zx16、zx19）
async fn changed_on_and_required(w: &ErpWorld, id: &str) -> (Option<NaiveDate>, bool) {
    match &w.backend {
        Backend::Memory(s) => {
            let c = s.credential_of(id).unwrap_or_default();
            (c.changed_on, c.change_required)
        }
        Backend::Pg(pool) => sqlx::query_as(
            "SELECT password_changed_on, password_change_required FROM user_credential WHERE user_id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap(),
    }
}

// ----- 假設 -----

#[given("已設定寄信伺服器")]
fn mail_configured(w: &mut ErpWorld) {
    let settings = MailSettings {
        mailer: Arc::new(w.outbox.clone()),
        public_url: PUBLIC_URL.into(),
    };
    w.password_reset = w.password_reset.clone().with_mail(settings);
}

#[given("未設定寄信伺服器")]
fn mail_not_configured(w: &mut ErpWorld) {
    w.password_reset = w.password_reset.clone().without_mail();
}

#[given(expr = "已有使用者 {string}，預設營運中心 {string}，Web 密碼 {string}，Email {string}")]
async fn user_with_email(
    w: &mut ErpWorld,
    id: String,
    plant: String,
    password: String,
    email: String,
) {
    user_with_password(w, id.clone(), plant, password).await;
    modify(w, &id, |r| r.email = Some(email)).await;
    w.last.take().expect("設定 Email").expect("設定 Email 失敗");
}

#[given(expr = "使用者 {string} 不存在")]
async fn user_absent(w: &mut ErpWorld, id: String) {
    assert!(w.users.get(&w.actor, &id).await.is_err(), "{id} 不應存在");
}

#[given(expr = "使用者 {string} 沒有 Email")]
async fn user_without_email(w: &mut ErpWorld, id: String) {
    modify(w, &id, |r| r.email = None).await;
    w.last.take().expect("清除 Email").expect("清除 Email 失敗");
}

#[given(expr = "使用者 {string} 已過有效日期")]
async fn user_expired(w: &mut ErpWorld, id: String) {
    crate::steps::azz_sign_in::user_valid_until(w, id, "昨天".into()).await;
}

#[given(expr = "使用者 {string} 的語系為 {string}")]
async fn user_lang(w: &mut ErpWorld, id: String, lang: String) {
    modify(w, &id, |r| r.lang = Some(lang)).await;
    w.last.take().expect("設定語系").expect("設定語系失敗");
}

#[given(expr = "使用者 {string} 已申請重設密碼")]
async fn requested(w: &mut ErpWorld, id: String) {
    request(w, &id).await;
    w.last.take().expect("申請").expect("申請重設密碼失敗");
    assert!(w.reset_token.is_some(), "應寄出重設連結");
}

#[given(expr = "使用者 {string} 又申請了一次重設密碼")]
async fn requested_again(w: &mut ErpWorld, id: String) {
    // 新連結另外收下，舊的 w.reset_token 留給「當」步驟使用
    let old = w.reset_token.clone();
    w.password_reset.request(&id).await.unwrap();
    w.reset_token = old;
}

#[given(expr = "使用者 {string} 在 1 小時內已申請重設密碼 {int} 次")]
async fn requested_times(w: &mut ErpWorld, id: String, times: u32) {
    for _ in 0..times {
        w.password_reset.request(&id).await.unwrap();
    }
}

/// 權杖的建立與到期時間往前推（不改系統時鐘）
#[given(expr = "已過 {int} 分鐘")]
async fn minutes_pass(w: &mut ErpWorld, minutes: i64) {
    let text = minutes.to_string();
    seed(
        w,
        |s| s.age_reset_tokens(minutes),
        "UPDATE password_reset_token SET created_at = created_at - make_interval(mins => $1::int),
             expires_at = expires_at - make_interval(mins => $1::int)",
        &[&text],
    )
    .await;
}

#[given(expr = "已用此連結將密碼設為 {string}")]
async fn used_link(w: &mut ErpWorld, password: String) {
    let t = token(w);
    w.password_reset
        .reset(&t, &password, &password)
        .await
        .unwrap_or_else(|e| panic!("以連結重設失敗：{e}"));
}

// ----- 當 -----

#[when(expr = "以使用者代號 {string} 申請重設密碼")]
async fn request_reset(w: &mut ErpWorld, id: String) {
    request(w, &id).await;
}

#[when(expr = "以收到的重設連結將密碼設為 {string}，確認 {string}")]
async fn reset_with_link(w: &mut ErpWorld, new: String, confirm: String) {
    let t = token(w);
    reset(w, &t, &new, &confirm).await;
}

#[when(expr = "以重設連結 {string} 將密碼設為 {string}，確認 {string}")]
async fn reset_with_token(w: &mut ErpWorld, token: String, new: String, confirm: String) {
    reset(w, &token, &new, &confirm).await;
}

// ----- 那麼 -----

/// 不論是否寄信都回應成功、顯示同一個訊息（R2）
#[then("應顯示「若此帳號設有 Email，重設連結已寄出」")]
fn same_message(w: &mut ErpWorld) {
    match w.last.as_ref().expect("尚未申請") {
        Ok(()) => {}
        Err(e) => panic!("應顯示同一個訊息，實際錯誤 {}：{e}", e.code()),
    }
}

#[then(expr = "{string} 應收到 {int} 封重設密碼信")]
#[then(expr = "{string} 共應收到 {int} 封重設密碼信")]
fn reset_mails(w: &mut ErpWorld, to: String, n: usize) {
    assert_eq!(sent(w, &to, MailTemplate::PasswordReset).len(), n);
}

#[then(expr = "{string} 應收到 {int} 封密碼已變更通知信")]
fn changed_mails(w: &mut ErpWorld, to: String, n: usize) {
    assert_eq!(sent(w, &to, MailTemplate::PasswordChanged).len(), n);
}

#[then("不應寄出任何信件")]
fn no_mail(w: &mut ErpWorld) {
    assert!(
        w.outbox.sent().is_empty(),
        "不應寄信：{:?}",
        w.outbox.sent()
    );
}

#[then(expr = "{string} 收到的重設密碼信應為 {string} 語系")]
fn mail_lang(w: &mut ErpWorld, to: String, lang: String) {
    let mails = sent(w, &to, MailTemplate::PasswordReset);
    let m = mails.last().expect("沒有重設密碼信");
    assert_eq!(m.lang.as_str(), lang);
}

#[then(expr = "使用者 {string} 的密碼更新日應為今天")]
async fn changed_today(w: &mut ErpWorld, id: String) {
    let (on, _) = changed_on_and_required(w, &id).await;
    assert_eq!(on, Some(Local::now().date_naive()));
}

#[then(expr = "使用者 {string} 不須改密碼")]
async fn no_change_required(w: &mut ErpWorld, id: String) {
    let (_, required) = changed_on_and_required(w, &id).await;
    assert!(!required, "不應須改密碼");
}

/// 新密碼不合格時權杖不消耗：之後仍可用合格的密碼完成重設
#[then("重設連結仍可使用")]
async fn link_still_usable(w: &mut ErpWorld) {
    let t = token(w);
    w.password_reset
        .reset(&t, "Valid0Pass", "Valid0Pass")
        .await
        .unwrap_or_else(|e| panic!("重設連結應仍可使用，實際錯誤 {}：{e}", e.code()));
}

/// 重設成功後原本的連線失效（R6）
#[then(expr = "使用者 {string} 原本的連線應已失效")]
async fn session_ended(w: &mut ErpWorld, id: String) {
    let s = w.session.as_ref().expect("尚未登入");
    assert_eq!(s.user_id, id);
    match w.sign_in.check_session(&id, s.session_version).await {
        Ok(()) => panic!("原本的連線應已失效"),
        Err(e) => assert_eq!(e.code(), "NOT_SIGNED_IN"),
    }
}

#[then("資料庫中不應存有收到的重設權杖原文")]
async fn token_not_stored(w: &mut ErpWorld) {
    let t = token(w);
    let stored: Vec<String> = match &w.backend {
        Backend::Memory(s) => s.reset_token_hashes(),
        Backend::Pg(pool) => sqlx::query_scalar("SELECT token_hash FROM password_reset_token")
            .fetch_all(pool)
            .await
            .unwrap(),
    };
    assert!(!stored.is_empty(), "應有重設權杖紀錄");
    assert!(
        stored.iter().all(|h| !h.contains(&t)),
        "資料庫不應存權杖原文"
    );
}
