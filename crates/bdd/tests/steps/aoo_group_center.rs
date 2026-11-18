//! 集團中心維護 — specs/features/aoo/group_center.feature（GC1–GC11）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{DataCenterTypeInput, GroupCenterDto, GroupCenterInput, QueryRequest};

use crate::{ErpWorld, steps::common::rows};

const PROGRAM: &str = "group-center-maintenance";

fn head(code: &str, name: &str, category: &str, version: Option<i32>) -> GroupCenterInput {
    GroupCenterInput {
        center_code: code.to_owned(),
        name: Some(name.to_owned()),
        category: Some(category.to_owned()),
        version,
    }
}

fn data_type(t: &str, plant: &str) -> DataCenterTypeInput {
    DataCenterTypeInput {
        data_type: t.to_owned(),
        plant: plant.to_owned(),
        version: None,
    }
}

async fn center(w: &ErpWorld, code: &str) -> GroupCenterDto {
    w.group_centers
        .get(&w.actor, code)
        .await
        .unwrap_or_else(|e| panic!("查詢集團中心 {code} 失敗：{e}"))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn field_key(label: &str) -> &'static str {
    match label {
        "類別" => "category",
        "代碼" => "center_code",
        "名稱" => "name",
        other => panic!("未支援的查詢欄位：{other}"),
    }
}

fn ok<T>(r: Result<T, ttrs_app::AppError>, what: &str) -> T {
    r.unwrap_or_else(|e| panic!("{what} 失敗：{e}"))
}

#[given(regex = r#"^已有集團中心 "([^"]+)"，類別 "([^"]*)"$"#)]
async fn has(w: &mut ErpWorld, code: String, category: String) {
    let r = w
        .group_centers
        .create(&w.actor, head(&code, &code, &category, None))
        .await;
    ok(r, "建立集團中心");
}

#[given(regex = r#"^已有集團中心 "([^"]+)"，名稱 "([^"]*)"，類別 "([^"]*)"$"#)]
async fn has_named(w: &mut ErpWorld, code: String, name: String, category: String) {
    let r = w
        .group_centers
        .create(&w.actor, head(&code, &name, &category, None))
        .await;
    ok(r, "建立集團中心");
}

#[given(regex = r#"^集團中心 "([^"]+)" 已有資料類別 "([^"]*)"，營運中心 "([^"]*)"$"#)]
async fn has_type(w: &mut ErpWorld, code: String, t: String, plant: String) {
    let r = w
        .group_centers
        .add_data_type(&w.actor, &code, data_type(&t, &plant))
        .await;
    ok(r, "新增資料類別");
}

#[given(regex = r#"^使用者已(停用|確認)集團中心 "([^"]+)"$"#)]
async fn already(w: &mut ErpWorld, what: String, code: String) {
    let r = if what == "停用" {
        w.group_centers
            .set_active(&w.actor, &code, false, None)
            .await
    } else {
        w.group_centers.confirm(&w.actor, &code, true, None).await
    };
    ok(r, &what);
}

#[given(regex = r#"^另一位使用者已修改集團中心 "([^"]+)"$"#)]
async fn changed_by_other(w: &mut ErpWorld, code: String) {
    let c = center(w, &code).await;
    w.group_center_version = Some(c.version);
    let r = w
        .group_centers
        .update(
            &w.actor,
            &code,
            head(&code, "他人修改", &c.category, Some(c.version)),
        )
        .await;
    ok(r, "他人修改");
}

#[when(regex = r#"^使用者新增集團中心 "([^"]*)"，名稱 "([^"]*)"，類別 "([^"]*)"$"#)]
async fn add(w: &mut ErpWorld, code: String, name: String, category: String) {
    let r = w
        .group_centers
        .create(&w.actor, head(&code, &name, &category, None))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(以舊版本)?修改集團中心 "([^"]+)"，名稱 "([^"]*)"，類別 "([^"]*)"$"#)]
async fn modify(w: &mut ErpWorld, stale: String, code: String, name: String, category: String) {
    let version = if stale.is_empty() {
        w.group_centers
            .get(&w.actor, &code)
            .await
            .ok()
            .map(|c| c.version)
    } else {
        w.group_center_version
    };
    let r = w
        .group_centers
        .update(&w.actor, &code, head(&code, &name, &category, version))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(停用|啟用)集團中心 "([^"]+)"$"#)]
