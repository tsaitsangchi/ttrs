//! Authorization 權限檢查規則（TipTop: cl_set_priv、cl_chk_act_auth、cl_detail_input_auth / zy_file、zxw_file）
//! 規格：specs/features/azz/authorization.feature（AQ1–AQ13）；框架：docs/adr/0006 §4、§6
//!
//! 純規則：合併有效權限、依功能與資料所有者／所有部門判斷可否執行、查詢的資料範圍。
//! 讀取權限資料、決定相關部門由 app 層的 Authorizer 負責。

use crate::permission::{DetailRows, OtherDeptData, OtherUserData, PermissionAttrs, parse_actions};

const Q: u8 = 1;
const M: u8 = 2;
const D: u8 = 4;
const ADD_ROW: u8 = 1;
const DELETE_ROW: u8 = 2;

/// 對資料的操作（他人資料、他部門資料權限的查詢／修改／刪除）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataOp {
    Query,
    Modify,
    Delete,
}

impl DataOp {
    /// 功能代號要檢查的資料操作（cl_chk_act_auth.4gl:97-122）：modify、detail 為修改，delete 為刪除，query 為查詢；
    /// 其他功能不檢查資料權限（自訂動作對應 gbl_file 未移植，AQ7）
    pub fn of_action(action: &str) -> Option<Self> {
        match action {
            "query" => Some(Self::Query),
            "modify" | "detail" => Some(Self::Modify),
            "delete" => Some(Self::Delete),
            _ => None,
        }
    }

    fn bit(self) -> u8 {
        match self {
            Self::Query => Q,
            Self::Modify => M,
            Self::Delete => D,
        }
    }
}

/// 拒絕的原因（TipTop 訊息 lib-214、lib-219、9007）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyReason {
    /// 沒有本作業的權限列（lib-214）
    NoProgram,
    /// 功能不在允許清單（lib-219）
    NoAction,
    /// 他人資料權限不允許（9007）
    OtherUser,
    /// 他部門資料權限不允許（9007）
    OtherDept,
    /// 單身權限不允許新增或刪除列
    DetailRow,
    /// AI 代理不可執行此功能（ADR 0006 §6）
    AiNotAllowed,
}

impl DenyReason {
    /// 穩定代碼
    pub fn code(self) -> &'static str {
        match self {
            Self::NoProgram => "no_program",
            Self::NoAction => "no_action",
            Self::OtherUser => "other_user",
            Self::OtherDept => "other_dept",
            Self::DetailRow => "detail_row",
            Self::AiNotAllowed => "ai_not_allowed",
        }
    }

    /// 錯誤訊息（繁中，畫面直接顯示伺服器訊息）
    pub fn message(self, program: &str, action: &str) -> String {
        match self {
            Self::NoProgram => format!("您沒有執行 {program} 的權限（請檢查權限設定）"),
            Self::NoAction => {
                format!("您在 {program} 中沒有執行 {action} 的權限（請檢查權限設定）")
            }
            Self::OtherUser => {
                format!("您對此筆資料沒有 {action} 的權限（他人資料，請檢查權限設定及資料所有者）")
            }
            Self::OtherDept => format!(
                "您對此筆資料沒有 {action} 的權限（他部門資料，請檢查權限設定及資料所有部門）"
            ),
            Self::DetailRow => format!("您在 {program} 中沒有新增或刪除單身資料列的權限"),
            Self::AiNotAllowed => format!("AI 代理不可執行 {action}，請由使用者本人操作"),
        }
    }
}

/// AI 代理可執行的功能（AQ9）：查詢與草稿類；其餘即使代表的使用者有權限也拒絕
pub const AI_ALLOWED_ACTIONS: [&str; 6] =
    ["query", "output", "insert", "modify", "detail", "reproduce"];

pub fn ai_may(action: &str) -> bool {
    AI_ALLOWED_ACTIONS.contains(&action)
}

/// 目前使用者：代號、部門與相關部門（含自己的部門；p_zyw 移植前只有自己的部門，AQ6）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataContext {
    pub user: String,
    pub dept: String,
    pub related_depts: Vec<String>,
}

