//! Project Maintenance 專案維護 use case（TipTop: aooi200 / gja_file）
//! 規格：specs/features/aoo/project_maintenance.feature（PJ1–PJ6）
//!
//! 工廠層（PJ2）。TipTop 為單筆作業：新增 insert、修改 modify、刪除 delete、查詢 query、複製 reproduce、
//! 失效 invalid、列印 output；依 ADR 0013 補上匯出 Excel、異動記錄、確認／取消確認。
//! 修改、刪除、失效依資料擁有者套用資料權限（ADR 0006）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    plant::PlantId,
    project::{Project, ProjectAttrs, ProjectCode},
};
use ttrs_shared::{ChangeDto, Page, ProjectDto, ProjectInput, QbeCondition, QueryRequest};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    change_log::HistoryReader,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{MasterDataRepository, ProjectRepository},
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "project-maintenance";

/// 清單報表（ADR 0013 SA3；aooi200 output）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "project-report-title",
    columns: &[
        ("project_code", "project-col-code"),
        ("name", "project-col-name"),
        ("start_on", "project-col-start"),
        ("end_on", "project-col-end"),
        ("active", "project-col-active"),
    ],
};

#[derive(Clone)]
pub struct ProjectService {
    authz: Arc<dyn Authorizer>,
    projects: Arc<dyn ProjectRepository>,
    master: Arc<dyn MasterDataRepository>,
    history: HistoryReader,
    reports: ListReporter,
}

