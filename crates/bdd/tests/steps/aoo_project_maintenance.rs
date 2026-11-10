//! 專案維護 — specs/features/aoo/project_maintenance.feature（PJ1–PJ6）；工廠層，預設營運中心 TW01
//! 查詢、複製、確認、刪除等標準功能的步驟在 std_actions.rs。

use chrono::NaiveDate;
use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{ProjectDto, ProjectInput};

use crate::{ErpWorld, steps::common::rows};

const PLANT: &str = "TW01";

fn day(s: &str) -> NaiveDate {
    s.parse().unwrap_or_else(|_| panic!("日期格式錯誤：{s}"))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

async fn create(w: &mut ErpWorld, plant: &str, req: ProjectInput) {
    let r = w.projects.create(&w.actor, plant, req).await;
    w.last = Some(r.map(drop));
}

fn input(code: &str, name: &str, start: Option<&str>, end: Option<&str>) -> ProjectInput {
    ProjectInput {
        project_code: code.to_owned(),
        name: Some(name.to_owned()),
        start_on: start.map(day),
        end_on: end.map(day),
        version: None,
    }
}

async fn project(w: &ErpWorld, code: &str) -> ProjectDto {
    w.projects
        .get(&w.actor, PLANT, code)
        .await
        .unwrap_or_else(|e| panic!("查詢專案 {code} 失敗：{e}"))
}

fn ok(w: &ErpWorld, what: &str) {
    if let Some(Err(e)) = &w.last {
        panic!("{what}失敗：{e}");
    }
}

#[given(expr = "已有專案 {string}")]
async fn has(w: &mut ErpWorld, code: String) {
    create(w, PLANT, input(&code, &code, None, None)).await;
    ok(w, &format!("建立專案 {code} "));
}

#[given(expr = "已有專案 {string}，起始日期 {string}，結束日期 {string}")]
async fn has_dated(w: &mut ErpWorld, code: String, start: String, end: String) {
    create(w, PLANT, input(&code, &code, Some(&start), Some(&end))).await;
    ok(w, &format!("建立專案 {code} "));
}

#[given(expr = "營運中心 {string} 已有專案 {string}")]
async fn plant_has(w: &mut ErpWorld, plant: String, code: String) {
    create(w, &plant, input(&code, &code, None, None)).await;
    ok(w, &format!("建立專案 {code} "));
}

#[when(expr = "使用者新增專案 {string}，名稱 {string}")]
async fn add(w: &mut ErpWorld, code: String, name: String) {
    create(w, PLANT, input(&code, &name, None, None)).await;
}

#[when(expr = "使用者新增專案 {string}，名稱 {string}，起始日期 {string}，結束日期 {string}")]
async fn add_dated(w: &mut ErpWorld, code: String, name: String, start: String, end: String) {
    create(w, PLANT, input(&code, &name, Some(&start), Some(&end))).await;
}

#[when(expr = "使用者將專案 {string} 的結束日期改為 {string}")]
async fn change_end(w: &mut ErpWorld, code: String, end: String) {
    let mut req = project(w, &code).await.to_input();
    req.end_on = Some(day(&end));
    let r = w.projects.update(&w.actor, PLANT, &code, req).await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者(停用|啟用)專案 "(\S+)"$"#)]
async fn toggle(w: &mut ErpWorld, op: String, code: String) {
    let r = w
        .projects
        .set_active(&w.actor, PLANT, &code, op == "啟用", None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者在營運中心 {string} 查詢專案 {string}")]
async fn get_in(w: &mut ErpWorld, plant: String, code: String) {
    let r = w.projects.get(&w.actor, &plant, &code).await;
    w.last = Some(r.map(drop));
}

#[then(expr = "專案 {string} 應為:")]
async fn project_is(w: &mut ErpWorld, code: String, step: &Step) {
    let x = project(w, &code).await;
    let d = |v: Option<NaiveDate>| v.map(|d| d.to_string()).unwrap_or_default();
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "名稱" => x.name.clone().unwrap_or_default(),
            "起始日期" => d(x.start_on),
            "結束日期" => d(x.end_on),
            "資料有效" => yes_no(x.active).to_owned(),
            "資料所有者" => x.created_by.clone(),
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(expr = "專案 {string} 的資料有效應為 {string}")]
async fn active_is(w: &mut ErpWorld, code: String, expected: String) {
    assert_eq!(yes_no(project(w, &code).await.active), expected);
}

#[then(expr = "專案 {string} 應不存在")]
async fn absent(w: &mut ErpWorld, code: String) {
    let r = w.projects.get(&w.actor, PLANT, &code).await;
    assert!(r.is_err(), "專案 {code} 應不存在");
}
