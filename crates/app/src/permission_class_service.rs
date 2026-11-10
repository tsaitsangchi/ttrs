//! Permission Class Maintenance 權限類別維護 use case（TipTop: p_zw）
//! 規格：specs/features/azz/permission_class_maintenance.feature（W1–W11）；權限框架：docs/adr/0006

use crate::list_report::{ListReporter, ListSpec};
use std::{collections::HashMap, sync::Arc};

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    permission_class::{ClassCode, ClassIdleControl, PermissionClass, PermissionClassAttrs},
    program::{IdleAction, ProgramNo},
};
use ttrs_shared::{
    ClassIdleControlDto, IdleActionDto, Page, PermissionClassDto, PermissionClassInput,
    QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    ports::{PermissionClassRepository, ProgramRepository, UserRepository},
    query,
};

/// 權限檢查用的程式代號
const PROGRAM: &str = "permission-class-maintenance";

#[derive(Clone)]
pub struct PermissionClassService {
    authz: Arc<dyn Authorizer>,
    classes: Arc<dyn PermissionClassRepository>,
    programs: Arc<dyn ProgramRepository>,
    users: Arc<dyn UserRepository>,
    /// 列印、匯出 Excel（ADR 0013 SA3）
    reports: ListReporter,
}

/// 清單報表（ADR 0013 SA3；permission_class_maintenance_standard_actions.feature）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "class-report-title",
    columns: &[
        ("class_code", "class-col-code"),
        ("name", "class-col-name"),
        ("default_menu", "class-col-menu"),
        ("active", "class-col-active"),
    ],
};