impl ProjectService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        projects: Arc<dyn ProjectRepository>,
        master: Arc<dyn MasterDataRepository>,
        history: HistoryReader,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            projects,
            master,
            history,
            reports,
        }
    }

    /// QBE 查詢（aooi200.4gl:103；ADR 0013 SA1）：依專案編號排序並分頁
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<ProjectDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let filters = query::project::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.projects.list_projects(&plant).await.map_err(infra)?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::project::value, p);
        Ok(Page {
            items: rows.iter().map(to_dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    pub async fn get(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
    ) -> Result<ProjectDto, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        Ok(to_dto(&self.load(&plant, code).await?))
    }

    /// 列印 output、匯出 Excel exporttoexcel（ADR 0013 SA3）
    pub async fn report_in(
        &self,
        actor: &Actor,
        plant: &str,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let plant = PlantId::parse(plant)?;
        let filters = query::project::filters(&conditions)?;
        let all = self.projects.list_projects(&plant).await.map_err(infra)?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::project::value, p);
        self.reports.check(total)?;
        let t = |id: &str| report::text(lang, id);
        let day = |d: Option<NaiveDate>| d.map(|d| d.to_string()).unwrap_or_default();
        let lines = rows
            .iter()
            .map(|x| {
                vec![
                    x.code.as_str().to_owned(),
                    x.attrs.name.clone().unwrap_or_default(),
                    day(x.attrs.start_on),
                    day(x.attrs.end_on),
                    t(if x.active {
                        "report-active"
                    } else {
                        "report-inactive"
                    }),
                ]
            })
            .collect();
        self.reports
            .build(
                actor,
                Some(&plant),
                &REPORT,
                lines,
                &conditions,
                format,
                lang,
            )
            .await
    }

    /// 測試用：調整同步產生報表的筆數上限
    pub fn with_report_limit(mut self, limit: u64) -> Self {
        self.reports.limit = limit;
        self
    }

    /// 新增（aooi200.4gl:255-317）：資料有效 Y；編號不可重複（-239）；結束日期不可早於起始日期（apj-018）
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        input: ProjectInput,
    ) -> Result<ProjectDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let plant = PlantId::parse(plant)?;
            if !self.master.plant_active(&plant).await.map_err(infra)? {
                return Err(DomainError::PlantNotFound.into());
            }
            let code = ProjectCode::parse(&input.project_code)?;
            let attrs = ProjectAttrs::new(input.name, input.start_on, input.end_on)?;
            let x = Project::new(code, attrs, &actor.user_id, &actor.dept, today());
            self.insert(&plant, &x).await
        })
        .await
    }

    /// 修改（aooi200.4gl:290-336）：專案編號建立後不可修改（PJ5）
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        input: ProjectInput,
    ) -> Result<ProjectDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            let grant = self.authorize(actor, "modify").await?;
            let plant = PlantId::parse(plant)?;
            let mut x = self.load(&plant, code).await?;
            grant.record(&x.created_by, &x.created_dept)?;
            x.stamp.check(input.version)?;
            let attrs = ProjectAttrs::new(input.name, input.start_on, input.end_on)?;
            x.update(attrs, &actor.user_id, today());
            self.save(&plant, &x).await
        })
        .await
    }

    /// 失效切換 invalid（aooi200.4gl:540-550）
    pub async fn set_active(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<ProjectDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "invalid", async {
            let grant = self.authorize(actor, "invalid").await?;
            let plant = PlantId::parse(plant)?;
            let mut x = self.load(&plant, code).await?;
            grant.record(&x.created_by, &x.created_dept)?;
            x.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            x.set_active(active, &actor.user_id, today());
            self.save(&plant, &x).await
        })
        .await
    }

    /// 複製 reproduce（aooi200.4gl:600-680）：新編號不可重複
    pub async fn copy(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        new_code: &str,
    ) -> Result<ProjectDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let plant = PlantId::parse(plant)?;
            let source = self.load(&plant, code).await?;
            let x = source.copy_as(
                ProjectCode::parse(new_code)?,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            self.insert(&plant, &x).await
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<ProjectDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let plant = PlantId::parse(plant)?;
            let mut x = self.load(&plant, code).await?;
            x.stamp.check(version)?;
            x.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&plant, &x).await
        })
        .await
    }

    /// 刪除：已確認不可刪除（SA9-2）；被單據引用時拒絕，請改為停用（PJ4；引用的單據移植後才有）
    pub async fn delete(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "delete", async {
            let grant = self.authorize(actor, "delete").await?;
            let plant = PlantId::parse(plant)?;
            let x = self.load(&plant, code).await?;
            grant.record(&x.created_by, &x.created_dept)?;
            x.stamp.check(version)?;
            x.confirmation.check_deletable()?;
            match self.projects.delete_project(&plant, &x.code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::ProjectNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::ProjectInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 異動記錄（ADR 0012 §8，功能 history）
    pub async fn history(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        self.authorize(actor, "history").await?;
        let plant = PlantId::parse(plant)?;
        let code = ProjectCode::parse(code).map_err(|_| DomainError::ProjectNotFound)?;
        self.history
            .read(
                Some(&plant),
                &["project"],
                &[("project_code", code.as_str())],
                lang,
            )
            .await
    }

    async fn insert(&self, plant: &PlantId, x: &Project) -> Result<ProjectDto, AppError> {
        match self.projects.insert_project(plant, x).await {
            Ok(()) => Ok(to_dto(&self.load(plant, x.code.as_str()).await?)),
            Err(RepoError::Conflict) => Err(DomainError::ProjectAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, plant: &PlantId, x: &Project) -> Result<ProjectDto, AppError> {
        self.projects
            .update_project(plant, x)
            .await
            .map_err(infra)?;
        Ok(to_dto(&self.load(plant, x.code.as_str()).await?))
    }

    async fn load(&self, plant: &PlantId, code: &str) -> Result<Project, AppError> {
        let code = ProjectCode::parse(code).map_err(|_| DomainError::ProjectNotFound)?;
        self.projects
            .get_project(plant, &code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::ProjectNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn to_dto(x: &Project) -> ProjectDto {
    ProjectDto {
        project_code: x.code.as_str().to_owned(),
        name: x.attrs.name.clone(),
        start_on: x.attrs.start_on,
        end_on: x.attrs.end_on,
        active: x.active,
        confirmed: x.confirmation.confirmed,
        confirmed_by: x.confirmation.by.clone(),
        confirmed_at: x.confirmation.at,
        created_by: x.created_by.clone(),
        created_dept: x.created_dept.clone(),
        created_on: x.created_on,
        updated_by: x.updated_by.clone(),
        updated_on: x.updated_on,
        version: x.stamp.version,
        created_at: x.stamp.created_at,
        updated_at: x.stamp.updated_at,
    }
}

fn infra(e: RepoError) -> AppError {
    match e {
        RepoError::Stale => DomainError::RecordChanged.into(),
        e => AppError::Infra(e.to_string()),
    }
}

/// 系統日期（TipTop: g_today）
fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}
