//! 資料字典
//! TipTop 對應：p_zta（檔案架構修整記錄維護）／ zta_file、gat_file（表名稱多語言）、gaq_file（欄位說明多語言）
//! 規格：specs/features/azz/data_dictionary.feature（D1–D8）
//!
//! 只描述資料表與欄位，不改變資料庫結構；結構由 migrations/ 決定。

use chrono::NaiveDate;

use crate::{
    DomainError,
    program::{ModuleCode, check_length},
};

pub use crate::lang::{LANGS, Lang};

/// 長度上限（D8）：資料表代碼依 PostgreSQL 識別字上限，其餘依 gat_file.sch、gaq_file.sch、zta_file.sch
pub mod max_len {
    pub const TABLE_NAME: usize = 63;
    pub const COLUMN_NAME: usize = 63;
    pub const TABLE_TEXT_NAME: usize = 120; // gat03
    pub const TABLE_PURPOSE: usize = 255; // gat04
    pub const TABLE_REMARK: usize = 255; // gat05
    pub const COLUMN_TEXT_NAME: usize = 255; // gaq03
    pub const COLUMN_DESCRIPTION: usize = 500; // gaq04
    pub const COLUMN_EXTRA: usize = 2000; // gaq05
    pub const UPSTREAM: usize = 80; // zta11（4fd 上游檔案）
    pub const DOWNSTREAM: usize = 60; // zta12（4fd 下游檔案）
}

/// 資料表或欄位名稱（資料庫識別字）
fn identifier(raw: &str, field: &'static str, max: usize) -> Result<String, DomainError> {
    let v = raw.trim().to_lowercase();
    if v.is_empty() {
        return Err(DomainError::DictTableNameEmpty);
    }
    check_length(field, Some(&v), max)?;
    Ok(v)
}

/// 資料表類型（gat07，D7）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableType {
    Parameter,
    Master,
    Transaction,
    Statistics,
    Base,
}

impl TableType {
    pub fn code(self) -> &'static str {
        match self {
            Self::Parameter => "P",
            Self::Master => "M",
            Self::Transaction => "T",
            Self::Statistics => "S",
            Self::Base => "B",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "P" => Self::Parameter,
            "M" => Self::Master,
            "T" => Self::Transaction,
            "S" => Self::Statistics,
            "B" => Self::Base,
            _ => return None,
        })
    }
}

/// 表頭（zta_file + gat_file 的語系共用欄位）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDictionary {
    pub table_name: String,
    pub module: ModuleCode,
    pub table_type: Option<TableType>,
    pub upstream: Option<String>,
    pub downstream: Option<String>,
    pub created_by: String,
    pub created_on: NaiveDate,
}

/// 可修改的表頭欄位
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableDictionaryAttrs {
    pub table_type: Option<TableType>,
    pub upstream: Option<String>,
    pub downstream: Option<String>,
}

impl TableDictionary {
    /// 新增（p_zta_a :1265）：產生者＝目前使用者、產生日期＝今天
    pub fn new(
        table_name: &str,
        module: ModuleCode,
        attrs: TableDictionaryAttrs,
        created_by: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        let mut d = Self {
            table_name: Self::parse_name(table_name)?,
            module,
            table_type: None,
            upstream: None,
            downstream: None,
            created_by: created_by.to_owned(),
            created_on: today,
        };
        d.apply(attrs)?;
        Ok(d)
    }

    /// 修改（p_zta_u）：資料表代碼不可改
    pub fn update(
        &mut self,
        module: ModuleCode,
        attrs: TableDictionaryAttrs,
    ) -> Result<(), DomainError> {
        let mut next = self.clone();
        next.module = module;
        next.apply(attrs)?;
        *self = next;
        Ok(())
    }

    pub fn parse_name(raw: &str) -> Result<String, DomainError> {
        identifier(raw, "資料表代碼", max_len::TABLE_NAME)
    }

    fn apply(&mut self, a: TableDictionaryAttrs) -> Result<(), DomainError> {
        let upstream = clean(a.upstream);
        let downstream = clean(a.downstream);
        check_length("上游檔案", upstream.as_deref(), max_len::UPSTREAM)?;
        check_length("下游檔案", downstream.as_deref(), max_len::DOWNSTREAM)?;
        self.table_type = a.table_type;
        self.upstream = upstream;
        self.downstream = downstream;
        Ok(())
    }
}

