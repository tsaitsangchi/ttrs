use cucumber::{gherkin::Step, given, then};
use ttrs_app::AppError;
use ttrs_shared::{Page, PlantInput, QbeCondition, QueryRequest};

use crate::{Backend, ErpWorld};

/// 讀取表格（略過標題列）
pub fn rows(step: &Step) -> Vec<Vec<String>> {
    let table = step.table.as_ref().expect("此步驟需要表格");
    table.rows.iter().skip(1).cloned().collect()
}

/// 「狀態」頁籤的欄位（ADR 0012 §9）：每個作業的查詢都可用
fn status_key(label: &str) -> Option<&'static str> {
    Some(match label {
        "資料有效" => "active",
        "資料所有者" => "created_by",
        "資料所有部門" => "created_dept",
        "建立時間" => "created_on",
        "最後修改者" => "updated_by",
        "最後修改時間" => "updated_on",
        "版本" => "version",
        _ => return None,
    })
}

/// QBE 查詢步驟的「欄位／條件」表格 → 查詢請求；`key` 把畫面欄位名稱轉成查詢欄位鍵值，
/// `paging` 為（每頁筆數, 頁次）
pub fn query_request(
    step: &Step,
    key: fn(&str) -> &'static str,
    paging: Option<(u32, u32)>,
) -> QueryRequest {
    QueryRequest {
        conditions: rows(step)
            .iter()
            .map(|r| QbeCondition {
                field: status_key(&r[0]).unwrap_or_else(|| key(&r[0])).to_owned(),
                expr: r[1].clone(),
            })
            .collect(),
        page_size: paging.map(|(size, _)| size),
        page: paging.map(|(_, page)| page),
        lang: None,
    }
}

/// 記錄 QBE 查詢的結果（鍵值依序）供「查詢結果應為」比對
pub fn set_query_result<T>(
    w: &mut ErpWorld,
    result: Result<Page<T>, AppError>,
    key: fn(&T) -> String,
) {
    match result {
        Ok(page) => {
            w.query_result = Some((page.items.iter().map(key).collect(), page.total));
            w.last = Some(Ok(()));
        }
        Err(e) => {
            w.query_result = None;
            w.last = Some(Err(e));
        }
    }
}

fn query_keys(w: &ErpWorld) -> &(Vec<String>, u64) {
    w.query_result.as_ref().expect("沒有查詢結果（查詢失敗？）")
}

#[then(expr = "查詢結果應為 {string}")]
fn result_is(w: &mut ErpWorld, expected: String) {
    assert_eq!(query_keys(w).0.join(","), expected);
}

#[then(expr = "查詢結果應為 {string} 共 {int} 筆")]
fn result_is_with_total(w: &mut ErpWorld, expected: String, total: u64) {
    let (keys, actual) = query_keys(w);
    assert_eq!(keys.join(","), expected);
    assert_eq!(*actual, total, "總筆數");
}

#[then(expr = "查詢結果應有 {int} 筆")]
fn result_count(w: &mut ErpWorld, total: u64) {
    assert_eq!(query_keys(w).1, total);
}

/// 經營運中心維護建立，PostgreSQL 版一併建立工廠 schema（ADR 0007）；資料庫代碼依建立順序給號
#[given(expr = "營運中心 {string} 已啟用")]
async fn plant_active(w: &mut ErpWorld, plant: String) {
    let req = PlantInput {
        plant_id: plant.clone(),
        name: Some(plant),
        ..Default::default()
    };
    w.plants.create(&w.actor, req).await.unwrap();
}

#[given(expr = "單位 {string} 已存在")]
async fn unit_exists(w: &mut ErpWorld, unit: String) {
    match &w.backend {
        Backend::Memory(s) => s.add_unit(&unit),
        Backend::Pg(pool) => {
            sqlx::query("INSERT INTO unit (unit_code, name) VALUES ($1, $1)")
                .bind(&unit)
                .execute(pool)
                .await
                .unwrap();
        }
    }
}

#[then("操作應成功")]
fn should_succeed(w: &mut ErpWorld) {
    match w.last.as_ref().expect("尚未執行任何操作") {
        Ok(()) => {}
        Err(e) => panic!("預期成功，實際錯誤 {}：{e}", e.code()),
    }
}

#[then(expr = "應出現錯誤 {string}")]
fn should_fail_with(w: &mut ErpWorld, code: String) {
    match w.last.as_ref().expect("尚未執行任何操作") {
        Ok(()) => panic!("預期錯誤 {code}，實際成功"),
        Err(e) => assert_eq!(e.code(), code, "錯誤訊息：{e}"),
    }
}
