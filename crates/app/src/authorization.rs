//! Authorizer 權限檢查（TipTop: cl_set_priv、cl_chk_act_auth、cl_detail_input_auth / zy_file、zxw_file）
//! 規格：specs/features/azz/authorization.feature（AQ1–AQ13）；框架：docs/adr/0006 §4、§6
//!
//! 每個 use case 先以（程式代號, 動作）呼叫 [`Authorizer::authorize`]，取得 [`Grant`] 後再依作業需要
//! 檢查資料所有者（[`Grant::record`]）、單身列（[`Grant::add_row`] / [`Grant::delete_row`]）或取查詢範圍（[`Grant::scope`]）。

use std::sync::Arc;

use async_trait::async_trait;
use ttrs_domain::{
    DomainError,
    authorization::{DataContext, DataOp, DataScope, DenyReason, EffectivePermission, ai_may},
    permission::PermissionAttrs,
    permission_class::ClassCode,
    program::ProgramNo,
    user::UserId,
    user_permission::SpecialTarget,
};

use ttrs_shared::MyPermissionDto;

use crate::{
    AppError,
    ports::{ProgramPermissionRepository, UserPermissionRepository},
};

/// TTRS 已移植、經 Authorizer 檢查的作業：(英文程式代號, TipTop 程式代號, 模組, 功能清單)
/// （docs/adr/0008-english-program-codes.md、docs/legacy-map/programs.yaml）。
/// 起始資料（AQ11）：登錄這些程式並給權限類別 ADMIN 全部功能；須與 migrations/0015、0017、0018、0022、0023 對齊
pub const PORTED_PROGRAMS: &[(&str, &str, &str, &str)] = &[
    (
        "program-maintenance",
        "p_zz",
        "AZZ",
        "insert, query, modify, delete, reproduce, authorization, history, output, exporttoexcel",
    ),
    (
        "data-dictionary",
        "p_zta",
        "AZZ",
        "insert, query, modify, delete, maintain_table_description, maintain_field_description, exporttoexcel, zta_import, history, output",
    ),
    (
        "user-maintenance",
        "p_zx",
        "AZZ",
        "insert, query, modify, delete, reproduce, invalid, user_unlock, history, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "permission-class-maintenance",
        "p_zw",
        "AZZ",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "plant-maintenance",
        "aooi901",
        "AOO",
        "insert, query, modify, delete, reproduce, history, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "program-permission-maintenance",
        "p_zy",
        "AZZ",
        "query, delete, reproduce, detail, batch_gen_by_prog, batch_gen_by_menu, batch_del_by_menu, history, output, exporttoexcel",
    ),
    (
        "user-permission-maintenance",
        "p_zxw",
        "AZZ",
        "query, delete, reproduce, detail, history, output, exporttoexcel",
    ),
    (
        "item-maintenance",
        "aimi100",
        "AIM",
        "insert, query, modify, delete, invalid, reproduce, confirm, notconfirm, order_invalid, update_item, output, exporttoexcel, history",
    ),
    (
        "department-maintenance",
        "aooi030",
        "AOO",
        "query, detail, account, output, exporttoexcel, history, reproduce, confirm, notconfirm",
    ),
    (
        "employee-maintenance",
        "aooi040",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "classification-code-maintenance",
        "aooi300",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "menu-maintenance",
        "p_zm",
        "AZZ",
        "insert, query, delete, detail, reproduce, history",
    ),
    (
        "unit-maintenance",
        "aooi101",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "project-maintenance",
        "aooi200",
        "AOO",
        "insert, query, modify, delete, reproduce, invalid, history, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "tax-type-maintenance",
        "aooi150",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "currency-maintenance",
        "aooi050",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "monthly-exchange-rate-maintenance",
        "aooi060",
        "AOO",
        "insert, query, modify, delete, reproduce, invalid, history, confirm, notconfirm, output, exporttoexcel, re_rate",
    ),
    (
        "daily-exchange-rate-maintenance",
        "aooi070",
        "AOO",
        "insert, query, modify, delete, reproduce, invalid, history, confirm, notconfirm, output, exporttoexcel, unify_rate, auto_import",
    ),
    // 參數作業（GP8）：只有一筆，沒有新增、刪除、複製、失效、確認
    (
        "general-management-parameters",
        "aoos010",
        "AOO",
        "query, modify, history, output, exporttoexcel",
    ),
    // 工具作業（ET7）：只有執行
    ("epub-to-text", "", "AZZ", "run, translate"),
    (
        "github-epub-search",
        "",
        "AZZ",
        "insert, query, delete, run, share",
    ),
    // 編碼性質（EN7）：比照單位維護
    (
        "encoding-nature-maintenance",
        "aooi401",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    // 編碼獨立段（IS8）：單頭單身，沿用 TipTop 功能代號並補上確認與異動記錄
    (
        "encoding-segment-maintenance",
        "aooi403",
        "AOO",
        "insert, query, modify, delete, invalid, reproduce, detail, history, confirm, notconfirm, output, exporttoexcel",
    ),
    // 單位換算（UC9）：比照單位維護
    (
        "unit-conversion-maintenance",
        "aooi102",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    // 料件單位換算（UC9）：單頭是料件
    (
        "item-unit-conversion-maintenance",
        "aooi103",
        "AOO",
        "query, detail, history, output, exporttoexcel",
    ),
    // 會計期間（PD9）：沒有失效
    (
        "accounting-period-maintenance",
        "aoos020",
        "AOO",
        "insert, query, modify, delete, reproduce, confirm, notconfirm, output, exporttoexcel, history",
    ),
    // 批次作業（CP7）：執行 run、列印、匯出 Excel
    (
        "daily-exchange-rate-copy",
        "aoop010",
        "AOO",
        "run, output, exporttoexcel",
    ),
    (
        "region-maintenance",
        "aooi100",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "country-maintenance",
        "aooi090",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "area-maintenance",
        "aooi110",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "shipping-method-maintenance",
        "aooi160",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "report-title-maintenance",
        "aooi210",
        "AOO",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "product-classification-maintenance",
        "axmi110",
        "AXM",
        "query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel",
    ),
    (
        "item-group-maintenance",
        "aimi110",
        "AIM",
        "insert, query, modify, delete, reproduce, confirm, notconfirm, invalid, output, exporttoexcel, history",
    ),
];

/// 身分種類（ADR 0006 §6）
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ActorKind {
    #[default]
    Person,
    /// AI 代表使用者操作：只能執行查詢與草稿類功能（AQ9）
    Ai,
    /// 背景作業、系統整合的服務帳號
    Service,
}

/// 執行操作的使用者（TipTop: g_user / g_grup / 權限類別 g_clas）；登入時決定，檢查時不再重查（AQ1）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Actor {
    pub user_id: String,
    pub dept: String,
    pub permission_class: String,
    pub kind: ActorKind,
}

impl Actor {
    /// 目前使用者與相關部門。部門權限群組（p_zyw）移植前，相關部門只有自己的部門（AQ6）
    fn context(&self) -> DataContext {
        DataContext {
            user: self.user_id.clone(),
            dept: self.dept.clone(),
            related_depts: vec![self.dept.clone()],
        }
    }
}

#[async_trait]
pub trait Authorizer: Send + Sync {
    /// 目前使用者在此作業的有效權限（cl_set_priv）；沒有本作業權限時為 None
    async fn effective(
        &self,
        actor: &Actor,
        program: &str,
    ) -> Result<Option<EffectivePermission>, AppError>;

    /// 檢查功能（cl_chk_act_auth.4gl:77-93），通過時回傳之後檢查資料權限用的 [`Grant`]
    async fn authorize(
        &self,
        actor: &Actor,
        program: &'static str,
        action: &'static str,
    ) -> Result<Grant, AppError> {
        let effective = self.effective(actor, program).await?;
        Grant::check(actor, program, action, effective)
    }
}

/// 目前使用者在此作業的有效權限（GET /api/me/permissions/{作業}，AQ13）
pub async fn my_permission(
    authz: &dyn Authorizer,
    actor: &Actor,
    program: &str,
) -> Result<MyPermissionDto, AppError> {
    let program = program.trim().to_owned();
    Ok(match authz.effective(actor, &program).await? {
        Some(p) => MyPermissionDto {
            program,
            allowed: true,
            actions: p.actions,
            other_user_data: p.other_user.code().to_owned(),
            other_dept_data: p.other_dept.code().to_owned(),
            detail_rows: p.detail.code().to_owned(),
        },
        None => MyPermissionDto {
            program,
            ..MyPermissionDto::default()
        },
    })
}

/// 依權限資料檢查（zy_file、zxw_file）
pub struct PermissionAuthorizer {
    classes: Arc<dyn ProgramPermissionRepository>,
    special: Arc<dyn UserPermissionRepository>,
}

impl PermissionAuthorizer {
    pub fn new(
        classes: Arc<dyn ProgramPermissionRepository>,
        special: Arc<dyn UserPermissionRepository>,
    ) -> Self {
        Self { classes, special }
    }

    async fn class_row(
        &self,
        class: &ClassCode,
        program: &ProgramNo,
    ) -> Result<Option<PermissionAttrs>, AppError> {
        Ok(self
            .classes
            .get(class, program)
            .await
            .map_err(|e| AppError::Infra(e.to_string()))?
            .map(|p| p.attrs))
    }
}

#[async_trait]
impl Authorizer for PermissionAuthorizer {
    async fn effective(
        &self,
        actor: &Actor,
        program: &str,
    ) -> Result<Option<EffectivePermission>, AppError> {
        let Ok(program) = ProgramNo::parse(program) else {
            return Ok(None);
        };
        let specials = match UserId::parse(&actor.user_id) {
            Ok(user) => self
                .special
                .list(&user)
                .await
                .map_err(|e| AppError::Infra(e.to_string()))?,
            Err(_) => Vec::new(),
        };
        let mut rows = Vec::new();
        if specials.is_empty() {
            // 沒有特殊權限：用權限類別的程式列（cl_setup.4gl:551-567）
            if let Ok(class) = ClassCode::parse(&actor.permission_class)
                && let Some(row) = self.class_row(&class, &program).await?
            {
                rows.push(row);
            }
        } else {
            // 有特殊權限：只用特殊權限，依項次合併類別列與本程式的程式列（:569-602）
            for item in &specials {
                match &item.permission.target {
                    SpecialTarget::Class(class) => {
                        if let Some(row) = self.class_row(class, &program).await? {
                            rows.push(row);
                        }
                    }
                    SpecialTarget::Program { program: p, attrs } if *p == program => {
                        rows.push(attrs.clone());
                    }
                    SpecialTarget::Program { .. } => {}
                }
            }
        }
        Ok(EffectivePermission::merge(&rows))
    }
}

/// 通過功能檢查後的權限，用來再檢查資料所有者、單身列與查詢範圍
#[derive(Debug, Clone)]
pub struct Grant {
    program: &'static str,
    action: &'static str,
    context: DataContext,
    /// None 為不限制（測試時未啟用權限檢查）
    permission: Option<EffectivePermission>,
}

impl Grant {
    /// 依有效權限檢查功能；AI 代理只能執行查詢與草稿類功能（AQ9）
    pub fn check(
        actor: &Actor,
        program: &'static str,
        action: &'static str,
        effective: Option<EffectivePermission>,
    ) -> Result<Self, AppError> {
        let reason = match &effective {
            None => Some(DenyReason::NoProgram),
            Some(p) if !p.allows(action) => Some(DenyReason::NoAction),
            Some(_) if actor.kind == ActorKind::Ai && !ai_may(action) => {
                Some(DenyReason::AiNotAllowed)
            }
            Some(_) => None,
        };
        let grant = Self {
            program,
            action,
            context: actor.context(),
            permission: effective,
        };
        match reason {
            Some(r) => Err(grant.deny(r)),
            None => Ok(grant),
        }
    }

    /// 不檢查權限（BDD 未啟用權限檢查時使用）
    pub fn unrestricted(actor: &Actor, program: &'static str, action: &'static str) -> Self {
        Self {
            program,
            action,
            context: actor.context(),
            permission: None,
        }
    }

    /// 檢查一筆資料的所有者與所有部門（cl_chk_act_auth.4gl:97-122）。
    /// 只用在 TipTop 有設定資料所有者的作業（p_zx、p_zz，AQ5）
    pub fn record(&self, owner: &str, dept: &str) -> Result<(), AppError> {
        let (Some(p), Some(op)) = (&self.permission, DataOp::of_action(self.action)) else {
            return Ok(());
        };
        p.check_record(op, owner, dept, &self.context)
            .map_err(|r| self.deny(r))
    }

    /// 單身新增列（cl_detail_input_auth "insert"）
    pub fn add_row(&self) -> Result<(), AppError> {
        match &self.permission {
            Some(p) if !p.can_add_row() => Err(self.deny(DenyReason::DetailRow)),
            _ => Ok(()),
        }
    }

    /// 單身刪除列（cl_detail_input_auth "delete"）
    pub fn delete_row(&self) -> Result<(), AppError> {
        match &self.permission {
            Some(p) if !p.can_delete_row() => Err(self.deny(DenyReason::DetailRow)),
            _ => Ok(()),
        }
    }

    /// 查詢的資料範圍（p_zx.4gl:194-204）
    pub fn scope(&self) -> DataScope {
        self.permission
            .as_ref()
            .map_or_else(DataScope::unrestricted, |p| p.scope(&self.context))
    }

    /// 拒絕並寫應用程式日誌（稽核事件表待稽核 ADR，AQ10）
    fn deny(&self, reason: DenyReason) -> AppError {
        tracing::warn!(
            user = %self.context.user,
            program = self.program,
            action = self.action,
            reason = reason.code(),
            "permission denied"
        );
        DomainError::PermissionDenied {
            program: self.program.to_owned(),
            action: self.action.to_owned(),
            reason,
        }
        .into()
    }
}
