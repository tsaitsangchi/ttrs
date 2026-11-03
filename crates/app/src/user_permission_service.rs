//! User Special Permission Maintenance 使用者特殊權限維護 use case（TipTop: p_zxw）
//! 規格：specs/features/azz/user_permission_maintenance.feature（X1–X11）；權限框架：docs/adr/0006 §4
//!
//! 有特殊權限的使用者只用特殊權限（cl_set_priv）；多列的合併與檢查屬 Authorizer（X8，authorization.rs）。
//! 本作業 TipTop 沒有設定資料所有者，不檢查資料權限（AQ5）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    lang::{Lang, localized, non_empty},
    permission::{PermissionAttrs, parse_actions},
    permission_class::ClassCode,
    program::ProgramNo,
    user::{User, UserId},
    user_permission::{SpecialTarget, UserPermission, next_line_no},
};
use ttrs_shared::{
    PermissionBatchResultDto, SpecialPermissionKindDto, UserPermissionDto, UserPermissionInput,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    ports::{
        PermissionClassRepository, ProgramRepository, UserPermissionItem, UserPermissionRepository,
        UserRepository,
    },
};

/// 權限檢查用的程式代號
const PROGRAM: &str = "p_zxw";

#[derive(Clone)]
pub struct UserPermissionService {
    authz: Arc<dyn Authorizer>,
    perms: Arc<dyn UserPermissionRepository>,
    users: Arc<dyn UserRepository>,
    programs: Arc<dyn ProgramRepository>,
    classes: Arc<dyn PermissionClassRepository>,
}

