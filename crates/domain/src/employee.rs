//! 員工（TipTop: aooi040 / gen_file）；工廠層，每廠的 schema 各一份（docs/adr/0007-multi-plant.md）
//! 規格：specs/features/aoo/employee_maintenance.feature（EQ1–EQ9）

use chrono::NaiveDate;

use crate::record::Stamp;
use crate::{
    DomainError, department::DeptCode, lang::normalize_text, program::check_length, user::is_email,
};

/// 欄位上限（gen_file.sch）
pub mod max_len {
    pub const ID: usize = 10; // gen01
    pub const NAME: usize = 40; // gen02
    pub const TITLE: usize = 80; // gen04
    pub const EXTENSION: usize = 5; // gen05
    pub const EMAIL: usize = 80; // gen06
}

/// 員工編號（gen01）：必填、最多 10 碼、轉大寫；建立後不可修改（EQ2）
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EmployeeId(String);

impl EmployeeId {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::EmployeeIdEmpty);
        }
        if v.chars().count() > max_len::ID {
            return Err(DomainError::EmployeeIdTooLong { max: max_len::ID });
        }
        Ok(Self(v.to_uppercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 可由畫面輸入的欄位：姓名必填、E-mail 檢查格式（EQ2）；部門是否存在且有效由應用層檢查（aooi040.4gl:608-622）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmployeeAttrs {
    pub name: String,
    pub dept_code: Option<DeptCode>,
    pub title: Option<String>,
    pub extension: Option<String>,
    pub email: Option<String>,
}

impl EmployeeAttrs {
    pub fn new(
        name: Option<String>,
        dept_code: Option<&str>,
        title: Option<String>,
        extension: Option<String>,
        email: Option<String>,
    ) -> Result<Self, DomainError> {
        let name = normalize_text(name).ok_or(DomainError::EmployeeNameEmpty)?;
        let dept_code = match dept_code.map(str::trim).filter(|s| !s.is_empty()) {
            None => None,
            Some(d) => Some(DeptCode::parse(d).map_err(|_| DomainError::DepartmentNotFound)?),
        };
        let attrs = Self {
            name,
            dept_code,
            title: normalize_text(title),
            extension: normalize_text(extension),
            email: normalize_text(email),
        };
        check_length("員工姓名", Some(&attrs.name), max_len::NAME)?;
        check_length("職稱", attrs.title.as_deref(), max_len::TITLE)?;
        check_length("分機", attrs.extension.as_deref(), max_len::EXTENSION)?;
        check_length("E-mail", attrs.email.as_deref(), max_len::EMAIL)?;
        if let Some(e) = &attrs.email
            && !is_email(e)
        {
            return Err(DomainError::EmployeeEmailInvalid(e.clone()));
        }
        Ok(attrs)
    }
}

/// 員工（gen_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Employee {
    pub id: EmployeeId,
    pub attrs: EmployeeAttrs,
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；gen_file 無此欄）
    pub confirmation: crate::confirmation::Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本與建立、修改時間（ADR 0012 §1）
    pub stamp: Stamp,
}

impl Employee {
    /// 新增：資料有效（aooi040.4gl:254）；資料所有者／部門＝目前使用者
    pub fn new(
        id: EmployeeId,
        attrs: EmployeeAttrs,
        actor: &str,
        actor_dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            id,
            attrs,
            active: true,
            confirmation: crate::confirmation::Confirmation::default(),
            created_by: actor.to_owned(),
            created_dept: actor_dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
            stamp: Stamp::default(),
        }
    }

    pub fn update(&mut self, attrs: EmployeeAttrs, actor: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(actor, today);
    }

    pub fn set_active(&mut self, active: bool, actor: &str, today: NaiveDate) {
        self.active = active;
        self.touch(actor, today);
    }

    /// 複製（ADR 0013 SA5；aooi040 原本沒有）：新員工編號，其餘欄位帶入；資料所有者為目前使用者、未確認
    pub fn copy_as(&self, id: EmployeeId, actor: &str, actor_dept: &str, today: NaiveDate) -> Self {
        Self::new(id, self.attrs.clone(), actor, actor_dept, today)
    }

    /// 確認／取消確認（ADR 0013 SA9）
    pub fn set_confirmed(
        &mut self,
        confirm: bool,
        actor: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), crate::DomainError> {
        if confirm {
            self.confirmation.confirm(actor, now)?;
        } else {
            self.confirmation.unconfirm()?;
        }
        self.touch(actor, now.date_naive());
        Ok(())
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
    fn id_and_email_rules() {
        assert_eq!(EmployeeId::parse(" e001 ").unwrap().as_str(), "E001");
        assert_eq!(
            EmployeeId::parse("EMP01234567").unwrap_err().code(),
            "EMPLOYEE_ID_TOO_LONG"
        );
        let err =
            EmployeeAttrs::new(Some("王".into()), None, None, None, Some("x".into())).unwrap_err();
        assert_eq!(err.code(), "EMPLOYEE_EMAIL_INVALID");
    }
}