impl PermissionClassService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        classes: Arc<dyn PermissionClassRepository>,
        programs: Arc<dyn ProgramRepository>,
        users: Arc<dyn UserRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            classes,
            programs,
            users,
            reports,
        }
    }

    /// 複製（ADR 0013 SA5；p_zw 原本沒有）：新代號、全部欄位；程式執行權限不複製
    pub async fn copy(
        &self,
        actor: &Actor,
        code: &str,
        new_code: &str,
    ) -> Result<PermissionClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load(code).await?;
            let new_code = ClassCode::parse(new_code)?;
            let class = source.copy_as(new_code, &actor.user_id, &actor.dept, today());
            match self.classes.insert(&class).await {
                Ok(()) => Ok(to_dto(&class)),
                Err(RepoError::Conflict) => Err(DomainError::PermissionClassAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        code: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<PermissionClassDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let mut class = self.load(code).await?;
            class.stamp.check(version)?;
            class.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.classes.update(&class).await.map_err(infra)?;
            Ok(to_dto(&self.load(code).await?))
        })
        .await
    }

    /// 列印 output、匯出 Excel exporttoexcel（ADR 0013 SA3；p_zw 原本沒有）：依查詢條件、權限類別代號排序
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<ttrs_shared::QbeCondition>,
        format: crate::report::ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<crate::report::ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::permission_class::filters(&conditions)?;
        let (rows, total) = self
            .classes
            .query(&filters, 0, self.reports.fetch_limit())
            .await
            .map_err(infra)?;
        self.reports.check(total)?;
        let t = |id: &str| crate::report::text(lang, id);
        let lines = rows
            .iter()
            .map(|c| {
                vec![
                    c.code.as_str().to_owned(),
                    c.attrs.name.clone(),
                    c.attrs.default_menu.clone().unwrap_or_default(),
                    t(if c.active {
                        "report-active"
                    } else {
                        "report-inactive"
                    }),
                ]
            })
            .collect();
        self.reports
            .build(actor, None, &REPORT, lines, &conditions, format, lang)
            .await
    }

    /// 測試用：調整同步產生報表的筆數上限
    pub fn with_report_limit(mut self, limit: u64) -> Self {
        self.reports.limit = limit;
        self
    }

    /// 新增：不自動加入基本程式權限（W8，TipTop p_zw.4gl:333-343 不移植）。
    /// TipTop 在單身新增列，需 detail 與單身新增權限（AQ3、AQ8）
    pub async fn create(
        &self,
        actor: &Actor,
        input: PermissionClassInput,
    ) -> Result<PermissionClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let code = ClassCode::parse(&input.class_code)?;
            let attrs = self.attrs(&input).await?;
            let class = PermissionClass::new(code, attrs, &actor.user_id, &actor.dept, today());
            match self.classes.insert(&class).await {
                Ok(()) => Ok(to_dto(&class)),
                Err(RepoError::Conflict) => Err(DomainError::PermissionClassAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 修改：代號不可變（W1）
    pub async fn update(
        &self,
        actor: &Actor,
        code: &str,
        input: PermissionClassInput,
    ) -> Result<PermissionClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut class = self.load(code).await?;
            class.stamp.version = input.version.unwrap_or(class.stamp.version);
            let requested = input.class_code.trim();
            if !requested.is_empty() && requested != class.code.as_str() {
                return Err(DomainError::PermissionClassCodeImmutable.into());
            }
            let attrs = self.attrs(&input).await?;
            class.update(attrs, &actor.user_id, today());
            self.classes.update(&class).await.map_err(infra)?;
            class.stamp.version += 1;
            Ok(to_dto(&class))
        })
        .await
    }

    pub async fn get(&self, actor: &Actor, code: &str) -> Result<PermissionClassDto, AppError> {
        self.authorize(actor, "query").await?;
        Ok(to_dto(&self.load(code).await?))
    }

    /// QBE 查詢（p_zw_curs，p_zw.4gl:185-241）；欄位見 query::permission_class（WQ1–WQ3）
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<PermissionClassDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::permission_class::filters(&req.conditions)?;
        let p = query::paging(&req);
        let (rows, total) = self
            .classes
            .query(&filters, p.offset, u64::from(p.page_size))
            .await
            .map_err(infra)?;
        let items = rows.iter().map(to_dto).collect();
        Ok(Page {
            items,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 全部清單：下拉選單、欄位檢查用的參考資料，只要求登入（AQ4）
    pub async fn list(&self) -> Result<Vec<PermissionClassDto>, AppError> {
        let classes = self.classes.list().await.map_err(infra)?;
        Ok(classes.iter().map(to_dto).collect())
    }

    /// 刪除：仍有使用者（azz-217）或程式權限（azz-005）時拒絕，不做連帶刪除（W7，p_zw.4gl:537-582）。
    /// TipTop 在單身刪除列，需 detail 與單身刪除權限（AQ3、AQ8）
    pub async fn delete(
        &self,
        actor: &Actor,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let class = self.load(code).await?;
            class.stamp.check(version)?;
            self.ensure_unused(&class.code).await?;
            if self
                .classes
                .permission_count(&class.code)
                .await
                .map_err(infra)?
                > 0
            {
                return Err(DomainError::PermissionClassHasPermissions.into());
            }
            // 已確認不可刪除，須先取消確認（ADR 0013 SA9-2）；仍被使用時先回報使用中
            class.confirmation.check_deletable()?;
            self.classes.delete(&class.code).await.map_err(infra)
        })
        .await
    }

    /// 停用：仍有使用者時拒絕（W6，p_zw.4gl:511-535）
    pub async fn deactivate(
        &self,
        actor: &Actor,
        code: &str,
        version: Option<i32>,
    ) -> Result<PermissionClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut class = self.load(code).await?;
            class.stamp.version = version.unwrap_or(class.stamp.version);
            crate::change_log::set_operation("deactivate");
            self.ensure_unused(&class.code).await?;
            class.set_active(false, &actor.user_id, today());
            self.classes.update(&class).await.map_err(infra)?;
            class.stamp.version += 1;
            Ok(to_dto(&class))
        })
        .await
    }

    pub async fn activate(
        &self,
        actor: &Actor,
        code: &str,
        version: Option<i32>,
    ) -> Result<PermissionClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut class = self.load(code).await?;
            class.stamp.version = version.unwrap_or(class.stamp.version);
            crate::change_log::set_operation("activate");
            class.set_active(true, &actor.user_id, today());
            self.classes.update(&class).await.map_err(infra)?;
            class.stamp.version += 1;
            Ok(to_dto(&class))
        })
        .await
    }

    /// 更新所有使用者目錄編號（p_zw.4gl:761-776）：有預設選單的權限類別，其使用者的選單改為預設選單；
    /// 只更新選單不同的使用者並記錄修改者（W9）。回傳更新的使用者數。
    /// TipTop 是單身內的動作 update_all_user_menu（p_zw.4gl:682-683），需 detail（AQ3）
    pub async fn update_user_menus(&self, actor: &Actor) -> Result<u64, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let menus: HashMap<String, String> = self
                .classes
                .list()
                .await
                .map_err(infra)?
                .into_iter()
                .filter_map(|c| Some((c.code.as_str().to_owned(), c.attrs.default_menu?)))
                .collect();
            let mut updated = 0;
            for mut user in self.users.list().await.map_err(infra)? {
                let Some(menu) = menus.get(&user.attrs.permission_class) else {
                    continue;
                };
                if &user.attrs.menu == menu {
                    continue;
                }
                user.set_menu(menu, &actor.user_id, today());
                let plants = self.users.plants(&user.user_id).await.map_err(infra)?;
                self.users.update(&user, &plants).await.map_err(infra)?;
                updated += 1;
            }
            Ok(updated)
        })
        .await
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }

    async fn load(&self, code: &str) -> Result<PermissionClass, AppError> {
        let code = ClassCode::parse(code).map_err(|_| DomainError::PermissionClassNotFound)?;
        self.classes
            .get(&code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::PermissionClassNotFound.into())
    }

    /// 仍有使用者屬於此權限類別時不可停用或刪除（azz-217）
    async fn ensure_unused(&self, code: &ClassCode) -> Result<(), AppError> {
        if self.classes.user_count(code).await.map_err(infra)? > 0 {
            return Err(DomainError::PermissionClassInUse.into());
        }
        Ok(())
    }

    /// 檢核輸入並組出欄位：預設選單須為 MENU 程式（W4，p_zw.4gl:489-497）
    async fn attrs(&self, i: &PermissionClassInput) -> Result<PermissionClassAttrs, AppError> {
        let attrs = PermissionClassAttrs::new(
            i.name.clone(),
            i.default_menu.clone(),
            idle_control(i.idle_control.unwrap_or_default()),
            i.idle_action.map(idle_action),
            i.idle_seconds,
        )?;
        if let Some(menu) = &attrs.default_menu {
            let ok = match ProgramNo::parse(menu) {
                Ok(no) => self
                    .programs
                    .get(&no)
                    .await
                    .map_err(infra)?
                    .is_some_and(|p| p.module.is_menu()),
                Err(_) => false,
            };
            if !ok {
                return Err(DomainError::PermissionClassMenuInvalid(menu.clone()).into());
            }
        }
        Ok(attrs)
    }
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

