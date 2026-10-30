//! Data Dictionary Maintenance 資料字典維護 — specs/features/azz/data_dictionary.feature

use chrono::Local;
use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::{
    ColumnTextDto, ConstraintKindDto, TableDictionaryInput, TableTextDto, TableTypeDto,
};

use crate::ErpWorld;

fn table_type(label: &str) -> TableTypeDto {
    TableTypeDto::ALL
        .into_iter()
        .find(|t| t.label() == label)
        .unwrap_or_else(|| panic!("未知的資料表類型：{label}"))
}

fn rows(step: &Step) -> Vec<(String, String)> {
    let table = step.table.as_ref().expect("此步驟需要表格");
    table
        .rows
        .iter()
        .skip(1)
        .map(|r| (r[0].clone(), r[1].clone()))
        .collect()
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn opt(v: &str) -> Option<String> {
    Some(v.to_owned()).filter(|s| !s.is_empty())
}

async fn create(
    w: &mut ErpWorld,
    table: &str,
    module: &str,
    kind: &str,
) -> Result<(), ttrs_app::AppError> {
    let input = TableDictionaryInput {
        table_name: table.into(),
        module_code: module.into(),
        table_type: Some(table_type(kind)),
        ..Default::default()
    };
    w.dictionary.create(&w.actor, input).await.map(|_| ())
}

// ----- 假設 -----

#[given(expr = "資料庫中已有資料表 {string}")]
async fn table_in_schema(w: &mut ErpWorld, table: String) {
    assert!(
        w.dictionary.table_exists(&table).await.unwrap(),
        "資料庫中沒有資料表 {table}"
    );
}

#[given(expr = "資料表 {string} 已有字典，模組 {string}")]
async fn existing_dictionary(w: &mut ErpWorld, table: String, module: String) {
    create(w, &table, &module, "主檔").await.unwrap();
}

#[given(expr = "資料表 {string} 在語系 {string} 已命名為 {string}")]
async fn existing_table_name(w: &mut ErpWorld, table: String, lang: String, name: String) {
    let text = TableTextDto {
        lang,
        name: Some(name),
        ..Default::default()
    };
    w.dictionary.set_table_text(&table, text).await.unwrap();
}

#[given(expr = "資料表 {string} 欄位 {string} 在語系 {string} 已說明為 {string}")]
async fn existing_column_name(
    w: &mut ErpWorld,
    table: String,
    column: String,
    lang: String,
    name: String,
) {
    let text = ColumnTextDto {
        column_name: column,
        lang,
        name: Some(name),
        ..Default::default()
    };
    w.dictionary.set_column_text(&table, text).await.unwrap();
}

// ----- 當 -----

#[when(expr = "使用者為資料表 {string} 建立字典，模組 {string}，類型 {string}")]
async fn create_dictionary(w: &mut ErpWorld, table: String, module: String, kind: String) {
    let result = create(w, &table, &module, &kind).await;
    w.last = Some(result);
}

#[when(
    expr = "使用者將資料表 {string} 的字典改為模組 {string}，類型 {string}，上游 {string}，下游 {string}"
)]
async fn update_dictionary(
    w: &mut ErpWorld,
    table: String,
    module: String,
    kind: String,
    upstream: String,
    downstream: String,
) {
    let input = TableDictionaryInput {
        table_name: table.clone(),
        module_code: module,
        table_type: Some(table_type(&kind)),
        upstream: opt(&upstream),
        downstream: opt(&downstream),
    };
    w.last = Some(w.dictionary.update(&table, input).await.map(|_| ()));
}

#[when(expr = "使用者以語系 {string} 將資料表 {string} 命名為 {string}，目的 {string}")]
async fn name_table(w: &mut ErpWorld, lang: String, table: String, name: String, purpose: String) {
    let text = TableTextDto {
        lang,
        name: opt(&name),
        purpose: opt(&purpose),
        remark: None,
    };
    w.last = Some(w.dictionary.set_table_text(&table, text).await);
}

#[when(
    expr = "使用者以語系 {string} 說明資料表 {string} 的欄位 {string}：名稱 {string}，說明 {string}，額外說明 {string}"
)]
async fn describe_column(
    w: &mut ErpWorld,
    lang: String,
    table: String,
    column: String,
    name: String,
    description: String,
    extra: String,
) {
    let text = ColumnTextDto {
        column_name: column,
        lang,
        name: opt(&name),
        description: opt(&description),
        extra: opt(&extra),
    };
    w.last = Some(w.dictionary.set_column_text(&table, text).await);
}