impl UserPermissionService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        perms: Arc<dyn UserPermissionRepository>,
        users: Arc<dyn UserRepository>,
        programs: Arc<dyn ProgramRepository>,
        classes: Arc<dyn PermissionClassRepository>,
    ) -> Self {
        Self {
            authz,
            perms,
            users,
            programs,
            classes,
        }
    }

    /// 使用者的特殊權限（依項次），程式名稱依 `lang` 顯示
    pub async fn list(
        &self,
        actor: &Actor,
        user: &str,
        lang: &str,
    ) -> Result<Vec<UserPermissionDto>, AppError> {
        self.authorize(actor, "query").await?;
        self.list_in(user, lang).await
    }

    async fn list_in(&self, user: &str, lang: &str) -> Result<Vec<UserPermissionDto>, AppError> {
        let user = self.user(user).await?;
        let lang = Lang::parse(lang)?;
        let items = self.perms.list(&user.user_id).await.map_err(infra)?;
        Ok(items.iter().map(|i| to_dto(i, &lang)).collect())
    }

    /// 新增一列（p_zxw.4gl:624-795）；使用者還沒有特殊權限且勾選時，先加入原權限類別（azz-006，X1）。
    /// 回傳新增後的全部特殊權限
    pub async fn add(
        &self,
        actor: &Actor,
        user: &str,
        input: UserPermissionInput,
    ) -> Result<Vec<UserPermissionDto>, AppError> {
        // TipTop 在單身新增列：需 detail 與單身新增權限（AQ3、AQ8）
        self.authorize(actor, "detail").await?.add_row()?;
        let user = self.user(user).await?;
        let target = self.target(&input).await?;
        let existing = self.existing(&user.user_id).await?;
        let mut line_no = next_line_no(&existing);
        if existing.is_empty()
            && input.include_own_class
            && let Ok(own) = ClassCode::parse(&user.attrs.permission_class)
            && target.class() != Some(&own)
        {
            let row = self.row(actor, &user.user_id, line_no, SpecialTarget::Class(own));
            self.insert(&row).await?;
            line_no += 1;
        }
        self.insert(&self.row(actor, &user.user_id, line_no, target))
            .await?;
        self.list_in(user.user_id.as_str(), ttrs_domain::lang::DEFAULT_LANG)
            .await
    }

    /// 修改程式列的功能與資料權限（p_zxw.4gl:820-850）；未指定的值沿用原值
    pub async fn update_program(
        &self,
        actor: &Actor,
        user: &str,
        program: &str,
        input: UserPermissionInput,
    ) -> Result<UserPermissionDto, AppError> {
        self.authorize(actor, "detail").await?;
        let user = self.user(user).await?;
        let program = self.program(program).await?;
        let mut row = self
            .existing(&user.user_id)
            .await?
            .into_iter()
            .find(|r| r.target.program() == Some(&program.program_no))
            .ok_or(DomainError::PermissionNotFound)?;
        let SpecialTarget::Program { attrs: base, .. } = &row.target else {
            return Err(DomainError::PermissionNotFound.into());
        };
        let offered = parse_actions(program.other.default_actions.as_deref());
        let attrs = program_attrs(base, &offered, &input)?;
        row.update_program(attrs, &actor.user_id, today());
        self.perms.update(&row).await.map_err(infra)?;
        let lang = Lang::default_lang();
        self.perms
            .list(&user.user_id)
            .await
            .map_err(infra)?
            .iter()
            .find(|i| i.permission.line_no == row.line_no)
            .map(|i| to_dto(i, &lang))
            .ok_or_else(|| DomainError::PermissionNotFound.into())
    }

    /// 移除一列（p_zxw.4gl:797-818，X4）
    pub async fn remove(&self, actor: &Actor, user: &str, line_no: i32) -> Result<(), AppError> {
        // TipTop 在單身刪除列：需 detail 與單身刪除權限（AQ3、AQ8）
        self.authorize(actor, "detail").await?.delete_row()?;
        let user = self.user(user).await?;
        if self
            .perms
            .delete(&user.user_id, line_no)
            .await
            .map_err(infra)?
        {
            Ok(())
        } else {
            Err(DomainError::PermissionNotFound.into())
        }
    }

    /// 刪除使用者的全部特殊權限（p_zxw.4gl:494-530，X4）：之後回到使用者維護的權限類別
    pub async fn remove_all(
        &self,
        actor: &Actor,
        user: &str,
    ) -> Result<PermissionBatchResultDto, AppError> {
        self.authorize(actor, "delete").await?;
        let user = self.user(user).await?;
        let removed = self.perms.delete_all(&user.user_id).await.map_err(infra)?;
        Ok(PermissionBatchResultDto {
            removed,
            ..Default::default()
        })
    }

    /// 複製到另一個使用者（p_zxw.4gl:1167-1237）：目標須存在且尚無特殊權限（X5）
    pub async fn copy(
        &self,
        actor: &Actor,
        from: &str,
        to: &str,
    ) -> Result<PermissionBatchResultDto, AppError> {
        self.authorize(actor, "reproduce").await?;
        let from = self.user(from).await?;
        let to = self.user(to).await?;
        if !self.existing(&to.user_id).await?.is_empty() {
            return Err(DomainError::UserHasSpecialPermissions.into());
        }
        let mut added = 0;
        for row in self.existing(&from.user_id).await? {
            let copy = row.copy_to(to.user_id.clone(), &actor.user_id, &actor.dept, today());
            self.insert(&copy).await?;
            added += 1;
        }
        Ok(PermissionBatchResultDto {
            added,
            ..Default::default()
        })
    }

    async fn existing(&self, user: &UserId) -> Result<Vec<UserPermission>, AppError> {
        Ok(self
            .perms
            .list(user)
            .await
            .map_err(infra)?
            .into_iter()
            .map(|i| i.permission)
            .collect())
    }

    async fn insert(&self, row: &UserPermission) -> Result<(), AppError> {
        match self.perms.insert(row).await {
            Ok(()) => Ok(()),
            Err(RepoError::Conflict) => Err(DomainError::UserPermissionDuplicate.into()),
            Err(e) => Err(infra(e)),
        }
    }

    fn row(
        &self,
        actor: &Actor,
        user: &UserId,
        line_no: i32,
        target: SpecialTarget,
    ) -> UserPermission {
        UserPermission::new(
            user.clone(),
            line_no,
            target,
            &actor.user_id,
            &actor.dept,
            today(),
        )
    }

    /// 檢核一列的對象（X2）：權限類別須存在且有效；程式須存在且非選單，功能只能選程式提供的
    async fn target(&self, input: &UserPermissionInput) -> Result<SpecialTarget, AppError> {
        match input.kind {
            SpecialPermissionKindDto::Class => {
                let code = ClassCode::parse(&input.target)
                    .map_err(|_| DomainError::PermissionClassNotFound)?;
                let class = self
                    .classes
                    .get(&code)
                    .await
                    .map_err(infra)?
                    .ok_or(DomainError::PermissionClassNotFound)?;
                if !class.active {
                    return Err(DomainError::PermissionClassInactive.into());
                }
                Ok(SpecialTarget::Class(class.code))
            }
            SpecialPermissionKindDto::Program => {
                let program = self.program(&input.target).await?;
                let offered = parse_actions(program.other.default_actions.as_deref());
                let attrs = program_attrs(&PermissionAttrs::defaults(&offered), &offered, input)?;
                Ok(SpecialTarget::Program {
                    program: program.program_no,
                    attrs,
                })
            }
        }
    }

    async fn program(&self, no: &str) -> Result<ttrs_domain::program::Program, AppError> {
        let no = ProgramNo::parse(no)?;
        let program = self
            .programs
            .get(&no)
            .await
            .map_err(infra)?
            .ok_or(DomainError::ProgramNotFound)?;
        if program.module.is_menu() {
            return Err(DomainError::MenuHasNoPermission.into());
        }
        Ok(program)
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }

    async fn user(&self, id: &str) -> Result<User, AppError> {
        let id = UserId::parse(id).map_err(|_| DomainError::UserNotFound)?;
        self.users
            .get(&id)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::UserNotFound.into())
    }
}

