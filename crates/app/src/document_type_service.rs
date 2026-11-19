//! Document Type Maintenance 單別維護（TipTop asmi300 / smy_file、smz_file、smu_file、smv_file）
//! 規格：specs/features/aim/document_type.feature（DT1–DT14）。寫入一律經 change_log::run（ADR 0012）。
//! 單據使用單別時的檢查（s_mfgslip、s_tlf1）：[`DocumentTypeService::check_use`]、[`DocumentTypeService::check_storage`]。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    department::DeptCode,
    document_type::{
        self, DocTypeAttrs, DocTypeCode, DocumentType, StorageRestriction, StorageRule,
    },
    plant::PlantId,
    user::UserId,
};
use ttrs_shared::{
    ChangeDto, DocTypeAccessInput, DocTypeStorageDto, DocTypeStorageInput, DocumentTypeDto,
    DocumentTypeInput, Page, QbeCondition, QueryRequest,
};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    change_log::HistoryReader,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{
        DepartmentRepository, DocumentTypeRepository, SystemParameterRepository, UserRepository,
    },
    query,
    report::{self, ReportData, ReportFormat},
};

pub const PROGRAM: &str = "document-type-maintenance";

const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "doc-type-report-title",
    columns: &[
        ("doc_type_code", "doc-type-col-code"),
        ("name", "doc-type-col-name"),
        ("system", "doc-type-col-system"),
        ("kind", "doc-type-col-kind"),
        ("auto_number", "doc-type-col-auto-number"),
        ("numbering", "doc-type-col-numbering"),
        ("print_now", "doc-type-col-print-now"),
        ("confirm_now", "doc-type-col-confirm-now"),
        ("restriction", "doc-type-col-restriction"),
        ("active", "doc-type-col-active"),
    ],
};

#[derive(Clone)]
pub struct DocumentTypeService {
    authz: Arc<dyn Authorizer>,
    docs: Arc<dyn DocumentTypeRepository>,
    params: Arc<dyn SystemParameterRepository>,
    users: Arc<dyn UserRepository>,
    depts: Arc<dyn DepartmentRepository>,
    history: HistoryReader,
    reports: ListReporter,
}

