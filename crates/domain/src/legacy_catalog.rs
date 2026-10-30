//! TipTop 對照目錄（TipTop Catalog）：TipTop 資料表／欄位 → 英文名稱的唯讀對照。
//! 來源：各模組 .sch（legacy/catalog/tables.json）＋命名檔 docs/legacy-map/tiptop-naming/*.yaml
//! 規格：specs/features/azz/legacy_catalog.feature（C1–C8）

use std::collections::HashSet;

use crate::DomainError;

/// 一張 TipTop 資料表的對照
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyTable {
    /// TipTop 表代碼，如 zx_file
    pub legacy_name: String,
    /// 英文表名，如 user_account
    pub name: String,
    /// .sch 表說明，如 部門使用者檔
    pub title: String,
    pub purpose: Option<String>,
    /// TipTop 模組，如 azz
    pub module: String,
    pub columns: Vec<LegacyColumn>,
}

/// 一個 TipTop 欄位的對照
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyColumn {
    /// TipTop 欄位代碼，如 zx01
    pub legacy_name: String,
    /// 英文欄位名，如 user_id
    pub name: String,
    /// TipTop 型別，如 varchar2(10)
    pub data_type: String,
    pub primary_key: bool,
    /// .sch 欄位說明
    pub description: String,
    pub notes: Option<String>,
    /// .sch 標示 No Use（C5）
    pub unused: bool,
    /// 命名待確認
    pub verify: bool,
}

/// PostgreSQL 保留字（常見而易誤用者），英文名稱不可使用
const RESERVED: &[&str] = &[
    "all",
    "and",
    "any",
    "array",
    "as",
    "asc",
    "both",
    "case",
    "cast",
    "check",
    "collate",
    "column",
    "constraint",
    "create",
    "current_date",
    "current_time",
    "current_user",
    "default",
    "desc",
    "distinct",
    "do",
    "else",
    "end",
    "except",
    "false",
    "for",
    "foreign",
    "from",
    "grant",
    "group",
    "having",
    "in",
    "into",
    "is",
    "join",
    "limit",
    "not",
    "null",
    "offset",
    "on",
    "only",
    "or",
    "order",
    "primary",
    "references",
    "select",
    "some",
    "table",
    "then",
    "to",
    "true",
    "union",
    "unique",
    "user",
    "using",
    "when",
    "where",
    "with",
    "window",
];

/// 英文名稱規則：小寫英文開頭，僅小寫英數與底線，最多 63 字（PostgreSQL 識別字上限），非保留字
pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    first_ok
        && name.len() <= 63
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !RESERVED.contains(&name)
}

/// 整批檢核：名稱合法（LEGACY_NAME_INVALID）、表名不重複、同表欄位名不重複（C6，LEGACY_NAME_DUPLICATE）
pub fn validate(tables: &[LegacyTable]) -> Result<(), DomainError> {
    let mut table_names = HashSet::new();
    for t in tables {
        if !is_valid_name(&t.name) {
            return Err(DomainError::LegacyNameInvalid(format!(
                "{}：{}",
                t.legacy_name, t.name
            )));
        }
        if !table_names.insert(t.name.as_str()) {
            return Err(DomainError::LegacyNameDuplicate(format!(
                "{}：{}",
                t.legacy_name, t.name
            )));
        }
        let mut column_names = HashSet::new();
        for c in &t.columns {
            let at = format!("{}.{}：{}", t.legacy_name, c.legacy_name, c.name);
            if !is_valid_name(&c.name) {
                return Err(DomainError::LegacyNameInvalid(at));
            }
            if !column_names.insert(c.name.as_str()) {
                return Err(DomainError::LegacyNameDuplicate(at));
            }
        }
    }
    Ok(())
}

/// 查詢條件比對：含 `*`/`?` 時以萬用字元比對舊表代碼或英文名稱，
/// 否則以「包含」比對舊表代碼、英文名稱或說明；不分大小寫，空白為全部
pub fn matches(query: &str, legacy_name: &str, name: &str, title: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    if q.contains(['*', '?']) {
        return glob(&q, &legacy_name.to_lowercase()) || glob(&q, &name.to_lowercase());
    }
    [legacy_name, name, title]
        .iter()
        .any(|s| s.to_lowercase().contains(&q))
}

fn glob(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some('*'), _) => go(&p[1..], t) || (!t.is_empty() && go(p, &t[1..])),
            (Some('?'), Some(_)) => go(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &t[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    go(&p, &t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(name: &str, cols: &[&str]) -> LegacyTable {
        LegacyTable {
            legacy_name: "zx_file".into(),
            name: name.into(),
            title: "部門使用者檔".into(),
            purpose: None,
            module: "azz".into(),
            columns: cols
                .iter()
                .enumerate()
                .map(|(i, c)| LegacyColumn {
                    legacy_name: format!("zx{i:02}"),
                    name: (*c).into(),
                    data_type: "varchar2(10)".into(),
                    primary_key: false,
                    description: String::new(),
                    notes: None,
                    unused: false,
                    verify: false,
                })
                .collect(),
        }
    }

    #[test]
    fn names_follow_rules() {
        assert!(is_valid_name("user_account"));
        assert!(is_valid_name("sub_report_sql15"));
        assert!(!is_valid_name("User-Account"));
        assert!(!is_valid_name("1abc"));
        assert!(!is_valid_name("user"), "保留字");
        assert!(!is_valid_name(&"a".repeat(64)));
    }

    #[test]
    fn rejects_duplicate_column_names() {
        let err = validate(&[table("user_account", &["user_id", "user_id"])]).unwrap_err();
        assert_eq!(err.code(), "LEGACY_NAME_DUPLICATE");
        assert!(validate(&[table("user_account", &["user_id", "user_name"])]).is_ok());
    }

    #[test]
    fn query_matches_code_name_or_title() {
        assert!(matches("使用者", "zx_file", "user_account", "部門使用者檔"));
        assert!(matches(
            "user_acc*",
            "zx_file",
            "user_account",
            "部門使用者檔"
        ));
        assert!(matches("ZX*", "zx_file", "user_account", ""));
        assert!(!matches("zz*", "zx_file", "user_account", ""));
        assert!(matches("", "zx_file", "user_account", ""));
    }
}
