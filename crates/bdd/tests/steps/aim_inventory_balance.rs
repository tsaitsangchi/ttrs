//! 雜項發料／收料／報廢規格的前置資料：庫存明細（img_file）、已確認料件、單別設定、庫存參數、單號位數
//! （misc_issue.feature、misc_receipt.feature、misc_scrap.feature 的「假設」）

use cucumber::{gherkin::Step, given, then, when};
use rust_decimal::Decimal;
use ttrs_app::ports::InventoryBalanceRepository;
use ttrs_domain::{
    inventory_balance::{BalanceKey, InventoryBalance},
    plant::PlantId,
};
use ttrs_shared::{
    DocTypeStorageDto, DocTypeStorageInput, DocumentTypeInput, GeneralParametersInput, ItemInput,
};

use crate::{Backend, ErpWorld, steps::common::rows};

fn repo(w: &ErpWorld) -> Box<dyn InventoryBalanceRepository> {
    match &w.backend {
        Backend::Memory(s) => Box::new(s.clone()),
        Backend::Pg(pool) => Box::new(ttrs_infra::PgStore::new(pool.clone())),
    }
}

fn key(item: &str, wh: &str, loc: &str, lot: &str) -> BalanceKey {
    BalanceKey::new(item, wh, loc, lot)
}

#[given(expr = "營運中心 {string} 已有已確認料件 {string}，品名 {string}")]
async fn has_confirmed_item(w: &mut ErpWorld, plant: String, no: String, name: String) {
    let req = ItemInput {
        item_no: no.clone(),
        name: Some(name),
        stock_unit: Some("PCS".into()),
        product_class: Some("P01".into()),
        ..ItemInput::default()
    };
    let d = w
        .items
        .create(&w.actor, &plant, req)
        .await
        .unwrap_or_else(|e| panic!("建立料件 {no} 失敗：{e}"));
    w.items
        .confirm(&w.actor, &plant, &no, Some(d.version))
        .await
        .unwrap_or_else(|e| panic!("確認料件 {no} 失敗：{e}"));
}

#[given(
    regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 在倉庫 "([^"]*)" 儲位 "([^"]*)" 批號 "([^"]*)" 的庫存量為 (\d+)$"#
)]
async fn balance(
    w: &mut ErpWorld,
    plant: String,
    item: String,
    wh: String,
    loc: String,
    lot: String,
    qty: i64,
) {
    let plant = PlantId::parse(&plant).unwrap();
    let k = key(&item, &wh, &loc, &lot);
    let r = repo(w);
    let b = match r.get_inventory_balance(&plant, &k).await.unwrap() {
        Some(mut b) => {
            b.qty = Decimal::from(qty);
            b
        }
        None => InventoryBalance::simple(k, "PCS", Decimal::from(qty)),
    };
    r.put_inventory_balance(&plant, &b)
        .await
        .expect("建立庫存明細");
}

#[given(
    regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 在倉庫 "([^"]*)" 儲位 "([^"]*)" 批號 "([^"]*)" 的有效日期為 "([^"]*)"$"#
)]
async fn expiry(
    w: &mut ErpWorld,
    plant: String,
    item: String,
    wh: String,
    loc: String,
    lot: String,
    d: String,
) {
    let plant = PlantId::parse(&plant).unwrap();
    let r = repo(w);
    let mut b = r
        .get_inventory_balance(&plant, &key(&item, &wh, &loc, &lot))
        .await
        .unwrap()
        .expect("須先有庫存明細");
    b.expiry = Some(chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").unwrap());
    r.put_inventory_balance(&plant, &b)
        .await
        .expect("設定有效日期");
}

#[given(
    regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 在倉庫 "([^"]*)" 儲位 "([^"]*)" 批號 "([^"]*)" 的庫存資料已刪除$"#
)]
async fn balance_deleted(
    w: &mut ErpWorld,
    plant: String,
    item: String,
    wh: String,
    loc: String,
    lot: String,
) {
    let plant = PlantId::parse(&plant).unwrap();
    repo(w)
        .delete_inventory_balance(&plant, &key(&item, &wh, &loc, &lot))
        .await
        .expect("刪除庫存明細");
}

