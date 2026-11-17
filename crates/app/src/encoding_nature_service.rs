//! Encoding Nature Maintenance 編碼性質維護 use case（TipTop: aooi401 / geh_file）
//! 規格：specs/features/aoo/encoding_nature.feature（EN1–EN8）
//!
//! 共用 schema（EN2）。TipTop 為多筆單身維護：查詢 query、維護 detail（新增列 add_row、刪除列 delete_row）；
//! TTRS 依 ADR 0013 補上複製、確認／取消確認、列印、匯出 Excel、異動記錄（EN7）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    encoding_nature::{EncodingNature, NatureAttrs, NatureCode},
};
use ttrs_shared::{EncodingNatureDto, EncodingNatureInput, Page, QbeCondition, QueryRequest};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::EncodingNatureRepository,
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "encoding-nature-maintenance";

/// 清單報表（ADR 0013 SA3；EN7）：性質、說明、類別、資料有效
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "encoding-nature-report-title",
    columns: &[
        ("nature_code", "encoding-nature-col-code"),
        ("name", "encoding-nature-col-name"),
        ("category", "encoding-nature-col-category"),
        ("active", "encoding-nature-col-active"),
    ],
};

#[derive(Clone)]
pub struct EncodingNatureService {
    authz: Arc<dyn Authorizer>,
    natures: Arc<dyn EncodingNatureRepository>,
    reports: ListReporter,
}

impl EncodingNatureService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        natures: Arc<dyn EncodingNatureRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            natures,
            reports,
        }
    }

    /// 全部性質，依類別、性質排序（aooi401.4gl:519-548 ORDER BY geh04, geh01）
    async fn sorted(&self) -> Result<Vec<EncodingNature>, AppError> {
        let mut all = self.natures.list_encoding_natures().await.map_err(infra)?;
        all.sort_by(|a, b| {
            (a.attrs.category.code(), a.code.as_str())
                .cmp(&(b.attrs.category.code(), b.code.as_str()))
        });
        Ok(all)
    }

    /// QBE 查詢（aooi401.4gl:471-517 CONSTRUCT；ADR 0013 SA1）：依類別、性質排序並分頁
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<EncodingNatureDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::encoding_nature::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.sorted().await?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::encoding_nature::value, p);
        Ok(Page {
            items: rows.iter().map(to_dto).collect(),
            total,
            page: p.page,
            page_size: p.page_size,
        })
    }

    /// 列印 output、匯出 Excel exporttoexcel（aooi401.4gl:638-673；ADR 0013 SA3）
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::encoding_nature::filters(&conditions)?;
        let all = self.sorted().await?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::encoding_nature::value, p);
        self.reports.check(total)?;
        let t = |id: &str| report::text(lang, id);
        let lines = rows
            .iter()
            .map(|n| {
                vec![
                    n.code.as_str().to_owned(),
                    n.attrs.name.clone().unwrap_or_default(),
                    t(&format!(
                        "encoding-nature-category-{}",
                        n.attrs.category.code()
                    )),
                    t(if n.active {
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

    /// 複製（ADR 0013 SA5；aooi401 原本沒有）
    pub async fn copy(
        &self,
        actor: &Actor,
        code: &str,
        new_code: &str,
    ) -> Result<EncodingNatureDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load(code).await?;
            let nature = source.copy_as(
                NatureCode::parse(new_code)?,
                &actor.user_id,
                &actor.dept,
                today(),
            );
            self.insert(&nature).await
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
    ) -> Result<EncodingNatureDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let mut nature = self.load(code).await?;
            nature.stamp.check(version)?;
            nature.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&nature).await
        })
        .await
    }

    pub async fn get(&self, actor: &Actor, code: &str) -> Result<EncodingNatureDto, AppError> {
        self.authorize(actor, "query").await?;
        Ok(to_dto(&self.load(code).await?))
    }

    /// 新增（aooi401.4gl:200-252）：資料有效 Y；性質不可重複（-239）；類別只能 1–6（:267-270）
    pub async fn create(
        &self,
        actor: &Actor,
        input: EncodingNatureInput,
    ) -> Result<EncodingNatureDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let code = NatureCode::parse(&input.nature_code)?;
            let attrs = NatureAttrs::new(input.name, &input.category)?;
            let nature = EncodingNature::new(code, attrs, &actor.user_id, &actor.dept, today());
            self.insert(&nature).await
        })
        .await
    }

    /// 修改說明、類別；性質建立後不可修改（:720-736）。
    /// 已被編碼類別引用後不可改類別（EN4）：aooi402 移植時接上，目前沒有引用
    pub async fn update(
        &self,
        actor: &Actor,
        code: &str,
        input: EncodingNatureInput,
    ) -> Result<EncodingNatureDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut nature = self.load(code).await?;
            nature.stamp.check(input.version)?;
            nature.update(
                NatureAttrs::new(input.name, &input.category)?,
                &actor.user_id,
                today(),
            );
            self.save(&nature).await
        })
        .await
    }

    /// 停用／啟用（gehacti，:353-360）
    pub async fn set_active(
        &self,
        actor: &Actor,
        code: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<EncodingNatureDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut nature = self.load(code).await?;
            nature.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            nature.set_active(active, &actor.user_id, today());
            self.save(&nature).await
        })
        .await
    }

    /// 刪除（:362-385）：已確認不可刪除（SA9-2）；已被編碼類別引用時拒絕（EN6，aooi402 移植時接上）
    pub async fn delete(
        &self,
        actor: &Actor,
        code: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let nature = self.load(code).await?;
            nature.stamp.check(version)?;
            nature.confirmation.check_deletable()?;
            match self.natures.delete_encoding_nature(&nature.code).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::EncodingNatureNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::EncodingNatureInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    async fn insert(&self, nature: &EncodingNature) -> Result<EncodingNatureDto, AppError> {
        match self.natures.insert_encoding_nature(nature).await {
            Ok(()) => Ok(to_dto(&self.load(nature.code.as_str()).await?)),
            Err(RepoError::Conflict) => Err(DomainError::EncodingNatureAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, nature: &EncodingNature) -> Result<EncodingNatureDto, AppError> {
        self.natures
            .update_encoding_nature(nature)
            .await
            .map_err(infra)?;
        Ok(to_dto(&self.load(nature.code.as_str()).await?))
    }

    async fn load(&self, code: &str) -> Result<EncodingNature, AppError> {
        let code = NatureCode::parse(code).map_err(|_| DomainError::EncodingNatureNotFound)?;
        self.natures
            .get_encoding_nature(&code)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::EncodingNatureNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn to_dto(n: &EncodingNature) -> EncodingNatureDto {
    EncodingNatureDto {
        nature_code: n.code.as_str().to_owned(),
        name: n.attrs.name.clone(),
        category: n.attrs.category.code().to_owned(),
        active: n.active,
        confirmed: n.confirmation.confirmed,
        confirmed_by: n.confirmation.by.clone(),
        confirmed_at: n.confirmation.at,
        created_by: n.created_by.clone(),
        created_dept: n.created_dept.clone(),
        created_on: n.created_on,
        updated_by: n.updated_by.clone(),
        updated_on: n.updated_on,
        version: n.stamp.version,
        created_at: n.stamp.created_at,
        updated_at: n.stamp.updated_at,
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