/// 查詢的資料範圍（p_zx.4gl:194-204）：`None` 為不限制
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataScope {
    /// 只查此使用者建立的資料（他人資料 = 4）
    pub owner: Option<String>,
    /// 只查這些部門的資料（他部門資料 = 4 為自己的部門，5–8 為相關部門）
    pub depts: Option<Vec<String>>,
}

impl DataScope {
    pub fn unrestricted() -> Self {
        Self::default()
    }

    pub fn includes(&self, owner: &str, dept: &str) -> bool {
        self.owner.as_deref().is_none_or(|o| o == owner)
            && self
                .depts
                .as_ref()
                .is_none_or(|ds| ds.iter().any(|d| d == dept))
    }
}

/// 有效權限（cl_set_priv 算出的 g_priv1 功能、g_priv2 他人資料、g_priv3 他部門資料、g_priv5 單身）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectivePermission {
    pub actions: Vec<String>,
    pub other_user: OtherUserData,
    pub other_dept: OtherDeptData,
    pub detail: DetailRows,
}

impl EffectivePermission {
    /// 依序合併多列權限（cl_setup.4gl:605-853）；沒有任何一列時為 None（沒有本作業權限，lib-214）。
    /// 功能取聯集；他人資料、單身取聯集；他部門資料取聯集，沒有剛好的代碼時取較寬的（MOD-7B0137，AQ1）
    pub fn merge<'a>(rows: impl IntoIterator<Item = &'a PermissionAttrs>) -> Option<Self> {
        let rows: Vec<&PermissionAttrs> = rows.into_iter().collect();
        if rows.is_empty() {
            return None;
        }
        let actions = parse_actions(
            rows.iter()
                .flat_map(|r| r.actions.iter().map(String::as_str)),
        );
        let user = rows.iter().fold(0, |c, r| c | user_caps(r.other_user));
        let (all, related) = rows.iter().fold((0, 0), |(a, rel), r| {
            let (a2, rel2) = dept_caps(r.other_dept);
            (a | a2, rel | rel2)
        });
        let detail = rows.iter().fold(0, |c, r| c | detail_caps(r.detail));
        Some(Self {
            actions,
            other_user: user_level(user),
            other_dept: dept_level(all, related),
            detail: detail_level(detail),
        })
    }

    /// 功能代號在允許清單中（不分大小寫，cl_chk_act_auth.4gl:91）
    pub fn allows(&self, action: &str) -> bool {
        self.actions.iter().any(|a| a.eq_ignore_ascii_case(action))
    }

    /// 單身可新增列（cl_detail_input_auth.4gl:55-60）
    pub fn can_add_row(&self) -> bool {
        detail_caps(self.detail) & ADD_ROW != 0
    }

    /// 單身可刪除列（cl_detail_input_auth.4gl:61-66）
    pub fn can_delete_row(&self) -> bool {
        detail_caps(self.detail) & DELETE_ROW != 0
    }

    /// 對一筆資料可否執行（cl_chk_act_auth.4gl:97-122）：所有者不是自己看他人資料權限，
    /// 所有部門不是自己的部門再看他部門資料權限；5–8 只限相關部門（AQ6）。
    /// 所有者、所有部門空白視為別人的資料（與查詢範圍一致）。
    pub fn check_record(
        &self,
        op: DataOp,
        owner: &str,
        dept: &str,
        me: &DataContext,
    ) -> Result<(), DenyReason> {
        if owner != me.user && user_caps(self.other_user) & op.bit() == 0 {
            return Err(DenyReason::OtherUser);
        }
        if dept != me.dept {
            let (all, related) = dept_caps(self.other_dept);
            let caps = if me.related_depts.iter().any(|d| d == dept) {
                related
            } else {
                all
            };
            if caps & op.bit() == 0 {
                return Err(DenyReason::OtherDept);
            }
        }
        Ok(())
    }

    /// 查詢的資料範圍（p_zx.4gl:194-204）
    pub fn scope(&self, me: &DataContext) -> DataScope {
        DataScope {
            owner: (self.other_user == OtherUserData::None).then(|| me.user.clone()),
            depts: match self.other_dept {
                OtherDeptData::None => Some(vec![me.dept.clone()]),
                OtherDeptData::RelatedQueryModify
                | OtherDeptData::RelatedQueryDelete
                | OtherDeptData::RelatedQueryOnly
                | OtherDeptData::RelatedAll => Some(me.related_depts.clone()),
                _ => None,
            },
        }
    }
}

