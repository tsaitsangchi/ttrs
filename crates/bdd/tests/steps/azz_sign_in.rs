//! Sign In 登入與切換營運中心 — specs/features/azz/sign_in.feature

use chrono::{Duration, Local};
use cucumber::{given, then, when};
use ttrs_app::SignInIdentity;
use ttrs_shared::{SessionDto, UserInput};

use crate::{
    ErpWorld,
    steps::{azz_program_maintenance::seed, azz_user_maintenance::standard},
};

fn identity(username: &str, subject: &str) -> SignInIdentity {
    SignInIdentity {
        subject: subject.to_owned(),
        username: username.to_owned(),
    }
}

/// 登入結果：成功時記下目前的連線
fn keep_session(w: &mut ErpWorld, result: Result<SessionDto, ttrs_app::AppError>) {
    w.last = Some(result.map(|s| {
        w.session = Some(s);
    }));
}

fn session(w: &ErpWorld) -> &SessionDto {
    w.session.as_ref().expect("尚未登入")
}

async fn user_input(w: &ErpWorld, id: &str) -> UserInput {
    w.users
        .get(&w.actor, id)
        .await
        .unwrap_or_else(|e| panic!("讀取使用者 {id} 失敗：{e}"))
        .to_input()
}

/// 改為多營運中心，再依「變更預設營運中心時加入清單」的規則加入其餘營運中心，最後改回第一個為預設
async fn make_multi_plant(w: &mut ErpWorld, id: &str, plants: &[&str]) {
    let mut req = user_input(w, id).await;
    req.multi_plant = true;
    req.default_plant = plants[0].into();
    w.users.update(&w.actor, id, req).await.unwrap();
    for p in plants[1..].iter().chain(&plants[..1]) {
        let mut req = user_input(w, id).await;
        req.default_plant = (*p).into();
        w.users.update(&w.actor, id, req).await.unwrap();
    }
}

// ----- 假設 -----

#[given(expr = "已有使用者 {string}，公司帳號 {string}，預設營運中心 {string}")]
async fn existing_user(w: &mut ErpWorld, id: String, account: String, plant: String) {
    let req = UserInput {
        idp_username: Some(account),
        default_plant: plant,
        ..standard(&id)
    };
    w.users.create(&w.actor, req).await.unwrap();
}

#[given(expr = "公司帳號 {string}（識別碼 {string}）已登入過")]
async fn signed_in_before(w: &mut ErpWorld, account: String, subject: String) {
    w.sign_in
        .sign_in(identity(&account, &subject))
        .await
        .unwrap_or_else(|e| panic!("{account} 登入失敗：{e}"));
}

#[given(expr = "使用者 {string} 已停用")]
async fn user_deactivated(w: &mut ErpWorld, id: String) {
    w.users.deactivate(&w.actor, &id).await.unwrap();
}

/// 有效日期是先前設定的（資料有效仍為是）：使用者維護在存檔當天就把到期的帳號設為無效
/// （p_zx.4gl:977-981），登入時只擋早於今天的有效日期（weblogin.4gl:465）
#[given(expr = "使用者 {string} 的有效日期為 {word}")]
async fn user_valid_until(w: &mut ErpWorld, id: String, day: String) {
    let today = Local::now().date_naive();
    let date = match day.as_str() {
        "昨天" => Some(today - Duration::days(1)),
        "今天" => Some(today),
        "空白" => None,
        other => panic!("未知的日期：{other}"),
    };
    let text = date.map(|d| d.to_string()).unwrap_or_default();
    seed(
        w,
        |s| s.set_user_valid_until(&id, date),
        "UPDATE user_account SET valid_until = NULLIF($2, '')::date WHERE user_id = $1",
        &[&id, &text],
    )
    .await;
}

#[given(expr = "營運中心 {string} 已停用")]
async fn plant_deactivated(w: &mut ErpWorld, plant: String) {
    w.plants.deactivate(&w.actor, &plant).await.unwrap();
}

#[given(expr = "使用者 {string} 為多營運中心，可使用營運中心 {string}、{string}")]
async fn multi_plant_user(w: &mut ErpWorld, id: String, a: String, b: String) {
    make_multi_plant(w, &id, &[&a, &b]).await;
}

/// zxy03＝ALL（SQ5）：使用者維護尚無可使用營運中心的維護畫面，直接寫入
#[given(expr = "使用者 {string} 為多營運中心，可使用營運中心 {string}")]
async fn multi_plant_user_all(w: &mut ErpWorld, id: String, plant: String) {
    assert_eq!(plant, "ALL", "只支援 ALL");
    let default_plant = user_input(w, &id).await.default_plant;
    make_multi_plant(w, &id, &[&default_plant]).await;
    seed(
        w,
        |s| s.set_user_all_plants(&id),
        "UPDATE user_account SET all_plants = true WHERE user_id = $1",
        &[&id],
    )
    .await;
}

#[given(expr = "使用者 {string} 已登入")]
async fn user_signed_in(w: &mut ErpWorld, id: String) {
    let account = user_input(w, &id)
        .await
        .idp_username
        .expect("未設定公司帳號");
    let s = w
        .sign_in
        .sign_in(identity(&account, &format!("sub-{id}")))
        .await
        .unwrap_or_else(|e| panic!("{id} 登入失敗：{e}"));
    w.session = Some(s);
}

// ----- 當 -----

#[when(expr = "公司帳號 {string}（識別碼 {string}）登入")]
async fn sign_in(w: &mut ErpWorld, account: String, subject: String) {
    let result = w.sign_in.sign_in(identity(&account, &subject)).await;
    keep_session(w, result);
}

#[when(expr = "使用者切換到營運中心 {string}")]
async fn switch_plant(w: &mut ErpWorld, plant: String) {
    let user = session(w).user_id.clone();
    let result = w.sign_in.switch_plant(&user, &plant).await;
    keep_session(w, result);
}

#[when(expr = "重新檢查使用者 {string} 的連線")]
async fn revalidate(w: &mut ErpWorld, id: String) {
    let result = w.sign_in.revalidate(&id).await;
    keep_session(w, result);
}

// ----- 那麼 -----

#[then(expr = "登入的使用者應為 {string}")]
fn signed_in_user(w: &mut ErpWorld, expected: String) {
    assert_eq!(session(w).user_id, expected);
}

#[then(expr = "目前營運中心應為 {string}")]
fn current_plant(w: &mut ErpWorld, expected: String) {
    assert_eq!(session(w).plant, expected);
}

/// 以識別碼登入（公司帳號名稱不同）仍對應到同一使用者，表示識別碼已記下
#[then(expr = "使用者 {string} 的公司帳號識別碼應為 {string}")]
async fn bound_subject(w: &mut ErpWorld, id: String, subject: String) {
    let s = w
        .sign_in
        .sign_in(identity("someone-else@corp.example", &subject))
        .await
        .unwrap_or_else(|e| panic!("以識別碼 {subject} 登入失敗：{e}"));
    assert_eq!(s.user_id, id);
}

#[then(expr = "可切換的營運中心應為 {string}")]
fn switchable_plants(w: &mut ErpWorld, expected: String) {
    let ids: Vec<&str> = session(w)
        .plants
        .iter()
        .map(|p| p.plant_id.as_str())
        .collect();
    assert_eq!(ids.join(","), expected);
}

#[then(expr = "使用者 {string} 的預設營運中心應為 {string}")]
async fn default_plant(w: &mut ErpWorld, id: String, expected: String) {
    assert_eq!(user_input(w, &id).await.default_plant, expected);
}
