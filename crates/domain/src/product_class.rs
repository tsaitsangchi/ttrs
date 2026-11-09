//! 產品分類（TipTop: oba_file 產品分類檔；axmi110 產品分類維護作業）
//! 規格：specs/features/axm/product_classification.feature（PC1–PC9）

use chrono::NaiveDate;

use crate::{department::DeptCode, error::DomainError, record::Stamp};

/// 欄位長度（oba_file.sch）
pub mod max_len {
    pub const CODE: usize = 10;
    pub const NAME: usize = 80;
    pub const PERIOD_FORMAT: usize = 1;
    pub const ACCOUNT: usize = 24;
}

/// 產品分類碼：去頭尾空白、區分大小寫、1–10 字（PC3）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProductClassCode(String);

impl ProductClassCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() || v.chars().count() > max_len::CODE {
            return Err(DomainError::ProductClassCodeInvalid { max: max_len::CODE });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 可修改的欄位
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProductClassAttrs {
    /// oba02 分類名稱
    pub name: Option<String>,
    /// oba08 計劃期別格式（PC6，代碼意義待 Key User 說明）
    pub period_format: Option<String>,
    /// oba09 期數（不可小於 0，axm-096）
    pub periods: Option<i32>,
    /// oba10 成本中心（有效、管理類別 1／2 的部門，PC4；檢查在 use case）
    pub cost_center: Option<DeptCode>,
    /// oba11 銷貨收入科目（會計科目未移植，不檢查，PC5）
    pub revenue_account: Option<String>,
    /// oba111 銷貨收入科目二
    pub revenue_account2: Option<String>,
}

fn text(v: Option<String>, field: &'static str, max: usize) -> Result<Option<String>, DomainError> {
    let v = v.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    if v.as_ref().is_some_and(|s| s.chars().count() > max) {
        return Err(DomainError::FieldTooLong { field, max });
    }
    Ok(v)
}

impl ProductClassAttrs {
    pub fn new(
        name: Option<String>,
        period_format: Option<String>,
        periods: Option<i32>,
        cost_center: Option<String>,
        revenue_account: Option<String>,
        revenue_account2: Option<String>,
    ) -> Result<Self, DomainError> {
        if periods.is_some_and(|p| p < 0) {
            return Err(DomainError::ProductClassPeriodsInvalid);
        }
        let cost_center = match text(cost_center, "成本中心", 10)? {
            Some(c) => {
                Some(DeptCode::parse(&c).map_err(|_| DomainError::ProductClassCostCenterInvalid)?)
            }
            None => None,
        };
        Ok(Self {
            name: text(name, "分類名稱", max_len::NAME)?,
            period_format: text(period_format, "計劃期別格式", max_len::PERIOD_FORMAT)?,
            periods,
            cost_center,
            revenue_account: text(revenue_account, "銷貨收入科目", max_len::ACCOUNT)?,
            revenue_account2: text(revenue_account2, "銷貨收入科目二", max_len::ACCOUNT)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductClass {
    pub code: ProductClassCode,
    pub attrs: ProductClassAttrs,
    /// 資料有效（TTRS 新增，PC7）
    pub active: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl ProductClass {
    pub fn new(
        code: ProductClassCode,
        attrs: ProductClassAttrs,
        user: &str,
        dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            code,
            attrs,
            active: true,
            created_by: user.to_owned(),
            created_dept: dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn update(&mut self, attrs: ProductClassAttrs, user: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(user, today);
    }

    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
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
        assert!(ProductClassCode::parse("P01").is_ok());
        assert!(ProductClassCode::parse("").is_err());
        assert!(ProductClassCode::parse("ABCDEFGHIJK").is_err());
        assert_eq!(
            ProductClassAttrs::new(None, None, Some(-1), None, None, None),
            Err(DomainError::ProductClassPeriodsInvalid)
        );
    }
}