fn user_caps(l: OtherUserData) -> u8 {
    match l {
        OtherUserData::All => Q | M | D,
        OtherUserData::QueryModify => Q | M,
        OtherUserData::QueryDelete => Q | D,
        OtherUserData::QueryOnly => Q,
        OtherUserData::None => 0,
    }
}

fn user_level(c: u8) -> OtherUserData {
    match (c & Q != 0, c & M != 0, c & D != 0) {
        (false, ..) => OtherUserData::None,
        (true, true, true) => OtherUserData::All,
        (true, true, false) => OtherUserData::QueryModify,
        (true, false, true) => OtherUserData::QueryDelete,
        (true, false, false) => OtherUserData::QueryOnly,
    }
}

/// (其他部門, 相關部門) 的權限；0–4 兩者相同，5–8 只有相關部門
fn dept_caps(l: OtherDeptData) -> (u8, u8) {
    match l {
        OtherDeptData::All => (Q | M | D, Q | M | D),
        OtherDeptData::QueryModify => (Q | M, Q | M),
        OtherDeptData::QueryDelete => (Q | D, Q | D),
        OtherDeptData::QueryOnly => (Q, Q),
        OtherDeptData::None => (0, 0),
        OtherDeptData::RelatedQueryModify => (0, Q | M),
        OtherDeptData::RelatedQueryDelete => (0, Q | D),
        OtherDeptData::RelatedQueryOnly => (0, Q),
        OtherDeptData::RelatedAll => (0, Q | M | D),
    }
}

/// 涵蓋 (其他部門, 相關部門) 權限的最小代碼：其他部門有權限時取 0–3（以相關部門的權限為準，因為兩者相同），
/// 否則取 4–8
fn dept_level(all: u8, related: u8) -> OtherDeptData {
    let r = all | related;
    let (q, m, d) = (r & Q != 0, r & M != 0, r & D != 0);
    if all == 0 {
        match (q, m, d) {
            (false, ..) => OtherDeptData::None,
            (true, true, true) => OtherDeptData::RelatedAll,
            (true, true, false) => OtherDeptData::RelatedQueryModify,
            (true, false, true) => OtherDeptData::RelatedQueryDelete,
            (true, false, false) => OtherDeptData::RelatedQueryOnly,
        }
    } else {
        match (m, d) {
            (true, true) => OtherDeptData::All,
            (true, false) => OtherDeptData::QueryModify,
            (false, true) => OtherDeptData::QueryDelete,
            (false, false) => OtherDeptData::QueryOnly,
        }
    }
}

fn detail_caps(l: DetailRows) -> u8 {
    match l {
        DetailRows::AddDelete => ADD_ROW | DELETE_ROW,
        DetailRows::AddOnly => ADD_ROW,
        DetailRows::DeleteOnly => DELETE_ROW,
        DetailRows::None => 0,
    }
}

