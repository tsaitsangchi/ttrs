//! Program Permission Maintenance 程式執行權限維護 use case（TipTop: p_zy）
//! 規格：specs/features/azz/program_permission_maintenance.feature（Y1–Y12）；權限框架：docs/adr/0006 §4
//!
//! 只維護權限資料；依資料執行檢查（Authorizer、資料權限、查詢過濾）見 ADR 0006。

use std::{collections::HashSet, sync::Arc};

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    lang::{DEFAULT_LANG, Lang, localized, non_empty},
    permission::{PermissionAttrs, ProgramPermission, parse_actions},
    permission_class::{ClassCode, PermissionClass},
    program::{Program, ProgramNo},
};
use ttrs_shared::{PermissionBatchResultDto, ProgramPermissionDto, ProgramPermissionInput};

use crate::{
    Actor, AppError,
    error::RepoError,
    ports::{
        PermissionClassRepository, PermissionListItem, ProgramPermissionRepository,
        ProgramRepository,
    },
};

/// 選單展開的最大層數（TipTop p_zy_bom mfg2733）
const MAX_MENU_DEPTH: usize = 20;

#[derive(Clone)]
pub struct ProgramPermissionService {
    perms: Arc<dyn ProgramPermissionRepository>,
    programs: Arc<dyn ProgramRepository>,
    classes: Arc<dyn PermissionClassRepository>,
}

impl ProgramPermissionService {
    pub fn new(
        perms: Arc<dyn ProgramPermissionRepository>,
        programs: Arc<dyn ProgramRepository>,
        classes: Arc<dyn PermissionClassRepository>,
    ) -> Self {
        Self {
            perms,
            programs,
            classes,
        }
    }

    /// 權限類別的全部程式權限（依程式代號排序），程式名稱依 `lang` 顯示
    pub async fn list(
        &self,
        class: &str,
        lang: &str,
    ) -> Result<Vec<ProgramPermissionDto>, AppError> {
        let class = self.class(class).await?;
        let lang = Lang::parse(lang)?;
        let items = self.perms.list(&class.code).await.map_err(infra)?;
        Ok(items.iter().map(|i| to_dto(i, &lang)).collect())
    }

    pub async fn get(
        &self,
        class: &str,
        program: &str,
        lang: &str,
    ) -> Result<ProgramPermissionDto, AppError> {
        let program = ProgramNo::parse(program)?;
        self.list(class, lang)
            .await?
            .into_iter()
            .find(|p| p.program_no == program.as_str())
            .ok_or_else(|| DomainError::PermissionNotFound.into())
    }

