use cucumber::{given, then};

use crate::{Backend, ErpWorld};

#[given(expr = "營運中心 {string} 已啟用")]
async fn plant_active(w: &mut ErpWorld, plant: String) {
    match &w.backend {
        Backend::Memory(s) => s.add_plant(&plant),
        Backend::Pg(pool) => {
            sqlx::query("INSERT INTO plant (plant_id, name) VALUES ($1, $1)")
                .bind(&plant)
                .execute(pool)
                .await
                .unwrap();
        }
    }
}

#[given(expr = "單位 {string} 已存在")]
async fn unit_exists(w: &mut ErpWorld, unit: String) {
    match &w.backend {
        Backend::Memory(s) => s.add_unit(&unit),
        Backend::Pg(pool) => {
            sqlx::query("INSERT INTO unit (unit_code, name) VALUES ($1, $1)")
                .bind(&unit)
                .execute(pool)
                .await
                .unwrap();
        }
    }
}

#[then("操作應成功")]
fn should_succeed(w: &mut ErpWorld) {
    match w.last.as_ref().expect("尚未執行任何操作") {
        Ok(()) => {}
        Err(e) => panic!("預期成功，實際錯誤 {}：{e}", e.code()),
    }
}

#[then(expr = "應出現錯誤 {string}")]
fn should_fail_with(w: &mut ErpWorld, code: String) {
    match w.last.as_ref().expect("尚未執行任何操作") {
        Ok(()) => panic!("預期錯誤 {code}，實際成功"),
        Err(e) => assert_eq!(e.code(), code, "錯誤訊息：{e}"),
    }
}
