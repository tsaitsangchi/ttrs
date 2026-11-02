//! User Special Permission 使用者特殊權限（TipTop: p_zxw / zxw_file）
//! 規格：specs/features/azz/user_permission_maintenance.feature（X1–X11）；權限框架：docs/adr/0006 §4
//!
//! 使用者有特殊權限時只用特殊權限，使用者維護的權限類別不再使用（除非也列在特殊權限中，cl_set_priv）。
//! 多列給同一程式時的合併規則屬 Authorizer（X8），不在此。

use chrono::NaiveDate;

use crate::{
    permission::PermissionAttrs, permission_class::ClassCode, program::ProgramNo, user::UserId,
};

/// 一列特殊權限的對象（zxw03 權限分類：1 權限類別、2 程式編號）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialTarget {
    /// 取得該權限類別的程式執行權限（zy_file）；本列不帶功能與資料權限（TQC-5A0094）
    Class(ClassCode),
    /// 本列自帶允許執行功能與資料權限（zxw05–zxw08）
    Program {
        program: ProgramNo,
        attrs: PermissionAttrs,
    },
}

impl SpecialTarget {
    /// TipTop 代碼（zxw03）
    pub fn kind_code(&self) -> &'static str {
        match self {
            Self::Class(_) => "1",
            Self::Program { .. } => "2",
        }
    }

    pub fn class(&self) -> Option<&ClassCode> {
        match self {
            Self::Class(c) => Some(c),
            Self::Program { .. } => None,
        }
    }

    pub fn program(&self) -> Option<&ProgramNo> {
        match self {
            Self::Class(_) => None,
            Self::Program { program, .. } => Some(program),
        }
    }
}

/// 使用者特殊權限（zxw_file：使用者＋項次一筆）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserPermission {
    pub user_id: UserId,
    /// zxw02 項次：自動給號（max + 1），只決定顯示順序（X3）
    pub line_no: i32,
    pub target: SpecialTarget,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
}

impl UserPermission {
    pub fn new(
        user_id: UserId,
        line_no: i32,
        target: SpecialTarget,
        actor: &str,
        actor_dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            user_id,
            line_no,
            target,
            created_by: actor.to_owned(),
            created_dept: actor_dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
        }
    }

    /// 修改程式列的功能與資料權限；權限類別列沒有可修改的值，回傳 false
    pub fn update_program(
        &mut self,
        new_attrs: PermissionAttrs,
        actor: &str,
        today: NaiveDate,
    ) -> bool {
        let SpecialTarget::Program { attrs, .. } = &mut self.target else {
            return false;
        };
        *attrs = new_attrs;
        self.touch(actor, today);
        true
    }

    /// 使用者改權限類別時，原權限類別列改為新權限類別（p_zx.4gl:1003-1018，X6）
    pub fn move_class(&mut self, class: ClassCode, actor: &str, today: NaiveDate) {
        self.target = SpecialTarget::Class(class);
        self.touch(actor, today);
    }

    /// 複製到另一個使用者（p_zxw.4gl:1167-1237）：項次與內容不變，資料所有者為目前使用者
    pub fn copy_to(
        &self,
        user_id: UserId,
        actor: &str,
        actor_dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self::new(
            user_id,
            self.line_no,
            self.target.clone(),
            actor,
            actor_dept,
            today,
        )
    }

    fn touch(&mut self, actor: &str, today: NaiveDate) {
        self.updated_by = Some(actor.to_owned());
        self.updated_on = Some(today);
    }
}

/// 下一個項次（p_zxw.4gl:660-667）
pub fn next_line_no(existing: &[UserPermission]) -> i32 {
    existing.iter().map(|p| p.line_no).max().unwrap_or(0) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()
    }

    fn class_row(line: i32, class: &str) -> UserPermission {
        UserPermission::new(
            UserId::parse("u001").unwrap(),
            line,
            SpecialTarget::Class(ClassCode::parse(class).unwrap()),
            "tiptop",
            "MIS",
            date(),
        )
    }

    #[test]
    fn line_numbers_follow_the_highest() {
        assert_eq!(next_line_no(&[]), 1);
        assert_eq!(next_line_no(&[class_row(1, "A"), class_row(5, "B")]), 6);
    }

    #[test]
    fn class_rows_have_nothing_to_update() {
        let mut row = class_row(1, "A");
        assert!(!row.update_program(PermissionAttrs::defaults(&[]), "tiptop", date()));
        assert_eq!(row.updated_by, None);
        row.move_class(ClassCode::parse("B").unwrap(), "tiptop", date());
        assert_eq!(row.target.class().map(ClassCode::as_str), Some("B"));
        assert_eq!(row.target.kind_code(), "1");
    }
}
