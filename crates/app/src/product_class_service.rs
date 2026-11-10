//! Product Classification Maintenance 產品分類維護 use case（TipTop: axmi110 / oba_file）
//! 規格：specs/features/axm/product_classification.feature（PC1–PC9）
//!
//! 工廠層（PC2）。TipTop 為多筆單身維護：查詢 query、維護 detail（新增列 add_row、刪除列 delete_row）、異動記錄 history。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    department::{ControlType, DeptCode},
    plant::PlantId,
    product_class::{ProductClass, ProductClassAttrs, ProductClassCode},
};
use ttrs_shared::{
    ChangeDto, Page, ProductClassDto, ProductClassInput, QbeCondition, QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    change_log::HistoryReader,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{DepartmentRepository, MasterDataRepository, ProductClassRepository},
    query,
    report::{self, ReportData, ReportFormat},
};

/// 清單報表（ADR 0013 SA3；product_classification_standard_actions.feature）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "pclass-report-title",
    columns: &[
        ("class_code", "pclass-col-code"),
        ("name", "pclass-col-name"),
        ("cost_center", "pclass-col-cost-center"),
        ("revenue_account", "pclass-col-account"),
        ("active", "pclass-col-active"),
    ],
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "product-classification-maintenance";

#[derive(Clone)]
pub struct ProductClassService {
    authz: Arc<dyn Authorizer>,
    classes: Arc<dyn ProductClassRepository>,
    depts: Arc<dyn DepartmentRepository>,
    master: Arc<dyn MasterDataRepository>,
    history: HistoryReader,
    /// 列印、匯出 Excel（ADR 0013 SA3）
    reports: ListReporter,
}