#[when(expr = "使用者以語系 {string} 將資料表 {string} 的 {string} 設為 {int} 個字元")]
async fn set_length(w: &mut ErpWorld, lang: String, table: String, item: String, len: usize) {
    let v = Some("x".repeat(len));
    let result = match item.as_str() {
        "表名稱" | "表目的" => {
            let mut text = TableTextDto {
                lang,
                ..Default::default()
            };
            if item == "表名稱" {
                text.name = v;
            } else {
                text.purpose = v;
            }
            w.dictionary.set_table_text(&table, text).await
        }
        "欄位名稱" | "欄位說明" | "額外說明" => {
            let mut text = ColumnTextDto {
                column_name: "program_no".into(),
                lang,
                ..Default::default()
            };
            match item.as_str() {
                "欄位名稱" => text.name = v,
                "欄位說明" => text.description = v,
                _ => text.extra = v,
            }
            w.dictionary.set_column_text(&table, text).await
        }
        other => panic!("未知項目：{other}"),
    };
    w.last = Some(result);
}

#[when(expr = "使用者查看資料表 {string} 的欄位")]
async fn view_columns(w: &mut ErpWorld, table: String) {
    w.dict_columns = Some(w.dictionary.columns(&table).await.unwrap());
}

#[when(expr = "使用者查看資料表 {string} 的限制")]
async fn view_constraints(w: &mut ErpWorld, table: String) {
    w.dict_constraints = Some(w.dictionary.constraints(&table).await.unwrap());
}

#[when(expr = "使用者刪除資料表 {string} 的字典")]
async fn delete_dictionary(w: &mut ErpWorld, table: String) {
    w.last = Some(w.dictionary.delete(&table).await);
}

#[when(expr = "使用者查看資料表 {string} 的變更紀錄")]
async fn view_history(w: &mut ErpWorld, table: String) {
    w.dict_history = Some(w.dictionary.history(&table).await.unwrap());
}

#[when(expr = "使用者以語系 {string} 匯出資料表 {string} 的資料字典")]
async fn export(w: &mut ErpWorld, lang: String, table: String) {
    w.dict_export = Some(w.dictionary.export_csv(&table, &lang).await.unwrap());
}

// ----- 那麼 -----

#[when(expr = "使用者以語系 {string} 查看資料表 {string} 的字典")]
async fn view_dictionary_in(w: &mut ErpWorld, lang: String, table: String) {
    w.displayed_dict = Some(w.dictionary.get_localized(&table, &lang).await.unwrap());
}

#[then(expr = "資料表 {string} 的字典應為:")]
async fn dictionary_fields(w: &mut ErpWorld, step: &Step, table: String) {
    let d = w.dictionary.get(&table).await.unwrap();
    for (field, expected) in rows(step) {
        let actual = match field.as_str() {
            "模組" => d.module_code.clone(),
            "類型" => d
                .table_type
                .map(|t| t.label().to_owned())
                .unwrap_or_default(),
            "建立者" => d.created_by.clone(),
            "上游" => d.upstream.clone().unwrap_or_default(),
            "下游" => d.downstream.clone().unwrap_or_default(),
            "建立日期" => {
                if d.created_on == Local::now().date_naive() {
                    "今天".to_owned()
                } else {
                    d.created_on.to_string()
                }
            }
            other => panic!("未知欄位：{other}"),
        };
        assert_eq!(actual, expected, "欄位「{field}」");
    }
}

#[then(expr = "資料表 {string} 在語系 {string} 的名稱應為 {string}")]
async fn table_name_in_lang(w: &mut ErpWorld, table: String, lang: String, expected: String) {
    let texts = w.dictionary.table_texts(&table).await.unwrap();
    let actual = texts
        .into_iter()
        .find(|t| t.lang == lang)
        .and_then(|t| t.name)
        .unwrap_or_default();
    assert_eq!(actual, expected);
}

#[then(expr = "欄位 {string} 應為:")]
fn column_fields(w: &mut ErpWorld, step: &Step, column: String) {
    let cols = w.dict_columns.as_ref().expect("尚未查看欄位");
    let c = cols
        .iter()
        .find(|c| c.name == column)
        .unwrap_or_else(|| panic!("沒有欄位 {column}"));
    for (field, expected) in rows(step) {
        let actual = match field.as_str() {
            "型別" => c.data_type.clone(),
            "不可空白" => yes_no(c.not_null).to_owned(),
            "主鍵" => yes_no(c.primary_key).to_owned(),
            other => panic!("未知項目：{other}"),
        };
        assert_eq!(actual, expected, "欄位 {column} 的「{field}」");
    }
}

