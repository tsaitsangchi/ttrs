//! QBE（Query By Example）條件解析與比對
//! TipTop 對應：Genero BDL CONSTRUCT 的條件語法（各維護作業的 Q.查詢）
//! 規格：specs/features/azz/program_maintenance_query.feature（QBE1–QBE7）
//!
//! 語法（以 `|` 分隔多個選項，彼此為 OR）：
//!   - `abc`        等於
//!   - `a*` `a?c`   萬用字元：`*` 任意多字元、`?` 單一字元（僅文字欄位）
//!   - `a:b`        範圍，含兩端
//!   - `>a` `>=a` `<a` `<=a` `<>a`（`!=a` 同 `<>`）
//!   - `=`          只有等號：空值
//!   - `\x`         跳脫，`x` 視為一般字元（如 `\*`、`\|`、`\:`）
//!
//! 字串比較區分大小寫、以位元組順序比較（同 Oracle 預設的 BINARY 排序）。

use std::cmp::Ordering;

use chrono::NaiveDate;

use crate::DomainError;

/// 欄位型別：決定條件值如何解析
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Integer,
    Date,
    /// Y/N 旗標；只允許等於、不等於、空值
    Flag,
}

/// 已依欄位型別解析的值
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QbeValue {
    Text(String),
    Integer(i64),
    Date(NaiveDate),
    Flag(bool),
}

impl QbeValue {
    fn compare(&self, other: &QbeValue) -> Option<Ordering> {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => Some(a.as_bytes().cmp(b.as_bytes())),
            (Self::Integer(a), Self::Integer(b)) => Some(a.cmp(b)),
            (Self::Date(a), Self::Date(b)) => Some(a.cmp(b)),
            (Self::Flag(a), Self::Flag(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

/// 萬用字元樣式的片段
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternPart {
    Literal(String),
    /// `*`
    AnyMany,
    /// `?`
    AnyOne,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QbeTerm {
    Eq(QbeValue),
    Ne(QbeValue),
    Lt(QbeValue),
    Le(QbeValue),
    Gt(QbeValue),
    Ge(QbeValue),
    Range(QbeValue, QbeValue),
    Like(Vec<PatternPart>),
    IsNull,
}

/// 單一欄位的條件：`terms` 之間為 OR
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QbeFilter {
    pub field: &'static str,
    pub terms: Vec<QbeTerm>,
}

/// 解析使用者輸入的條件。空白字串回傳 `None`（不限制）。
/// `map_value` 讓呼叫端把畫面顯示文字轉成存檔代碼（如報表類型「明細表」→ `2`）。
pub fn parse(
    field: &'static str,
    field_type: FieldType,
    expr: &str,
    map_value: impl Fn(&str) -> String,
) -> Result<Option<QbeFilter>, DomainError> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Ok(None);
    }
    let terms = split_unescaped(expr, '|')
        .into_iter()
        .map(|alt| parse_term(field_type, alt.trim(), &map_value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(QbeFilter { field, terms }))
}

fn parse_term(
    ty: FieldType,
    alt: &str,
    map_value: &impl Fn(&str) -> String,
) -> Result<QbeTerm, DomainError> {
    let syntax = || DomainError::QbeInvalidSyntax(alt.to_owned());
    if alt.is_empty() {
        return Err(syntax());
    }
    if alt == "=" {
        return Ok(QbeTerm::IsNull);
    }
    let value = |raw: &str| -> Result<QbeValue, DomainError> {
        if raw.is_empty() {
            return Err(syntax());
        }
        typed(ty, &map_value(&unescape(raw)))
    };

    for (op, make) in [
        (">=", QbeTerm::Ge as fn(QbeValue) -> QbeTerm),
        ("<=", QbeTerm::Le),
        ("<>", QbeTerm::Ne),
        ("!=", QbeTerm::Ne),
        (">", QbeTerm::Gt),
        ("<", QbeTerm::Lt),
        ("=", QbeTerm::Eq),
    ] {
        if let Some(rest) = alt.strip_prefix(op) {
            let term = make(value(rest.trim())?);
            if ty == FieldType::Flag && !matches!(term, QbeTerm::Eq(_) | QbeTerm::Ne(_)) {
                return Err(syntax());
            }
            return Ok(term);
        }
    }

    let range = split_unescaped(alt, ':');
    if range.len() > 1 {
        if range.len() != 2 || ty == FieldType::Flag {
            return Err(syntax());
        }
        return Ok(QbeTerm::Range(
            value(range[0].trim())?,
            value(range[1].trim())?,
        ));
    }

    if has_unescaped_wildcard(alt) {
        return match ty {
            FieldType::Text => Ok(QbeTerm::Like(pattern(alt))),
            FieldType::Integer => Err(DomainError::QbeInvalidNumber(alt.to_owned())),
            FieldType::Date => Err(DomainError::QbeInvalidDate(alt.to_owned())),
            FieldType::Flag => Err(syntax()),
        };
    }
    Ok(QbeTerm::Eq(value(alt)?))
}

fn typed(ty: FieldType, raw: &str) -> Result<QbeValue, DomainError> {
    match ty {
        FieldType::Text => Ok(QbeValue::Text(raw.to_owned())),
        FieldType::Integer => raw
            .parse()
            .map(QbeValue::Integer)
            .map_err(|_| DomainError::QbeInvalidNumber(raw.to_owned())),
        // QBE3：接受 2026/10/29 與 2026-10-29
        FieldType::Date => NaiveDate::parse_from_str(raw, "%Y/%m/%d")
            .or_else(|_| NaiveDate::parse_from_str(raw, "%Y-%m-%d"))
            .map(QbeValue::Date)
            .map_err(|_| DomainError::QbeInvalidDate(raw.to_owned())),
        FieldType::Flag => match raw {
            "Y" | "y" | "是" => Ok(QbeValue::Flag(true)),
            "N" | "n" | "否" => Ok(QbeValue::Flag(false)),
            _ => Err(DomainError::QbeInvalidSyntax(raw.to_owned())),
        },
    }
}

/// 依未跳脫的分隔字元切開（保留跳脫字元，交給後續處理）
fn split_unescaped(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == sep {
            parts.push(&s[start..i]);
            start = i + c.len_utf8();
        }
    }
    parts.push(&s[start..]);
    parts
}

fn has_unescaped_wildcard(s: &str) -> bool {
    let mut escaped = false;
    for c in s.chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '*' || c == '?' {
            return true;
        }
    }
    false
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn pattern(s: &str) -> Vec<PatternPart> {
    let mut parts = Vec::new();
    let mut lit = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    lit.push(n);
                }
            }
            '*' | '?' => {
                if !lit.is_empty() {
                    parts.push(PatternPart::Literal(std::mem::take(&mut lit)));
                }
                parts.push(if c == '*' {
                    PatternPart::AnyMany
                } else {
                    PatternPart::AnyOne
                });
            }
            _ => lit.push(c),
        }
    }
    if !lit.is_empty() {
        parts.push(PatternPart::Literal(lit));
    }
    parts
}