impl ProductClassService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        classes: Arc<dyn ProductClassRepository>,
        depts: Arc<dyn DepartmentRepository>,
        master: Arc<dyn MasterDataRepository>,
        history: HistoryReader,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            classes,
            depts,
            master,
            history,
            reports,
        }
    }

    /// QBE 查詢（axmi110.4gl:477-541；ADR 0013 SA1）：依分類碼排序並分頁
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<ProductClassDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let filters = query::product_class::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self
            .classes
            .list_product_classes(&plant)
            .await
            .map_err(infra)?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::product_class::value, p);
        let mut items = Vec::with_capacity(rows.len());
        for c in &rows {
            items.push(self.to_dto(&plant, c).await?);
        }
        Ok(Page {
            items,
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印 output、匯出 Excel exporttoexcel（axmi110.4gl:110-153；ADR 0013 SA3 通用清單報表）
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
        let filters = query::product_class::filters(&conditions)?;
        let all = self
            .classes
            .list_product_classes(&plant)
            .await
            .map_err(infra)?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::product_class::value, p);
        self.reports.check(total)?;
        let t = |id: &str| report::text(lang, id);
        let lines = rows
            .iter()
            .map(|c| {
                let a = &c.attrs;
                vec![
                    c.code.as_str().to_owned(),
                    a.name.clone().unwrap_or_default(),
                    a.cost_center
                        .as_ref()
                        .map(|d| d.as_str().to_owned())
                        .unwrap_or_default(),
                    a.revenue_account.clone().unwrap_or_default(),
                    t(if c.active {
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

    /// 複製（ADR 0013 SA5；axmi110 原本沒有）
    pub async fn copy(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        new_code: &str,
    ) -> Result<ProductClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let plant = PlantId::parse(plant)?;
            let source = self.load(&plant, code).await?;
            let c = source.copy_as(
                ProductClassCode::parse(new_code)?,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            match self.classes.insert_product_class(&plant, &c).await {
                Ok(()) => {
                    let saved = self.load(&plant, c.code.as_str()).await?;
                    self.to_dto(&plant, &saved).await
                }
                Err(RepoError::Conflict) => Err(DomainError::ProductClassAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
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
    ) -> Result<ProductClassDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let plant = PlantId::parse(plant)?;
            let mut c = self.load(&plant, code).await?;
            c.stamp.check(version)?;
            c.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&plant, &c).await
        })
        .await
    }

    pub async fn list(&self, actor: &Actor, plant: &str) -> Result<Vec<ProductClassDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let mut out = Vec::new();
        for c in self
            .classes
            .list_product_classes(&plant)
            .await
            .map_err(infra)?
        {
            out.push(self.to_dto(&plant, &c).await?);
        }
        Ok(out)
    }

    pub async fn get(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
    ) -> Result<ProductClassDto, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let c = self.load(&plant, code).await?;
        self.to_dto(&plant, &c).await
    }

    /// 新增（axmi110.4gl:159-274）：分類碼不可重複（-239）
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        input: ProductClassInput,
    ) -> Result<ProductClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let plant = PlantId::parse(plant)?;
            if !self.master.plant_active(&plant).await.map_err(infra)? {
                return Err(DomainError::PlantNotFound.into());
            }
            let code = ProductClassCode::parse(&input.class_code)?;
            let attrs = self.attrs(&plant, input).await?;
            let c = ProductClass::new(code, attrs, &actor.user_id, &actor.dept, today());
            match self.classes.insert_product_class(&plant, &c).await {
                Ok(()) => {
                    let saved = self.load(&plant, c.code.as_str()).await?;
                    self.to_dto(&plant, &saved).await
                }
                Err(RepoError::Conflict) => Err(DomainError::ProductClassAlreadyExists.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    /// 修改：分類碼建立後不可修改（:810-831）
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        input: ProductClassInput,
    ) -> Result<ProductClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let mut c = self.load(&plant, code).await?;
            c.stamp.check(input.version)?;
            let attrs = self.attrs(&plant, input).await?;
            c.update(attrs, &actor.user_id, today());
            self.save(&plant, &c).await
        })
        .await
    }

    /// 停用／啟用（TTRS 新增，PC7）
    pub async fn set_active(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<ProductClassDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let mut c = self.load(&plant, code).await?;
            c.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            c.set_active(active, &actor.user_id, today());
            self.save(&plant, &c).await
        })
        .await
    }

    /// 刪除（:331-345）：已被料件引用時拒絕，請改為停用（PC8）
    pub async fn delete(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let plant = PlantId::parse(plant)?;
            let c = self.load(&plant, code).await?;
            c.stamp.check(version)?;
            // 已確認不可刪除，須先取消確認（ADR 0013 SA9-2）
            c.confirmation.check_deletable()?;
            match self.classes.delete_product_class(&plant, &c.code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::ProductClassNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::ProductClassInUse.into()),
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
        let code = ProductClassCode::parse(code).map_err(|_| DomainError::ProductClassNotFound)?;
        self.history
            .read(
                Some(&plant),
                &["product_class"],
                &[("class_code", code.as_str())],
                lang,
            )
            .await
    }

    /// 欄位檢查：成本中心須為本營運中心有效、管理類別 1／2 的部門（:286-305，PC4）
    async fn attrs(
        &self,
        plant: &PlantId,
        i: ProductClassInput,
    ) -> Result<ProductClassAttrs, AppError> {
        let attrs = ProductClassAttrs::new(
            i.name,
            i.period_format,
            i.periods,
            i.cost_center,
            i.revenue_account,
            i.revenue_account2,
        )?;
        if let Some(cc) = &attrs.cost_center {
            let ok = self
                .depts
                .get(plant, cc)
                .await
                .map_err(infra)?
                .is_some_and(|d| {
                    d.active
                        && matches!(
                            d.accounting.control_type,
                            ControlType::CostCenter | ControlType::ProfitCenter
                        )
                });
            if !ok {
                return Err(DomainError::ProductClassCostCenterInvalid.into());
            }
        }
        Ok(attrs)
    }

    async fn save(&self, plant: &PlantId, c: &ProductClass) -> Result<ProductClassDto, AppError> {
        self.classes
            .update_product_class(plant, c)
            .await
            .map_err(infra)?;
        let saved = self.load(plant, c.code.as_str()).await?;
        self.to_dto(plant, &saved).await
    }

    async fn load(&self, plant: &PlantId, code: &str) -> Result<ProductClass, AppError> {
        let code = ProductClassCode::parse(code).map_err(|_| DomainError::ProductClassNotFound)?;
        self.classes
            .get_product_class(plant, &code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::ProductClassNotFound.into())
    }

    async fn to_dto(&self, plant: &PlantId, c: &ProductClass) -> Result<ProductClassDto, AppError> {
        let a = &c.attrs;
        let cost_center_name = match &a.cost_center {
            Some(cc) => self.dept_name(plant, cc).await?,
            None => None,
        };
        Ok(ProductClassDto {
            class_code: c.code.as_str().to_owned(),
            name: a.name.clone(),
            period_format: a.period_format.clone(),
            periods: a.periods,
            cost_center: a.cost_center.as_ref().map(|d| d.as_str().to_owned()),
            cost_center_name,
            revenue_account: a.revenue_account.clone(),
            revenue_account2: a.revenue_account2.clone(),
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
        })
    }

    async fn dept_name(
        &self,
        plant: &PlantId,
        code: &DeptCode,
    ) -> Result<Option<String>, AppError> {
        Ok(self
            .depts
            .get(plant, code)
            .await
            .map_err(infra)?
            .map(|d| d.attrs.name))
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
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