fn idle_control(v: ClassIdleControlDto) -> ClassIdleControl {
    match v {
        ClassIdleControlDto::Off => ClassIdleControl::Off,
        ClassIdleControlDto::ByClass => ClassIdleControl::ByClass,
        ClassIdleControlDto::BySystem => ClassIdleControl::BySystem,
    }
}

fn idle_control_dto(v: ClassIdleControl) -> ClassIdleControlDto {
    match v {
        ClassIdleControl::Off => ClassIdleControlDto::Off,
        ClassIdleControl::ByClass => ClassIdleControlDto::ByClass,
        ClassIdleControl::BySystem => ClassIdleControlDto::BySystem,
    }
}

fn idle_action(v: IdleActionDto) -> IdleAction {
    match v {
        IdleActionDto::WarnOnly => IdleAction::WarnOnly,
        IdleActionDto::WarnThenClose => IdleAction::WarnThenClose,
        IdleActionDto::Close => IdleAction::Close,
    }
}

fn idle_action_dto(v: IdleAction) -> IdleActionDto {
    match v {
        IdleAction::WarnOnly => IdleActionDto::WarnOnly,
        IdleAction::WarnThenClose => IdleActionDto::WarnThenClose,
        IdleAction::Close => IdleActionDto::Close,
    }
}

pub(crate) fn to_dto(c: &PermissionClass) -> PermissionClassDto {
    let a = &c.attrs;
    PermissionClassDto {
        class_code: c.code.as_str().to_owned(),
        name: a.name.clone(),
        default_menu: a.default_menu.clone(),
        idle_control: idle_control_dto(a.idle_control),
        idle_action: a.idle_action.map(idle_action_dto),
        idle_seconds: a.idle_seconds,
        active: c.active,
        confirmed: c.confirmation.confirmed,
        confirmed_by: c.confirmation.by.clone(),
        confirmed_at: c.confirmation.at,
        created_by: c.created_by.clone(),
        created_dept: c.created_dept.clone(),
        created_on: c.created_on,
        updated_by: c.updated_by.clone(),
        updated_on: c.updated_on,
        version: c.stamp.version,
        created_at: c.stamp.created_at,
        updated_at: c.stamp.updated_at,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_max_len_matches_domain() {
        use ttrs_domain::permission_class::max_len as d;
        use ttrs_shared::permission_class_max_len as s;
        assert_eq!(d::CODE, s::CODE);
        assert_eq!(d::NAME, s::NAME);
    }
}
