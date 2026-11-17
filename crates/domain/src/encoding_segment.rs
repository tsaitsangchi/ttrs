//! 編碼獨立段（TipTop: gej_file 獨立段資料單頭檔、gek_file 獨立段資料單身檔；aooi403 編碼獨立段資料維護作業）
//! 規格：specs/features/aoo/encoding_segment.feature（IS1–IS8）

use chrono::NaiveDate;

use crate::{error::DomainError, record::Stamp};

/// 獨立段編號最多 10 字（gej01 varchar2(10)，IS3）
pub const CODE_MAX: usize = 10;
/// 名稱、編碼名稱最多 80 字（gej02、gek04）
pub const NAME_MAX: usize = 80;
/// 長度上限：編碼編號 gek03 最多 30 字（IS3）
pub const LENGTH_MAX: i16 = 30;
/// 編碼編號不可含的字元（aooi403.4gl:826 MATCHES '*[*%_? ]*'，aoo-124）
const INVALID_CHARS: [char; 5] = ['*', '%', '_', '?', ' '];

/// 獨立段編號：去頭尾空白、區分大小寫、1–10 字（IS3）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SegmentCode(String);

impl SegmentCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::EncodingSegmentCodeEmpty);
        }
        if v.chars().count() > CODE_MAX {
            return Err(DomainError::EncodingSegmentCodeTooLong { max: CODE_MAX });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn name_of(raw: Option<String>, field: &'static str) -> Result<Option<String>, DomainError> {
    let name = raw.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    if name.as_ref().is_some_and(|n| n.chars().count() > NAME_MAX) {
        return Err(DomainError::FieldTooLong {
            field,
            max: NAME_MAX,
        });
    }
    Ok(name)
}

/// 單頭可修改的欄位：名稱、長度
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentAttrs {
    pub name: Option<String>,
    /// gej03 長度：必填、1–30（IS3）
    pub length: i16,
}

impl SegmentAttrs {
    pub fn new(name: Option<String>, length: Option<i16>) -> Result<Self, DomainError> {
        let length = length.ok_or(DomainError::EncodingSegmentLengthInvalid { max: LENGTH_MAX })?;
        if !(1..=LENGTH_MAX).contains(&length) {
            return Err(DomainError::EncodingSegmentLengthInvalid { max: LENGTH_MAX });
        }
        Ok(Self {
            name: name_of(name, "名稱")?,
            length,
        })
    }
}

/// 單身一列：項次、編碼編號、編碼名稱（gek02、gek03、gek04）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentValue {
    pub line: i32,
    pub value: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodingSegment {
    pub code: SegmentCode,
    pub attrs: SegmentAttrs,
    /// 單身，依項次排序
    pub values: Vec<SegmentValue>,
    /// gejacti 資料有效碼
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；gej_file 無此欄）
    pub confirmation: crate::confirmation::Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl EncodingSegment {
    /// 新增：資料有效預設 Y（aooi403.4gl:292-335）
    pub fn new(
        code: SegmentCode,
        attrs: SegmentAttrs,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            code,
            attrs,
            values: Vec::new(),
            active: true,
            confirmation: crate::confirmation::Confirmation::default(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    /// 停用的不可修改單頭或單身（:342-346 9027、:709-713 aom-000，IS5(a)）
    fn check_active(&self) -> Result<(), DomainError> {
        if self.active {
            Ok(())
        } else {
            Err(DomainError::EncodingSegmentInactive)
        }
    }

    /// 修改名稱、長度：已有單身時不可修改長度（IS5(b)）
    pub fn update(
        &mut self,
        attrs: SegmentAttrs,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        if attrs.length != self.attrs.length && !self.values.is_empty() {
            return Err(DomainError::EncodingSegmentLengthHasValues);
        }
        self.attrs = attrs;
        self.touch(user, today);
        Ok(())
    }

    /// 編碼編號：必填、不可含 * % _ ? 與空白（aoo-124）、長度等於單頭長度（aoo-121）、
    /// 同一獨立段內不可重複（IS4(c)；`except` 為正在修改的項次）
    fn check_value(&self, raw: &str, except: Option<i32>) -> Result<String, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::EncodingSegmentValueEmpty);
        }
        if v.chars().any(|c| INVALID_CHARS.contains(&c)) {
            return Err(DomainError::EncodingSegmentValueInvalidChar);
        }
        if v.chars().count() != usize::try_from(self.attrs.length).unwrap_or(0) {
            return Err(DomainError::EncodingSegmentValueLengthMismatch {
                length: self.attrs.length,
            });
        }
        if self
            .values
            .iter()
            .any(|x| x.value == v && Some(x.line) != except)
        {
            return Err(DomainError::EncodingSegmentValueDuplicate);
        }
        Ok(v.to_owned())
    }

    /// 新增單身（:780-805）：項次未給時為目前最大項次＋1（沒有單身時為 1）；項次須為正整數、不可重複
    pub fn add_value(
        &mut self,
        line: Option<i32>,
        value: &str,
        name: Option<String>,
        user: &str,
        today: NaiveDate,
    ) -> Result<i32, DomainError> {
        self.check_active()?;
        let line =
            line.unwrap_or_else(|| self.values.iter().map(|v| v.line).max().unwrap_or(0) + 1);
        if line < 1 {
            return Err(DomainError::EncodingSegmentLineInvalid);
        }
        if self.values.iter().any(|v| v.line == line) {
            return Err(DomainError::EncodingSegmentLineDuplicate);
        }
        let value = self.check_value(value, None)?;
        self.values.push(SegmentValue {
            line,
            value,
            name: name_of(name, "編碼名稱")?,
        });
        self.values.sort_by_key(|v| v.line);
        self.touch(user, today);
        Ok(line)
    }

    /// 修改單身的編碼編號與名稱（項次不變）
    pub fn update_value(
        &mut self,
        line: i32,
        value: &str,
        name: Option<String>,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        let value = self.check_value(value, Some(line))?;
        let name = name_of(name, "編碼名稱")?;
        let row = self
            .values
            .iter_mut()
            .find(|v| v.line == line)
            .ok_or(DomainError::EncodingSegmentLineNotFound)?;
        row.value = value;
        row.name = name;
        self.touch(user, today);
        Ok(())
    }

    /// 刪除單身一列（:832-860）
    pub fn remove_value(
        &mut self,
        line: i32,
        user: &str,
        today: NaiveDate,
    ) -> Result<(), DomainError> {
        self.check_active()?;
        let before = self.values.len();
        self.values.retain(|v| v.line != line);
        if self.values.len() == before {
            return Err(DomainError::EncodingSegmentLineNotFound);
        }
        self.touch(user, today);
        Ok(())
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製單頭與單身（:1192-1290）：資料所有者為目前使用者、資料有效、未確認
    pub fn copy_as(&self, code: SegmentCode, user: &str, dept: &str, today: NaiveDate) -> Self {
        let mut s = Self::new(code, self.attrs.clone(), user, dept, today);
        s.values = self.values.clone();
        s
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

    /// 修改單頭或單身都更新單頭的最後修改者與日期（FUN-5B0136，IS5(c)）
    fn touch(&mut self, user: &str, today: NaiveDate) {
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(length: i16) -> EncodingSegment {
        let today = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        EncodingSegment::new(
            SegmentCode::parse("COLOR").unwrap(),
            SegmentAttrs::new(None, Some(length)).unwrap(),
            "u",
            "d",
            today,
        )
    }

    #[test]
    fn values() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let mut s = seg(2);
        assert_eq!(s.add_value(None, "01", None, "u", today), Ok(1));
        assert_eq!(s.add_value(None, "02", None, "u", today), Ok(2));
        assert_eq!(
            s.add_value(None, "01", None, "u", today),
            Err(DomainError::EncodingSegmentValueDuplicate)
        );
        assert_eq!(
            s.add_value(None, "A?", None, "u", today),
            Err(DomainError::EncodingSegmentValueInvalidChar)
        );
        assert_eq!(
            s.add_value(None, "1", None, "u", today),
            Err(DomainError::EncodingSegmentValueLengthMismatch { length: 2 })
        );
        assert_eq!(
            s.add_value(Some(1), "03", None, "u", today),
            Err(DomainError::EncodingSegmentLineDuplicate)
        );
        assert!(
            s.update_value(1, "01", Some("紅".into()), "u", today)
                .is_ok()
        );
        assert_eq!(
            s.update(SegmentAttrs::new(None, Some(3)).unwrap(), "u", today),
            Err(DomainError::EncodingSegmentLengthHasValues)
        );
    }

    #[test]
    fn lengths() {
        assert!(SegmentAttrs::new(None, Some(30)).is_ok());
        for l in [Some(0), Some(31), None] {
            assert_eq!(
                SegmentAttrs::new(None, l),
                Err(DomainError::EncodingSegmentLengthInvalid { max: 30 })
            );
        }
    }
}
