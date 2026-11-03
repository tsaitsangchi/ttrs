//! 記憶體後端用的資料庫結構與 migration（資料字典規格需要）。
//!
//! 記憶體版無法讀取真正的資料庫，因此這裡手寫 migrations 0003–0007 套用後的實際結構。
//! PostgreSQL 後端讀系統目錄，同一份規格兩邊都要通過，結構若與此不一致會在 `make bdd-pg` 失敗。

use std::path::Path;

use ttrs_app::{
    memory::InMemoryStore,
    ports::{ColumnSchema, ConstraintKind, ConstraintSchema, MigrationSource, TableSchema},
};

fn col(name: &str, data_type: &str, not_null: bool, default: Option<&str>) -> ColumnSchema {
    ColumnSchema {
        name: name.into(),
        data_type: data_type.into(),
        not_null,
        default: default.map(Into::into),
    }
}

fn constraint(
    name: &str,
    kind: ConstraintKind,
    columns: &[&str],
    reference: Option<(&str, &[&str])>,
) -> ConstraintSchema {
    ConstraintSchema {
        name: name.into(),
        kind,
        columns: columns.iter().map(|c| (*c).into()).collect(),
        ref_table: reference.map(|(t, _)| t.into()),
        ref_columns: reference
            .map(|(_, cs)| cs.iter().map(|c| (*c).into()).collect())
            .unwrap_or_default(),
        // 本專案外鍵都參照對方主鍵，PostgreSQL 預設名稱為 <表>_pkey
        ref_constraint: reference.map(|(t, _)| format!("{t}_pkey")),
        check_clause: None,
    }
}

/// program（0003 建立、0005 改名、0006 加欄位、0007 改名與長度、0018 英文程式代號）
fn program() -> TableSchema {
    TableSchema {
        name: "program".into(),
        columns: vec![
            col("program_no", "varchar(40)", true, None),
            col("module_code", "varchar(10)", true, None),
            col("program_type", "varchar(1)", false, None),
            col("report_type", "text", true, Some("'5'::text")),
            col("idle_control", "text", true, Some("'3'::text")),
            col("idle_seconds", "integer", false, None),
            col("idle_action", "text", false, None),
            col("pass_book_no", "boolean", true, Some("false")),
            col("deferred_print_count", "integer", true, Some("0")),
            col("current_print_count", "integer", true, Some("0")),
            col("report_width", "integer", false, None),
            col("run_command", "varchar(60)", false, None),
            col("window_style", "varchar(20)", false, None),
            col("report_ext_mode", "text", false, None),
            col("program_group", "varchar(10)", false, None),
            col("fixed_print_method", "varchar(1)", false, None),
            col("fixed_print_condition", "varchar(80)", false, None),
            col("sql_explain", "boolean", true, Some("false")),
            col("log_errors", "boolean", true, Some("false")),
            col("created_by", "text", true, None),
            col("created_dept", "text", true, None),
            col("updated_by", "text", false, None),
            col("created_at", "timestamptz", true, Some("now()")),
            col("updated_at", "timestamptz", true, Some("now()")),
            col("default_actions", "varchar(3000)", false, None),
            col("print_conditions", "boolean", true, Some("false")),
            col("allow_key_update", "boolean", true, Some("false")),
            col("remark", "varchar(255)", false, None),
            col("designed_on", "date", true, Some("CURRENT_DATE")),
            col("updated_on", "date", true, Some("CURRENT_DATE")),
            col("tiptop_program_no", "varchar(20)", false, None),
        ],
        indexes: vec![],
        constraints: vec![constraint(
            "program_pkey",
            ConstraintKind::PrimaryKey,
            &["program_no"],
            None,
        )],
    }
}

/// program_permission（0003）
fn program_permission() -> TableSchema {
    TableSchema {
        name: "program_permission".into(),
        columns: vec![
            col("class_code", "text", true, None),
            col("program_no", "text", true, None),
            col("created_at", "timestamptz", true, Some("now()")),
        ],
        indexes: vec![],
        constraints: vec![
            constraint(
                "program_permission_pkey",
                ConstraintKind::PrimaryKey,
                &["class_code", "program_no"],
                None,
            ),
            constraint(
                "program_permission_class_code_fkey",
                ConstraintKind::ForeignKey,
                &["class_code"],
                Some(("permission_class", &["class_code"])),
            ),
            constraint(
                "program_permission_program_no_fkey",
                ConstraintKind::ForeignKey,
                &["program_no"],
                Some(("program", &["program_no"])),
            ),
        ],
    }
}

/// 登錄記憶體後端的資料庫結構與 migration 內容
pub fn load(store: &InMemoryStore) {
    store.add_table_schema(program());
    store.add_table_schema(program_permission());

    // migration 檔名格式同 sqlx：<版本>_<說明>.sql
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("read migrations")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "sql"))
        .collect();
    files.sort();
    for path in files {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let (version, desc) = stem
            .split_once('_')
            .expect("migration 檔名需為 <版本>_<說明>");
        store.add_migration(MigrationSource {
            version: version.parse().expect("migration 版本需為數字"),
            description: desc.replace('_', " "),
            sql: std::fs::read_to_string(&path).expect("read migration"),
        });
    }
}
