//! Program Permission 程式執行權限（TipTop: p_zy / zy_file；動作設定 s_act_define）
//! 規格：specs/features/azz/program_permission_maintenance.feature（Y1–Y12）；權限框架：docs/adr/0006 §4
//!
//! 本模組只維護權限資料；依資料執行檢查（Authorizer、資料權限、查詢過濾）見 ADR 0006。

use chrono::NaiveDate;

use crate::{
    DomainError,
    permission_class::ClassCode,
    program::{ProgramNo, coded_enum},
};

coded_enum!(
    /// 他人資料權限（zy04）：對非自己資料的查詢／修改／刪除
    OtherUserData { All = "0", QueryModify = "1", QueryDelete = "2", QueryOnly = "3", None = "4" }
);
coded_enum!(
    /// 他部門資料權限（zy05）：0–4 同 zy04；5–8 為相關部門（部門群組 zyw_file，p_zyw 移植後生效）
    OtherDeptData {
        All = "0",
        QueryModify = "1",
        QueryDelete = "2",
        QueryOnly = "3",
        None = "4",
        RelatedQueryModify = "5",
        RelatedQueryDelete = "6",
        RelatedQueryOnly = "7",
        RelatedAll = "8",
    }
);
coded_enum!(
    /// 單身權限（zy07）：單身可否新增／刪除列
    DetailRows { AddDelete = "0", AddOnly = "1", DeleteOnly = "2", None = "3" }
);

/// 動作代號清單（TipTop zz04 / zy03）：以逗號分隔，去空白、轉小寫，略過空白與重複（Y1，cl_set_priv DOWNSHIFT）
pub fn parse_actions<'a>(items: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in items.into_iter().flat_map(|s| s.split(',')) {
        let a = a.trim().to_lowercase();
        if !a.is_empty() && !out.contains(&a) {
            out.push(a);
        }
    }
    out
}

fn level<T>(raw: Option<&str>, parse: fn(&str) -> Option<T>, default: T) -> Result<T, DomainError> {
    match raw.map(str::trim) {
        None | Some("") => Ok(default),
        Some(code) => {
            parse(code).ok_or_else(|| DomainError::PermissionLevelInvalid(code.to_owned()))
        }
    }
}

/// 可由畫面輸入的欄位
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionAttrs {
    /// zy03 允許執行功能，依程式的功能順序
    pub actions: Vec<String>,
    pub other_user: OtherUserData,
    pub other_dept: OtherDeptData,
    pub detail: DetailRows,
}

impl PermissionAttrs {
    /// 新增時的預設值：程式的全部功能、資料權限 0／0／0（Y3）
    pub fn defaults(program_actions: &[String]) -> Self {
        Self {
            actions: program_actions.to_vec(),
            other_user: OtherUserData::All,
            other_dept: OtherDeptData::All,
            detail: DetailRows::AddDelete,
        }
    }

    /// 檢核輸入：功能只能選程式提供的（Y1），代碼須在範圍內（Y4）；未指定的值沿用 `base`
    pub fn new(
        base: &Self,
        program_actions: &[String],
        actions: Option<&[String]>,
        other_user: Option<&str>,
        other_dept: Option<&str>,
        detail: Option<&str>,
    ) -> Result<Self, DomainError> {
        let actions = match actions {
            None => base.actions.clone(),
            Some(raw) => {
                let wanted = parse_actions(raw.iter().map(String::as_str));
                if let Some(bad) = wanted.iter().find(|a| !program_actions.contains(a)) {
                    return Err(DomainError::PermissionActionNotAllowed(bad.clone()));
                }
                program_actions
                    .iter()
                    .filter(|a| wanted.contains(a))
                    .cloned()
                    .collect()
            }
        };
        Ok(Self {
            actions,
            other_user: level(other_user, OtherUserData::from_code, base.other_user)?,
            other_dept: level(other_dept, OtherDeptData::from_code, base.other_dept)?,
            detail: level(detail, DetailRows::from_code, base.detail)?,
        })
    }
}

/// 程式執行權限（zy_file：權限類別＋程式一筆）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramPermission {
    pub class: ClassCode,
    pub program: ProgramNo,
    pub attrs: PermissionAttrs,
    /// zyuser / zygrup / zydate
    pub created_by: String,
    pub created_dept: String,
    pub created_on: NaiveDate,
    /// zymodu / zydate
    pub updated_by: Option<String>,
    pub updated_on: Option<NaiveDate>,
}

impl ProgramPermission {
    pub fn new(
        class: ClassCode,
        program: ProgramNo,
        attrs: PermissionAttrs,
        actor: &str,
        actor_dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self {
            class,
            program,
            attrs,
            created_by: actor.to_owned(),
            created_dept: actor_dept.to_owned(),
            created_on: today,
            updated_by: None,
            updated_on: None,
        }
    }

    pub fn update(&mut self, attrs: PermissionAttrs, actor: &str, today: NaiveDate) {
        self.attrs = attrs;
        self.updated_by = Some(actor.to_owned());
        self.updated_on = Some(today);
    }

    /// 複製到另一個權限類別（p_zy.4gl:1231-1240）：資料所有者為目前使用者
    pub fn copy_to(
        &self,
        class: ClassCode,
        actor: &str,
        actor_dept: &str,
        today: NaiveDate,
    ) -> Self {
        Self::new(
            class,
            self.program.clone(),
            self.attrs.clone(),
            actor,
            actor_dept,
            today,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acts(s: &str) -> Vec<String> {
        parse_actions([s])
    }

    #[test]
    fn actions_are_normalized() {
        assert_eq!(acts(" Insert , QUERY,, insert "), ["insert", "query"]);
        assert!(acts("").is_empty());
    }

    #[test]
    fn actions_follow_program_order_and_must_be_offered() {
        let program = acts("insert, query, modify");
        let base = PermissionAttrs::defaults(&program);
        let a = PermissionAttrs::new(
            &base,
            &program,
            Some(&acts("modify, insert")),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(a.actions, ["insert", "modify"]);
        let err = PermissionAttrs::new(&base, &program, Some(&acts("delete")), None, None, None)
            .unwrap_err();
        assert_eq!(err.code(), "PERMISSION_ACTION_NOT_ALLOWED");
    }

    #[test]
    fn levels_must_be_in_range() {
        let base = PermissionAttrs::defaults(&[]);
        let code = |u, d, r| {
            PermissionAttrs::new(&base, &[], None, u, d, r)
                .map(|_| ())
                .unwrap_err()
                .code()
        };
        assert_eq!(code(Some("5"), None, None), "PERMISSION_LEVEL_INVALID");
        assert_eq!(code(None, Some("9"), None), "PERMISSION_LEVEL_INVALID");
        assert_eq!(code(None, None, Some("4")), "PERMISSION_LEVEL_INVALID");
        let ok = PermissionAttrs::new(&base, &[], None, Some("3"), Some("8"), Some("2")).unwrap();
        assert_eq!(
            (ok.other_user, ok.other_dept, ok.detail),
            (
                OtherUserData::QueryOnly,
                OtherDeptData::RelatedAll,
                DetailRows::DeleteOnly
            )
        );
    }
}