/// 單別的單頭設定（沿用其餘欄位）
async fn change_doc_type(
    w: &mut ErpWorld,
    plant: &str,
    code: &str,
    f: impl FnOnce(&mut DocumentTypeInput),
) {
    let d = w
        .document_types
        .get(&w.actor, plant, code)
        .await
        .unwrap_or_else(|e| panic!("查詢單別 {code} 失敗：{e}"));
    let mut input = DocumentTypeInput {
        doc_type_code: d.doc_type_code.clone(),
        name: d.name.clone(),
        system: d.system.clone(),
        kind: d.kind.clone(),
        version: Some(d.version),
        ..DocumentTypeInput::default()
    };
    f(&mut input);
    w.document_types
        .update(&w.actor, plant, code, input)
        .await
        .unwrap_or_else(|e| panic!("修改單別 {code} 失敗：{e}"));
}

#[given(regex = r#"^營運中心 "([^"]*)" 單別 "([^"]*)" 的編號方式為 "([^"]*)"$"#)]
async fn numbering(w: &mut ErpWorld, plant: String, code: String, n: String) {
    change_doc_type(w, &plant, &code, |i| i.numbering = Some(n)).await;
}

#[given(regex = r#"^營運中心 "([^"]*)" 單別 "([^"]*)" 不自動編號$"#)]
async fn no_auto_number(w: &mut ErpWorld, plant: String, code: String) {
    change_doc_type(w, &plant, &code, |i| i.auto_number = Some(false)).await;
}

#[given(regex = r#"^營運中心 "([^"]*)" 單別 "([^"]*)" 設定立即確認$"#)]
async fn confirm_now(w: &mut ErpWorld, plant: String, code: String) {
    change_doc_type(w, &plant, &code, |i| i.confirm_now = Some(true)).await;
}

#[given(regex = r#"^營運中心 "([^"]*)" 單別 "([^"]*)" 的倉庫設限為 "([^"]*)"，倉儲:$"#)]
async fn restriction(w: &mut ErpWorld, plant: String, code: String, r: String, step: &Step) {
    let d = w
        .document_types
        .get(&w.actor, &plant, &code)
        .await
        .expect("單別");
    let input = DocTypeStorageInput {
        restriction: r,
        storages: rows(step)
            .into_iter()
            .map(|x| DocTypeStorageDto {
                warehouse: x[0].clone(),
                location: x[1].clone(),
            })
            .collect(),
        version: Some(d.version),
    };
    w.document_types
        .set_storages(&w.actor, &plant, &code, input)
        .await
        .unwrap_or_else(|e| panic!("設定倉庫設限失敗：{e}"));
}

#[given(regex = r#"^營運中心 "([^"]*)" 的庫存參數「(雜發|雜收)須輸入部門」為是$"#)]
async fn dept_required(w: &mut ErpWorld, plant: String, which: String) {
    let cur = w
        .inventory_parameters
        .get(&w.actor, &plant)
        .await
        .expect("庫存參數");
    let mut input = cur.to_input();
    if which == "雜發" {
        input.misc_issue_dept = true;
    } else {
        input.misc_receipt_dept = true;
    }
    w.inventory_parameters
        .update(&w.actor, &plant, input)
        .await
        .unwrap_or_else(|e| panic!("設定庫存參數失敗：{e}"));
}

async fn serial_length(w: &mut ErpWorld, len: i16) -> Result<(), ttrs_app::AppError> {
    let cur = w.general.get(&w.actor).await.expect("整體參數");
    let req = GeneralParametersInput {
        accounting_periods: cur.accounting_periods,
        base_currency: cur.base_currency.clone(),
        rate_basis: cur.rate_basis.clone(),
        auto_item_number: None,
        doc_code_length: None,
        doc_serial_length: Some(len),
        version: Some(cur.version),
    };
    w.general.update(&w.actor, req).await.map(drop)
}

#[given(regex = r#"^整體管理系統參數的單號位數為 (\d+)$"#)]
async fn given_serial_length(w: &mut ErpWorld, len: i16) {
    serial_length(w, len)
        .await
        .unwrap_or_else(|e| panic!("設定單號位數失敗：{e}"));
}

#[when(regex = r#"^使用者把整體管理系統參數的單號位數改為 (\d+)$"#)]
async fn change_serial_length(w: &mut ErpWorld, len: i16) {
    let r = serial_length(w, len).await;
    w.last = Some(r);
}

