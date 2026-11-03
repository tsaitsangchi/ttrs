//! aimi100 料件基本資料 — specs/features/aim/item_master.feature

use cucumber::{given, then, when};
use ttrs_app::AppError;
use ttrs_domain::{DomainError, item::ItemNo, plant::PlantId};
use ttrs_shared::CreateItemRequest;

use crate::{Backend, ErpWorld};

fn req(plant: &str, no: &str, name: &str, unit: &str) -> CreateItemRequest {
    CreateItemRequest {
        plant_id: plant.into(),
        item_no: no.into(),
        name: name.into(),
        spec: None,
        stock_unit: unit.into(),
    }
}

#[given(expr = "{string} 已有料件 {string}")]
async fn existing_item(w: &mut ErpWorld, plant: String, no: String) {
    w.items
        .create(&w.actor, req(&plant, &no, "既有料件", "PCS"))
        .await
        .unwrap();
}

#[given(expr = "{string} 已有已確認料件 {string}")]
async fn existing_confirmed_item(w: &mut ErpWorld, plant: String, no: String) {
    w.items
        .create(&w.actor, req(&plant, &no, "既有料件", "PCS"))
        .await
        .unwrap();
    w.items.confirm(&w.actor, &plant, &no).await.unwrap();
}

#[given(expr = "{string} 料件 {string} 已有一筆庫存異動")]
async fn item_has_txn(w: &mut ErpWorld, plant: String, no: String) {
    match &w.backend {
        Backend::Memory(s) => {
            s.add_transaction(
                &PlantId::parse(&plant).unwrap(),
                &ItemNo::parse(&no).unwrap(),
            );
        }
        Backend::Pg(pool) => {
            // 庫存異動在營運中心的工廠 schema（ADR 0007）
            let mut tx = pool.begin().await.unwrap();
            sqlx::query(
                "SELECT set_config('search_path', schema_name, true) FROM plant WHERE plant_id = $1",
            )
            .bind(&plant)
            .execute(&mut *tx)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO inventory_txn (item_no, txn_date, doc_no, doc_line, qty, unit_code)
                 VALUES ($1, CURRENT_DATE, 'TEST-0001', 1, 1, 'PCS')",
            )
            .bind(&no)
            .execute(&mut *tx)
            .await
            .unwrap();
            tx.commit().await.unwrap();
        }
    }
}

#[when(expr = "使用者在 {string} 新增料件 {string} 品名 {string} 庫存單位 {string}")]
async fn create_item(w: &mut ErpWorld, plant: String, no: String, name: String, unit: String) {
    w.last = Some(
        w.items
            .create(&w.actor, req(&plant, &no, &name, &unit))
            .await
            .map(|_| ()),
    );
}

#[when(expr = "使用者確認 {string} 料件 {string}")]
async fn confirm_item(w: &mut ErpWorld, plant: String, no: String) {
    w.last = Some(w.items.confirm(&w.actor, &plant, &no).await.map(|_| ()));
}

#[when(expr = "使用者取消確認 {string} 料件 {string}")]
async fn unconfirm_item(w: &mut ErpWorld, plant: String, no: String) {
    w.last = Some(w.items.unconfirm(&w.actor, &plant, &no).await.map(|_| ()));
}

#[when(expr = "使用者刪除 {string} 料件 {string}")]
async fn delete_item(w: &mut ErpWorld, plant: String, no: String) {
    w.last = Some(w.items.delete(&w.actor, &plant, &no).await);
}

#[then(expr = "{string} 料件 {string} 狀態應為 {string}")]
async fn item_status(w: &mut ErpWorld, plant: String, no: String, label: String) {
    let item = w.items.get(&w.actor, &plant, &no).await.unwrap();
    assert_eq!(item.status.label(), label);
}

#[then(expr = "{string} 料件 {string} 應存在")]
async fn item_exists(w: &mut ErpWorld, plant: String, no: String) {
    w.items.get(&w.actor, &plant, &no).await.unwrap();
}

#[then(expr = "{string} 料件 {string} 應不存在")]
async fn item_absent(w: &mut ErpWorld, plant: String, no: String) {
    match w.items.get(&w.actor, &plant, &no).await {
        Err(AppError::Domain(DomainError::ItemNotFound)) => {}
        other => panic!("預期料件不存在，實際：{other:?}"),
    }
}

#[then(expr = "{string} 應沒有料件")]
async fn no_items(w: &mut ErpWorld, plant: String) {
    let items = w.items.list(&w.actor, &plant).await.unwrap();
    assert!(items.is_empty(), "{plant} 仍有料件：{items:?}");
}
