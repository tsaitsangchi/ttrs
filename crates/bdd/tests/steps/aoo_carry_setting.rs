//! 資料中心拋轉設定 — specs/features/aoo/carry_setting.feature（CS1–CS12）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{
    CarryCopyInput, CarryDefaultInput, CarrySettingDto, CarrySettingInput, CarryTargetInput,
    DataControlRuleInput, QueryRequest,
};

use crate::{ErpWorld, steps::common::rows};

const PROGRAM: &str = "carry-setting-maintenance";

fn head(center: &str, t: &str, mode: &str, version: Option<i32>) -> CarrySettingInput {
    CarrySettingInput {
        center_code: center.to_owned(),
        data_type: t.to_owned(),
        mode: mode.to_owned(),
        version,
    }
}

async fn setting(w: &ErpWorld, center: &str, t: &str) -> CarrySettingDto {
    w.carry_settings
        .get(&w.actor, center, t)
        .await
        .unwrap_or_else(|e| panic!("查詢拋轉設定 {center} {t} 失敗：{e}"))
}

async fn version(w: &ErpWorld, center: &str, t: &str) -> Option<i32> {
    w.carry_settings
        .get(&w.actor, center, t)
        .await
        .ok()
        .map(|c| c.version)
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn flag(v: &str) -> bool {
    v == "是"
}

fn field_key(label: &str) -> &'static str {
    match label {
        "資料中心" => "center_code",
        "資料類別" => "data_type",
        "拋轉方式" => "mode",
        "拋轉營運中心" => "target_plant",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

fn ok<T>(r: Result<T, ttrs_app::AppError>, what: &str) -> T {
    r.unwrap_or_else(|e| panic!("{what} 失敗：{e}"))
}

fn rule(f: &str, c: &str, v: &str, version: Option<i32>) -> DataControlRuleInput {
    DataControlRuleInput {
        field: f.to_owned(),
        condition: c.to_owned(),
        values: v.to_owned(),
        version,
    }
}

// ----- 假設 -----

#[given(regex = r#"^已有拋轉設定：資料中心 "([^"]*)"，資料類別 "([^"]*)"，拋轉方式 "([^"]*)"$"#)]
async fn has(w: &mut ErpWorld, center: String, t: String, mode: String) {
    let r = w
        .carry_settings
        .create(&w.actor, head(&center, &t, &mode, None))
        .await;
    ok(r, "新增拋轉設定");
}

#[given(regex = r#"^拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 已有拋轉營運中心 "([^"]*)"$"#)]
async fn has_target(w: &mut ErpWorld, center: String, t: String, plant: String) {
    let v = version(w, &center, &t).await;
    let r = w
        .carry_settings
        .add_target(
            &w.actor,
            &center,
            &t,
            CarryTargetInput {
                plant,
                version: v,
                ..Default::default()
            },
        )
        .await;
    ok(r, "新增拋轉營運中心");
}

#[given(
    regex = r#"^拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 營運中心 "([^"]*)" 已有拋轉條件：欄位 "([^"]*)"，條件 "([^"]*)"，值 "([^"]*)"$"#
)]
async fn has_condition(
    w: &mut ErpWorld,
    center: String,
    t: String,
    plant: String,
    f: String,
    c: String,
    v: String,
) {
    let ver = version(w, &center, &t).await;
    let r = w
        .carry_settings
        .put_condition(&w.actor, &center, &t, &plant, rule(&f, &c, &v, ver), false)
        .await;
    ok(r, "新增拋轉條件");
}

#[given(regex = r#"^另一位使用者已修改拋轉設定 "([^"]*)" 資料類別 "([^"]*)"$"#)]
async fn changed_by_other(w: &mut ErpWorld, center: String, t: String) {
    let c = setting(w, &center, &t).await;
    w.carry_version = Some(c.version);
    let r = w
        .carry_settings
        .update_mode(
            &w.actor,
            &center,
            &t,
            head(&center, &t, "2", Some(c.version)),
        )
        .await;
    ok(r, "他人修改");
}

// ----- 當 -----

