//! 料件基本資料維護 — specs/features/aim/item_maintenance.feature（IM-1～IM-24）

use cucumber::{gherkin::Step, given, then, when};
use rust_decimal::Decimal;
use ttrs_domain::{item::ItemNo, plant::PlantId};
use ttrs_shared::{CopyItemRequest, ItemDto, ItemInput, ItemTextDto};

use crate::{Backend, ErpWorld, steps::common::rows};

const PLANT: &str = "TW01";

/// 新增料件的基本欄位（背景已有分群 RM、單位 PCS、產品分類 P01）
fn base(no: &str) -> ItemInput {
    ItemInput {
        item_no: no.to_owned(),
        name: Some("測試".into()),
        group_code: Some("RM".into()),
        stock_unit: Some("PCS".into()),
        product_class: Some("P01".into()),
        ..ItemInput::default()
    }
}

pub(crate) fn opt(v: &str) -> Option<String> {
    Some(v.to_owned()).filter(|s| !s.is_empty())
}

pub(crate) fn dec(v: &str) -> Option<Decimal> {
    Some(v.parse().unwrap_or_else(|_| panic!("數字：{v}")))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn dec_text(v: Option<Decimal>) -> String {
    v.map(|d| d.normalize().to_string()).unwrap_or_default()
}

/// 畫面欄位名稱 → 設定輸入值
fn set_field(i: &mut ItemInput, label: &str, v: &str) {
    match label {
        "品名" => i.name = opt(v),
        "分群碼" => i.group_code = opt(v),
        "來源碼" => i.source_code = opt(v),
        "庫存單位" => i.stock_unit = opt(v),
        "產品分類" => i.product_class = opt(v),
        "ABC 碼" => i.abc_code = opt(v),
        "補貨策略碼" => i.replenish_policy = opt(v),
        "供應商管制" => i.supplier_control = opt(v),
        "產品週期狀態" => i.life_cycle = opt(v),
        "經濟訂購量" => i.economic_order_qty = dec(v),
        "平均訂購量" => i.average_order_qty = dec(v),
        "安全庫存量" => i.safety_stock = dec(v),
        "最高儲存數量" => i.max_stock_qty = dec(v),
        "儲存有效天數" => i.shelf_life_days = Some(v.parse().expect("天數")),
        "其他分群碼一" => i.other_group1 = opt(v),
        "其他分群碼二" => i.other_group2 = opt(v),
        "材料類別" => i.material_category = opt(v),
        "倉管員" => i.stock_keeper = opt(v),
        "申請部門" => i.apply_dept = opt(v),
        "規格組件料件編號" => i.configurable_item_no = opt(v),
        "可否做聯產品入庫" => i.coproduct = v == "是",
        "目前使用版本" => i.current_version = opt(v),
        other => panic!("未知欄位：{other}"),
    }
}

/// 輸入欄位的值（料件與分群預設值共用）
fn input_field(i: &ItemInput, label: &str) -> Option<String> {
    Some(match label {
        "品名" => i.name.clone().unwrap_or_default(),
        "分群碼" => i.group_code.clone().unwrap_or_default(),
        "來源碼" => i.source_code.clone().unwrap_or_default(),
        "庫存單位" => i.stock_unit.clone().unwrap_or_default(),
        "產品分類" => i.product_class.clone().unwrap_or_default(),
        "ABC 碼" => i.abc_code.clone().unwrap_or_default(),
        "補貨策略碼" => i.replenish_policy.clone().unwrap_or_default(),
        "經濟訂購量" => dec_text(i.economic_order_qty),
        "平均訂購量" => dec_text(i.average_order_qty),
        "安全庫存量" => dec_text(i.safety_stock),
        "規格組件料件編號" => i.configurable_item_no.clone().unwrap_or_default(),
        "可否做聯產品入庫" => yes_no(i.coproduct).to_owned(),
        "目前使用版本" => i.current_version.clone().unwrap_or_default(),
        _ => return None,
    })
}

fn active_label(code: &str) -> &'static str {
    match code {
        "P" => "處理中",
        "Y" => "已確認",
        "N" => "無效",
        other => panic!("資料有效碼：{other}"),
    }
}

