//! 單位（TipTop: gfe_file 單位檔；aooi101 單位資料維護）
//! 規格：specs/features/aoo/unit_maintenance.feature（UN1–UN7）

use chrono::NaiveDate;

use crate::{error::DomainError, record::Stamp};

/// 單位代碼最多 4 字（gfe01 varchar2(4)，UN3）
pub const CODE_MAX: usize = 4;
/// 說明最多 80 字（gfe02）
pub const NAME_MAX: usize = 80;
/// 小數位數上限（UN4；TipTop 只檢查不可小於 0）
pub const DECIMALS_MAX: i16 = 6;

/// 單位代碼：去頭尾空白、區分大小寫、1–4 字（UN3）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitCode(String);

impl UnitCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::UnitCodeEmpty);
        }
        if v.chars().count() > CODE_MAX {
            return Err(DomainError::UnitCodeTooLong { max: CODE_MAX });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 可修改的欄位：說明、小數位數
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitAttrs {
    pub name: Option<String>,
    pub decimals: i16,
}

impl UnitAttrs {
    /// 小數位數 0–6（:261-267，UN4）；說明最多 80 字
    pub fn new(name: Option<String>, decimals: Option<i16>) -> Result<Self, DomainError> {
        let name = name.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        if name.as_ref().is_some_and(|n| n.chars().count() > NAME_MAX) {
            return Err(DomainError::FieldTooLong {
                field: "說明",
                max: NAME_MAX,
            });
        }
        let decimals = decimals.unwrap_or(0);
        if !(0..=DECIMALS_MAX).contains(&decimals) {
            return Err(DomainError::UnitDecimalsInvalid { max: DECIMALS_MAX });
        }
        Ok(Self { name, decimals })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub code: UnitCode,
    pub attrs: UnitAttrs,
    /// gfeacti 資料有效碼
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；gfe_file 無此欄）
    pub confirmation: crate::confirmation::Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl Unit {
    /// 新增：資料有效預設 Y（:195-198）
    pub fn new(code: UnitCode, attrs: UnitAttrs, user: &str, dept: &str, today: NaiveDate) -> Self {
        Self {
            code,
            attrs,
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

    pub fn update(&mut self, attrs: UnitAttrs, user: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(user, today);
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製（ADR 0013 SA5；aooi101 原本沒有）：新單位代碼，說明與小數位數帶入；資料所有者為目前使用者、未確認
    pub fn copy_as(&self, code: UnitCode, user: &str, dept: &str, today: NaiveDate) -> Self {
        Self::new(code, self.attrs.clone(), user, dept, today)
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
    fn code_rules() {
        assert_eq!(UnitCode::parse(" pcs ").unwrap().as_str(), "pcs");
        assert_eq!(UnitCode::parse(""), Err(DomainError::UnitCodeEmpty));
        assert_eq!(
            UnitCode::parse("PIECE"),
            Err(DomainError::UnitCodeTooLong { max: 4 })
        );
    }

    #[test]
    fn decimals_range() {
        assert_eq!(UnitAttrs::new(None, None).unwrap().decimals, 0);
        assert!(UnitAttrs::new(None, Some(6)).is_ok());
        assert_eq!(
            UnitAttrs::new(None, Some(-1)),
            Err(DomainError::UnitDecimalsInvalid { max: 6 })
        );
        assert!(UnitAttrs::new(None, Some(7)).is_err());
    }
}