#[when(
    regex = r#"^使用者新增拋轉設定：資料中心 "([^"]*)"，資料類別 "([^"]*)"，拋轉方式 "([^"]*)"$"#
)]
async fn add(w: &mut ErpWorld, center: String, t: String, mode: String) {
    let r = w
        .carry_settings
        .create(&w.actor, head(&center, &t, &mode, None))
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者(以舊版本)?把拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 的拋轉方式改為 "([^"]*)"$"#
)]
async fn modify(w: &mut ErpWorld, stale: String, center: String, t: String, mode: String) {
    let v = if stale.is_empty() {
        version(w, &center, &t).await
    } else {
        w.carry_version
    };
    let r = w
        .carry_settings
        .update_mode(&w.actor, &center, &t, head(&center, &t, &mode, v))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 新增拋轉營運中心 "([^"]*)"$"#)]
async fn add_target(w: &mut ErpWorld, center: String, t: String, plant: String) {
    let v = version(w, &center, &t).await;
    let r = w
        .carry_settings
        .add_target(
            &w.actor,
            &center,
            &t,
            CarryTargetInput {
                plant,
                version: v,
                ..Default::default()
            },
        )
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者把拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 的拋轉營運中心 "([^"]*)" 改為子公司可刪除 "(是|否)"，更新已存在資料 "(是|否)"，通知人員 "([^"]*)"$"#
)]
async fn update_target(
    w: &mut ErpWorld,
    center: String,
    t: String,
    plant: String,
    del: String,
    upd: String,
    notify: String,
) {
    let v = version(w, &center, &t).await;
    let r = w
        .carry_settings
        .update_target(
            &w.actor,
            &center,
            &t,
            CarryTargetInput {
                plant,
                deletable: flag(&del),
                update_existing: flag(&upd),
                notify: Some(notify),
                version: v,
            },
        )
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者為拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 營運中心 "([^"]*)" 新增拋轉條件：欄位 "([^"]*)"，條件 "([^"]*)"，值 "([^"]*)"$"#
)]
async fn add_condition(
    w: &mut ErpWorld,
    center: String,
    t: String,
    plant: String,
    f: String,
    c: String,
    v: String,
) {
    let ver = version(w, &center, &t).await;
    let r = w
        .carry_settings
        .put_condition(&w.actor, &center, &t, &plant, rule(&f, &c, &v, ver), false)
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者刪除拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 營運中心 "([^"]*)" 的拋轉條件 "([^"]*)"$"#
)]
async fn remove_condition(w: &mut ErpWorld, center: String, t: String, plant: String, f: String) {
    let v = version(w, &center, &t).await;
    let r = w
        .carry_settings
        .remove_condition(&w.actor, &center, &t, &plant, &f, v)
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者為拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 營運中心 "([^"]*)" 新增預設值：欄位 "([^"]*)"，值 "([^"]*)"$"#
)]
async fn add_default(
    w: &mut ErpWorld,
    center: String,
    t: String,
    plant: String,
    f: String,
    value: String,
) {
    let v = version(w, &center, &t).await;
    let r = w
        .carry_settings
        .put_default(
            &w.actor,
            &center,
            &t,
            &plant,
            CarryDefaultInput {
                field: f,
                value,
                version: v,
            },
            false,
        )
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者刪除拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 的拋轉營運中心 "([^"]*)"$"#)]
async fn remove_target(w: &mut ErpWorld, center: String, t: String, plant: String) {
    let v = version(w, &center, &t).await;
    let r = w
        .carry_settings
        .remove_target(&w.actor, &center, &t, &plant, v)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者刪除拋轉設定 "([^"]*)" 資料類別 "([^"]*)"$"#)]
async fn delete(w: &mut ErpWorld, center: String, t: String) {
    let v = version(w, &center, &t).await;
    let r = w.carry_settings.delete(&w.actor, &center, &t, v).await;
    w.last = Some(r);
}

#[when(
    regex = r#"^使用者把拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 複製為資料中心 "([^"]*)"，資料類別 "([^"]*)"$"#
)]
async fn copy(w: &mut ErpWorld, center: String, t: String, to_center: String, to_type: String) {
    let r = w
        .carry_settings
        .copy(
            &w.actor,
            &center,
            &t,
            CarryCopyInput {
                center_code: to_center,
                data_type: to_type,
            },
        )
        .await;
    w.last = Some(r.map(drop));
}