// ----- 過帳的前置資料與檢查（misc_*_posting.feature） -----

#[given(
    regex = r#"^營運中心 "([^"]*)" 的庫存參數現行年月為 (\d+) 年 (\d+) 期，成本關帳日 "([^"]*)"$"#
)]
async fn current_period(w: &mut ErpWorld, plant: String, year: i32, period: i16, closing: String) {
    let cur = w
        .inventory_parameters
        .get(&w.actor, &plant)
        .await
        .expect("庫存參數");
    let mut input = cur.to_input();
    input.current_year = Some(year);
    input.current_period = Some(period);
    input.closing_date = Some(chrono::NaiveDate::parse_from_str(&closing, "%Y-%m-%d").unwrap());
    w.inventory_parameters
        .update(&w.actor, &plant, input)
        .await
        .unwrap_or_else(|e| panic!("設定現行年月失敗：{e}"));
}

#[given(regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 的儲存有效天數為 (\d+)$"#)]
async fn shelf_life(w: &mut ErpWorld, plant: String, no: String, days: i32) {
    let d = w
        .items
        .get(&w.actor, &plant, &no, None)
        .await
        .expect("料件");
    let mut input = d.to_input();
    input.shelf_life_days = Some(days);
    w.items
        .update(&w.actor, &plant, &no, input)
        .await
        .unwrap_or_else(|e| panic!("設定儲存有效天數失敗：{e}"));
}

#[given(
    regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 在倉庫 "([^"]*)" 儲位 "([^"]*)" 的最高存量為 (\d+)$"#
)]
async fn max_qty(w: &mut ErpWorld, plant: String, item: String, wh: String, loc: String, max: i64) {
    let input = ttrs_shared::ItemStorageInput {
        warehouse_code: wh,
        location_code: loc,
        max_qty: Some(Decimal::from(max)),
        version: None,
    };
    w.item_storages
        .create(&w.actor, &plant, &item, input)
        .await
        .unwrap_or_else(|e| panic!("設定最高存量失敗：{e}"));
}

async fn balance_of(
    w: &ErpWorld,
    plant: &str,
    item: &str,
    wh: &str,
    loc: &str,
    lot: &str,
) -> Option<InventoryBalance> {
    repo(w)
        .get_inventory_balance(&PlantId::parse(plant).unwrap(), &key(item, wh, loc, lot))
        .await
        .unwrap()
}

#[then(
    regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 在倉庫 "([^"]*)" 儲位 "([^"]*)" 批號 "([^"]*)" 的庫存量應為 (\d+)$"#
)]
async fn balance_is(
    w: &mut ErpWorld,
    plant: String,
    item: String,
    wh: String,
    loc: String,
    lot: String,
    qty: i64,
) {
    if let Some(Err(e)) = &w.last
        && e.code() != "INVENTORY_INSUFFICIENT"
    {
        panic!("操作失敗 {}：{e}", e.code());
    }
    let b = balance_of(w, &plant, &item, &wh, &loc, &lot)
        .await
        .expect("庫存明細");
    assert_eq!(b.qty, Decimal::from(qty));
}

#[then(
    regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 在倉庫 "([^"]*)" 儲位 "([^"]*)" 批號 "([^"]*)" 的最近發料日應為 "([^"]*)"$"#
)]
async fn last_issued(
    w: &mut ErpWorld,
    plant: String,
    item: String,
    wh: String,
    loc: String,
    lot: String,
    d: String,
) {
    let b = balance_of(w, &plant, &item, &wh, &loc, &lot)
        .await
        .expect("庫存明細");
    assert_eq!(
        b.last_issued_on.map(|x| x.to_string()).unwrap_or_default(),
        d
    );
}

