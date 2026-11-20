//! 整體管理系統參數（TipTop: aza_file 整體管理系統參數檔；aoos010 整體管理系統參數設定）
//! 規格：specs/features/aoo/general_parameters.feature（GP1–GP10）
//!
//! 全公司一份、放共用 schema（GP2）。只移植會計期間 aza02、本國幣別 aza17、匯率採用 aza19（GP3）；
//! 其他參數等用到的模組移植時再加（GP9）。

use chrono::NaiveDate;

use crate::{DomainError, currency::CurrencyCode, program::coded_enum, record::Stamp};

coded_enum!(
    /// 匯率採用（aza19：1 月平均匯率 → monthly、2 每日匯率 → daily；GP6）
    RateBasis { Monthly = "monthly", Daily = "daily" }
);

impl RateBasis {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        Self::from_code(raw.trim())
            .ok_or_else(|| DomainError::RateBasisInvalid(raw.trim().to_owned()))
    }
}

/// 會計期間（aza02：1 → 12 期、2 → 13 期；GP4）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountingPeriods(i16);

impl AccountingPeriods {
    pub const TWELVE: Self = Self(12);
    pub const THIRTEEN: Self = Self(13);

    pub fn parse(v: i16) -> Result<Self, DomainError> {
        match v {
            12 | 13 => Ok(Self(v)),
            _ => Err(DomainError::AccountingPeriodsInvalid(v)),
        }
    }

    pub fn get(self) -> i16 {
        self.0
    }
}

/// 單別位數（aza41：1 → 3 碼、2 → 4 碼、3 → 5 碼；DT5(a)）
pub fn parse_doc_code_length(v: i16) -> Result<u8, DomainError> {
    match v {
        3..=5 => Ok(v as u8),
        _ => Err(DomainError::DocCodeLengthInvalid),
    }
}

/// 單號位數（aza42：1 → 流水號 8 碼、2 → 9 碼、3 → 10 碼；misc_transaction.feature MT4(a)）
pub fn parse_doc_serial_length(v: i16) -> Result<u8, DomainError> {
    match v {
        8..=10 => Ok(v as u8),
        _ => Err(DomainError::DocSerialLengthInvalid),
    }
}

/// 整體管理系統參數（只有一筆，aza01＝'0'）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralParameters {
    pub accounting_periods: AccountingPeriods,
    /// aza17 本國幣別
    pub base_currency: CurrencyCode,
    pub rate_basis: RateBasis,
    /// aza28 料件編號自動編碼否（item_auto_number.feature AN1）
    pub auto_item_number: bool,
    /// aza41 單別位數：3、4、5 碼（TipTop 1／2／3；document_type.feature DT5(a)）
    pub doc_code_length: u8,
    /// aza42 單號流水號位數：8、9、10 碼（TipTop 1／2／3；misc_transaction.feature MT4(a)）
    pub doc_serial_length: u8,
    /// azauser 資料所有者
    pub created_by: String,
    /// azagrup 資料所有群
    pub created_dept: String,
    pub created_on: NaiveDate,
    /// azamodu 資料修改者
    pub updated_by: Option<String>,
    /// azadate 最近修改日
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl GeneralParameters {
    /// 沒有資料時的預設值（aoos010.4gl:251-342：會計期間 12 期、本國幣別 TWD、匯率採用每日）
    pub fn defaults(user: &str, dept: &str, today: NaiveDate) -> Self {
        Self {
            accounting_periods: AccountingPeriods::TWELVE,
            base_currency: CurrencyCode::parse("TWD").expect("TWD"),
            rate_basis: RateBasis::Daily,
            auto_item_number: false,
            doc_code_length: 3,
            doc_serial_length: 8,
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    /// 修改（aoos010.4gl:614-615 記錄修改者與日期）；本國幣別的檢查在 use case（需查幣別與匯率）
    pub fn update(
        &mut self,
        accounting_periods: AccountingPeriods,
        base_currency: CurrencyCode,
        rate_basis: RateBasis,
        user: &str,
        today: NaiveDate,
    ) {
        self.accounting_periods = accounting_periods;
        self.base_currency = base_currency;
        self.rate_basis = rate_basis;
        self.updated_by = Some(user.to_owned());
        self.updated_on = Some(today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_are_12_or_13() {
        assert_eq!(AccountingPeriods::parse(12).unwrap().get(), 12);
        assert_eq!(AccountingPeriods::parse(13).unwrap().get(), 13);
        assert_eq!(
            AccountingPeriods::parse(14).unwrap_err().code(),
            "ACCOUNTING_PERIODS_INVALID"
        );
    }

    #[test]
    fn rate_basis_codes() {
        assert_eq!(RateBasis::parse(" monthly ").unwrap(), RateBasis::Monthly);
        assert_eq!(RateBasis::Daily.code(), "daily");
        assert_eq!(
            RateBasis::parse("1").unwrap_err().code(),
            "RATE_BASIS_INVALID"
        );
    }

    #[test]
    fn defaults_follow_tiptop() {
        let d = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
        let p = GeneralParameters::defaults("tiptop", "", d);
        assert_eq!(p.accounting_periods, AccountingPeriods::TWELVE);
        assert_eq!(p.base_currency.as_str(), "TWD");
        assert_eq!(p.rate_basis, RateBasis::Daily);
    }
}
