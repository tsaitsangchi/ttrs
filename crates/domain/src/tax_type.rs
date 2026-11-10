//! 稅別（TipTop: gec_file 稅別檔；aooi150 稅別資料維護）。主鍵為稅別碼 gec01＋進／銷項 gec011
//! 規格：specs/features/aoo/tax_type_maintenance.feature（TX1–TX9）
//!
//! 共用 schema（TX2）。只做台灣版（TX3）：大陸版（aza26＝2）的發票種類不移植。

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::{DomainError, confirmation::Confirmation, program::coded_enum, record::Stamp};

/// 稅別碼最多 4 字（gec01 varchar2(4)，TX8）
pub const CODE_MAX: usize = 4;
/// 名稱最多 80 字（gec02）
pub const NAME_MAX: usize = 80;
/// 會計科目最多 24 字（gec03、gec031）
pub const ACCOUNT_MAX: usize = 24;
/// 稅率上限（%，TX5；gec04 number(9,4)）
pub const RATE_MAX: Decimal = Decimal::ONE_HUNDRED;
/// 稅率小數位數（gec04 number(9,4)）
pub const RATE_SCALE: u32 = 4;

coded_enum!(
    /// 進／銷項（gec011，aooi150.4gl:312-317：只能 1、2）
    TaxDirection { Purchase = "1", Sales = "2" }
);

coded_enum!(
    /// 發票聯數（gec05，台灣版 aooi150.4gl:389-394）：0 園區收據、2 二聯、3 三聯、4 收據、X 不申報
    InvoiceCopies { ParkReceipt = "0", Duplicate = "2", Triplicate = "3", Receipt = "4", NotFiled = "X" }
);

coded_enum!(
    /// 課稅別（gec06，aooi150.4gl:398-407）：1 應稅、2 零稅率、3 免稅
    TaxClass { Taxable = "1", ZeroRated = "2", Exempt = "3" }
);

impl TaxDirection {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        Self::from_code(raw.trim())
            .ok_or_else(|| DomainError::TaxDirectionInvalid(raw.trim().to_owned()))
    }

    /// 可用的媒體申報格式（gec08，aooi150.4gl:423-447）：進項 21–29、銷項 31–38，另有 XX 不申報
    pub fn allows_filing_format(self, format: &str) -> bool {
        let range = match self {
            Self::Purchase => 21..=29,
            Self::Sales => 31..=38,
        };
        format == FILING_NONE || format.parse::<u8>().is_ok_and(|n| range.contains(&n))
    }
}

/// 不申報的媒體申報格式；新增預設（TX6）
pub const FILING_NONE: &str = "XX";
/// 二聯式發票單價可不含稅的申報格式（aooi150.4gl:415）
const FILING_TWO_COPY_EXEMPT: &str = "28";

/// 稅別碼：去頭尾空白、區分大小寫、1–4 字（TX8）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaxCode(String);

impl TaxCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::TaxCodeEmpty);
        }
        if v.chars().count() > CODE_MAX {
            return Err(DomainError::TaxCodeTooLong { max: CODE_MAX });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 稅別的鍵值：稅別碼＋進／銷項
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaxKey {
    pub code: TaxCode,
    pub direction: TaxDirection,
}

impl PartialOrd for TaxDirection {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaxDirection {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.code().cmp(other.code())
    }
}

impl std::hash::Hash for TaxDirection {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.code().hash(state);
    }
}

/// 輸入的原始值（尚未檢查）
#[derive(Debug, Clone, Default)]
pub struct TaxTypeFields {
    pub name: Option<String>,
    pub account: Option<String>,
    pub account2: Option<String>,
    pub invoice_copies: Option<String>,
    pub tax_class: Option<String>,
    pub rate: Option<Decimal>,
    pub include_tax: Option<bool>,
    pub filing_format: Option<String>,
}

/// 可修改的欄位
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxTypeAttrs {
    /// gec02 稅別名稱
    pub name: Option<String>,
    /// gec03 會計科目（主帳套）；會計科目尚未移植，先存代碼不檢查（TX4）
    pub account: Option<String>,
    /// gec031 會計科目二（第二帳套）
    pub account2: Option<String>,
    /// gec05 發票聯數
    pub invoice_copies: Option<InvoiceCopies>,
    /// gec06 課稅別
    pub tax_class: Option<TaxClass>,
    /// gec04 稅率（%）
    pub rate: Decimal,
    /// gec07 單價含稅否
    pub include_tax: bool,
    /// gec08 媒體申報格式
    pub filing_format: String,
}