#[then(
    regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 在倉庫 "([^"]*)" 儲位 "([^"]*)" 批號 "([^"]*)" 的庫存資料應為:$"#
)]
async fn balance_fields(
    w: &mut ErpWorld,
    plant: String,
    item: String,
    wh: String,
    loc: String,
    lot: String,
    step: &Step,
) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗 {}：{e}", e.code());
    }
    let b = balance_of(w, &plant, &item, &wh, &loc, &lot)
        .await
        .expect("庫存明細");
    let date = |d: Option<chrono::NaiveDate>| d.map(|x| x.to_string()).unwrap_or_default();
    for r in rows(step) {
        let got = match r[0].as_str() {
            "庫存量" => b.qty.normalize().to_string(),
            "單位" => b.unit.clone(),
            "有效日期" => date(b.expiry),
            "最近收料日" => date(b.last_received_on),
            "最近發料日" => date(b.last_issued_on),
            "呆滯日期" => date(b.idle_on),
            "參考單號" => b.ref_no.clone().unwrap_or_default(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(got, r[1], "欄位 {}", r[0]);
    }
}

#[then(regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 的最近(出庫|入庫)日應為 "([^"]*)"$"#)]
async fn item_date(w: &mut ErpWorld, plant: String, no: String, which: String, d: String) {
    let item = w
        .items
        .get(&w.actor, &plant, &no, None)
        .await
        .expect("料件");
    let v = if which == "出庫" {
        item.last_issued_on
    } else {
        item.last_received_on
    };
    assert_eq!(v.map(|x| x.to_string()).unwrap_or_default(), d);
}

/// 依單號所屬的作業取異動記錄（三支作業各自的 service）
async fn records(
    w: &ErpWorld,
    plant: &str,
    no: &str,
) -> Vec<ttrs_domain::inventory_balance::InventoryTxnRecord> {
    let mut v = w
        .misc_issues
        .txn_records(&w.actor, plant, no)
        .await
        .expect("異動記錄");
    v.extend(
        w.misc_receipts
            .txn_records(&w.actor, plant, no)
            .await
            .expect("異動記錄"),
    );
    v.extend(
        w.misc_scraps
            .txn_records(&w.actor, plant, no)
            .await
            .expect("異動記錄"),
    );
    v.extend(
        w.warehouse_transfers
            .txn_records(&w.actor, plant, no)
            .await
            .expect("異動記錄"),
    );
    v
}

#[then(regex = r#"^營運中心 "([^"]*)" 單號 "([^"]*)" 的庫存異動記錄應為:$"#)]
async fn records_are(w: &mut ErpWorld, plant: String, no: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗 {}：{e}", e.code());
    }
    let got = records(w, &plant, &no).await;
    let table = step.table.as_ref().expect("表格");
    let header = &table.rows[0];
    let expected = rows(step);
    assert_eq!(got.len(), expected.len(), "異動記錄筆數");
    for (r, e) in got.iter().zip(expected) {
        for (i, h) in header.iter().enumerate() {
            let v = match h.as_str() {
                "項次" => r.doc_line.to_string(),
                "料件" => r.key.item.clone(),
                "倉庫" => r.key.warehouse.clone(),
                "儲位" => r.key.location.clone(),
                "批號" => r.key.lot.clone(),
                "入出庫" => r.direction.to_string(),
                "來源" => r.source.to_string(),
                "目的" => r.dest.to_string(),
                "數量" => r.qty.normalize().to_string(),
                "單位" => r.unit.clone(),
                "異動後庫存" => r.balance_after.normalize().to_string(),
                "單據日期" => r.txn_date.to_string(),
                "程式" => r.program.clone(),
                other => panic!("未知欄位：{other}"),
            };
            assert_eq!(v, e[i], "第 {} 項 {h}", r.doc_line);
        }
    }
}

#[then(regex = r#"^營運中心 "([^"]*)" 單號 "([^"]*)" 應沒有庫存異動記錄$"#)]
async fn no_records(w: &mut ErpWorld, plant: String, no: String) {
    assert!(records(w, &plant, &no).await.is_empty());
}

#[given(regex = r#"^營運中心 "([^"]*)" 的庫存參數「料件存放位置管制」為 "([^"]*)"$"#)]
async fn storage_control(w: &mut ErpWorld, plant: String, v: String) {
    let cur = w
        .inventory_parameters
        .get(&w.actor, &plant)
        .await
        .expect("庫存參數");
    let mut input = cur.to_input();
    input.storage_control = v;
    w.inventory_parameters
        .update(&w.actor, &plant, input)
        .await
        .unwrap_or_else(|e| panic!("設定存放位置管制失敗：{e}"));
}
