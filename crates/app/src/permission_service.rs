//! Program Permission Maintenance 程式執行權限維護 use case（TipTop: p_zy）
//! 規格：specs/features/azz/program_permission_maintenance.feature（Y1–Y12）；權限框架：docs/adr/0006 §4
//!
//! 只維護權限資料；依資料執行檢查見 authorization.rs。本作業 TipTop 沒有設定資料所有者，不檢查資料權限（AQ5）。

use std::{collections::HashSet, sync::Arc};

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    lang::{DEFAULT_LANG, Lang, localized, non_empty},
    permission::{PermissionAttrs, ProgramPermission, parse_actions},
    permission_class::{ClassCode, PermissionClass},
    program::{Program, ProgramNo},
};
use ttrs_shared::{
    Page, PermissionBatchResultDto, PermissionClassDto, ProgramPermissionDto,
    ProgramPermissionInput, QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    permission_class_service::to_dto as class_dto,
    ports::{
        PermissionClassRepository, PermissionListItem, ProgramPermissionRepository,
        ProgramRepository,
    },
    query,
};

/// 選單展開的最大層數（TipTop p_zy_bom mfg2733）
const MAX_MENU_DEPTH: usize = 20;
/// 權限檢查用的程式代號
const PROGRAM: &str = "program-permission-maintenance";

#[derive(Clone)]
pub struct ProgramPermissionService {
    authz: Arc<dyn Authorizer>,
    perms: Arc<dyn ProgramPermissionRepository>,
    programs: Arc<dyn ProgramRepository>,
    classes: Arc<dyn PermissionClassRepository>,
}

