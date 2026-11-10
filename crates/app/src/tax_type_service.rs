//! Tax Type Maintenance 稅別維護 use case（TipTop: aooi150 / gec_file）
//! 規格：specs/features/aoo/tax_type_maintenance.feature（TX1–TX9）
//!
//! 共用 schema（TX2）。鍵值為稅別碼＋進／銷項。TipTop 為多筆單身維護：查詢 query、維護 detail
//! （新增列 add_row、刪除列 delete_row）、列印 output、匯出 Excel exporttoexcel；依 ADR 0013 補上複製、確認、異動記錄。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    tax_type::{TaxClass, TaxCode, TaxDirection, TaxKey, TaxType, TaxTypeAttrs, TaxTypeFields},
};
use ttrs_shared::{Page, QbeCondition, QueryRequest, TaxTypeDto, TaxTypeInput};

use crate::{
    Actor, AppError, Authorizer,
    authorization::Grant,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::TaxTypeRepository,
    query,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "tax-type-maintenance";

/// 清單報表（ADR 0013 SA3）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "tax-report-title",
    columns: &[
        ("tax_code", "tax-col-code"),
        ("name", "tax-col-name"),
        ("direction", "tax-col-direction"),
        ("account", "tax-col-account"),
        ("account2", "tax-col-account2"),
        ("invoice_copies", "tax-col-invoice-copies"),
        ("tax_class", "tax-col-class"),
        ("rate", "tax-col-rate"),
        ("include_tax", "tax-col-include-tax"),
        ("filing_format", "tax-col-filing-format"),
        ("active", "tax-col-active"),
    ],
};

/// 鍵值的文字（異動記錄、畫面選取用）：「稅別碼 進銷項代碼」
pub fn key_text(key: &TaxKey) -> String {
    format!("{} {}", key.code.as_str(), key.direction.code())
}

#[derive(Clone)]
pub struct TaxTypeService {
    authz: Arc<dyn Authorizer>,
    taxes: Arc<dyn TaxTypeRepository>,
    reports: ListReporter,
}