#[when("使用者查詢拋轉設定")]
async fn query_all(w: &mut ErpWorld) {
    let r = w
        .carry_settings
        .query(&w.actor, QueryRequest::default())
        .await;
    crate::steps::common::set_query_result(w, r, |c| format!("{} {}", c.center_code, c.data_type));
}

#[when("使用者以下列條件查詢拋轉設定:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.carry_settings.query(&w.actor, req).await;
    crate::steps::common::set_query_result(w, r, |c| format!("{} {}", c.center_code, c.data_type));
}

#[when(regex = r"^使用者(列印拋轉設定|匯出拋轉設定為 Excel)$")]
async fn report(w: &mut ErpWorld, what: String) {
    let format = if what.starts_with("列印") {
        ReportFormat::Pdf
    } else {
        ReportFormat::Excel
    };
    let r = w
        .carry_settings
        .report_in(&w.actor, Vec::new(), format, &Lang::default_lang())
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

// ----- 那麼 -----

#[then(regex = r#"^拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 的拋轉方式應為 "([^"]*)"$"#)]
async fn mode_is(w: &mut ErpWorld, center: String, t: String, mode: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    assert_eq!(setting(w, &center, &t).await.mode, mode);
}

#[then(regex = r#"^拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 的拋轉營運中心應為:$"#)]
async fn targets_are(w: &mut ErpWorld, center: String, t: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let got: Vec<Vec<String>> = setting(w, &center, &t)
        .await
        .targets
        .into_iter()
        .map(|g| {
            vec![
                g.plant,
                yes_no(g.deletable).to_owned(),
                yes_no(g.update_existing).to_owned(),
                g.conditions
                    .iter()
                    .map(|c| format!("{} {} {}", c.field, c.condition, c.values))
                    .collect::<Vec<_>>()
                    .join("、"),
                g.notify.unwrap_or_default(),
            ]
        })
        .collect();
    assert_eq!(got, rows(step));
}

#[then(regex = r#"^拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 營運中心 "([^"]*)" 的拋轉條件應為:$"#)]
async fn conditions_are(w: &mut ErpWorld, center: String, t: String, plant: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let g = setting(w, &center, &t)
        .await
        .targets
        .into_iter()
        .find(|g| g.plant == plant)
        .unwrap_or_else(|| panic!("找不到拋轉營運中心 {plant}"));
    let got: Vec<Vec<String>> = g
        .conditions
        .into_iter()
        .map(|c| vec![c.field, c.condition, c.values])
        .collect();
    assert_eq!(got, rows(step));
}

#[then(regex = r#"^拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 營運中心 "([^"]*)" 的預設值應為:$"#)]
async fn defaults_are(w: &mut ErpWorld, center: String, t: String, plant: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let g = setting(w, &center, &t)
        .await
        .targets
        .into_iter()
        .find(|g| g.plant == plant)
        .unwrap_or_else(|| panic!("找不到拋轉營運中心 {plant}"));
    let got: Vec<Vec<String>> = g
        .defaults
        .into_iter()
        .map(|d| vec![d.field, d.value])
        .collect();
    assert_eq!(got, rows(step));
}

#[then(regex = r#"^拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 應不存在$"#)]
async fn gone(w: &mut ErpWorld, center: String, t: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let r = w.carry_settings.get(&w.actor, &center, &t).await;
    assert_eq!(r.err().map(|e| e.code()), Some("CARRY_SETTING_NOT_FOUND"));
}

#[then(regex = r#"^拋轉設定清單順序應為 "([^"]*)"$"#)]
async fn order_is(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^拋轉設定 "([^"]*)" 資料類別 "([^"]*)" 的異動記錄應有 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, center: String, _t: String, expected: String) {
    let h = w
        .shared_history
        .history(&w.actor, PROGRAM, &center, None)
        .await
        .expect("異動記錄");
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}
