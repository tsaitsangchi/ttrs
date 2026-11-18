//! 資料控管原則維護 — specs/features/aoo/data_control.feature（DC1–DC11）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{
    CopyItemRequest, DataControlCopyInput, DataControlDto, DataControlPlantInput,
    DataControlRuleInput, ItemGroupInput, ProductClassInput, QueryRequest, UnitInput,
};

use crate::{ErpWorld, steps::common::rows};

const PROGRAM: &str = "data-control-maintenance";

fn rule(field: &str, condition: &str, values: &str, version: Option<i32>) -> DataControlRuleInput {
    DataControlRuleInput {
        field: field.to_owned(),
        condition: condition.to_owned(),
        values: values.to_owned(),
        version,
    }
}

async fn data_type(w: &ErpWorld, t: &str) -> DataControlDto {
    w.data_controls
        .get(&w.actor, t)
        .await
        .unwrap_or_else(|e| panic!("查詢資料類別 {t} 失敗：{e}"))
}

/// 受控營運中心目前的版本
async fn version(w: &ErpWorld, t: &str, plant: &str) -> Option<i32> {
    data_type(w, t)
        .await
        .plants
        .into_iter()
        .find(|p| p.plant == plant)
        .map(|p| p.version)
}

fn field_key(label: &str) -> &'static str {
    match label {
        "資料類別" => "data_type",
        "營運中心" => "plant",
        "欄位" => "field",
        "條件" => "condition",
        "值" => "values",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

async fn add_rule(w: &mut ErpWorld, t: &str, plant: &str, req: DataControlRuleInput) {
    let r = w.data_controls.add_rule(&w.actor, t, plant, req).await;
    w.last = Some(r.map(drop));
}

// ----- 假設 -----

#[given(regex = r#"^資料類別 "([^"]*)" 已有受控營運中心 "([^"]*)"$"#)]
async fn has_plant(w: &mut ErpWorld, t: String, plant: String) {
    let req = DataControlPlantInput {
        data_type: t.clone(),
        plant: plant.clone(),
    };
    if let Err(e) = w.data_controls.add_plant(&w.actor, req).await {
        panic!("新增受控營運中心 {plant} 失敗：{e}");
    }
}

#[given(
    regex = r#"^資料類別 "([^"]*)" 營運中心 "([^"]*)" 已有欄位條件：欄位 "([^"]*)"，條件 "([^"]*)"，值 "([^"]*)"$"#
)]
async fn has_rule(w: &mut ErpWorld, t: String, plant: String, f: String, c: String, v: String) {
    let ver = version(w, &t, &plant).await;
    if let Err(e) = w
        .data_controls
        .add_rule(&w.actor, &t, &plant, rule(&f, &c, &v, ver))
        .await
    {
        panic!("新增欄位條件 {f} 失敗：{e}");
    }
}

#[given(regex = r#"^另一位使用者已修改資料類別 "([^"]*)" 營運中心 "([^"]*)" 的欄位條件$"#)]
async fn changed_by_other(w: &mut ErpWorld, t: String, plant: String) {
    let ver = version(w, &t, &plant).await;
    w.data_control_version = ver;
    if let Err(e) = w
        .data_controls
        .add_rule(
            &w.actor,
            &t,
            &plant,
            rule("material_category", "6", "Z", ver),
        )
        .await
    {
        panic!("他人修改失敗：{e}");
    }
}

/// 料件需要的單位、產品分類 P01 與料件分群（已存在時略過）
#[given(regex = r#"^營運中心 "([^"]*)" 已有單位與料件分群 "([^"]*)"$"#)]
async fn has_item_basics(w: &mut ErpWorld, plant: String, group: String) {
    let _ = w
        .units
        .create(
            &w.actor,
            UnitInput {
                unit_code: "PCS".into(),
                name: Some("PCS".into()),
                decimals: None,
                version: None,
            },
        )
        .await;
    let _ = w
        .product_classes
        .create(
            &w.actor,
            &plant,
            ProductClassInput {
                class_code: "P01".into(),
                name: Some("產品".into()),
                ..ProductClassInput::default()
            },
        )
        .await;
    if let Err(e) = w
        .item_groups
        .create(
            &w.actor,
            &plant,
            ItemGroupInput {
                group_code: group.clone(),
                ..ItemGroupInput::default()
            },
        )
        .await
    {
        panic!("建立料件分群 {group} 失敗：{e}");
    }
}

// ----- 當 -----

#[when(regex = r#"^使用者在資料類別 "([^"]*)" 新增受控營運中心 "([^"]*)"$"#)]
async fn add_plant(w: &mut ErpWorld, t: String, plant: String) {
    let req = DataControlPlantInput {
        data_type: t,
        plant,
    };
    let r = w.data_controls.add_plant(&w.actor, req).await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者(以舊版本)?為資料類別 "([^"]*)" 營運中心 "([^"]*)" 新增欄位條件：欄位 "([^"]*)"，條件 "([^"]*)"，值 "([^"]*)"$"#
)]
async fn add(
    w: &mut ErpWorld,
    stale: String,
    t: String,
    plant: String,
    f: String,
    c: String,
    v: String,
) {
    let ver = if stale.is_empty() {
        version(w, &t, &plant).await
    } else {
        w.data_control_version
    };
    add_rule(w, &t, &plant, rule(&f, &c, &v, ver)).await;
}