fn item_field(d: &ItemDto, label: &str) -> String {
    if let Some(v) = input_field(&d.fields, label) {
        return v;
    }
    match label {
        "資料有效碼" => active_label(&d.active_code).to_owned(),
        "狀態碼" => match d.status_code.as_str() {
            "0" => "開立".to_owned(),
            "1" => "確認".to_owned(),
            other => panic!("狀態碼：{other}"),
        },
        "低階碼" => d.low_level_code.to_string(),
        "新料件處理狀況" => d.new_part_status.clone(),
        "資料來源" => d.data_source.clone(),
        "訂單無效否" => yes_no(d.order_invalid).to_owned(),
        "單位使用方式" => d.unit_usage.clone(),
        "Phase Out 否" => yes_no(d.phase_out).to_owned(),
        "資料所有者" => d.created_by.clone(),
        other => panic!("未知欄位：{other}"),
    }
}

async fn create(w: &mut ErpWorld, plant: &str, req: ItemInput) {
    let r = w.items.create(&w.actor, plant, req).await;
    w.last = Some(r.map(drop));
}

pub(crate) async fn must_create(w: &mut ErpWorld, plant: &str, req: ItemInput) {
    let no = req.item_no.clone();
    if let Err(e) = w.items.create(&w.actor, plant, req).await {
        panic!("建立料件 {no} 失敗：{e}");
    }
}

async fn item(w: &ErpWorld, no: &str) -> ItemDto {
    w.items
        .get(&w.actor, PLANT, no, None)
        .await
        .unwrap_or_else(|e| panic!("查詢料件 {no} 失敗：{e}"))
}

async fn version(w: &ErpWorld, no: &str) -> Option<i32> {
    Some(item(w, no).await.version)
}

/// 建立料件並轉到指定狀態
pub(crate) async fn existing(w: &mut ErpWorld, plant: &str, no: &str, state: &str, req: ItemInput) {
    must_create(w, plant, req).await;
    let v = Some(1);
    let r = match state {
        "處理中" => return,
        "已確認" => w.items.confirm(&w.actor, plant, no, v).await,
        "無效" => w.items.toggle_void(&w.actor, plant, no, v).await,
        other => panic!("未知狀態：{other}"),
    };
    r.unwrap_or_else(|e| panic!("料件 {no} 轉為{state}失敗：{e}"));
}

// ----- 假設 -----

#[given(regex = r#"^營運中心 "(\S+)" 已有(處理中|已確認|無效)?料件 "(\S+)"$"#)]
async fn has_item(w: &mut ErpWorld, plant: String, state: String, no: String) {
    let state = if state.is_empty() {
        "處理中"
    } else {
        &state
    };
    existing(w, &plant, &no, state, base(&no)).await;
}

/// 其他規格（營運中心維護等）用：只要有一筆料件；產品分類必填，沒有時先建立
#[given(expr = "{string} 已有料件 {string}")]
async fn has_plain_item(w: &mut ErpWorld, plant: String, no: String) {
    let class = ttrs_shared::ProductClassInput {
        class_code: "P01".into(),
        name: Some("產品".into()),
        ..Default::default()
    };
    let _ = w.product_classes.create(&w.actor, &plant, class).await;
    let req = ItemInput {
        group_code: None,
        ..base(&no)
    };
    must_create(w, &plant, req).await;
}

#[given(
    expr = "營運中心 {string} 已有已確認料件 {string}，目前使用版本 {string}，安全庫存量 {int}"
)]
async fn has_item_with(w: &mut ErpWorld, plant: String, no: String, ver: String, qty: i64) {
    let req = ItemInput {
        current_version: Some(ver),
        safety_stock: Some(Decimal::from(qty)),
        ..base(&no)
    };
    existing(w, &plant, &no, "已確認", req).await;
}

#[given(expr = "營運中心 {string} 已有料件 {string}，品名 {string}，規格 {string}")]
async fn has_item_named(w: &mut ErpWorld, plant: String, no: String, name: String, spec: String) {
    let req = ItemInput {
        name: Some(name),
        spec: Some(spec),
        ..base(&no)
    };
    existing(w, &plant, &no, "處理中", req).await;
}

#[given(expr = "營運中心 {string} 料件 {string} 已有一筆庫存異動")]
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

#[given(expr = "料件分群 {string} 的預設值為:")]
async fn group_defaults_are(w: &mut ErpWorld, code: String, step: &Step) {
    let g = w
        .item_groups
        .get(&w.actor, PLANT, &code)
        .await
        .expect("料件分群");
    let mut req = g.to_input();
    for r in rows(step) {
        let v = r[1].as_str();
        match r[0].as_str() {
            "來源碼" => req.source_code = opt(v),
            "庫存單位" => req.stock_unit = opt(v),
            "ABC 碼" => req.abc_code = opt(v),
            "補貨策略碼" => req.replenish_policy = opt(v),
            "安全庫存量" => req.safety_stock = dec(v),
            "產品分類" => req.product_class = opt(v),
            "經濟訂購量" => req.economic_order_qty = dec(v),
            "平均訂購量" => req.average_order_qty = dec(v),
            other => panic!("未知欄位：{other}"),
        }
    }
    w.item_groups
        .update(&w.actor, PLANT, &code, req)
        .await
        .unwrap_or_else(|e| panic!("設定料件分群 {code} 失敗：{e}"));
}