/// 某語系的表名稱、目的、備註（gat03/gat04/gat05）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableText {
    pub lang: Lang,
    pub name: Option<String>,
    pub purpose: Option<String>,
    pub remark: Option<String>,
}

impl TableText {
    pub fn new(
        lang: &str,
        name: Option<String>,
        purpose: Option<String>,
        remark: Option<String>,
    ) -> Result<Self, DomainError> {
        let t = Self {
            lang: Lang::parse(lang)?,
            name: clean(name),
            purpose: clean(purpose),
            remark: clean(remark),
        };
        check_length("表名稱", t.name.as_deref(), max_len::TABLE_TEXT_NAME)?;
        check_length("表目的", t.purpose.as_deref(), max_len::TABLE_PURPOSE)?;
        check_length("備註", t.remark.as_deref(), max_len::TABLE_REMARK)?;
        Ok(t)
    }
}

/// 某語系的欄位名稱、說明、額外說明（gaq03/gaq04/gaq05；鍵值為資料表＋欄位，D2）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnText {
    pub column_name: String,
    pub lang: Lang,
    pub name: Option<String>,
    pub description: Option<String>,
    pub extra: Option<String>,
}

impl ColumnText {
    pub fn new(
        column_name: &str,
        lang: &str,
        name: Option<String>,
        description: Option<String>,
        extra: Option<String>,
    ) -> Result<Self, DomainError> {
        let t = Self {
            column_name: identifier(column_name, "欄位代碼", max_len::COLUMN_NAME)?,
            lang: Lang::parse(lang)?,
            name: clean(name),
            description: clean(description),
            extra: clean(extra),
        };
        check_length("欄位名稱", t.name.as_deref(), max_len::COLUMN_TEXT_NAME)?;
        check_length(
            "欄位說明",
            t.description.as_deref(),
            max_len::COLUMN_DESCRIPTION,
        )?;
        check_length("額外說明", t.extra.as_deref(), max_len::COLUMN_EXTRA)?;
        Ok(t)
    }
}

fn clean(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// migration 的 SQL 是否涉及某資料表（D5）。
/// 略過註解與字串常值，以識別字邊界比對（不分大小寫），避免 program 誤中 program_name。
pub fn migration_touches(sql: &str, table: &str) -> bool {
    let table = table.to_lowercase();
    let code = strip_comments_and_literals(sql).to_lowercase();
    let bytes = code.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut start = 0;
    while let Some(pos) = code[start..].find(&table) {
        let i = start + pos;
        let j = i + table.len();
        let before_ok = i == 0 || !is_ident(bytes[i - 1]);
        let after_ok = j >= bytes.len() || !is_ident(bytes[j]);
        if before_ok && after_ok {
            return true;
        }
        start = i + 1;
    }
    false
}

fn strip_comments_and_literals(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '-' if chars.peek() == Some(&'-') => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
                out.push(' ');
            }
            '\'' => {
                // 字串常值（'' 為跳脫的單引號）
                while let Some(n) = chars.next() {
                    if n == '\'' {
                        if chars.peek() == Some(&'\'') {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_touches_uses_identifier_boundaries() {
        let sql = "-- program 註解\nCREATE TABLE program_name (program_no TEXT REFERENCES program (program_no));";
        assert!(migration_touches(sql, "program"));
        assert!(migration_touches(sql, "program_name"));
        assert!(!migration_touches(
            "CREATE TABLE program_name (x int);",
            "program"
        ));
        assert!(!migration_touches("-- program\nSELECT 1;", "program"));
        assert!(!migration_touches(
            "COMMENT ON TABLE x IS 'program';",
            "program"
        ));
        assert!(migration_touches(
            "ALTER TABLE PROGRAM ADD COLUMN y int;",
            "program"
        ));
    }

    #[test]
    fn unsupported_language() {
        assert_eq!(
            Lang::parse("ja"),
            Err(DomainError::LanguageNotSupported("ja".into()))
        );
        assert!(Lang::parse("zh-CN").is_ok());
    }
}
