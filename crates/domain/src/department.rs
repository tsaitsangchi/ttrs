//! 部門（TipTop: aooi030 / gem_file）；工廠層，每廠的 schema 各一份（docs/adr/0007-multi-plant.md）
//! 規格：specs/features/aoo/department_maintenance.feature（DQ1–DQ10）
//!
//! 基本欄位（代號、簡稱、全稱、資料有效）與會計欄位（會計部門、費用類別、管理類別、對應成本中心）分開維護（DQ4）。

use chrono::NaiveDate;

use crate::record::Stamp;
use crate::{
    DomainError,
    lang::normalize_text,
    program::{check_length, coded_enum},
};

/// 欄位上限（gem_file.sch）
pub mod max_len {
    pub const CODE: usize = 10; // gem01
    pub const NAME: usize = 80; // gem02
    pub const FULL_NAME: usize = 80; // gem03
}

/// 部門編號（gem01）：必填、最多 10 碼、轉大寫；建立後不可修改（DQ2）
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeptCode(String);

impl DeptCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::DepartmentCodeEmpty);
        }
        if v.chars().count() > max_len::CODE {
            return Err(DomainError::DepartmentCodeTooLong { max: max_len::CODE });
        }
        Ok(Self(v.to_uppercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

coded_enum!(
    /// 費用類別（gem07）
    ExpenseType { Administration = "M", Production = "P", Research = "R", Sales = "S" }
);
coded_enum!(
    /// 管理類別（gem09）
    ControlType { CostCenter = "1", ProfitCenter = "2", Other = "3" }
);

/// 基本欄位：簡稱必填（DQ2）、全稱選填
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepartmentAttrs {
    pub name: String,
    pub full_name: Option<String>,
}

impl DepartmentAttrs {
    pub fn new(name: Option<String>, full_name: Option<String>) -> Result<Self, DomainError> {
        let name = normalize_text(name).ok_or(DomainError::DepartmentNameEmpty)?;
        let full_name = normalize_text(full_name);
        check_length("部門簡稱", Some(&name), max_len::NAME)?;
        check_length("部門全稱", full_name.as_deref(), max_len::FULL_NAME)?;
        Ok(Self { name, full_name })
    }
}

/// 會計欄位（aooi030「會計資料維護」）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accounting {
    pub accounting_dept: bool,
    pub expense_type: Option<ExpenseType>,
    pub control_type: ControlType,
    pub cost_center: Option<DeptCode>,
}

impl Accounting {
    /// 新增預設：會計部門 Y、管理類別 3（aooi030.4gl:301-304）
    pub fn defaults() -> Self {
        Self {
            accounting_dept: true,
            expense_type: None,
            control_type: ControlType::Other,
            cost_center: None,
        }
    }

    /// 費用類別只能是 M／P／R／S（:368-372）；管理類別 1、2 時對應成本中心必填，未給為本部門（:375-401），
    /// 3 時清空。對應成本中心是否為有效部門由應用層檢查（:402-420）
    pub fn new(
        own: &DeptCode,
        accounting_dept: bool,
        expense_type: Option<&str>,
        control_type: &str,
        cost_center: Option<&str>,
    ) -> Result<Self, DomainError> {
        let expense_type = match expense_type.map(str::trim).filter(|s| !s.is_empty()) {
            None => None,
            Some(v) => Some(
                ExpenseType::from_code(&v.to_uppercase())
                    .ok_or_else(|| DomainError::DepartmentExpenseTypeInvalid(v.to_owned()))?,
            ),
        };
        let control_type = ControlType::from_code(control_type.trim())
            .ok_or_else(|| DomainError::DepartmentControlTypeInvalid(control_type.to_owned()))?;
        let cost_center = match control_type {
            ControlType::Other => None,
            _ => Some(match cost_center.map(str::trim).filter(|s| !s.is_empty()) {
                None => own.clone(),
                Some(c) => DeptCode::parse(c)
                    .map_err(|_| DomainError::DepartmentCostCenterInvalid(c.to_owned()))?,
            }),
        };
        Ok(Self {
            accounting_dept,
            expense_type,
            control_type,
            cost_center,
        })
    }
}

/// 部門（gem_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Department {
    pub code: DeptCode,
    pub attrs: DepartmentAttrs,
    pub accounting: Accounting,
    pub active: bool,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本與建立、修改時間（ADR 0012 §1）
    pub stamp: Stamp,
}

impl Department {
    /// 新增：資料有效（:300）、會計欄位用預設；資料所有者／部門＝目前使用者
    pub fn new(
        code: DeptCode,
        attrs: DepartmentAttrs,
        actor: &str,
        actor_dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            code,
            attrs,
            accounting: Accounting::defaults(),
            active: true,
            created_by: actor.to_owned(),
            created_dept: actor_dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn update(&mut self, attrs: DepartmentAttrs, actor: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(actor, today);
    }

    pub fn set_accounting(&mut self, accounting: Accounting, actor: &str, today: NaiveDate) {
        self.accounting = accounting;
        self.touch(actor, today);
    }

    pub fn set_active(&mut self, active: bool, actor: &str, today: NaiveDate) {
        self.active = active;
        self.touch(actor, today);
    }

    fn touch(&mut self, actor: &str, today: NaiveDate) {
        self.updated_by = Some(actor.to_owned());
        self.updated_on = Some(today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_rules() {
        assert_eq!(DeptCode::parse(" sal ").unwrap().as_str(), "SAL");
        assert_eq!(
            DeptCode::parse("").unwrap_err().code(),
            "DEPARTMENT_CODE_EMPTY"
        );
        assert_eq!(
            DeptCode::parse("DEPT0123456").unwrap_err().code(),
            "DEPARTMENT_CODE_TOO_LONG"
        );
    }

    #[test]
    fn accounting_rules() {
        let own = DeptCode::parse("SAL").unwrap();
        let a = Accounting::new(&own, true, Some("m"), "1", None).unwrap();
        assert_eq!(a.expense_type, Some(ExpenseType::Administration));
        assert_eq!(a.cost_center, Some(own.clone()));
        let a = Accounting::new(&own, true, None, "3", Some("MFG")).unwrap();
        assert_eq!(a.cost_center, None);
        assert_eq!(
            Accounting::new(&own, true, Some("X"), "3", None)
                .unwrap_err()
                .code(),
            "DEPARTMENT_EXPENSE_TYPE_INVALID"
        );
    }
}