#[given(expr = "營運中心 {string} 料件分群 {string} 已停用")]
async fn group_inactive(w: &mut ErpWorld, plant: String, code: String) {
    let g = w.item_groups.get(&w.actor, &plant, &code).await.unwrap();
    w.item_groups
        .set_active(&w.actor, &plant, &code, false, Some(g.version))
        .await
        .unwrap_or_else(|e| panic!("停用料件分群失敗：{e}"));
}

// ----- 當 -----

#[when(
    expr = "使用者在營運中心 {string} 新增料件 {string}，品名 {string}，分群碼 {string}，庫存單位 {string}，產品分類 {string}"
)]
async fn add(
    w: &mut ErpWorld,
    plant: String,
    no: String,
    name: String,
    group: String,
    unit: String,
    class: String,
) {
    let req = ItemInput {
        item_no: no,
        name: opt(&name),
        group_code: opt(&group),
        stock_unit: opt(&unit),
        product_class: opt(&class),
        ..ItemInput::default()
    };
    create(w, &plant, req).await;
}

#[when(regex = r#"^使用者在營運中心 "(\S+)" 新增料件 "(\S+)"，(.+?) 為 "(.*)"$"#)]
async fn add_with_field(w: &mut ErpWorld, plant: String, no: String, label: String, v: String) {
    let mut req = base(&no);
    set_field(&mut req, &label, &v);
    create(w, &plant, req).await;
}

#[when(expr = "使用者在營運中心 {string} 新增料件 {string}，來源碼 {string}，補貨策略碼 {string}")]
async fn add_replenish(w: &mut ErpWorld, plant: String, no: String, source: String, rp: String) {
    let req = ItemInput {
        source_code: opt(&source),
        replenish_policy: opt(&rp),
        ..base(&no)
    };
    create(w, &plant, req).await;
}

#[when(
    expr = "使用者在營運中心 {string} 新增料件 {string}，來源碼 {string}，規格組件料件編號 {string}，可否做聯產品入庫 {string}"
)]
async fn add_source_links(
    w: &mut ErpWorld,
    plant: String,
    no: String,
    source: String,
    conf: String,
    coproduct: String,
) {
    let req = ItemInput {
        source_code: opt(&source),
        configurable_item_no: opt(&conf),
        coproduct: coproduct == "是",
        ..base(&no)
    };
    create(w, &plant, req).await;
}

#[when(expr = "使用者在營運中心 {string} 取得料件分群 {string} 的料件預設值")]
async fn get_defaults(w: &mut ErpWorld, plant: String, code: String) {
    let d = w
        .items
        .group_defaults(&w.actor, &plant, &code)
        .await
        .unwrap_or_else(|e| panic!("取得分群預設值失敗：{e}"));
    w.item_defaults = Some(d);
}

