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
        .create(req(&plant, &no, "既有料件", "PCS"))
        .await
        .unwrap();
}

#[given(expr = "{string} 已有已確認料件 {string}")]
async fn existing_confirmed_item(w: &mut ErpWorld, plant: String, no: String) {
    w.items
        .create(req(&plant, &no, "既有料件", "PCS"))
        .await
        .unwrap();
    w.items.confirm(&plant, &no).await.unwrap();
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
            sqlx::query(
                "INSERT INTO inventory_txn (plant_id, item_no, txn_date, doc_no, doc_line, qty, unit_code)
                 VALUES ($1, $2, CURRENT_DATE, 'TEST-0001', 1, 1, 'PCS')",
            )
            .bind(&plant)
            .bind(&no)
            .execute(pool)
            .await
            .unwrap();
        }
    }
}

#[when(expr = "使用者在 {string} 新增料件 {string} 品名 {string} 庫存單位 {string}")]
async fn create_item(w: &mut ErpWorld, plant: String, no: String, name: String, unit: String) {
    w.last = Some(
        w.items
            .create(req(&plant, &no, &name, &unit))
            .await
            .map(|_| ()),
    );
}

#[when(expr = "使用者確認 {string} 料件 {string}")]
async fn confirm_item(w: &mut ErpWorld, plant: String, no: String) {
    w.last = Some(w.items.confirm(&plant, &no).await.map(|_| ()));
}

#[when(expr = "使用者刪除 {string} 料件 {string}")]
async fn delete_item(w: &mut ErpWorld, plant: String, no: String) {
    w.last = Some(w.items.delete(&plant, &no).await);
}

#[then(expr = "{string} 料件 {string} 狀態應為 {string}")]
async fn item_status(w: &mut ErpWorld, plant: String, no: String, label: String) {
    let item = w.items.get(&plant, &no).await.unwrap();
    assert_eq!(item.status.label(), label);
}

#[then(expr = "{string} 料件 {string} 應存在")]
async fn item_exists(w: &mut ErpWorld, plant: String, no: String) {
    w.items.get(&plant, &no).await.unwrap();
}

#[then(expr = "{string} 料件 {string} 應不存在")]
async fn item_absent(w: &mut ErpWorld, plant: String, no: String) {
    match w.items.get(&plant, &no).await {
        Err(AppError::Domain(DomainError::ItemNotFound)) => {}
        other => panic!("預期料件不存在，實際：{other:?}"),
    }
}