#[then(expr = "資料表 {string} 欄位 {string} 在語系 {string} 的名稱應為 {string}")]
async fn column_name_in_lang(
    w: &mut ErpWorld,
    table: String,
    column: String,
    lang: String,
    expected: String,
) {
    let texts = w.dictionary.column_texts(&table).await.unwrap();
    let actual = texts
        .into_iter()
        .find(|t| t.column_name == column && t.lang == lang)
        .and_then(|t| t.name)
        .unwrap_or_default();
    assert_eq!(actual, expected);
}

#[then(expr = "應有主鍵限制，欄位為 {string}")]
fn has_primary_key(w: &mut ErpWorld, columns: String) {
    let cs = w.dict_constraints.as_ref().expect("尚未查看限制");
    assert!(
        cs.iter()
            .any(|c| c.kind == ConstraintKindDto::PrimaryKey && c.columns.join(",") == columns),
        "限制：{cs:?}"
    );
}

#[then(expr = "應有外鍵限制，欄位 {string} 參照 {string}")]
fn has_foreign_key(w: &mut ErpWorld, column: String, reference: String) {
    let cs = w.dict_constraints.as_ref().expect("尚未查看限制");
    let (table, ref_col) = reference.split_once('.').expect("參照格式為 表.欄位");
    assert!(
        cs.iter().any(|c| c.kind == ConstraintKindDto::ForeignKey
            && c.columns == [column.clone()]
            && c.ref_table.as_deref() == Some(table)
            && c.ref_columns == [ref_col.to_owned()]),
        "限制：{cs:?}"
    );
}

#[then(expr = "資料表 {string} 應無字典")]
async fn no_dictionary(w: &mut ErpWorld, table: String) {
    match w.dictionary.get(&table).await {
        Err(e) if e.code() == "DICT_NOT_FOUND" => {}
        other => panic!("預期無字典，實際：{other:?}"),
    }
}

#[then(expr = "資料表 {string} 應無任何語系的表說明")]
async fn no_table_texts(w: &mut ErpWorld, table: String) {
    let texts = w.dictionary.table_texts(&table).await.unwrap();
    assert!(
        texts
            .iter()
            .all(|t| t.name.is_none() && t.purpose.is_none() && t.remark.is_none()),
        "{texts:?}"
    );
}

#[then(expr = "資料表 {string} 應無任何欄位說明")]
async fn no_column_texts(w: &mut ErpWorld, table: String) {
    let texts = w.dictionary.column_texts(&table).await.unwrap();
    assert!(texts.is_empty(), "{texts:?}");
}

#[then(expr = "資料庫中仍應有資料表 {string}")]
async fn still_in_schema(w: &mut ErpWorld, table: String) {
    assert!(w.dictionary.table_exists(&table).await.unwrap());
}

fn history_has(w: &ErpWorld, version: &str) -> bool {
    let v: i64 = version.parse().expect("migration 版本需為數字");
    w.dict_history
        .as_ref()
        .expect("尚未查看變更紀錄")
        .iter()
        .any(|m| m.version == v)
}

#[then(expr = "變更紀錄應包含 migration {string}")]
fn history_contains(w: &mut ErpWorld, version: String) {
    assert!(history_has(w, &version), "變更紀錄：{:?}", w.dict_history);
}

#[then(expr = "變更紀錄不應包含 migration {string}")]
fn history_excludes(w: &mut ErpWorld, version: String) {
    assert!(!history_has(w, &version), "變更紀錄：{:?}", w.dict_history);
}

#[then(expr = "匯出內容應包含一列 {string}")]
fn export_contains(w: &mut ErpWorld, line: String) {
    let csv = w.dict_export.as_ref().expect("尚未匯出");
    assert!(csv.lines().any(|l| l == line), "匯出內容：\n{csv}");
}

#[then(expr = "顯示的表名稱應為 {string}")]
fn displayed_table_name(w: &mut ErpWorld, name: String) {
    let d = w.displayed_dict.as_ref().expect("尚未查看字典");
    assert_eq!(d.name.as_deref().unwrap_or(""), name);
}

#[then(expr = "字典應附 {int} 個語系的說明")]
fn displayed_text_count(w: &mut ErpWorld, n: usize) {
    let d = w.displayed_dict.as_ref().expect("尚未查看字典");
    assert_eq!(d.texts.len(), n, "說明：{:?}", d.texts);
}