impl TaxTypeAttrs {
    /// 檢查規則（aooi150.4gl:334-447、:1000-1012）
    pub fn new(direction: TaxDirection, f: TaxTypeFields) -> Result<Self, DomainError> {
        let clean = |v: Option<String>| v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        let name = clean(f.name);
        crate::program::check_length("名稱", name.as_deref(), NAME_MAX)?;
        let account = clean(f.account);
        crate::program::check_length("會計科目", account.as_deref(), ACCOUNT_MAX)?;
        let account2 = clean(f.account2);
        crate::program::check_length("會計科目二", account2.as_deref(), ACCOUNT_MAX)?;
        let invoice_copies = match clean(f.invoice_copies) {
            Some(v) => Some(
                InvoiceCopies::from_code(&v.to_uppercase())
                    .ok_or(DomainError::TaxInvoiceCopiesInvalid(v))?,
            ),
            None => None,
        };
        let tax_class = match clean(f.tax_class) {
            Some(v) => Some(TaxClass::from_code(&v).ok_or(DomainError::TaxClassInvalid(v))?),
            None => None,
        };
        let mut rate = f.rate.unwrap_or_default();
        if rate < Decimal::ZERO || rate > RATE_MAX || rate.round_dp(RATE_SCALE) != rate {
            return Err(DomainError::TaxRateInvalid);
        }
        let mut include_tax = f.include_tax.unwrap_or(false);
        // 零稅率、免稅：稅率為 0、不含稅（:1000-1012）
        if matches!(tax_class, Some(TaxClass::ZeroRated | TaxClass::Exempt)) {
            rate = Decimal::ZERO;
            include_tax = false;
        }
        // 稅率不為 0 時會計科目必填（:365-371）
        if !rate.is_zero() && account.is_none() {
            return Err(DomainError::TaxAccountRequired);
        }
        let filing_format = clean(f.filing_format)
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| FILING_NONE.to_owned());
        if !direction.allows_filing_format(&filing_format) {
            return Err(DomainError::TaxFilingFormatInvalid(filing_format));
        }
        // 二聯式且申報格式不是 28 時單價須含稅（:409-421，mfg2703）
        if invoice_copies == Some(InvoiceCopies::Duplicate)
            && filing_format != FILING_TWO_COPY_EXEMPT
            && !include_tax
        {
            return Err(DomainError::TaxTwoCopyMustIncludeTax);
        }
        Ok(Self {
            name,
            account,
            account2,
            invoice_copies,
            tax_class,
            rate: rate.normalize(),
            include_tax,
            filing_format,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxType {
    pub key: TaxKey,
    pub attrs: TaxTypeAttrs,
    /// gecacti 資料有效碼
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；gec_file 無此欄）
    pub confirmation: Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl TaxType {
    /// 新增：資料有效預設 Y、單價含稅否預設 N（aooi150.4gl:260-273）
    pub fn new(key: TaxKey, attrs: TaxTypeAttrs, user: &str, dept: &str, today: NaiveDate) -> Self {
        Self {
            key,
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

    pub fn update(&mut self, attrs: TaxTypeAttrs, user: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(user, today);
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製（ADR 0013 SA5）：新稅別碼與進／銷項，其餘欄位帶入；須符合新進／銷項的規則
    pub fn copy_as(
        &self,
        key: TaxKey,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Result<Self, DomainError> {
        let a = &self.attrs;
        let attrs = TaxTypeAttrs::new(
            key.direction,
            TaxTypeFields {
                name: a.name.clone(),
                account: a.account.clone(),
                account2: a.account2.clone(),
                invoice_copies: a.invoice_copies.map(|c| c.code().to_owned()),
                tax_class: a.tax_class.map(|c| c.code().to_owned()),
                rate: Some(a.rate),
                include_tax: Some(a.include_tax),
                filing_format: Some(a.filing_format.clone()),
            },
        )?;
        Ok(Self::new(key, attrs, user, dept, today))
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

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    fn sales(f: TaxTypeFields) -> Result<TaxTypeAttrs, DomainError> {
        TaxTypeAttrs::new(TaxDirection::Sales, f)
    }

    #[test]
    fn rules() {
        // 稅率不為 0 時會計科目必填
        assert_eq!(
            sales(TaxTypeFields {
                rate: Some(dec("5")),
                ..Default::default()
            }),
            Err(DomainError::TaxAccountRequired)
        );
        // 零稅率：稅率 0、不含稅
        let a = sales(TaxTypeFields {
            tax_class: Some("2".into()),
            rate: Some(dec("5")),
            include_tax: Some(true),
            ..Default::default()
        })
        .unwrap();
        assert!(a.rate.is_zero() && !a.include_tax);
        assert_eq!(a.filing_format, "XX");
        // 申報格式依進銷項
        assert!(TaxDirection::Purchase.allows_filing_format("29"));
        assert!(!TaxDirection::Purchase.allows_filing_format("31"));
        assert!(TaxDirection::Sales.allows_filing_format("38"));
        // 二聯式須含稅
        assert_eq!(
            sales(TaxTypeFields {
                invoice_copies: Some("2".into()),
                filing_format: Some("32".into()),
                ..Default::default()
            }),
            Err(DomainError::TaxTwoCopyMustIncludeTax)
        );
        assert_eq!(
            sales(TaxTypeFields {
                rate: Some(dec("101")),
                account: Some("2194".into()),
                ..Default::default()
            }),
            Err(DomainError::TaxRateInvalid)
        );
        assert!(
            sales(TaxTypeFields {
                invoice_copies: Some("1".into()),
                ..Default::default()
            })
            .is_err()
        );
    }
}
