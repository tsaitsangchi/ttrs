//! 專案（TipTop: gja_file 專案名稱檔；aooi200 專案名稱維護）
//! 規格：specs/features/aoo/project_maintenance.feature（PJ1–PJ6）
//!
//! 工廠層（PJ2）：每個營運中心的 schema 各一份。單筆（單頭）作業。

use chrono::NaiveDate;

use crate::{DomainError, confirmation::Confirmation, record::Stamp};

/// 專案編號最多 10 字（gja01 varchar2(10)，PJ5）
pub const CODE_MAX: usize = 10;
/// 專案名稱最多 80 字（gja02）
pub const NAME_MAX: usize = 80;

/// 專案編號：去頭尾空白、區分大小寫、1–10 字（PJ5）
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectCode(String);

impl ProjectCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::ProjectCodeEmpty);
        }
        if v.chars().count() > CODE_MAX {
            return Err(DomainError::ProjectCodeTooLong { max: CODE_MAX });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 可修改的欄位
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectAttrs {
    /// gja02 專案名稱
    pub name: Option<String>,
    /// gja03 專案起始日期
    pub start_on: Option<NaiveDate>,
    /// gja04 專案結束日期
    pub end_on: Option<NaiveDate>,
}

impl ProjectAttrs {
    /// 日期可空白；兩者都有值時結束不可早於起始（aooi200.4gl:318-336，apj-018，PJ3）
    pub fn new(
        name: Option<String>,
        start_on: Option<NaiveDate>,
        end_on: Option<NaiveDate>,
    ) -> Result<Self, DomainError> {
        let name = name.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
        crate::program::check_length("專案名稱", name.as_deref(), NAME_MAX)?;
        if let (Some(s), Some(e)) = (start_on, end_on)
            && e < s
        {
            return Err(DomainError::ProjectDateRangeInvalid);
        }
        Ok(Self {
            name,
            start_on,
            end_on,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub code: ProjectCode,
    pub attrs: ProjectAttrs,
    /// gjaacti 資料有效碼
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；gja_file 無此欄）
    pub confirmation: Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    pub stamp: Stamp,
}

impl Project {
    /// 新增：資料有效 Y、所有者、部門、日期（aooi200.4gl:255-262）
    pub fn new(
        code: ProjectCode,
        attrs: ProjectAttrs,
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

    pub fn update(&mut self, attrs: ProjectAttrs, user: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(user, today);
    }

    /// 失效切換（aooi200.4gl:540-550）
    pub fn set_active(&mut self, active: bool, user: &str, today: NaiveDate) {
        self.active = active;
        self.touch(user, today);
    }

    /// 複製（aooi200.4gl:600-680 reproduce）：新編號，其餘欄位帶入；資料所有者為目前使用者、未確認
    pub fn copy_as(&self, code: ProjectCode, user: &str, dept: &str, today: NaiveDate) -> Self {
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

    fn d(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn date_range() {
        assert!(ProjectAttrs::new(None, None, None).is_ok());
        assert!(ProjectAttrs::new(None, Some(d("2026-01-01")), Some(d("2026-01-01"))).is_ok());
        assert_eq!(
            ProjectAttrs::new(None, Some(d("2026-12-31")), Some(d("2026-01-01"))),
            Err(DomainError::ProjectDateRangeInvalid)
        );
    }

    #[test]
    fn code_rules() {
        assert_eq!(
            ProjectCode::parse("P2026ABCDEF"),
            Err(DomainError::ProjectCodeTooLong { max: 10 })
        );
    }
}