#[when(expr = "使用者將料件 {string} 的品名改為 {string}")]
async fn rename(w: &mut ErpWorld, no: String, name: String) {
    let mut req = item(w, &no).await.to_input();
    req.name = Some(name);
    let r = w.items.update(&w.actor, PLANT, &no, req).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者確認料件 {string}")]
async fn confirm(w: &mut ErpWorld, no: String) {
    let v = version(w, &no).await;
    let r = w.items.confirm(&w.actor, PLANT, &no, v).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者取消確認料件 {string}")]
async fn unconfirm(w: &mut ErpWorld, no: String) {
    let v = version(w, &no).await;
    let r = w.items.unconfirm(&w.actor, PLANT, &no, v).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者作廢料件 {string}")]
async fn void(w: &mut ErpWorld, no: String) {
    let v = version(w, &no).await;
    let r = w.items.toggle_void(&w.actor, PLANT, &no, v).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者刪除料件 {string}")]
async fn delete(w: &mut ErpWorld, no: String) {
    let v = version(w, &no).await;
    let r = w.items.delete(&w.actor, PLANT, &no, v).await;
    w.last = Some(r);
}

#[when(expr = "使用者將料件 {string} 複製為 {string}，品名 {string}")]
async fn copy(w: &mut ErpWorld, from: String, to: String, name: String) {
    let req = CopyItemRequest {
        new_item_no: to,
        name: opt(&name),
    };
    let r = w.items.copy(&w.actor, PLANT, &from, req).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者切換料件 {string} 的訂單無效")]
async fn toggle_order_invalid(w: &mut ErpWorld, no: String) {
    let v = version(w, &no).await;
    let r = w.items.toggle_order_invalid(&w.actor, PLANT, &no, v).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者設定料件 {string} 的語系 {string} 品名 {string}，規格 {string}")]
async fn set_text(w: &mut ErpWorld, no: String, lang: String, name: String, spec: String) {
    let t = ItemTextDto {
        lang,
        name: opt(&name),
        spec: opt(&spec),
    };
    let r = w.items.set_text(&w.actor, PLANT, &no, t).await;
    w.last = Some(r.map(drop));
}

// ----- 那麼 -----

#[then(expr = "料件 {string} 應為:")]
async fn item_is(w: &mut ErpWorld, no: String, step: &Step) {
    let d = item(w, &no).await;
    for r in rows(step) {
        assert_eq!(item_field(&d, &r[0]), r[1], "{}", r[0]);
    }
}

#[then(expr = "料件預設值應為:")]
async fn defaults_are(w: &mut ErpWorld, step: &Step) {
    let d = w.item_defaults.clone().expect("尚未取得分群預設值");
    for r in rows(step) {
        let v = input_field(&d, &r[0]).unwrap_or_else(|| panic!("未知欄位：{}", r[0]));
        assert_eq!(v, r[1], "{}", r[0]);
    }
}

#[then(expr = "料件 {string} 的建檔日應為今天")]
async fn created_today(w: &mut ErpWorld, no: String) {
    let today = chrono::Local::now().date_naive();
    assert_eq!(item(w, &no).await.item_created_on, today);
}

#[then(expr = "料件 {string} 的最近盤點日應為今天")]
async fn counted_today(w: &mut ErpWorld, no: String) {
    let today = chrono::Local::now().date_naive();
    assert_eq!(item(w, &no).await.last_count_on, Some(today));
}

#[then(expr = "營運中心 {string} 應有料件 {string} 與 {string}")]
async fn has_both(w: &mut ErpWorld, plant: String, a: String, b: String) {
    let list = w.items.list(&w.actor, &plant).await.expect("料件清單");
    for no in [a, b] {
        assert!(list.iter().any(|i| i.fields.item_no == no), "缺少料件 {no}");
    }
}

/// 營運中心維護（複製營運中心不帶料件）用
#[then(expr = "{string} 應沒有料件")]
async fn no_items(w: &mut ErpWorld, plant: String) {
    let list = w.items.list(&w.actor, &plant).await.expect("料件清單");
    assert!(list.is_empty(), "{plant} 仍有料件");
}

#[then(expr = "營運中心 {string} 不應有料件 {string}")]
async fn has_not(w: &mut ErpWorld, plant: String, no: String) {
    let list = w.items.list(&w.actor, &plant).await.expect("料件清單");
    assert!(
        !list.iter().any(|i| i.fields.item_no == no),
        "料件 {no} 仍存在"
    );
}

#[then(expr = "料件 {string} 的來源碼應為 {string}")]
async fn source_is(w: &mut ErpWorld, no: String, v: String) {
    assert_eq!(item(w, &no).await.fields.source_code, Some(v));
}

#[then(expr = "料件 {string} 的品名應為 {string}")]
async fn name_is(w: &mut ErpWorld, no: String, v: String) {
    assert_eq!(item(w, &no).await.fields.name, Some(v));
}

#[then(expr = "料件 {string} 的資料有效碼應為 {string}")]
async fn active_code_is(w: &mut ErpWorld, no: String, v: String) {
    assert_eq!(active_label(&item(w, &no).await.active_code), v);
}

#[then(expr = "料件 {string} 的訂單無效否應為 {string}")]
async fn order_invalid_is(w: &mut ErpWorld, no: String, v: String) {
    assert_eq!(yes_no(item(w, &no).await.order_invalid), v);
}

#[then(expr = "料件 {string} 的最後修改者應為 {string}")]
async fn updated_by_is(w: &mut ErpWorld, no: String, v: String) {
    assert_eq!(item(w, &no).await.updated_by, Some(v));
}

#[then(expr = "以語系 {string} 查詢料件 {string} 的品名應為 {string}")]
async fn name_in(w: &mut ErpWorld, lang: String, no: String, v: String) {
    let d = w
        .items
        .get(&w.actor, PLANT, &no, Some(&lang))
        .await
        .expect("查詢料件");
    assert_eq!(d.fields.name, Some(v));
}
