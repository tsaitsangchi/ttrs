//! 幣別（TipTop: azi_file 幣別檔；aooi050 幣別資料維護）
//! 規格：specs/features/aoo/currency_maintenance.feature（CY1–CY8）
//!
//! 共用 schema（CY2）。小數位數照程式檢查（aooi050.4gl:250-270），不照 .sch 註解。

use chrono::NaiveDate;

use crate::{DomainError, confirmation::Confirmation, program::coded_enum, record::Stamp};

/// 幣別代碼最多 4 字（azi01 varchar2(4)，CY5）
pub const CODE_MAX: usize = 4;
/// 說明最多 80 字（azi02）
pub const NAME_MAX: usize = 80;
/// 單價、金額、小計總計小數位數上限（aooi050.4gl:250-265）
pub const DECIMALS_MAX: i16 = 6;
/// 匯率小數位數上限（aooi050.4gl:266-270）
pub const RATE_DECIMALS_MAX: i16 = 10;

coded_enum!(
    /// 計算方式（azi10；aooi050.4fd：1 乘 Multiplication、2 除 Division）
    RateMethod { Multiply = "1", Divide = "2" }
);

impl RateMethod {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        Self::from_code(raw.trim())
            .ok_or_else(|| DomainError::CurrencyRateMethodInvalid(raw.trim().to_owned()))
    }
}

/// 幣別代碼：去頭尾空白、區分大小寫、1–4 字（CY5）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CurrencyCode(String);

impl CurrencyCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::CurrencyCodeEmpty);
        }
        if v.chars().count() > CODE_MAX {
            return Err(DomainError::CurrencyCodeTooLong { max: CODE_MAX });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 可修改的欄位
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrencyAttrs {
    /// azi02 幣別說明
    pub name: Option<String>,
    /// azi03 單價（成本）小數位數
    pub price_decimals: i16,
    /// azi04 金額小數位數
    pub amount_decimals: i16,
    /// azi05 小計、總計小數位數
    pub total_decimals: i16,
    /// azi07 匯率小數位數
    pub rate_decimals: i16,
    /// azi10 計算方式
    pub rate_method: RateMethod,
}

/// 新增時的預設值（CY3、CY4；TipTop 無預設值）
pub mod defaults {
    pub const PRICE_DECIMALS: i16 = 4;
    pub const AMOUNT_DECIMALS: i16 = 2;
    pub const TOTAL_DECIMALS: i16 = 2;
    pub const RATE_DECIMALS: i16 = 6;
}

impl CurrencyAttrs {
    /// 空白的欄位用預設值（CY3、CY4）；單價、金額、小計總計 0–6，匯率 0–10
    pub fn new(
        name: Option<String>,
        price_decimals: Option<i16>,
        amount_decimals: Option<i16>,
        total_decimals: Option<i16>,
        rate_decimals: Option<i16>,
        rate_method: Option<&str>,
    ) -> Result<Self, DomainError> {
        let name = name.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        if name.as_ref().is_some_and(|n| n.chars().count() > NAME_MAX) {
            return Err(DomainError::FieldTooLong {
                field: "說明",
                max: NAME_MAX,
            });
        }
        let check = |field: &'static str, v: i16, max: i16| {
            if (0..=max).contains(&v) {
                Ok(v)
            } else {
                Err(DomainError::CurrencyDecimalsInvalid { field, max })
            }
        };
        let rate_method = match rate_method.map(str::trim).filter(|s| !s.is_empty()) {
            Some(m) => RateMethod::parse(m)?,
            None => RateMethod::Multiply,
        };
        Ok(Self {
            name,
            price_decimals: check(
                "單價小數位數",
                price_decimals.unwrap_or(defaults::PRICE_DECIMALS),
                DECIMALS_MAX,
            )?,
            amount_decimals: check(
                "金額小數位數",
                amount_decimals.unwrap_or(defaults::AMOUNT_DECIMALS),
                DECIMALS_MAX,
            )?,
            total_decimals: check(
                "小計總計小數位數",
                total_decimals.unwrap_or(defaults::TOTAL_DECIMALS),
                DECIMALS_MAX,
            )?,
            rate_decimals: check(
                "匯率小數位數",
                rate_decimals.unwrap_or(defaults::RATE_DECIMALS),
                RATE_DECIMALS_MAX,
            )?,
            rate_method,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Currency {
    pub code: CurrencyCode,
    pub attrs: CurrencyAttrs,
    /// aziacti 資料有效碼
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；azi_file 無此欄）
    pub confirmation: Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl Currency {
    /// 新增：資料有效預設 Y、記錄建立者與日期（aooi050.4gl:222-224）
    pub fn new(
        code: CurrencyCode,
        attrs: CurrencyAttrs,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            code,
            attrs,
            active: true,
            confirmation: Confirmation::default(),
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn update(&mut self, attrs: CurrencyAttrs, user: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(user, today);
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製（ADR 0013 SA5）：新幣別代碼，其餘欄位帶入；資料所有者為目前使用者、未確認
    pub fn copy_as(&self, code: CurrencyCode, user: &str, dept: &str, today: NaiveDate) -> Self {
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
    fn defaults_and_ranges() {
        let a = CurrencyAttrs::new(None, None, None, None, None, None).unwrap();
        assert_eq!(
            (
                a.price_decimals,
                a.amount_decimals,
                a.total_decimals,
                a.rate_decimals
            ),
            (4, 2, 2, 6)
        );
        assert_eq!(a.rate_method, RateMethod::Multiply);
        assert!(CurrencyAttrs::new(None, Some(6), Some(0), Some(6), Some(10), Some("2")).is_ok());
        assert!(CurrencyAttrs::new(None, Some(7), None, None, None, None).is_err());
        assert!(CurrencyAttrs::new(None, None, Some(-1), None, None, None).is_err());
        assert!(CurrencyAttrs::new(None, None, None, None, Some(11), None).is_err());
        assert_eq!(
            CurrencyAttrs::new(None, None, None, None, None, Some("3")),
            Err(DomainError::CurrencyRateMethodInvalid("3".into()))
        );
    }

    #[test]
    fn code_rules() {
        assert_eq!(CurrencyCode::parse(" USD ").unwrap().as_str(), "USD");
        assert_eq!(
            CurrencyCode::parse("EURO1"),
            Err(DomainError::CurrencyCodeTooLong { max: 4 })
        );
    }
}