async fn set_active(w: &mut ErpWorld, what: String, code: String) {
    let r = w
        .group_centers
        .set_active(&w.actor, &code, what == "啟用", None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者為集團中心 "([^"]+)" 新增資料類別 "([^"]*)"，營運中心 "([^"]*)"$"#)]
async fn add_type(w: &mut ErpWorld, code: String, t: String, plant: String) {
    let r = w
        .group_centers
        .add_data_type(&w.actor, &code, data_type(&t, &plant))
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者刪除集團中心 "([^"]+)" 的資料類別 "([^"]*)"，營運中心 "([^"]*)"$"#)]
async fn remove_type(w: &mut ErpWorld, code: String, t: String, plant: String) {
    let r = w
        .group_centers
        .remove_data_type(&w.actor, &code, &t, &plant, None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者刪除集團中心 "([^"]+)"$"#)]
async fn delete(w: &mut ErpWorld, code: String) {
    let r = w.group_centers.delete(&w.actor, &code, None).await;
    w.last = Some(r);
}

#[when(regex = r#"^使用者(確認|取消確認)集團中心 "([^"]+)"$"#)]
async fn confirm(w: &mut ErpWorld, what: String, code: String) {
    let r = w
        .group_centers
        .confirm(&w.actor, &code, what == "確認", None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者把集團中心 "([^"]+)" 複製為 "([^"]+)"$"#)]
async fn copy(w: &mut ErpWorld, from: String, to: String) {
    let r = w.group_centers.copy(&w.actor, &from, &to).await;
    w.last = Some(r.map(drop));
}

#[when("使用者查詢集團中心")]
async fn query_all(w: &mut ErpWorld) {
    let r = w
        .group_centers
        .query(&w.actor, QueryRequest::default())
        .await;
    crate::steps::common::set_query_result(w, r, |c| c.center_code.clone());
}

#[when("使用者以下列條件查詢集團中心:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.group_centers.query(&w.actor, req).await;
    crate::steps::common::set_query_result(w, r, |c| c.center_code.clone());
}

#[when(regex = r"^使用者(列印集團中心|匯出集團中心為 Excel)$")]
async fn report(w: &mut ErpWorld, what: String) {
    let format = if what.starts_with("列印") {
        ReportFormat::Pdf
    } else {
        ReportFormat::Excel
    };
    let r = w
        .group_centers
        .report_in(&w.actor, Vec::new(), format, &Lang::default_lang())
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

#[then(regex = r#"^集團中心 "([^"]+)" 應為:$"#)]
async fn is(w: &mut ErpWorld, code: String, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let c = center(w, &code).await;
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "名稱" => c.name.clone().unwrap_or_default(),
            "類別" => c.category.clone(),
            "資料有效" => yes_no(c.active).to_owned(),
            "已確認" => yes_no(c.confirmed).to_owned(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(regex = r#"^集團中心 "([^"]+)" 的(資料有效|已確認)應為 "(是|否)"$"#)]
async fn flag_is(w: &mut ErpWorld, code: String, what: String, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let c = center(w, &code).await;
    let v = if what == "資料有效" {
        c.active
    } else {
        c.confirmed
    };
    assert_eq!(yes_no(v), expected);
}

#[then(regex = r#"^集團中心 "([^"]+)" 的資料類別應為 "([^"]*)"$"#)]
async fn types_are(w: &mut ErpWorld, code: String, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    let got: Vec<String> = center(w, &code)
        .await
        .data_types
        .into_iter()
        .map(|t| format!("{} {}", t.data_type, t.plant))
        .collect();
    assert_eq!(got.join("、"), expected);
}

#[then(regex = r#"^集團中心 "([^"]+)" 應沒有資料類別$"#)]
async fn no_types(w: &mut ErpWorld, code: String) {
    if let Some(Err(e)) = &w.last {
        panic!("操作失敗：{e}");
    }
    assert!(center(w, &code).await.data_types.is_empty());
}

#[then(regex = r#"^集團中心 "([^"]+)" 應不存在$"#)]
async fn gone(w: &mut ErpWorld, code: String) {
    let r = w.group_centers.get(&w.actor, &code).await;
    assert_eq!(r.err().map(|e| e.code()), Some("GROUP_CENTER_NOT_FOUND"));
}

#[then(regex = r#"^集團中心清單順序應為 "([^"]*)"$"#)]
async fn order_is(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^集團中心 "([^"]+)" 的異動記錄應有 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, code: String, expected: String) {
    let h = w
        .shared_history
        .history(&w.actor, PROGRAM, &code, None)
        .await
        .expect("異動記錄");
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}
