//! 單號自動編號（TipTop topcust/csub/4gl/s_auto_assign_no.4gl；misc_transaction.feature MT4）
//!
//! 單號＝單別＋「-」＋流水號；流水號長度依整體參數 aza42（8、9、10 碼）。
//! 編號方式（smydmy5）：1 依流水號、2 依年度期別 YYMM、3 依年度週別 YYWW、4 依年月日 YYMMDD，其後補序號。
//! 年度、期別、週別取自輸入日的會計期間（[`crate::accounting_period::Calendar::locate`]）。

use chrono::Datelike;

use crate::{DomainError, accounting_period::AccountingDay, document_type::DocTypeCode};

/// 單號前綴與序號長度：(前綴（含單別與「-」與日期碼）, 序號位數)（s_auto_assign_no:1397-1440、:1858-1900、:2320-2330）
pub fn prefix(
    code: &DocTypeCode,
    numbering: &str,
    day: &AccountingDay,
    serial_len: u8,
) -> (String, usize) {
    let yy = day.year.rem_euclid(100);
    let date = match numbering {
        "2" => format!("{yy:02}{:02}", day.period),
        "3" => format!("{yy:02}{:02}", day.week),
        "4" => format!(
            "{:02}{:02}{:02}",
            day.date.year().rem_euclid(100),
            day.date.month(),
            day.date.day()
        ),
        _ => String::new(),
    };
    let seq = usize::from(serial_len) - date.len();
    (format!("{}-{date}", code.as_str()), seq)
}

/// 下一個單號：同前綴已用的最大號＋1；用完回 DOC_NO_EXHAUSTED（sub-518）
pub fn next(prefix: &str, seq_len: usize, max_used: Option<&str>) -> Result<String, DomainError> {
    let last: u64 = max_used
        .and_then(|m| m.strip_prefix(prefix))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let n = last + 1;
    let s = format!("{n:0seq_len$}");
    if s.len() > seq_len {
        return Err(DomainError::DocNoExhausted);
    }
    Ok(format!("{prefix}{s}"))
}

/// 使用者輸入的完整單號（不自動編號，或自動編號但已給完整單號；sub-141）：
/// 前幾碼須為單別、接「-」、流水號長度正確且不含空白
pub fn check_manual(no: &str, code: &DocTypeCode, serial_len: u8) -> Result<String, DomainError> {
    let bad = || DomainError::DocNoInvalid { len: serial_len };
    let rest = no
        .strip_prefix(code.as_str())
        .and_then(|r| r.strip_prefix('-'))
        .ok_or_else(bad)?;
    if rest.chars().count() != usize::from(serial_len) || rest.chars().any(char::is_whitespace) {
        return Err(bad());
    }
    Ok(no.to_owned())
}

/// 單號的單別部分（s_get_doc_no）：「-」之前；沒有「-」時整串都是單別
pub fn doc_type_part(no: &str) -> &str {
    no.split('-').next().unwrap_or(no)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn day() -> AccountingDay {
        AccountingDay {
            date: NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
            year: 2026,
            quarter: 4,
            period: 11,
            week: 47,
        }
    }

    #[test]
    fn prefixes() {
        let c = DocTypeCode::existing("MI1").unwrap();
        assert_eq!(prefix(&c, "1", &day(), 8), ("MI1-".into(), 8));
        assert_eq!(prefix(&c, "2", &day(), 8), ("MI1-2611".into(), 4));
        assert_eq!(prefix(&c, "3", &day(), 8), ("MI1-2647".into(), 4));
        assert_eq!(prefix(&c, "4", &day(), 8), ("MI1-261120".into(), 2));
        assert_eq!(prefix(&c, "2", &day(), 10), ("MI1-2611".into(), 6));
    }

    #[test]
    fn next_numbers() {
        assert_eq!(next("MI1-2611", 4, None).unwrap(), "MI1-26110001");
        assert_eq!(
            next("MI1-2611", 4, Some("MI1-26110041")).unwrap(),
            "MI1-26110042"
        );
        assert_eq!(
            next("MI1-2611", 4, Some("MI1-26119999"))
                .unwrap_err()
                .code(),
            "DOC_NO_EXHAUSTED"
        );
    }

    #[test]
    fn manual() {
        let c = DocTypeCode::existing("MI1").unwrap();
        assert!(check_manual("MI1-00000001", &c, 8).is_ok());
        for bad in ["MI1-123", "MR1-00000001", "MI100000001", "MI1-0000 001"] {
            assert_eq!(
                check_manual(bad, &c, 8).unwrap_err().code(),
                "DOC_NO_INVALID"
            );
        }
        assert_eq!(doc_type_part("MI1-00000001"), "MI1");
        assert_eq!(doc_type_part("MI1"), "MI1");
    }
}