impl DocumentTypeService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        docs: Arc<dyn DocumentTypeRepository>,
        params: Arc<dyn SystemParameterRepository>,
        users: Arc<dyn UserRepository>,
        depts: Arc<dyn DepartmentRepository>,
        history: HistoryReader,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            docs,
            params,
            users,
            depts,
            history,
            reports,
        }
    }

    /// QBE 查詢（asmi300.4gl:408-705）；依單別排序
    pub async fn query(
        &self,
        actor: &Actor,
        plant: &str,
        req: QueryRequest,
    ) -> Result<Page<DocumentTypeDto>, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        let filters = query::document_type::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.sorted(&plant).await?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::document_type::value, p);
        Ok(Page {
            items: rows.iter().map(dto).collect(),
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
    ) -> Result<DocumentTypeDto, AppError> {
        self.authorize(actor, "query").await?;
        let plant = PlantId::parse(plant)?;
        Ok(dto(&self.load(&plant, code).await?))
    }

    /// 新增（insert；asmi300.4gl:809-925）：單別長度依 aza41（DT5）、預設值（DT6、DT7）
    pub async fn create(
        &self,
        actor: &Actor,
        plant: &str,
        input: DocumentTypeInput,
    ) -> Result<DocumentTypeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "insert", async {
            self.authorize(actor, "insert").await?;
            let plant = PlantId::parse(plant)?;
            let code = DocTypeCode::parse(&input.doc_type_code, self.code_length().await?)?;
            let d = DocumentType::new(
                code,
                attrs_of(&input, None),
                &actor.user_id,
                &actor.dept,
                today(),
            )?;
            self.insert(&plant, &d).await
        })
        .await
    }

    /// 修改（modify；asmi300.4gl:926-1045）：停用的不可修改（9027）；系統別、單據性質可改
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        input: DocumentTypeInput,
    ) -> Result<DocumentTypeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            let grant = self.authorize(actor, "modify").await?;
            let plant = PlantId::parse(plant)?;
            let mut d = self.load(&plant, code).await?;
            grant.record(&d.created_by, &d.created_dept)?;
            d.stamp.check(input.version)?;
            let attrs = attrs_of(&input, Some(&d.attrs));
            d.update(attrs, &actor.user_id, today())?;
            self.save(&plant, &d).await
        })
        .await
    }

    /// 倉庫設限（detail；asmi300.4gl:2153-2320，DT8）
    pub async fn set_storages(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        input: DocTypeStorageInput,
    ) -> Result<DocumentTypeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            let grant = self.authorize(actor, "detail").await?;
            let plant = PlantId::parse(plant)?;
            let mut d = self.load(&plant, code).await?;
            grant.record(&d.created_by, &d.created_dept)?;
            d.stamp.check(input.version)?;
            let restriction = StorageRestriction::parse(&input.restriction)?;
            let rules = input
                .storages
                .iter()
                .filter(|r| !(r.warehouse.trim().is_empty() && r.location.trim().is_empty()))
                .map(|r| StorageRule::new(&r.warehouse, &r.location))
                .collect::<Result<Vec<_>, _>>()?;
            d.set_storages(restriction, rules, &actor.user_id, today())?;
            self.save(&plant, &d).await
        })
        .await
    }

    /// 使用者設限（authorization）與部門設限（dept_authorization）（s_smu、s_smv，DT9）：
    /// 使用者須存在、部門須存在且有效。改哪一份清單就檢查哪一個功能代號
    pub async fn set_access(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        input: DocTypeAccessInput,
    ) -> Result<DocumentTypeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "authorization", async {
            if input.depts.is_some() && input.users.is_none() {
                crate::change_log::set_operation("dept_authorization");
            }
            let mut grant = None;
            if input.users.is_some() || input.depts.is_none() {
                grant = Some(self.authorize(actor, "authorization").await?);
            }
            if input.depts.is_some() {
                grant = Some(self.authorize(actor, "dept_authorization").await?);
            }
            let grant = grant.expect("至少檢查一個功能");
            let plant = PlantId::parse(plant)?;
            let mut d = self.load(&plant, code).await?;
            grant.record(&d.created_by, &d.created_dept)?;
            d.stamp.check(input.version)?;
            let mut users = Vec::new();
            for u in input
                .users
                .iter()
                .flatten()
                .map(|u| u.trim())
                .filter(|u| !u.is_empty())
            {
                let id = UserId::parse(u).map_err(|_| DomainError::DocumentTypeUserNotFound)?;
                if self.users.get(&id).await.map_err(infra)?.is_none() {
                    return Err(DomainError::DocumentTypeUserNotFound.into());
                }
                users.push(id.as_str().to_owned());
            }
            let mut depts = Vec::new();
            for c in input
                .depts
                .iter()
                .flatten()
                .map(|c| c.trim())
                .filter(|c| !c.is_empty())
            {
                let code = DeptCode::parse(c).map_err(|_| DomainError::DocumentTypeDeptNotFound)?;
                let ok = self
                    .depts
                    .get(&plant, &code)
                    .await
                    .map_err(infra)?
                    .is_some_and(|x| x.active);
                if !ok {
                    return Err(DomainError::DocumentTypeDeptNotFound.into());
                }
                depts.push(code);
            }
            if input.users.is_none() {
                users = d.users.clone();
            }
            if input.depts.is_none() {
                depts = d.depts.clone();
            }
            d.set_access(users, depts, &actor.user_id, today())?;
            self.save(&plant, &d).await
        })
        .await
    }

    /// 失效／生效（invalid；asmi300.4gl:2113-2150）
    pub async fn set_active(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<DocumentTypeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "invalid", async {
            let grant = self.authorize(actor, "invalid").await?;
            let plant = PlantId::parse(plant)?;
            let mut d = self.load(&plant, code).await?;
            grant.record(&d.created_by, &d.created_dept)?;
            d.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            d.set_active(active, &actor.user_id, today());
            self.save(&plant, &d).await
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
    ) -> Result<DocumentTypeDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let plant = PlantId::parse(plant)?;
            let mut d = self.load(&plant, code).await?;
            d.stamp.check(version)?;
            if confirm {
                d.confirmation.confirm(&actor.user_id, chrono::Utc::now())?;
            } else {
                d.confirmation.unconfirm()?;
            }
            d.touch(&actor.user_id, today());
            self.save(&plant, &d).await
        })
        .await
    }

    /// 複製（reproduce，DT11）：連同三個單身
    pub async fn copy(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        new_code: &str,
    ) -> Result<DocumentTypeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let plant = PlantId::parse(plant)?;
            let source = self.load(&plant, code).await?;
            let code = DocTypeCode::parse(new_code, self.code_length().await?)?;
            let d = source.copy_as(code, &actor.user_id, &actor.dept, today());
            self.insert(&plant, &d).await
        })
        .await
    }

    /// 刪除（delete；asmi300.4gl:2014-2111）：停用、已確認不可刪；連同三個單身。
    /// 已有單據使用時不可刪（asm-091）於庫存交易移植時接上（DT10）
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
            let d = self.load(&plant, code).await?;
            grant.record(&d.created_by, &d.created_dept)?;
            d.stamp.check(version)?;
            d.check_deletable()?;
            self.docs
                .delete_document_type(&plant, &d.code)
                .await
                .map_err(infra)
        })
        .await
    }

    /// 異動記錄：單頭與三個單身（ADR 0012 §8）
    pub async fn history(
        &self,
        actor: &Actor,
        plant: &str,
        code: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        self.authorize(actor, "history").await?;
        let plant = PlantId::parse(plant)?;
        self.history
            .read(
                Some(&plant),
                &[
                    "document_type",
                    "document_type_storage",
                    "document_type_user",
                    "document_type_dept",
                ],
                &[("doc_type_code", code.trim())],
                lang,
            )
            .await
    }

    /// 列印 output、匯出 Excel exporttoexcel（DT11）：每個單別一列
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
        let filters = query::document_type::filters(&conditions)?;
        let all = self.sorted(&plant).await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::MAX,
            offset: 0,
        };
        let (rows, _) = query::page_in_memory(&all, &filters, query::document_type::value, p);
        let t = |id: &str| report::text(lang, id);
        let yn = |v: bool| t(if v { "report-yes" } else { "report-no" });
        let lines: Vec<Vec<String>> = rows
            .iter()
            .map(|d| {
                let a = &d.attrs;
                vec![
                    d.code.as_str().to_owned(),
                    a.name.clone(),
                    a.system.clone(),
                    format!(
                        "{} {}",
                        a.kind,
                        t(&format!(
                            "doc-type-kind-{}-{}",
                            a.system,
                            a.kind.to_lowercase()
                        ))
                    ),
                    yn(a.auto_number),
                    t(&format!("doc-type-numbering-{}", a.numbering)),
                    yn(a.print_now),
                    yn(a.confirm_now),
                    t(&format!("doc-type-restriction-{}", d.restriction.code())),
                    yn(d.active),
                ]
            })
            .collect();
        self.reports.check(lines.len() as u64)?;
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

    /// 單據使用單別的檢查（s_mfgslip，DT9）：單別存在（mfg0014）、性質相符（mfg0015）、有效（9028）、使用者與部門設限（aoo-104）。
    /// 給庫存交易等作業呼叫；不另做權限檢查（呼叫端已檢查自己的作業權限）
    pub async fn check_use(
        &self,
        plant: &str,
        code: &str,
        kind: &str,
        user: &str,
    ) -> Result<DocumentType, AppError> {
        let plant = PlantId::parse(plant)?;
        let d = self.load(&plant, code).await?;
        let dept = match UserId::parse(user) {
            Ok(id) => self
                .users
                .get(&id)
                .await
                .map_err(infra)?
                .and_then(|u| u.attrs.dept_code),
            Err(_) => None,
        };
        d.check_use(kind, user, dept.as_deref())?;
        Ok(d)
    }

    /// 倉儲是否可用於此單別（s_tlf1，DT8）；給庫存交易呼叫
    pub async fn check_storage(
        &self,
        plant: &str,
        code: &str,
        warehouse: &str,
        location: Option<&str>,
    ) -> Result<(), AppError> {
        let plant = PlantId::parse(plant)?;
        let d = self.load(&plant, code).await?;
        Ok(d.check_storage(warehouse, location)?)
    }

    async fn code_length(&self) -> Result<u8, AppError> {
        Ok(self
            .params
            .general_parameters()
            .await
            .map_err(infra)?
            .map_or(3, |p| p.doc_code_length))
    }

    async fn sorted(&self, plant: &PlantId) -> Result<Vec<DocumentType>, AppError> {
        let mut all = self.docs.list_document_types(plant).await.map_err(infra)?;
        all.sort_by(|a, b| a.code.cmp(&b.code));
        Ok(all)
    }

    async fn insert(&self, plant: &PlantId, d: &DocumentType) -> Result<DocumentTypeDto, AppError> {
        match self.docs.insert_document_type(plant, d).await {
            Ok(()) => Ok(dto(&self.load(plant, d.code.as_str()).await?)),
            Err(RepoError::Conflict) => Err(DomainError::DocumentTypeAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, plant: &PlantId, d: &DocumentType) -> Result<DocumentTypeDto, AppError> {
        self.docs
            .update_document_type(plant, d)
            .await
            .map_err(infra)?;
        Ok(dto(&self.load(plant, d.code.as_str()).await?))
    }

    async fn load(&self, plant: &PlantId, code: &str) -> Result<DocumentType, AppError> {
        let code = DocTypeCode::existing(code)?;
        self.docs
            .get_document_type(plant, &code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::DocumentTypeNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

/// 輸入 → 單頭設定：新增時未給的用預設值（DT6、DT7），修改時沿用原值
fn attrs_of(i: &DocumentTypeInput, base: Option<&DocTypeAttrs>) -> DocTypeAttrs {
    let system = i.system.trim().to_owned();
    let kind = i.kind.trim().to_uppercase();
    let b = |v: Option<bool>, old: Option<bool>, default: bool| v.or(old).unwrap_or(default);
    let cost_class = i
        .cost_class
        .clone()
        .map(|c| c.trim().to_uppercase())
        .filter(|c| !c.is_empty())
        .or_else(|| {
            base.filter(|x| x.kind == kind && x.system == system)
                .map(|x| x.cost_class.clone())
        })
        .or_else(|| document_type::default_cost_class(&system, &kind).map(str::to_owned))
        .unwrap_or_default();
    DocTypeAttrs {
        name: i.name.clone(),
        auto_number: b(i.auto_number, base.map(|x| x.auto_number), true),
        numbering: i
            .numbering
            .clone()
            .map(|n| n.trim().to_owned())
            .or_else(|| base.map(|x| x.numbering.clone()))
            .unwrap_or_else(|| "2".into()),
        print_now: b(i.print_now, base.map(|x| x.print_now), false),
        confirm_now: b(i.confirm_now, base.map(|x| x.confirm_now), false),
        cost_enabled: b(i.cost_enabled, base.map(|x| x.cost_enabled), true),
        cost_class,
        affects_idle_date: b(i.affects_idle_date, base.map(|x| x.affects_idle_date), true),
        internal_trade: b(i.internal_trade, base.map(|x| x.internal_trade), false),
        budget_control: b(i.budget_control, base.map(|x| x.budget_control), false),
        memo: i.memo.clone(),
        system,
        kind,
    }
}

fn dto(d: &DocumentType) -> DocumentTypeDto {
    let a = &d.attrs;
    DocumentTypeDto {
        doc_type_code: d.code.as_str().to_owned(),
        name: a.name.clone(),
        system: a.system.clone(),
        kind: a.kind.clone(),
        auto_number: a.auto_number,
        numbering: a.numbering.clone(),
        print_now: a.print_now,
        confirm_now: a.confirm_now,
        cost_enabled: a.cost_enabled,
        cost_class: a.cost_class.clone(),
        affects_idle_date: a.affects_idle_date,
        internal_trade: a.internal_trade,
        budget_control: a.budget_control,
        memo: a.memo.clone(),
        restriction: d.restriction.code().to_owned(),
        storages: d
            .storages
            .iter()
            .map(|r| DocTypeStorageDto {
                warehouse: r.warehouse.clone(),
                location: r.location.clone().unwrap_or_default(),
            })
            .collect(),
        users: d.users.clone(),
        depts: d.depts.iter().map(|x| x.as_str().to_owned()).collect(),
        active: d.active,
        confirmed: d.confirmation.confirmed,
        confirmed_by: d.confirmation.by.clone(),
        confirmed_at: d.confirmation.at,
        created_by: d.created_by.clone(),
        created_dept: d.created_dept.clone(),
        created_on: d.created_on,
        updated_by: d.updated_by.clone(),
        updated_on: d.updated_on,
        version: d.stamp.version,
        created_at: d.stamp.created_at,
        updated_at: d.stamp.updated_at,
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
