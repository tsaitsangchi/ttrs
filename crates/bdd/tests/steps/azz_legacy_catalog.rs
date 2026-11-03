//! TipTop 對照目錄 — specs/features/azz/legacy_catalog.feature
//!
//! 「TipTop 資料目錄有資料表…」與「命名檔將…」在 World 組出待匯入的內容
//! （實際環境由 make legacy-catalog 讀 .sch 目錄與命名檔組出同樣的內容）。

use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{LegacyColumnDto, LegacyTableDto};

use crate::ErpWorld;

fn rows(step: &Step) -> Vec<Vec<String>> {
    step.table
        .as_ref()
        .expect("需要資料表格")
        .rows
        .iter()
        .skip(1)
        .cloned()
        .collect()
}

fn source<'a>(w: &'a mut ErpWorld, legacy: &str) -> &'a mut LegacyTableDto {
    w.legacy_source
        .iter_mut()
        .find(|t| t.legacy_name == legacy)
        .unwrap_or_else(|| panic!("資料目錄沒有 {legacy}"))
}

fn yes(v: &str) -> bool {
    v == "是"
}

#[given(expr = "TipTop 資料目錄有資料表 {string}，說明 {string}，欄位:")]
fn catalog_table(w: &mut ErpWorld, legacy: String, title: String, step: &Step) {
    let columns = rows(step)
        .into_iter()
        .map(|r| LegacyColumnDto {
            legacy_name: r[0].clone(),
            data_type: r[1].clone(),
            primary_key: yes(&r[2]),
            unused: r[3].to_lowercase().starts_with("no use"),
            description: r[3].clone(),
            notes: Some(r[4].clone()).filter(|s| !s.is_empty()),
            ..Default::default()
        })
        .collect();
    w.legacy_source.retain(|t| t.legacy_name != legacy);
    w.legacy_source.push(LegacyTableDto {
        legacy_name: legacy,
        title,
        module: "azz".into(),
        columns,
        ..Default::default()
    });
}

#[given(expr = "命名檔將 {string} 命名為 {string}，欄位:")]
fn naming_with_columns(w: &mut ErpWorld, legacy: String, name: String, step: &Step) {
    let names = rows(step);
    let t = source(w, &legacy);
    t.name = name;
    for r in names {
        let c = t
            .columns
            .iter_mut()
            .find(|c| c.legacy_name == r[0])
            .unwrap_or_else(|| panic!("{legacy} 沒有欄位 {}", r[0]));
        c.name = r[1].clone();
    }
}

#[given(expr = "命名檔將 {string} 命名為 {string}")]
fn naming_table(w: &mut ErpWorld, legacy: String, name: String) {
    source(w, &legacy).name = name;
}

#[given(expr = "命名檔將 {string} 的欄位 {string} 改名為 {string}")]
fn rename_column(w: &mut ErpWorld, legacy: String, column: String, name: String) {
    let c = source(w, &legacy)
        .columns
        .iter_mut()
        .find(|c| c.legacy_name == column)
        .unwrap_or_else(|| panic!("沒有欄位 {column}"));
    c.name = name;
}

async fn import(w: &mut ErpWorld) {
    let result = w.legacy.import(&w.actor, w.legacy_source.clone()).await;
    w.last = Some(result.map(|_| ()));
}

#[given("已匯入 TipTop 對照目錄")]
async fn imported(w: &mut ErpWorld) {
    import(w).await;
    if let Some(Err(e)) = &w.last {
        panic!("匯入失敗：{e}");
    }
}

#[when("系統管理員匯入 TipTop 對照目錄")]
async fn do_import(w: &mut ErpWorld) {
    import(w).await;
}

#[then(expr = "TipTop 資料表 {string} 的英文名稱應為 {string}，說明 {string}")]
async fn table_named(w: &mut ErpWorld, legacy: String, name: String, title: String) {
    let t = w.legacy.get(&w.actor, &legacy).await.unwrap();
    assert_eq!(t.name, name);
    assert_eq!(t.title, title);
}

#[then(expr = "TipTop 資料表 {string} 的欄位對照應為:")]
async fn columns_mapped(w: &mut ErpWorld, legacy: String, step: &Step) {
    let t = w.legacy.get(&w.actor, &legacy).await.unwrap();
    let expected = rows(step);
    assert_eq!(t.columns.len(), expected.len(), "欄位數：{:?}", t.columns);
    for (c, r) in t.columns.iter().zip(expected) {
        assert_eq!(c.legacy_name, r[0]);
        assert_eq!(c.name, r[1], "{}", c.legacy_name);
        assert_eq!(c.data_type, r[2], "{}", c.legacy_name);
        assert_eq!(c.primary_key, yes(&r[3]), "{} 主鍵", c.legacy_name);
        assert_eq!(c.description, r[4], "{}", c.legacy_name);
        assert_eq!(c.unused, yes(&r[5]), "{} 未使用", c.legacy_name);
    }
}

#[then(expr = "TipTop 資料表 {string} 欄位 {string} 的英文名稱應為 {string}")]
async fn column_named(w: &mut ErpWorld, legacy: String, column: String, name: String) {
    let t = w.legacy.get(&w.actor, &legacy).await.unwrap();
    let c = t
        .columns
        .iter()
        .find(|c| c.legacy_name == column)
        .expect("沒有此欄位");
    assert_eq!(c.name, name);
}

#[then("TipTop 對照目錄應為空")]
async fn catalog_empty(w: &mut ErpWorld) {
    let list = w.legacy.search(&w.actor, "").await.unwrap();
    assert!(list.is_empty(), "對照目錄：{list:?}");
}

#[when(expr = "使用者以 {string} 查詢 TipTop 對照目錄")]
async fn search(w: &mut ErpWorld, query: String) {
    w.legacy_results = Some(w.legacy.search(&w.actor, &query).await.unwrap());
}

#[then(expr = "查詢結果應包含 TipTop 資料表 {string}")]
fn results_contain(w: &mut ErpWorld, legacy: String) {
    let list = w.legacy_results.as_ref().expect("尚未查詢");
    assert!(
        list.iter().any(|t| t.legacy_name == legacy),
        "查詢結果：{list:?}"
    );
}

#[when(expr = "使用者查看資料表 {string} 的 TipTop 對照")]
async fn view_mapping(w: &mut ErpWorld, table: String) {
    w.legacy_view = w.legacy.for_table(&w.actor, &table).await.unwrap();
}

#[then(expr = "對應的 TipTop 舊表應為 {string}")]
fn mapped_table(w: &mut ErpWorld, legacy: String) {
    let t = w.legacy_view.as_ref().expect("沒有對應的 TipTop 舊表");
    assert_eq!(t.legacy_name, legacy);
}

#[then(expr = "欄位 {string} 對應的 TipTop 欄位應為 {string}")]
fn mapped_column(w: &mut ErpWorld, column: String, legacy: String) {
    let t = w.legacy_view.as_ref().expect("沒有對應的 TipTop 舊表");
    let c = t
        .columns
        .iter()
        .find(|c| c.name == column)
        .unwrap_or_else(|| panic!("沒有對應 {column} 的欄位"));
    assert_eq!(c.legacy_name, legacy);
}