    /// 新增（p_zy.4gl:700-741）：權限類別須有效、程式須存在且非選單；未指定的值用預設（Y3）
    pub async fn grant(
        &self,
        actor: &Actor,
        class: &str,
        input: ProgramPermissionInput,
    ) -> Result<ProgramPermissionDto, AppError> {
        let class = self.active_class(class).await?;
        let program = self.program(&input.program_no).await?;
        let offered = offered_actions(&program);
        let attrs = attrs(&PermissionAttrs::defaults(&offered), &offered, &input)?;
        let p = ProgramPermission::new(
            class.code.clone(),
            program.program_no,
            attrs,
            &actor.user_id,
            &actor.dept,
            today(),
        );
        match self.perms.insert(&p).await {
            Ok(()) => {
                self.get(class.code.as_str(), p.program.as_str(), DEFAULT_LANG)
                    .await
            }
            Err(RepoError::Conflict) => Err(DomainError::PermissionAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    /// 修改（p_zy.4gl:785-817）：未指定的值沿用原值；記錄修改者（Y12）
    pub async fn update(
        &self,
        actor: &Actor,
        class: &str,
        program: &str,
        input: ProgramPermissionInput,
    ) -> Result<ProgramPermissionDto, AppError> {
        let class = self.active_class(class).await?;
        let program = self.program(program).await?;
        let mut p = self
            .perms
            .get(&class.code, &program.program_no)
            .await
            .map_err(infra)?
            .ok_or(DomainError::PermissionNotFound)?;
        let offered = offered_actions(&program);
        let attrs = attrs(&p.attrs, &offered, &input)?;
        p.update(attrs, &actor.user_id, today());
        self.perms.update(&p).await.map_err(infra)?;
        self.get(class.code.as_str(), p.program.as_str(), DEFAULT_LANG)
            .await
    }

    /// 移除單一程式權限（p_zy.4gl:743-763，Y7 不另外檢查）
    pub async fn revoke(&self, class: &str, program: &str) -> Result<(), AppError> {
        let class = self.class(class).await?;
        let program = ProgramNo::parse(program)?;
        if self
            .perms
            .delete(&class.code, &program)
            .await
            .map_err(infra)?
        {
            Ok(())
        } else {
            Err(DomainError::PermissionNotFound.into())
        }
    }

    /// 刪除權限類別的全部權限（p_zy.4gl:561-593）：仍有使用者時拒絕（azz-217，Y7）
    pub async fn revoke_all(&self, class: &str) -> Result<PermissionBatchResultDto, AppError> {
        let class = self.class(class).await?;
        if self.classes.user_count(&class.code).await.map_err(infra)? > 0 {
            return Err(DomainError::PermissionClassInUse.into());
        }
        let removed = self.perms.delete_all(&class.code).await.map_err(infra)?;
        Ok(PermissionBatchResultDto {
            removed,
            ..Default::default()
        })
    }

    /// 複製到另一個權限類別（p_zy.4gl:1164-1255）：目標須有效且尚無權限（Y8）
    pub async fn copy(
        &self,
        actor: &Actor,
        from: &str,
        to: &str,
    ) -> Result<PermissionBatchResultDto, AppError> {
        let from = self.class(from).await?;
        let to = self.active_class(to).await?;
        if self
            .classes
            .permission_count(&to.code)
            .await
            .map_err(infra)?
            > 0
        {
            return Err(DomainError::PermissionClassHasPermissions.into());
        }
        let mut added = 0;
        for item in self.perms.list(&from.code).await.map_err(infra)? {
            let p = item
                .permission
                .copy_to(to.code.clone(), &actor.user_id, &actor.dept, today());
            self.perms.insert(&p).await.map_err(infra)?;
            added += 1;
        }
        Ok(PermissionBatchResultDto {
            added,
            ..Default::default()
        })
    }

    /// 依程式整批產生（p_zy.4gl:1257-1488）：已有的程式略過（Y9）
    pub async fn generate(
        &self,
        actor: &Actor,
        class: &str,
        program_nos: &[String],
    ) -> Result<PermissionBatchResultDto, AppError> {
        let class = self.active_class(class).await?;
        let mut programs = Vec::with_capacity(program_nos.len());
        for no in program_nos {
            programs.push(self.program(no).await?);
        }
        self.add_defaults(actor, &class, &programs, false).await
    }

    /// 依選單整批產生（p_zy.4gl:1490-1694）：含子選單內的程式，選單本身不設定（Y6）；
    /// 已有的權限預設不變，`reset_existing` 時重設為預設值（Y10）
    pub async fn generate_by_menu(
        &self,
        actor: &Actor,
        class: &str,
        menu: &str,
        reset_existing: bool,
    ) -> Result<PermissionBatchResultDto, AppError> {
        let class = self.active_class(class).await?;
        let programs = self.menu_programs(menu).await?;
        self.add_defaults(actor, &class, &programs, reset_existing)
            .await
    }

    /// 依選單整批刪除（p_zy.4gl:1696-1857，Y11）
    pub async fn delete_by_menu(
        &self,
        class: &str,
        menu: &str,
    ) -> Result<PermissionBatchResultDto, AppError> {
        let class = self.class(class).await?;
        let mut removed = 0;
        for p in self.menu_programs(menu).await? {
            if self
                .perms
                .delete(&class.code, &p.program_no)
                .await
                .map_err(infra)?
            {
                removed += 1;
            }
        }
        Ok(PermissionBatchResultDto {
            removed,
            ..Default::default()
        })
    }

    async fn add_defaults(
        &self,
        actor: &Actor,
        class: &PermissionClass,
        programs: &[Program],
        reset_existing: bool,
    ) -> Result<PermissionBatchResultDto, AppError> {
        let mut result = PermissionBatchResultDto::default();
        for program in programs {
            let defaults = PermissionAttrs::defaults(&offered_actions(program));
            match self
                .perms
                .get(&class.code, &program.program_no)
                .await
                .map_err(infra)?
            {
                None => {
                    let p = ProgramPermission::new(
                        class.code.clone(),
                        program.program_no.clone(),
                        defaults,
                        &actor.user_id,
                        &actor.dept,
                        today(),
                    );
                    self.perms.insert(&p).await.map_err(infra)?;
                    result.added += 1;
                }
                Some(mut p) if reset_existing => {
                    p.update(defaults, &actor.user_id, today());
                    self.perms.update(&p).await.map_err(infra)?;
                    result.reset += 1;
                }
                Some(_) => {}
            }
        }
        Ok(result)
    }

    /// 選單內的全部可執行程式（含子選單，最多 20 層；選單本身不列入）
    async fn menu_programs(&self, menu: &str) -> Result<Vec<Program>, AppError> {
        let invalid = || DomainError::MenuProgramInvalid(menu.trim().to_owned());
        let root = ProgramNo::parse(menu).map_err(|_| invalid())?;
        let root = self
            .programs
            .get(&root)
            .await
            .map_err(infra)?
            .filter(|p| p.module.is_menu())
            .ok_or_else(invalid)?;
        let mut seen: HashSet<ProgramNo> = HashSet::from([root.program_no.clone()]);
        let mut level = vec![root.program_no];
        let mut out = Vec::new();
        for _ in 0..MAX_MENU_DEPTH {
            let mut next = Vec::new();
            for m in &level {
                for child in self
                    .programs
                    .menu_children(m.as_str())
                    .await
                    .map_err(infra)?
                {
                    if !seen.insert(child.program_no.clone()) {
                        continue;
                    }
                    if child.module.is_menu() {
                        next.push(child.program_no);
                    } else {
                        out.push(child);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            level = next;
        }
        Ok(out)
    }

    async fn class(&self, code: &str) -> Result<PermissionClass, AppError> {
        let code = ClassCode::parse(code).map_err(|_| DomainError::PermissionClassNotFound)?;
        self.classes
            .get(&code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::PermissionClassNotFound.into())
    }

    /// 權限類別須存在且有效（azz-218，Y5）
    async fn active_class(&self, code: &str) -> Result<PermissionClass, AppError> {
        let class = self.class(code).await?;
        if !class.active {
            return Err(DomainError::PermissionClassInactive.into());
        }
        Ok(class)
    }

    /// 程式須存在（Y5）且非選單（Y6）
    async fn program(&self, no: &str) -> Result<Program, AppError> {
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
}

/// 程式提供的功能（zz04，Y1）
fn offered_actions(program: &Program) -> Vec<String> {
    parse_actions(program.other.default_actions.as_deref())
}

fn attrs(
    base: &PermissionAttrs,
    offered: &[String],
    i: &ProgramPermissionInput,
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

fn to_dto(item: &PermissionListItem, lang: &Lang) -> ProgramPermissionDto {
    let p = &item.permission;
    let a = &p.attrs;
    ProgramPermissionDto {
        class_code: p.class.as_str().to_owned(),
        program_no: p.program.as_str().to_owned(),
        program_name: localized(&item.names, lang, |n| non_empty(&n.name)),
        available_actions: parse_actions(item.default_actions.as_deref()),
        actions: a.actions.clone(),
        other_user_data: a.other_user.code().to_owned(),
        other_dept_data: a.other_dept.code().to_owned(),
        detail_rows: a.detail.code().to_owned(),
        created_by: p.created_by.clone(),
        created_dept: p.created_dept.clone(),
        created_on: p.created_on,
        updated_by: p.updated_by.clone(),
        updated_on: p.updated_on,
    }
}
