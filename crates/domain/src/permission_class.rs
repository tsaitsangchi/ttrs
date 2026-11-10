//! Permission Class Maintenance 權限類別（TipTop: p_zw / zw_file）
//! 規格：specs/features/azz/permission_class_maintenance.feature（W1–W11）；權限框架：docs/adr/0006
//!
//! 閒置設定只在此維護；執行期的閒置處理（程式 zz28 → 權限類別 zw04 → 系統 aza33）
//! 由網頁外框與登入連線負責（cl_user.4gl:355-380）。

use chrono::NaiveDate;

use crate::{
    DomainError,
    lang::normalize_text,
    program::{IdleAction, check_length, coded_enum},
};

/// 欄位上限（TipTop zw_file.sch；與 ttrs_shared::permission_class_max_len 一致）
pub mod max_len {
    pub const CODE: usize = 10; // zw01
    pub const NAME: usize = 80; // zw02
}

/// 閒置秒數範圍（zw06 number(5)）
pub const IDLE_SECONDS_MAX: i32 = 99_999;

coded_enum!(
    /// 閒置控制（zw04）：「依權限類別」才使用本類別的閒置處理與秒數
    ClassIdleControl { Off = "1", ByClass = "2", BySystem = "3" }
);

/// 權限類別代號（zw01）：必填、最多 10 碼；建立後不可修改（W1）
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClassCode(String);

impl ClassCode {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        let v = raw.trim();
        if v.is_empty() {
            return Err(DomainError::PermissionClassCodeEmpty);
        }
        if v.chars().count() > max_len::CODE {
            return Err(DomainError::PermissionClassCodeTooLong { max: max_len::CODE });
        }
        Ok(Self(v.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 可由畫面輸入的欄位
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionClassAttrs {
    pub name: String,
    /// zw03 預設選單（程式代號；是否為選單程式由 app 層查證）
    pub default_menu: Option<String>,
    pub idle_control: ClassIdleControl,
    pub idle_action: Option<IdleAction>,
    pub idle_seconds: Option<i32>,
}

impl PermissionClassAttrs {
    /// 說明必填（W2）；只有「依權限類別」保留閒置處理與秒數且兩者必填（W5，p_zw.4gl:503-509）
    pub fn new(
        name: Option<String>,
        default_menu: Option<String>,
        idle_control: ClassIdleControl,
        idle_action: Option<IdleAction>,
        idle_seconds: Option<i32>,
    ) -> Result<Self, DomainError> {
        let name = normalize_text(name).ok_or(DomainError::PermissionClassNameEmpty)?;
        check_length("權限說明", Some(&name), max_len::NAME)?;
        let (idle_action, idle_seconds) = if idle_control == ClassIdleControl::ByClass {
            let (Some(action), Some(seconds)) = (idle_action, idle_seconds) else {
                return Err(DomainError::IdleSettingRequired);
            };
            if !(1..=IDLE_SECONDS_MAX).contains(&seconds) {
                return Err(DomainError::IdleSecondsInvalid {
                    max: IDLE_SECONDS_MAX,
                });
            }
            (Some(action), Some(seconds))
        } else {
            (None, None)
        };
        Ok(Self {
            name,
            default_menu: normalize_text(default_menu),
            idle_control,
            idle_action,
            idle_seconds,
        })
    }
}

/// 權限類別（zw_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionClass {
    pub code: ClassCode,
    pub attrs: PermissionClassAttrs,
    /// zwacti 資料有效
    pub active: bool,
    /// 確認狀態（TTRS 標準確認欄位，ADR 0013 SA9；zw_file 無此欄）
    pub confirmation: crate::confirmation::Confirmation,
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
    /// 版本與建立、修改時間（ADR 0012 §1）
    pub stamp: crate::record::Stamp,
}

impl PermissionClass {
    /// 新增：資料有效；資料所有者／部門＝目前使用者（p_zw.4gl:434-449；W10）
    pub fn new(
        code: ClassCode,
        attrs: PermissionClassAttrs,
        actor: &str,
        actor_dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            stamp: crate::record::Stamp::default(),
            code,
            attrs,
            active: true,
            confirmation: crate::confirmation::Confirmation::default(),
            created_by: actor.to_owned(),
            created_dept: actor_dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
        }
    }

    pub fn update(&mut self, attrs: PermissionClassAttrs, actor: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.touch(actor, today);
    }

    /// 複製（ADR 0013 SA5；p_zw 原本沒有）：新代號、全部欄位，資料所有者為目前使用者、未確認；
    /// 程式執行權限不複製（由程式執行權限維護 p_zy 的複製處理）
    pub fn copy_as(
        &self,
        code: ClassCode,
        actor: &str,
        actor_dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self::new(code, self.attrs.clone(), actor, actor_dept, today)
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

    /// 有效／無效切換（是否仍有使用者由 app 層檢查，p_zw.4gl:511-535）
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

    fn attrs(
        control: ClassIdleControl,
        action: Option<IdleAction>,
        secs: Option<i32>,
    ) -> Result<PermissionClassAttrs, DomainError> {
        PermissionClassAttrs::new(Some("業務".into()), None, control, action, secs)
    }

    #[test]
    fn class_code_rules() {
        assert_eq!(
            ClassCode::parse(" ").unwrap_err().code(),
            "PERMISSION_CLASS_CODE_EMPTY"
        );
        assert_eq!(
            ClassCode::parse("CLASS012345").unwrap_err().code(),
            "PERMISSION_CLASS_CODE_TOO_LONG"
        );
        assert_eq!(ClassCode::parse(" SALES ").unwrap().as_str(), "SALES");
    }

    #[test]
    fn name_required() {
        let err =
            PermissionClassAttrs::new(Some(" ".into()), None, ClassIdleControl::Off, None, None);
        assert_eq!(err.unwrap_err().code(), "PERMISSION_CLASS_NAME_EMPTY");
    }

    #[test]
    fn idle_settings_kept_only_by_class() {
        let a = attrs(
            ClassIdleControl::BySystem,
            Some(IdleAction::Close),
            Some(600),
        )
        .unwrap();
        assert_eq!((a.idle_action, a.idle_seconds), (None, None));
        let a = attrs(
            ClassIdleControl::ByClass,
            Some(IdleAction::Close),
            Some(600),
        )
        .unwrap();
        assert_eq!(
            (a.idle_action, a.idle_seconds),
            (Some(IdleAction::Close), Some(600))
        );
    }

    #[test]
    fn idle_settings_required_and_in_range() {
        let code = |r: Result<PermissionClassAttrs, DomainError>| r.unwrap_err().code();
        assert_eq!(
            code(attrs(ClassIdleControl::ByClass, None, Some(600))),
            "IDLE_SETTING_REQUIRED"
        );
        assert_eq!(
            code(attrs(
                ClassIdleControl::ByClass,
                Some(IdleAction::WarnOnly),
                None
            )),
            "IDLE_SETTING_REQUIRED"
        );
        assert_eq!(
            code(attrs(
                ClassIdleControl::ByClass,
                Some(IdleAction::WarnOnly),
                Some(0)
            )),
            "IDLE_SECONDS_INVALID"
        );
        assert_eq!(
            code(attrs(
                ClassIdleControl::ByClass,
                Some(IdleAction::WarnOnly),
                Some(100_000)
            )),
            "IDLE_SECONDS_INVALID"
        );
        assert!(
            attrs(
                ClassIdleControl::ByClass,
                Some(IdleAction::WarnOnly),
                Some(99_999)
            )
            .is_ok()
        );
    }
}