fn detail_level(c: u8) -> DetailRows {
    match (c & ADD_ROW != 0, c & DELETE_ROW != 0) {
        (true, true) => DetailRows::AddDelete,
        (true, false) => DetailRows::AddOnly,
        (false, true) => DetailRows::DeleteOnly,
        (false, false) => DetailRows::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(actions: &str, user: &str, dept: &str, detail: &str) -> PermissionAttrs {
        PermissionAttrs {
            actions: parse_actions([actions]),
            other_user: OtherUserData::from_code(user).unwrap(),
            other_dept: OtherDeptData::from_code(dept).unwrap(),
            detail: DetailRows::from_code(detail).unwrap(),
        }
    }

    fn me() -> DataContext {
        DataContext {
            user: "u001".into(),
            dept: "SAL".into(),
            related_depts: vec!["SAL".into()],
        }
    }

    /// 他部門資料的合併與 TipTop 的 CASE 對照表（cl_setup.4gl:698-790）逐一相同
    #[test]
    fn dept_merge_matches_tiptop_table() {
        // tiptop(t, g)：t 為新的一列、g 為目前累積的值
        fn tiptop(t: u8, g: u8) -> u8 {
            if t == g {
                return g;
            }
            if t == 0 || g == 0 {
                return 0;
            }
            match (t, g) {
                (1, 2 | 6 | 8) => 0,
                (1, 3 | 4 | 5 | 7) => 1,
                (2, 1 | 5 | 8) => 0,
                (2, 3 | 4 | 6 | 7) => 2,
                (3, 1 | 5) => 1,
                (3, 2 | 6) => 2,
                (3, 4 | 7) => 3,
                (3, 8) => 0,
                (4, g) => g,
                (5, 2) => 0,
                (5, 1 | 3) => 1,
                (5, 6 | 8) => 8,
                (5, 4 | 7) => 5,
                (6, 1) => 0,
                (6, 2 | 3) => 2,
                (6, 5 | 8) => 8,
                (6, 4 | 7) => 6,
                (7, 4) => 7,
                (7, g) => g,
                (8, 1..=3) => 0,
                (8, _) => 8,
                _ => unreachable!(),
            }
        }
        for a in 0..=8u8 {
            for b in 0..=8u8 {
                let rows = [
                    row("query", "0", &a.to_string(), "0"),
                    row("query", "0", &b.to_string(), "0"),
                ];
                let merged = EffectivePermission::merge(&rows).unwrap();
                assert_eq!(
                    merged.other_dept.code(),
                    tiptop(b, a).to_string(),
                    "{a} 與 {b}"
                );
            }
        }
    }

    #[test]
    fn user_and_detail_merge_are_unions() {
        let rows = [
            row("query, modify", "1", "0", "1"),
            row("Query, DELETE", "2", "0", "2"),
        ];
        let m = EffectivePermission::merge(&rows).unwrap();
        assert_eq!(m.actions, ["query", "modify", "delete"]);
        assert_eq!(m.other_user, OtherUserData::All);
        assert_eq!(m.detail, DetailRows::AddDelete);
        assert!(EffectivePermission::merge(&[]).is_none());
    }

    #[test]
    fn record_checks_follow_cl_chk_act_auth() {
        let p = |user, dept| EffectivePermission::merge(&[row("modify", user, dept, "0")]).unwrap();
        let me = me();
        // 他人、同部門
        assert_eq!(
            p("1", "0").check_record(DataOp::Modify, "u009", "SAL", &me),
            Ok(())
        );
        assert_eq!(
            p("1", "0").check_record(DataOp::Delete, "u009", "SAL", &me),
            Err(DenyReason::OtherUser)
        );
        // 自己建立、他部門仍看他部門資料權限
        assert_eq!(
            p("4", "3").check_record(DataOp::Modify, "u001", "MFG", &me),
            Err(DenyReason::OtherDept)
        );
        // 5–8 只限相關部門
        assert_eq!(
            p("0", "8").check_record(DataOp::Modify, "u009", "MFG", &me),
            Err(DenyReason::OtherDept)
        );
        let related = DataContext {
            related_depts: vec!["SAL".into(), "MFG".into()],
            ..me.clone()
        };
        assert_eq!(
            p("0", "8").check_record(DataOp::Delete, "u009", "MFG", &related),
            Ok(())
        );
    }

    #[test]
    fn scope_from_levels() {
        let p = |user, dept| EffectivePermission::merge(&[row("query", user, dept, "0")]).unwrap();
        let me = me();
        assert_eq!(p("3", "3").scope(&me), DataScope::unrestricted());
        let s = p("4", "7").scope(&me);
        assert_eq!(s.owner.as_deref(), Some("u001"));
        assert_eq!(s.depts, Some(vec!["SAL".to_owned()]));
        assert!(s.includes("u001", "SAL"));
        assert!(!s.includes("u001", "MFG"));
    }
}