/// 使用者改權限類別時，特殊權限中的原權限類別列改為新權限類別（p_zx.4gl:1003-1018，X6）；
/// 已有新權限類別的列時，刪除原權限類別列，不重複
pub(crate) async fn move_special_class(
    perms: &dyn UserPermissionRepository,
    actor: &Actor,
    user: &UserId,
    old: &str,
    new: &str,
) -> Result<(), AppError> {
    let (Ok(old), Ok(new)) = (ClassCode::parse(old), ClassCode::parse(new)) else {
        return Ok(());
    };
    let rows: Vec<UserPermission> = perms
        .list(user)
        .await
        .map_err(infra)?
        .into_iter()
        .map(|i| i.permission)
        .collect();
    let Some(mut row) = rows
        .iter()
        .find(|r| r.target.class() == Some(&old))
        .cloned()
    else {
        return Ok(());
    };
    if rows.iter().any(|r| r.target.class() == Some(&new)) {
        perms.delete(user, row.line_no).await.map_err(infra)?;
    } else {
        row.move_class(new, &actor.user_id, today());
        perms.update(&row).await.map_err(infra)?;
    }
    Ok(())
}

fn program_attrs(
    base: &PermissionAttrs,
    offered: &[String],
    i: &UserPermissionInput,
) -> Result<PermissionAttrs, AppError> {
    Ok(PermissionAttrs::new(
        base,
        offered,
        i.actions.as_deref(),
        i.other_user_data.as_deref(),
        i.other_dept_data.as_deref(),
        i.detail_rows.as_deref(),
    )?)
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

fn to_dto(item: &UserPermissionItem, lang: &Lang) -> UserPermissionDto {
    let p = &item.permission;
    let (kind, target, target_name, available_actions, attrs) = match &p.target {
        SpecialTarget::Class(c) => (
            SpecialPermissionKindDto::Class,
            c.as_str().to_owned(),
            item.class_name.clone(),
            Vec::new(),
            None,
        ),
        SpecialTarget::Program { program, attrs } => (
            SpecialPermissionKindDto::Program,
            program.as_str().to_owned(),
            localized(&item.names, lang, |n| non_empty(&n.name)),
            parse_actions(item.default_actions.as_deref()),
            Some(attrs),
        ),
    };
    UserPermissionDto {
        user_id: p.user_id.as_str().to_owned(),
        line_no: p.line_no,
        kind,
        target,
        target_name,
        available_actions,
        actions: attrs.map(|a| a.actions.clone()).unwrap_or_default(),
        other_user_data: attrs.map(|a| a.other_user.code().to_owned()),
        other_dept_data: attrs.map(|a| a.other_dept.code().to_owned()),
        detail_rows: attrs.map(|a| a.detail.code().to_owned()),
        created_by: p.created_by.clone(),
        created_dept: p.created_dept.clone(),
        created_on: p.created_on,
        updated_by: p.updated_by.clone(),
        updated_on: p.updated_on,
    }
}
