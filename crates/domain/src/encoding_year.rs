//! 編碼年對照（TipTop: gff_file 編碼年對照表；aooi404 年－編碼維護）
//! 規格：specs/features/aoo/encoding_year.feature（EY1–EY8）

use chrono::NaiveDate;

use crate::{error::DomainError, record::Stamp};

/// 年的範圍（EY3(a)；TipTop 只檢查須大於 0，amm-110）
pub const YEAR_MAX: i32 = 9999;
/// 編碼編號最多 10 字（gff03 varchar2(10)）
pub const CODE_MAX: usize = 10;
/// 編碼編號不可含的字元（EY3(b)；同編碼獨立段 aoo-124）
const INVALID_CHARS: [char; 5] = ['*', '%', '_', '?', ' '];

/// 年：1–9999（EY3(a)）
pub fn parse_year(year: Option<i32>) -> Result<i32, DomainError> {
    match year {
        Some(y) if (1..=YEAR_MAX).contains(&y) => Ok(y),
        _ => Err(DomainError::EncodingYearInvalid { max: YEAR_MAX }),
    }
}

/// 編碼編號：去頭尾空白、必填、最多 10 字、區分大小寫、不可含 * % _ ? 與空白（EY3(b)）；
/// 回傳（編碼編號, 長度＝字數，:229-234）
pub fn parse_code(raw: &str) -> Result<(String, i16), DomainError> {
    let v = raw.trim();
    if v.is_empty() {
        return Err(DomainError::EncodingYearCodeEmpty);
    }
    let n = v.chars().count();
    if n > CODE_MAX {
        return Err(DomainError::EncodingYearCodeTooLong { max: CODE_MAX });
    }
    if v.chars().any(|c| INVALID_CHARS.contains(&c)) {
        return Err(DomainError::EncodingYearCodeInvalidChar);
    }
    Ok((v.to_owned(), i16::try_from(n).unwrap_or(i16::MAX)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodingYear {
    /// gff01 年（主鍵之一）
    pub year: i32,
    /// gff02 長度（主鍵之一）：編碼編號的字數
    pub length: i16,
    /// gff03 編碼編號
    pub code_value: String,
    /// TTRS 新增的資料有效（EY5；gff_file 無此欄）
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9）
    pub confirmation: crate::confirmation::Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl EncodingYear {
    /// 新增：長度由編碼編號帶出；資料有效、未確認
    pub fn new(
        year: Option<i32>,
        code: &str,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        let year = parse_year(year)?;
        let (code_value, length) = parse_code(code)?;
        Ok(Self {
            year,
            length,
            code_value,
            active: true,
            confirmation: crate::confirmation::Confirmation::default(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        })
    }

    /// 修改編碼編號：字數必須和原本一樣（年與長度是主鍵，EY4）
    pub fn update(&mut self, code: &str, user: &str, today: NaiveDate) -> Result<(), DomainError> {
        let (code_value, length) = parse_code(code)?;
        if length != self.length {
            return Err(DomainError::EncodingYearLengthChanged {
                length: self.length,
            });
        }
        self.code_value = code_value;
        self.touch(user, today);
        Ok(())
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製到新的年（ADR 0013 SA5；EY8）：長度與編碼編號帶入；資料所有者為目前使用者、未確認
    pub fn copy_to(
        &self,
        year: Option<i32>,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        Self::new(year, &self.code_value, user, dept, today)
    }

    /// 確認／取消確認（ADR 0013 SA9）
    pub fn set_confirmed(
        &mut self,
        confirm: bool,
        user: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), DomainError> {
        if confirm {
            self.confirmation.confirm(user, now)?;
        } else {
            self.confirmation.unconfirm()?;
        }
        self.touch(user, now.date_naive());
        Ok(())
    }

    fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules() {
        assert_eq!(parse_code(" 26 "), Ok(("26".to_owned(), 2)));
        assert_eq!(parse_code("年"), Ok(("年".to_owned(), 1)));
        assert_eq!(parse_code(""), Err(DomainError::EncodingYearCodeEmpty));
        assert_eq!(
            parse_code("2 6"),
            Err(DomainError::EncodingYearCodeInvalidChar)
        );
        assert_eq!(
            parse_code("ABCDEFGHIJK"),
            Err(DomainError::EncodingYearCodeTooLong { max: 10 })
        );
        assert!(parse_year(Some(2026)).is_ok());
        for y in [None, Some(0), Some(10000)] {
            assert_eq!(
                parse_year(y),
                Err(DomainError::EncodingYearInvalid { max: 9999 })
            );
        }
        let today = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let mut y = EncodingYear::new(Some(2026), "Q", "u", "d", today).unwrap();
        assert_eq!(
            y.update("QQ", "u", today),
            Err(DomainError::EncodingYearLengthChanged { length: 1 })
        );
        assert!(y.update("R", "u", today).is_ok());
    }
}