impl QbeFilter {
    /// 欄位值是否符合任一條件。`None` 或空字串視為空值。
    pub fn matches(&self, value: Option<&QbeValue>) -> bool {
        let value = value.filter(|v| !matches!(v, QbeValue::Text(s) if s.is_empty()));
        self.terms.iter().any(|t| t.matches(value))
    }
}

impl QbeTerm {
    pub fn matches(&self, value: Option<&QbeValue>) -> bool {
        let Some(v) = value else {
            return matches!(self, Self::IsNull);
        };
        let cmp = |x: &QbeValue| v.compare(x);
        match self {
            Self::IsNull => false,
            Self::Eq(x) => cmp(x) == Some(Ordering::Equal),
            Self::Ne(x) => cmp(x).is_some_and(|o| o != Ordering::Equal),
            Self::Lt(x) => cmp(x) == Some(Ordering::Less),
            Self::Le(x) => cmp(x).is_some_and(|o| o != Ordering::Greater),
            Self::Gt(x) => cmp(x) == Some(Ordering::Greater),
            Self::Ge(x) => cmp(x).is_some_and(|o| o != Ordering::Less),
            Self::Range(lo, hi) => {
                cmp(lo).is_some_and(|o| o != Ordering::Less)
                    && cmp(hi).is_some_and(|o| o != Ordering::Greater)
            }
            Self::Like(parts) => match v {
                QbeValue::Text(s) => like(parts, &s.chars().collect::<Vec<_>>()),
                _ => false,
            },
        }
    }
}

fn like(parts: &[PatternPart], s: &[char]) -> bool {
    match parts.split_first() {
        None => s.is_empty(),
        Some((PatternPart::Literal(lit), rest)) => {
            let lit: Vec<char> = lit.chars().collect();
            s.starts_with(&lit) && like(rest, &s[lit.len()..])
        }
        Some((PatternPart::AnyOne, rest)) => !s.is_empty() && like(rest, &s[1..]),
        Some((PatternPart::AnyMany, rest)) => (0..=s.len()).any(|i| like(rest, &s[i..])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(expr: &str) -> QbeFilter {
        parse("f", FieldType::Text, expr, str::to_owned)
            .unwrap()
            .unwrap()
    }

    fn hit(f: &QbeFilter, v: &str) -> bool {
        f.matches(Some(&QbeValue::Text(v.to_owned())))
    }

    #[test]
    fn wildcards_and_escape() {
        assert!(hit(&text("aim*"), "aimi100"));
        assert!(hit(&text("aimi10?"), "aimi101"));
        assert!(!hit(&text("aimi10?"), "aimi1000"));
        assert!(hit(&text("aim\\*"), "aim*"));
        assert!(!hit(&text("aim\\*"), "aimi100"));
    }

    #[test]
    fn or_range_compare() {
        let f = text("aimi100|agli100");
        assert!(hit(&f, "agli100") && !hit(&f, "aimi101"));
        assert!(hit(&text("aimi100:aimq102"), "aimi101"));
        assert!(hit(&text(">aimr"), "p_query"));
        assert!(hit(&text("<>aimi100"), "aimi101"));
        assert!(!hit(&text("<>aimi100"), "aimi100"));
    }

    #[test]
    fn null_term() {
        let f = text("=");
        assert!(f.matches(None));
        assert!(hit(&f, ""));
        assert!(!hit(&f, "x"));
    }

    #[test]
    fn typed_errors() {
        let num = parse("f", FieldType::Integer, "abc", str::to_owned);
        assert_eq!(num, Err(DomainError::QbeInvalidNumber("abc".into())));
        let date = parse("f", FieldType::Date, "2026/13/01", str::to_owned);
        assert_eq!(date, Err(DomainError::QbeInvalidDate("2026/13/01".into())));
        let range = parse("f", FieldType::Text, "a:", str::to_owned);
        assert_eq!(range, Err(DomainError::QbeInvalidSyntax("a:".into())));
    }

    #[test]
    fn date_formats() {
        let f = parse("f", FieldType::Date, "2026-10-01:2026/10/31", str::to_owned)
            .unwrap()
            .unwrap();
        let d = NaiveDate::from_ymd_opt(2026, 10, 29).unwrap();
        assert!(f.matches(Some(&QbeValue::Date(d))));
    }
}
