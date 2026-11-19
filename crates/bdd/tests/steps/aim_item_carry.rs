//! 料件拋轉 — specs/features/aim/item_carry.feature（IC1–IC13）

use cucumber::{gherkin::Step, given, then, when};
use rust_decimal::Decimal;
use ttrs_shared::{ItemCarryRequest, ItemDto, UnitConversionInput};

use crate::{ErpWorld, steps::common::rows};

fn split(list: &str) -> Vec<String> {
    list.split('、')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn outcome_label(o: &str) -> &'static str {
    match o {
        "inserted" => "新增",
        "updated" => "覆蓋",
        "skipped" => "略過",
        "not_matched" => "不符條件",
        _ => "失敗",
    }
}

fn status_label(code: &str) -> &'static str {
    match code {
        "Y" => "已確認",
        "N" => "無效",
        _ => "處理中",
    }
}

async fn item_in(w: &ErpWorld, plant: &str, no: &str) -> ItemDto {
    w.items
        .get(&w.actor, plant, no, None)
        .await
        .unwrap_or_else(|e| panic!("查詢 {plant} 料件 {no} 失敗：{e}"))
}

#[given(regex = r#"^使用者在營運中心 "([^"]*)" 拋轉料件 "([^"]*)" 到 "([^"]*)"$"#)]
#[when(regex = r#"^使用者在營運中心 "([^"]*)" 拋轉料件 "([^"]*)" 到 "([^"]*)"$"#)]
async fn carry(w: &mut ErpWorld, plant: String, no: String, targets: String) {
    let req = ItemCarryRequest {
        targets: Some(split(&targets)),
    };
    let r = w.item_carry.carry(&w.actor, &plant, &no, req).await;
    w.last = Some(r.map(|r| {
        w.item_carry_result = Some(r);
    }));
}

#[given(
    regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 已有單位換算 "([^"]*)" 換 "([^"]*)"，比率 (\d+)$"#
)]
async fn has_conversion(
    w: &mut ErpWorld,
    plant: String,
    no: String,
    from: String,
    to: String,
    ratio: i64,
) {
    let input = UnitConversionInput {
        from_unit: from,
        to_unit: to,
        from_quantity: Decimal::ONE,
        to_quantity: Decimal::from(ratio),
        note: None,
        version: None,
    };
    w.item_conversions
        .create(&w.actor, &plant, &no, input)
        .await
        .unwrap_or_else(|e| panic!("建立單位換算失敗：{e}"));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 把料件 "([^"]*)" 的品名改為 "([^"]*)"$"#)]
async fn rename_in(w: &mut ErpWorld, plant: String, no: String, name: String) {
    let mut req = item_in(w, &plant, &no).await.to_input();
    req.name = Some(name);
    let r = w.items.update(&w.actor, &plant, &no, req).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 取消確認料件 "([^"]*)"$"#)]
async fn unconfirm_in(w: &mut ErpWorld, plant: String, no: String) {
    let v = Some(item_in(w, &plant, &no).await.version);
    let r = w.items.unconfirm(&w.actor, &plant, &no, v).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 刪除料件 "([^"]*)"$"#)]
async fn delete_in(w: &mut ErpWorld, plant: String, no: String) {
    let v = Some(item_in(w, &plant, &no).await.version);
    let r = w.items.delete(&w.actor, &plant, &no, v).await;
    w.last = Some(r);
}

#[then("料件拋轉結果應為:")]
fn result_is(w: &mut ErpWorld, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let r = w.item_carry_result.clone().expect("沒有拋轉結果");
    let got: Vec<Vec<String>> = r
        .rows
        .iter()
        .map(|x| vec![x.plant.clone(), outcome_label(&x.outcome).to_owned()])
        .collect();
    assert_eq!(got, rows(step), "{:?}", r.rows);
}

#[then(regex = r#"^營運中心 "([^"]*)" 的料件 "([^"]*)" 應為:$"#)]
async fn item_is(w: &mut ErpWorld, plant: String, no: String, step: &Step) {
    let i = item_in(w, &plant, &no).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "資料來源" => i.data_source.clone(),
            "資料所有者" => i.created_by.clone(),
            "狀態" => status_label(&i.active_code).to_owned(),
            "分群碼" => i.fields.group_code.clone().unwrap_or_default(),
            "品名" => i.fields.name.clone().unwrap_or_default(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(regex = r#"^營運中心 "([^"]*)" 的料件 "([^"]*)" 應不存在$"#)]
async fn item_gone(w: &mut ErpWorld, plant: String, no: String) {
    let r = w.items.get(&w.actor, &plant, &no, None).await;
    assert_eq!(r.err().map(|e| e.code()), Some("ITEM_NOT_FOUND"));
}

#[then(regex = r#"^營運中心 "([^"]*)" 料件 "([^"]*)" 的單位換算應為 "([^"]*)"$"#)]
async fn conversions_are(w: &mut ErpWorld, plant: String, no: String, expected: String) {
    let list = w
        .item_conversions
        .list(&w.actor, &plant, &no)
        .await
        .expect("單位換算");
    let got: Vec<String> = list
        .iter()
        .map(|c| {
            format!(
                "{}→{} {}",
                c.from_unit,
                c.to_unit,
                (c.to_quantity / c.from_quantity).normalize()
            )
        })
        .collect();
    assert_eq!(got.join("、"), expected);
}