impl TaxTypeService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        taxes: Arc<dyn TaxTypeRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            taxes,
            reports,
        }
    }

    /// QBE 查詢（aooi150.4gl:617 CONSTRUCT；ADR 0013 SA1）：依稅別碼、進銷項排序並分頁
    pub async fn query(
        &self,
        actor: &Actor,
        req: QueryRequest,
    ) -> Result<Page<TaxTypeDto>, AppError> {
        self.authorize(actor, "query").await?;
        let filters = query::tax_type::filters(&req.conditions)?;
        let p = query::paging(&req);
        let all = self.taxes.list_tax_types().await.map_err(infra)?;
        let (rows, total) = query::page_in_memory(&all, &filters, query::tax_type::value, p);
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
        code: &str,
        direction: &str,
    ) -> Result<TaxTypeDto, AppError> {
        self.authorize(actor, "query").await?;
        Ok(to_dto(&self.load(code, direction).await?))
    }

    /// 列印 output、匯出 Excel exporttoexcel（ADR 0013 SA3）
    pub async fn report_in(
        &self,
        actor: &Actor,
        conditions: Vec<QbeCondition>,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authorize(actor, format.action()).await?;
        let filters = query::tax_type::filters(&conditions)?;
        let all = self.taxes.list_tax_types().await.map_err(infra)?;
        let p = query::Paging {
            page: 1,
            page_size: u32::try_from(self.reports.fetch_limit()).unwrap_or(u32::MAX),
            offset: 0,
        };
        let (rows, total) = query::page_in_memory(&all, &filters, query::tax_type::value, p);
        self.reports.check(total)?;
        let t = |id: &str| report::text(lang, id);
        let yes_no = |b: bool| t(if b { "report-yes" } else { "report-no" });
        let lines = rows
            .iter()
            .map(|x| {
                let a = &x.attrs;
                vec![
                    x.key.code.as_str().to_owned(),
                    a.name.clone().unwrap_or_default(),
                    t(match x.key.direction {
                        TaxDirection::Purchase => "tax-direction-1",
                        TaxDirection::Sales => "tax-direction-2",
                    }),
                    a.account.clone().unwrap_or_default(),
                    a.account2.clone().unwrap_or_default(),
                    a.invoice_copies
                        .map(|c| t(&format!("tax-copies-{}", c.code().to_lowercase())))
                        .unwrap_or_default(),
                    a.tax_class
                        .map(|c| {
                            t(match c {
                                TaxClass::Taxable => "tax-class-1",
                                TaxClass::ZeroRated => "tax-class-2",
                                TaxClass::Exempt => "tax-class-3",
                            })
                        })
                        .unwrap_or_default(),
                    a.rate.to_string(),
                    yes_no(a.include_tax),
                    a.filing_format.clone(),
                    t(if x.active {
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

    /// 新增（aooi150.4gl:260-333）：資料有效 Y、單價含稅否 N、申報格式 XX（TX6）；稅別碼＋進銷項不可重複（-239）
    pub async fn create(&self, actor: &Actor, input: TaxTypeInput) -> Result<TaxTypeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.add_row()?;
            let key = key_of(&input.tax_code, &input.direction)?;
            let attrs = attrs_of(key.direction, input)?;
            let tax = TaxType::new(key, attrs, &actor.user_id, &actor.dept, today());
            self.insert(&tax).await
        })
        .await
    }

    /// 修改；稅別碼與進／銷項建立後不可修改（TX8）
    pub async fn update(
        &self,
        actor: &Actor,
        code: &str,
        direction: &str,
        input: TaxTypeInput,
    ) -> Result<TaxTypeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut tax = self.load(code, direction).await?;
            tax.stamp.check(input.version)?;
            let attrs = attrs_of(tax.key.direction, input)?;
            tax.update(attrs, &actor.user_id, today());
            self.save(&tax).await
        })
        .await
    }

    /// 停用／啟用（gecacti，:449-456）
    pub async fn set_active(
        &self,
        actor: &Actor,
        code: &str,
        direction: &str,
        active: bool,
        version: Option<i32>,
    ) -> Result<TaxTypeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?;
            let mut tax = self.load(code, direction).await?;
            tax.stamp.check(version)?;
            crate::change_log::set_operation(if active { "activate" } else { "deactivate" });
            tax.set_active(active, &actor.user_id, today());
            self.save(&tax).await
        })
        .await
    }

    /// 複製（ADR 0013 SA5）：輸入新稅別碼與進／銷項
    pub async fn copy(
        &self,
        actor: &Actor,
        code: &str,
        direction: &str,
        new_code: &str,
        new_direction: &str,
    ) -> Result<TaxTypeDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "reproduce", async {
            self.authorize(actor, "reproduce").await?;
            let source = self.load(code, direction).await?;
            let key = key_of(new_code, new_direction)?;
            let tax = source.copy_as(key, &actor.user_id, &actor.dept, today())?;
            self.insert(&tax).await
        })
        .await
    }

    /// 確認（confirm）／取消確認（notconfirm）（ADR 0013 SA9）
    pub async fn confirm(
        &self,
        actor: &Actor,
        code: &str,
        direction: &str,
        confirm: bool,
        version: Option<i32>,
    ) -> Result<TaxTypeDto, AppError> {
        let action = if confirm { "confirm" } else { "notconfirm" };
        crate::change_log::run(actor, PROGRAM, action, async {
            self.authorize(actor, action).await?;
            let mut tax = self.load(code, direction).await?;
            tax.stamp.check(version)?;
            tax.set_confirmed(confirm, &actor.user_id, chrono::Utc::now())?;
            self.save(&tax).await
        })
        .await
    }

    /// 刪除（:457）：已確認不可刪除（SA9-2）；被客戶、供應商、單據引用時拒絕（TX7；引用的作業移植後才有）
    pub async fn delete(
        &self,
        actor: &Actor,
        code: &str,
        direction: &str,
        version: Option<i32>,
    ) -> Result<(), AppError> {
        crate::change_log::run(actor, PROGRAM, "detail", async {
            self.authorize(actor, "detail").await?.delete_row()?;
            let tax = self.load(code, direction).await?;
            tax.stamp.check(version)?;
            tax.confirmation.check_deletable()?;
            match self.taxes.delete_tax_type(&tax.key).await {
                Ok(true) => Ok(()),
                Ok(false) => Err(DomainError::TaxTypeNotFound.into()),
                Err(RepoError::InUse) => Err(DomainError::TaxTypeInUse.into()),
                Err(e) => Err(infra(e)),
            }
        })
        .await
    }

    async fn insert(&self, tax: &TaxType) -> Result<TaxTypeDto, AppError> {
        match self.taxes.insert_tax_type(tax).await {
            Ok(()) => Ok(to_dto(&self.load_key(&tax.key).await?)),
            Err(RepoError::Conflict) => Err(DomainError::TaxTypeAlreadyExists.into()),
            Err(e) => Err(infra(e)),
        }
    }

    async fn save(&self, tax: &TaxType) -> Result<TaxTypeDto, AppError> {
        self.taxes.update_tax_type(tax).await.map_err(infra)?;
        Ok(to_dto(&self.load_key(&tax.key).await?))
    }

    async fn load(&self, code: &str, direction: &str) -> Result<TaxType, AppError> {
        let key = key_of(code, direction).map_err(|_| DomainError::TaxTypeNotFound)?;
        self.load_key(&key).await
    }

    async fn load_key(&self, key: &TaxKey) -> Result<TaxType, AppError> {
        self.taxes
            .get_tax_type(key)
            .await
            .map_err(infra)?
            .ok_or_else(|| DomainError::TaxTypeNotFound.into())
    }

    async fn authorize(&self, actor: &Actor, action: &'static str) -> Result<Grant, AppError> {
        self.authz.authorize(actor, PROGRAM, action).await
    }
}

fn key_of(code: &str, direction: &str) -> Result<TaxKey, DomainError> {
    Ok(TaxKey {
        code: TaxCode::parse(code)?,
        direction: TaxDirection::parse(direction)?,
    })
}

fn attrs_of(direction: TaxDirection, input: TaxTypeInput) -> Result<TaxTypeAttrs, DomainError> {
    TaxTypeAttrs::new(
        direction,
        TaxTypeFields {
            name: input.name,
            account: input.account,
            account2: input.account2,
            invoice_copies: input.invoice_copies,
            tax_class: input.tax_class,
            rate: input.rate,
            include_tax: input.include_tax,
            filing_format: input.filing_format,
        },
    )
}

fn to_dto(x: &TaxType) -> TaxTypeDto {
    let a = &x.attrs;
    TaxTypeDto {
        tax_code: x.key.code.as_str().to_owned(),
        direction: x.key.direction.code().to_owned(),
        name: a.name.clone(),
        account: a.account.clone(),
        account2: a.account2.clone(),
        invoice_copies: a.invoice_copies.map(|c| c.code().to_owned()),
        tax_class: a.tax_class.map(|c| c.code().to_owned()),
        rate: a.rate,
        include_tax: a.include_tax,
        filing_format: a.filing_format.clone(),
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
