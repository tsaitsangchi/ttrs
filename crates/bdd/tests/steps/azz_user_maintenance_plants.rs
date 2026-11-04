//! 使用者可使用的營運中心（TipTop p_zxy）— specs/features/azz/user_maintenance_plants.feature

use cucumber::{then, when};
use ttrs_shared::UserInput;

use crate::{
    ErpWorld,
    steps::azz_user_maintenance::{modify, standard, user},
};

/// 「TW01、CN01」→ 營運中心清單
fn plants(list: &str) -> Vec<String> {
    list.split('、').map(str::to_owned).collect()
}

// ----- 當 -----

#[when(
    expr = "使用者新增多營運中心使用者 {string}，預設營運中心 {string}，可使用營運中心 {string}"
)]
async fn create_multi_plant_user(w: &mut ErpWorld, id: String, plant: String, list: String) {
    let req = UserInput {
        multi_plant: true,
        default_plant: plant,
        allowed_plants: Some(plants(&list)),
        ..standard(&id)
    };
    let result = w.users.create(&w.actor, req).await;
    w.last = Some(result.map(drop));
}

#[when(expr = "使用者將使用者 {string} 的可使用營運中心設為 {string}")]
async fn set_allowed_plants(w: &mut ErpWorld, id: String, list: String) {
    modify(w, &id, |r| r.allowed_plants = Some(plants(&list))).await;
}

#[when(expr = "使用者將使用者 {string} 設為可使用全部營運中心")]
async fn set_all_plants(w: &mut ErpWorld, id: String) {
    modify(w, &id, |r| r.all_plants = Some(true)).await;
}

// ----- 那麼 -----

#[then(expr = "使用者 {string} 應可使用全部營運中心")]
async fn all_plants(w: &mut ErpWorld, id: String) {
    assert!(user(w, &id).await.all_plants, "應可使用全部營運中心");
}

/// 由初始密碼（＝使用者代號）改密碼後登入，比對可切換的營運中心
#[then(expr = "以使用者 {string} 登入時可選擇的營運中心應為 {string}")]
async fn sign_in_plants(w: &mut ErpWorld, id: String, expected: String) {
    w.sign_in
        .change_password(&id, &id, "Plants123", "Plants123")
        .await
        .unwrap_or_else(|e| panic!("變更密碼失敗：{e}"));
    let session = w
        .sign_in
        .sign_in(&id, "Plants123")
        .await
        .unwrap_or_else(|e| panic!("登入失敗：{e}"));
    let ids: Vec<String> = session.plants.iter().map(|p| p.plant_id.clone()).collect();
    assert_eq!(ids, plants(&expected));
}