impl ProgramPermissionService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        perms: Arc<dyn ProgramPermissionRepository>,
        programs: Arc<dyn ProgramRepository>,
        classes: Arc<dyn PermissionClassRepository>,
    ) -> Self {
        Self {
            authz,
            perms,
            programs,
            classes,
        }
    }

    /// QBE 查詢（p_zy_curs，p_zy.4gl:143-203）：符合條件的權限類別（單頭）；欄位見
    /// query::program_permission（YQ1、YQ2）
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<PermissionClassDto>, AppError> {
        self.authorize(actor, "query").await?;
        let (header, detail) = query::program_permission::filters(&req.conditions)?;
        let p = query::paging(&req);
        let (classes, total) = self
            .perms
            .query_classes(&header, &detail, p.offset, u64::from(p.page_size))
            .await
            .map_err(infra)?;
        Ok(Page {
            items: classes.iter().map(class_dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 權限類別符合查詢條件的程式（p_zy_b_fill(g_wc)，p_zy.4gl:938-955，YQ3）：只套用單身條件，
    /// 程式名稱依 `req.lang` 顯示
    pub async fn query_class(
        &self,
        actor: &Actor,
        class: &str,
        req: QueryRequest,
    ) -> Result<Vec<ProgramPermissionDto>, AppError> {
        self.authorize(actor, "query").await?;
        let (_, detail) = query::program_permission::filters(&req.conditions)?;
        let class = self.class(class).await?;
        let lang = Lang::parse(req.lang.as_deref().unwrap_or(DEFAULT_LANG))?;
        let items = self.perms.list(&class.code).await.map_err(infra)?;
        Ok(items
            .iter()
            .filter(|i| {
                query::matches_all(&detail, |f| {
                    query::program_permission::value(&i.permission, f)
                })
            })
            .map(|i| to_dto(i, &lang))
            .collect())
    }

    /// 權限類別的全部程式權限（依程式代號排序），程式名稱依 `lang` 顯示
    pub async fn list(
        &self,
        actor: &Actor,
        class: &str,
        lang: &str,
    ) -> Result<Vec<ProgramPermissionDto>, AppError> {
        self.authorize(actor, "query").await?;
        self.list_in(class, lang).await
    }

    pub async fn get(
        &self,
        actor: &Actor,
        class: &str,
        program: &str,
        lang: &str,
    ) -> Result<ProgramPermissionDto, AppError> {
        self.authorize(actor, "query").await?;
        self.find(class, program, lang).await
    }

    /// 新增（p_zy.4gl:700-741）：權限類別須有效、程式須存在且非選單；未指定的值用預設（Y3）
    pub async fn grant(
        &self,
        actor: &Actor,
        class: &str,
        input: ProgramPermissionInput,
    ) -> Result<ProgramPermissionDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            // TipTop 在單身新增列：需 detail 與單身新增權限（AQ3、AQ8）
            self.authorize(actor, "detail").await?.add_row()?;
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
                    self.find(class.code.as_str(), p.program.as_str(), DEFAULT_LANG)
                        .await
                }
                Err(RepoError::Conflict) => Err(DomainError::PermissionAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 修改（p_zy.4gl:785-817）：未指定的值沿用原值；記錄修改者（Y12）
    pub async fn update(
        &self,
        actor: &Actor,
        class: &str,
        program: &str,
        input: ProgramPermissionInput,
    ) -> Result<ProgramPermissionDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let class = self.active_class(class).await?;
            let program = self.program(program).await?;
            let mut p = self
                .perms
                .get(&class.code, &program.program_no)
                .await
                .map_err(infra)?
                .ok_or(DomainError::PermissionNotFound)?;
            p.stamp.version = input.version.unwrap_or(p.stamp.version);
            let offered = offered_actions(&program);
            let attrs = attrs(&p.attrs, &offered, &input)?;
            p.update(attrs, &actor.user_id, today());
            self.perms.update(&p).await.map_err(infra)?;
            self.find(class.code.as_str(), p.program.as_str(), DEFAULT_LANG)
                .await
        })
        .await
    }

    /// 移除單一程式權限（p_zy.4gl:743-763，Y7 不另外檢查）
    pub async fn revoke(&self, actor: &Actor, class: &str, program: &str) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            // TipTop 在單身刪除列：需 detail 與單身刪除權限（AQ3、AQ8）
            self.authorize(actor, "detail").await?.delete_row()?;
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
        })
        .await
    }

    /// 刪除權限類別的全部權限（p_zy.4gl:561-593）：仍有使用者時拒絕（azz-217，Y7）
    pub async fn revoke_all(
        &self,
        actor: &Actor,
        class: &str,
    ) -> Result<PermissionBatchResultDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            self.authorize(actor, "delete").await?;
            let class = self.class(class).await?;
            if self.classes.user_count(&class.code).await.map_err(infra)? > 0 {
                return Err(DomainError::PermissionClassInUse.into());
            }
            let removed = self.perms.delete_all(&class.code).await.map_err(infra)?;
            Ok(PermissionBatchResultDto {
                removed,
                ..Default::default()
            })
        })
        .await
    }

    /// 複製到另一個權限類別（p_zy.4gl:1164-1255）：目標須有效且尚無權限（Y8）
    pub async fn copy(
        &self,
        actor: &Actor,
        from: &str,
        to: &str,
    ) -> Result<PermissionBatchResultDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
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
                let p =
                    item.permission
                        .copy_to(to.code.clone(), &actor.user_id, &actor.dept, today());
                self.perms.insert(&p).await.map_err(infra)?;
                added += 1;
            }
            Ok(PermissionBatchResultDto {
                added,
                ..Default::default()
            })
        })
        .await
    }

    /// 依程式整批產生（p_zy.4gl:1257-1488）：已有的程式略過（Y9）
    pub async fn generate(
        &self,
        actor: &Actor,
        class: &str,
        program_nos: &[String],
    ) -> Result<PermissionBatchResultDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "batch_gen_by_prog", async {
            self.authorize(actor, "batch_gen_by_prog").await?;
            let class = self.active_class(class).await?;
            let mut programs = Vec::with_capacity(program_nos.len());
            for no in program_nos {
                programs.push(self.program(no).await?);
            }
            self.add_defaults(actor, &class, &programs, false).await
        })
        .await
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
        crate::change_log::run(actor, PROGRAM, "batch_gen_by_menu", async {
            self.authorize(actor, "batch_gen_by_menu").await?;
            let class = self.active_class(class).await?;
            let programs = self.menu_programs(menu).await?;
            self.add_defaults(actor, &class, &programs, reset_existing)
                .await
        })
        .await
    }

    /// 依選單整批刪除（p_zy.4gl:1696-1857，Y11）
    pub async fn delete_by_menu(
        &self,
        actor: &Actor,
        class: &str,
        menu: &str,
    ) -> Result<PermissionBatchResultDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "batch_del_by_menu", async {
            self.authorize(actor, "batch_del_by_menu").await?;
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
        })
        .await
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

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }

    /// 權限類別的全部程式權限（依程式代號排序），程式名稱依 `lang` 顯示
    async fn list_in(
        &self,
        class: &str,
        lang: &str,
    ) -> Result<Vec<ProgramPermissionDto>, AppError> {
        let class = self.class(class).await?;
        let lang = Lang::parse(lang)?;
        let items = self.perms.list(&class.code).await.map_err(infra)?;
        Ok(items.iter().map(|i| to_dto(i, &lang)).collect())
    }

    async fn find(
        &self,
        class: &str,
        program: &str,
        lang: &str,
    ) -> Result<ProgramPermissionDto, AppError> {
        let program = ProgramNo::parse(program)?;
        self.list_in(class, lang)
            .await?
            .into_iter()
            .find(|p| p.program_no == program.as_str())
            .ok_or_else(|| DomainError::PermissionNotFound.into())
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
    match e {
        // 版本不符：他人已修改（樂觀鎖，ADR 0012 §4）
        RepoError::Stale => ttrs_domain::DomainError::RecordChanged.into(),
        e => AppError::Infra(e.to_string()),
    }
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
        version: p.stamp.version,
        created_at: p.stamp.created_at,
        updated_at: p.stamp.updated_at,
    }
}