#[when(
    regex = r#"^使用者把資料類別 "([^"]*)" 營運中心 "([^"]*)" 的欄位條件 "([^"]*)" 改為條件 "([^"]*)"，值 "([^"]*)"$"#
)]
async fn update(w: &mut ErpWorld, t: String, plant: String, f: String, c: String, v: String) {
    let ver = version(w, &t, &plant).await;
    let r = w
        .data_controls
        .update_rule(&w.actor, &t, &plant, rule(&f, &c, &v, ver))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者刪除資料類別 "([^"]*)" 營運中心 "([^"]*)" 的欄位條件 "([^"]*)"$"#)]
async fn remove_rule(w: &mut ErpWorld, t: String, plant: String, f: String) {
    let ver = version(w, &t, &plant).await;
    let r = w
        .data_controls
        .remove_rule(&w.actor, &t, &plant, &f, ver)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者刪除資料類別 "([^"]*)" 的受控營運中心 "([^"]*)"$"#)]
async fn remove_plant(w: &mut ErpWorld, t: String, plant: String) {
    let r = w
        .data_controls
        .remove_plant(&w.actor, &t, &plant, None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者把資料類別 "([^"]*)" 營運中心 "([^"]*)" 的欄位條件複製到營運中心 "([^"]*)"$"#
)]
async fn copy(w: &mut ErpWorld, t: String, plant: String, to: String) {
    let r = w
        .data_controls
        .copy_rules(&w.actor, &t, &plant, DataControlCopyInput { to_plant: to })
        .await;
    w.last = Some(r.map(drop));
}

#[when("使用者查詢資料控管原則")]
async fn query_all(w: &mut ErpWorld) {
    let r = w
        .data_controls
        .query(&w.actor, QueryRequest::default())
        .await;
    crate::steps::common::set_query_result(w, r, |d| d.data_type.clone());
}

#[when("使用者以下列條件查詢資料控管原則:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.data_controls.query(&w.actor, req).await;
    crate::steps::common::set_query_result(w, r, |d| d.data_type.clone());
}

#[when(regex = r"^使用者(列印資料控管原則|匯出資料控管原則為 Excel)$")]
async fn report(w: &mut ErpWorld, what: String) {
    let format = if what.starts_with("列印") {
        ReportFormat::Pdf
    } else {
        ReportFormat::Excel
    };
    let r = w
        .data_controls
        .report_in(&w.actor, Vec::new(), format, &Lang::default_lang())
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 將料件 "([^"]*)" 複製為 "([^"]*)"$"#)]
async fn copy_item(w: &mut ErpWorld, plant: String, from: String, to: String) {
    let req = CopyItemRequest {
        new_item_no: to,
        name: None,
    };
    let r = w.items.copy(&w.actor, &plant, &from, req).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 把料件 "([^"]*)" 的分群碼改為 "([^"]*)"$"#)]
async fn change_group(w: &mut ErpWorld, plant: String, no: String, group: String) {
    let item = w
        .items
        .get(&w.actor, &plant, &no, None)
        .await
        .unwrap_or_else(|e| panic!("查詢料件 {no} 失敗：{e}"));
    let mut req = item.to_input();
    req.group_code = Some(group);
    let r = w.items.update(&w.actor, &plant, &no, req).await;
    w.last = Some(r.map(drop));
}

// ----- 那麼 -----

#[then(regex = r#"^資料類別 "([^"]*)" 的營運中心應為:$"#)]
async fn plants_are(w: &mut ErpWorld, t: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let got: Vec<Vec<String>> = data_type(w, &t)
        .await
        .plants
        .into_iter()
        .map(|p| {
            vec![
                p.plant,
                yes_no(p.is_data_center).to_owned(),
                p.center_code.unwrap_or_default(),
            ]
        })
        .collect();
    assert_eq!(got, rows(step));
}

#[then(regex = r#"^資料類別 "([^"]*)" 營運中心 "([^"]*)" 的欄位條件應為:$"#)]
async fn rules_are(w: &mut ErpWorld, t: String, plant: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let got: Vec<Vec<String>> = data_type(w, &t)
        .await
        .plants
        .into_iter()
        .find(|p| p.plant == plant)
        .unwrap_or_else(|| panic!("找不到營運中心 {plant}"))
        .rules
        .into_iter()
        .map(|r| vec![r.field, r.condition, r.values])
        .collect();
    assert_eq!(got, rows(step));
}

#[then(regex = r#"^資料類別 "([^"]*)" 營運中心 "([^"]*)" 應沒有欄位條件$"#)]
async fn no_rules(w: &mut ErpWorld, t: String, plant: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let p = data_type(w, &t)
        .await
        .plants
        .into_iter()
        .find(|p| p.plant == plant)
        .unwrap_or_else(|| panic!("找不到營運中心 {plant}"));
    assert!(p.rules.is_empty(), "{:?}", p.rules);
}

#[then(regex = r#"^資料控管原則清單順序應為 "([^"]*)"$"#)]
async fn order_is(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^資料類別 "([^"]*)" 的資料控管異動記錄應有 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, t: String, expected: String) {
    let h = w
        .shared_history
        .history(&w.actor, PROGRAM, &t, None)
        .await
        .expect("異動記錄");
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}
