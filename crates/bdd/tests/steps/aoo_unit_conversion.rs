//! 單位換算 — specs/features/aoo/unit_conversion.feature（UC、IU、CV；TipTop aooi102 / smc_file、aooi103 / smd_file、s_umfchk）
//! 料件換算一律在營運中心 TW01（規格背景）。

use cucumber::{given, then, when};
use rust_decimal::Decimal;
use ttrs_shared::{ItemInput, UnitConversionInput};

use crate::ErpWorld;

const PLANT: &str = "TW01";

fn dec(s: &str) -> Decimal {
    s.parse().unwrap_or_else(|_| panic!("數字格式錯誤：{s}"))
}

fn input(from: &str, to: &str, a: &str, b: &str) -> UnitConversionInput {
    UnitConversionInput {
        from_unit: from.to_owned(),
        to_unit: to.to_owned(),
        from_quantity: dec(a),
        to_quantity: dec(b),
        note: None,
        version: None,
    }
}

fn ok(w: &ErpWorld, what: &str) {
    if let Some(Err(e)) = &w.last {
        panic!("{what}失敗：{e}");
    }
}

// ----- 一般換算（aooi102） -----

#[when(regex = r#"^使用者新增單位換算 "(\S+)" 換 "(\S+)"，數量 (\S+) 比 (\S+)$"#)]
async fn create(w: &mut ErpWorld, from: String, to: String, a: String, b: String) {
    let r = w
        .conversions
        .create(&w.actor, input(&from, &to, &a, &b))
        .await;
    w.last = Some(r.map(drop));
}

#[given(regex = r#"^已有單位換算 "(\S+)" 換 "(\S+)"，數量 (\S+) 比 (\S+)$"#)]
async fn has(w: &mut ErpWorld, from: String, to: String, a: String, b: String) {
    create(w, from, to, a, b).await;
    ok(w, "建立單位換算");
}

#[then(regex = r#"^單位換算 "(\S+)" 換 "(\S+)" 應為 (\S+) 比 (\S+)、(有效|無效)$"#)]
async fn is(w: &mut ErpWorld, from: String, to: String, a: String, b: String, state: String) {
    ok(w, "單位換算");
    let c = w
        .conversions
        .get(&w.actor, &from, &to)
        .await
        .unwrap_or_else(|e| panic!("查詢單位換算失敗：{e}"));
    assert_eq!((c.from_quantity, c.to_quantity), (dec(&a), dec(&b)));
    assert_eq!(c.active, state == "有效");
}

#[when(regex = r#"^使用者刪除單位換算 "(\S+)" 換 "(\S+)"$"#)]
async fn delete(w: &mut ErpWorld, from: String, to: String) {
    let r = w.conversions.delete(&w.actor, &from, &to, None).await;
    w.last = Some(r);
}

#[then(regex = r#"^單位換算 "(\S+)" 換 "(\S+)" 應不存在$"#)]
async fn absent(w: &mut ErpWorld, from: String, to: String) {
    assert!(w.conversions.get(&w.actor, &from, &to).await.is_err());
}

#[given(regex = r#"^單位換算 "(\S+)" 換 "(\S+)" 已停用$"#)]
async fn deactivated(w: &mut ErpWorld, from: String, to: String) {
    w.conversions
        .set_active(&w.actor, &from, &to, false, None)
        .await
        .unwrap_or_else(|e| panic!("停用單位換算失敗：{e}"));
}

// ----- 料件換算（aooi103） -----

#[given(regex = r#"^已有料件 "(\S+)"，庫存單位 "(\S+)"$"#)]
async fn has_item(w: &mut ErpWorld, no: String, unit: String) {
    let class = ttrs_shared::ProductClassInput {
        class_code: "P01".into(),
        name: Some("產品".into()),
        ..Default::default()
    };
    let _ = w.product_classes.create(&w.actor, PLANT, class).await;
    let req = ItemInput {
        item_no: no.clone(),
        name: Some("測試".into()),
        stock_unit: Some(unit),
        product_class: Some("P01".into()),
        ..ItemInput::default()
    };
    w.items
        .create(&w.actor, PLANT, req)
        .await
        .unwrap_or_else(|e| panic!("建立料件 {no} 失敗：{e}"));
}

#[given(regex = r#"^料件 "(\S+)" 已停用$"#)]
async fn item_void(w: &mut ErpWorld, no: String) {
    w.items
        .toggle_void(&w.actor, PLANT, &no, Some(1))
        .await
        .unwrap_or_else(|e| panic!("停用料件 {no} 失敗：{e}"));
}

#[when(regex = r#"^使用者為料件 "(\S+)" 新增單位換算 "(\S+)" 換 "(\S+)"，數量 (\S+) 比 (\S+)$"#)]
async fn create_item(w: &mut ErpWorld, no: String, from: String, to: String, a: String, b: String) {
    let r = w
        .item_conversions
        .create(&w.actor, PLANT, &no, input(&from, &to, &a, &b))
        .await;
    w.last = Some(r.map(drop));
}

#[given(regex = r#"^料件 "(\S+)" 已有單位換算 "(\S+)" 換 "(\S+)"，數量 (\S+) 比 (\S+)$"#)]
async fn has_item_conv(
    w: &mut ErpWorld,
    no: String,
    from: String,
    to: String,
    a: String,
    b: String,
) {
    create_item(w, no, from, to, a, b).await;
    ok(w, "建立料件單位換算");
}

// ----- 換算率（s_umfchk） -----

#[when(regex = r#"^取用料件 "(\S+)" 由 "(\S+)" 換 "(\S+)" 的換算率$"#)]
async fn factor(w: &mut ErpWorld, no: String, from: String, to: String) {
    let r = w
        .item_conversions
        .factor(&w.actor, PLANT, &no, &from, &to)
        .await;
    w.rate_value = r.as_ref().ok().copied();
    w.last = Some(r.map(drop));
}

#[then(regex = r"^換算率應為 (\S+)$")]
async fn factor_is(w: &mut ErpWorld, v: String) {
    ok(w, "取用換算率");
    assert_eq!(
        w.rate_value.expect("尚未取用換算率").normalize(),
        dec(&v).normalize()
    );
}
