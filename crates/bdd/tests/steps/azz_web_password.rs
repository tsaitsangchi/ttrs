//! Web 密碼登入 — specs/features/azz/web_password.feature（ADR 0009）

use chrono::{Duration, Local};
use cucumber::{gherkin::Step, given, then, when};
use ttrs_domain::password::{AccountPolicy, Complexity, Credential, ExpiryBasis};
use ttrs_shared::UserInput;

use crate::{
    Backend, ErpWorld,
    steps::{
        azz_program_maintenance::seed,
        azz_sign_in::{keep_session, user_input},
        azz_user_maintenance::{seed_user, standard},
        common::rows,
    },
};

/// 使用者目前的密碼狀態：記憶體後端讀 InMemoryStore，PostgreSQL 讀 user_credential
async fn credential(w: &ErpWorld, id: &str) -> Credential {
    match &w.backend {
        Backend::Memory(s) => s.credential_of(id).unwrap_or_default(),
        Backend::Pg(pool) => {
            let row: Option<(Option<String>, i32)> = sqlx::query_as(
                "SELECT password_hash, failed_sign_in_count FROM user_credential WHERE user_id = $1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await
            .unwrap();
            row.map(|(hash, failed)| Credential {
                password_hash: hash,
                failed_count: u32::try_from(failed).unwrap(),
                ..Credential::default()
            })
            .unwrap_or_default()
        }
    }
}

async fn sign_in(w: &mut ErpWorld, id: &str, password: &str) {
    w.session = None;
    let result = w.sign_in.sign_in(id, password).await;
    keep_session(w, result);
}

fn yes_no(v: &str) -> bool {
    match v {
        "是" => true,
        "否" => false,
        other => panic!("未知的是/否：{other}"),
    }
}

// ----- 假設 -----

#[given(expr = "帳號稽核原則為:")]
async fn account_policy(w: &mut ErpWorld, step: &Step) {
    let mut p = AccountPolicy::default();
    for r in rows(step) {
        let v = r[1].as_str();
        let n = || v.parse::<u32>().unwrap_or_else(|_| panic!("不是數字：{v}"));
        match r[0].as_str() {
            "密碼更新依據" => {
                p.expiry_basis = match v {
                    "天數" => ExpiryBasis::Days,
                    "次數" => ExpiryBasis::Uses,
                    "不要求" => ExpiryBasis::Never,
                    other => panic!("未知的更新依據：{other}"),
                }
            }
            "密碼更新週期（天）" => p.cycle_days = n(),
            "密碼可用次數" => p.use_limit = n(),
            "提前通知天數" => p.notify = n(),
            "密碼最小長度" => p.min_length = n(),
            "密碼複雜度" => {
                p.complexity =
                    Complexity::from_code(v).unwrap_or_else(|| panic!("未知的複雜度：{v}"))
            }
            "可與原密碼相同" => p.allow_same_as_old = yes_no(v),
            "允許最大試誤次數" => p.max_failures = n(),
            other => panic!("未知的項目：{other}"),
        }
    }
    let basis = match p.expiry_basis {
        ExpiryBasis::Never => "0",
        ExpiryBasis::Days => "1",
        ExpiryBasis::Uses => "2",
    };
    let nums = [
        p.cycle_days,
        p.use_limit,
        p.notify,
        p.min_length,
        p.max_failures,
    ]
    .map(|n| n.to_string());
    let same = p.allow_same_as_old.to_string();
    let complexity = p.complexity.code();
    seed(
        w,
        |s| s.set_account_policy(p.clone()),
        "UPDATE account_policy SET expiry_basis = $1, cycle_days = $2::int, use_limit = $3::int,
             notify = $4::int, min_length = $5::int, max_failures = $6::int,
             allow_same_as_old = $7::boolean, complexity = $8",
        &[
            basis, &nums[0], &nums[1], &nums[2], &nums[3], &nums[4], &same, complexity,
        ],
    )
    .await;
}

/// 建立使用者後由初始密碼（＝使用者代號）改為指定密碼，因此不須再改密碼
#[given(expr = "已有使用者 {string}，預設營運中心 {string}，Web 密碼 {string}")]
async fn user_with_password(w: &mut ErpWorld, id: String, plant: String, password: String) {
    new_user(w, id.clone(), plant).await;
    w.sign_in
        .change_password(&id, &id, &password, &password)
        .await
        .unwrap_or_else(|e| panic!("設定 {id} 的密碼失敗：{e}"));
    w.passwords.insert(id, password);
}

#[given(expr = "已有新使用者 {string}，預設營運中心 {string}")]
async fn new_user(w: &mut ErpWorld, id: String, plant: String) {
    let req = UserInput {
        default_plant: plant,
        ..standard(&id)
    };
    seed_user(w, &w.actor, req).await;
}

#[given(expr = "以使用者代號 {string}、密碼 {string} 登入 {int} 次")]
async fn sign_in_times(w: &mut ErpWorld, id: String, password: String, times: u32) {
    for _ in 0..times {
        let _ = w.sign_in.sign_in(&id, &password).await;
    }
}

#[given(expr = "使用者 {string} 的密碼更新日為 {int} 天前")]
async fn password_changed_days_ago(w: &mut ErpWorld, id: String, days: i64) {
    let date = Local::now().date_naive() - Duration::days(days);
    let text = days.to_string();
    seed(
        w,
        |s| s.set_password_changed_on(&id, date),
        "UPDATE user_credential SET password_changed_on = current_date - $2::int WHERE user_id = $1",
        &[&id, &text],
    )
    .await;
}

// ----- 當 -----

#[when(expr = "以使用者代號 {string}、密碼 {string} 登入")]
async fn sign_in_with(w: &mut ErpWorld, id: String, password: String) {
    sign_in(w, &id, &password).await;
}

#[when(expr = "使用者解除使用者 {string} 的鎖定")]
async fn unlock(w: &mut ErpWorld, id: String) {
    w.last = Some(w.users.unlock(&w.actor, &id).await);
}

#[when(expr = "使用者將使用者 {string} 的 Web 密碼設為 {string}")]
async fn set_web_password(w: &mut ErpWorld, id: String, password: String) {
    let req = UserInput {
        web_password: Some(password),
        ..user_input(w, &id).await
    };
    w.last = Some(w.users.update(&w.actor, &id, req).await.map(drop));
}

#[when(expr = "使用者 {string} 將密碼由 {string} 改為 {string}")]
async fn change_password(w: &mut ErpWorld, id: String, old: String, new: String) {
    change_password_confirm(w, id, old, new.clone(), new).await;
}

#[when(expr = "使用者 {string} 將密碼由 {string} 改為 {string}，確認 {string}")]
async fn change_password_confirm(
    w: &mut ErpWorld,
    id: String,
    old: String,
    new: String,
    confirm: String,
) {
    let result = w.sign_in.change_password(&id, &old, &new, &confirm).await;
    if result.is_ok() {
        w.passwords.insert(id, new);
    }
    w.last = Some(result);
}

// ----- 那麼 -----

#[then(expr = "使用者 {string} 的已試誤次數應為 {int}")]
async fn failed_count(w: &mut ErpWorld, id: String, expected: u32) {
    assert_eq!(credential(w, &id).await.failed_count, expected);
}

/// 須改密碼：沒有建立連線，但可以用剛才的密碼當舊密碼變更
#[then(expr = "使用者 {string} 只能變更密碼")]
async fn only_change_password(w: &mut ErpWorld, id: String) {
    assert!(w.session.is_none(), "不應建立連線");
    let old = w.passwords.get(&id).cloned().unwrap_or_else(|| id.clone());
    w.sign_in
        .change_password(&id, &old, "NewPass99", "NewPass99")
        .await
        .unwrap_or_else(|e| panic!("變更密碼失敗：{e}"));
    w.sign_in
        .sign_in(&id, "NewPass99")
        .await
        .unwrap_or_else(|e| panic!("改密碼後登入失敗：{e}"));
}

#[then(expr = "操作應成功，並提示密碼將於 {int} 天後到期")]
fn expiring(w: &mut ErpWorld, days: i64) {
    match w.last.as_ref().expect("尚未執行任何操作") {
        Ok(()) => {}
        Err(e) => panic!("預期成功，實際錯誤 {}：{e}", e.code()),
    }
    let s = w.session.as_ref().expect("尚未登入");
    assert_eq!(s.password_expires_in_days, Some(days));
}

#[then(expr = "以使用者代號 {string}、密碼 {string} 登入應成功")]
async fn sign_in_ok(w: &mut ErpWorld, id: String, password: String) {
    w.sign_in
        .sign_in(&id, &password)
        .await
        .unwrap_or_else(|e| panic!("預期登入成功，實際錯誤 {}：{e}", e.code()));
}

#[then(expr = "以使用者代號 {string}、密碼 {string} 登入應出現錯誤 {string}")]
async fn sign_in_fails(w: &mut ErpWorld, id: String, password: String, code: String) {
    match w.sign_in.sign_in(&id, &password).await {
        Ok(_) => panic!("預期錯誤 {code}，實際成功"),
        Err(e) => assert_eq!(e.code(), code),
    }
}

#[then(expr = "使用者 {string} 存放的密碼不等於 {string}")]
async fn stored_hash(w: &mut ErpWorld, id: String, password: String) {
    let hash = credential(w, &id)
        .await
        .password_hash
        .expect("尚未設定密碼");
    assert_ne!(hash, password);
    assert!(hash.starts_with("$argon2id$"), "應為 Argon2id 雜湊：{hash}");
}
