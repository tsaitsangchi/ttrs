//! General Management Parameters 整體管理系統參數 use case（TipTop: aoos010 / aza_file）
//! 規格：specs/features/aoo/general_parameters.feature（GP1–GP10）
//!
//! 全公司一份、共用 schema（GP2）。TipTop 只有「修改」；依 ADR 0013 保留查詢（開啟畫面）、修改、異動記錄、
//! 列印、匯出 Excel，其餘標準功能鈕列為例外（GP8）。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    currency::CurrencyCode,
    general_parameter::{AccountingPeriods, GeneralParameters, RateBasis},
};
use ttrs_shared::{GeneralParametersDto, GeneralParametersInput};

use crate::{
    Actor, AppError, Authorizer,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::{
        AccountingPeriodRepository, CurrencyRepository, DocumentTypeRepository,
        ExchangeRateRepository, SystemParameterRepository,
    },
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "general-management-parameters";

/// 列印、匯出 Excel：一列一個參數（GP8）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "general-params-report-title",
    columns: &[
        ("parameter", "general-params-col-parameter"),
        ("value", "general-params-col-value"),
    ],
};

#[derive(Clone)]
pub struct GeneralParameterService {
    authz: Arc<dyn Authorizer>,
    params: Arc<dyn SystemParameterRepository>,
    currencies: Arc<dyn CurrencyRepository>,
    rates: Arc<dyn ExchangeRateRepository>,
    /// 已有會計年度時不可變更期數（GP4、PD6）
    years: Arc<dyn AccountingPeriodRepository>,
    /// 已有單別時不可變更單別位數（DT5(a)）
    doc_types: Option<Arc<dyn DocumentTypeRepository>>,
    txns: Option<Arc<dyn crate::ports::InventoryTransactionRepository>>,
    reports: ListReporter,
}

impl GeneralParameterService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        params: Arc<dyn SystemParameterRepository>,
        currencies: Arc<dyn CurrencyRepository>,
        rates: Arc<dyn ExchangeRateRepository>,
        years: Arc<dyn AccountingPeriodRepository>,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            params,
            currencies,
            rates,
            years,
            doc_types: None,
            txns: None,
            reports,
        }
    }

    /// 接上單別（document_type.feature DT5(a)）
    /// 單號位數 aza42 已有單據時不可變更（misc_issue.feature MT4(a)）
    pub fn with_inventory_transactions(
        mut self,
        txns: Arc<dyn crate::ports::InventoryTransactionRepository>,
    ) -> Self {
        self.txns = Some(txns);
        self
    }

    pub fn with_doc_types(mut self, doc_types: Arc<dyn DocumentTypeRepository>) -> Self {
        self.doc_types = Some(doc_types);
        self
    }

    /// 開啟畫面（aoos010.4gl:235-477 s010_show）
    pub async fn get(&self, actor: &Actor) -> Result<GeneralParametersDto, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        let p = self.load().await?;
        self.dto(&p).await
    }

    /// 修改（aoos010.4gl:534-726）：會計期間 12／13（GP4）、本國幣別須為有效幣別且已有匯率時不可變更（GP5）、
    /// 匯率採用每月／每日（GP6）；比對版本（ADR 0012 §4）
    pub async fn update(
        &self,
        actor: &Actor,
        input: GeneralParametersInput,
    ) -> Result<GeneralParametersDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            self.authz.authorize(actor, PROGRAM, "modify").await?;
            let mut p = self.load().await?;
            p.stamp.check(input.version)?;
            let periods = AccountingPeriods::parse(input.accounting_periods)?;
            let basis = RateBasis::parse(&input.rate_basis)?;
            let base = CurrencyCode::parse(&input.base_currency)
                .map_err(|_| DomainError::CurrencyNotFound)?;
            // :875-886 本國幣別須存在；GP5 也不可為停用的幣別。只在變更時檢查
            // （幣別資料轉入前起始值 TWD 可能還不在幣別檔，不應因此無法修改其他參數）
            if base != p.base_currency {
                let cur = self
                    .currencies
                    .get_currency(&base)
                    .await
                    .map_err(infra)?
                    .ok_or(DomainError::CurrencyNotFound)?;
                if !cur.active {
                    return Err(DomainError::CurrencyInactive.into());
                }
            }
            // GP4、PD6：已有會計年度時不可變更期數
            if periods != p.accounting_periods
                && !self
                    .years
                    .list_accounting_years()
                    .await
                    .map_err(infra)?
                    .is_empty()
            {
                return Err(DomainError::AccountingPeriodsInUse.into());
            }
            if base != p.base_currency && self.rates.has_any_rate().await.map_err(infra)? {
                return Err(DomainError::BaseCurrencyInUse.into());
            }
            p.update(periods, base, basis, &actor.user_id, today());
            // aza28 料件編號自動編碼（item_auto_number.feature AN1）；未給時維持原值
            if let Some(auto) = input.auto_item_number {
                p.auto_item_number = auto;
            }
            // aza41 單別位數（DT5(a)）：已有單別時不可變更
            if let Some(len) = input.doc_code_length {
                let len = ttrs_domain::general_parameter::parse_doc_code_length(len)?;
                if len != p.doc_code_length {
                    if let Some(d) = &self.doc_types
                        && d.any_document_type().await.map_err(infra)?
                    {
                        return Err(DomainError::DocCodeLengthInUse.into());
                    }
                    p.doc_code_length = len;
                }
            }
            // aza42 單號位數（MT4(a)）：已有單據時不可變更
            if let Some(len) = input.doc_serial_length {
                let len = ttrs_domain::general_parameter::parse_doc_serial_length(len)?;
                if len != p.doc_serial_length {
                    if let Some(t) = &self.txns
                        && t.any_inventory_transaction().await.map_err(infra)?
                    {
                        return Err(DomainError::DocSerialLengthInUse.into());
                    }
                    p.doc_serial_length = len;
                }
            }
            self.params
                .update_general_parameters(&p)
                .await
                .map_err(infra)?;
            let saved = self.load().await?;
            self.dto(&saved).await
        })
        .await
    }

    /// 列印、匯出 Excel（GP8）：全部參數與值
    pub async fn report(
        &self,
        actor: &Actor,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authz
            .authorize(actor, PROGRAM, format.action())
            .await?;
        let p = self.load().await?;
        let d = self.dto(&p).await?;
        let t = |id: &str| report::text(lang, id);
        let base = match &d.base_currency_name {
            Some(n) => format!("{} {n}", d.base_currency),
            None => d.base_currency.clone(),
        };
        let rows = vec![
            vec![
                t("general-params-accounting-periods"),
                d.accounting_periods.to_string(),
            ],
            vec![t("general-params-base-currency"), base],
            vec![
                t("general-params-auto-item-number"),
                t(if p.auto_item_number {
                    "general-params-yes"
                } else {
                    "general-params-no"
                }),
            ],
            vec![
                t("general-params-doc-code-length"),
                p.doc_code_length.to_string(),
            ],
            vec![
                t("general-params-doc-serial-length"),
                p.doc_serial_length.to_string(),
            ],
            vec![
                t("general-params-rate-basis"),
                t(match p.rate_basis {
                    RateBasis::Monthly => "general-params-rate-monthly",
                    RateBasis::Daily => "general-params-rate-daily",
                }),
            ],
        ];
        self.reports
            .build(actor, None, &REPORT, rows, &[], format, lang)
            .await
    }

    async fn load(&self) -> Result<GeneralParameters, AppError> {
        Ok(self
            .params
            .general_parameters()
            .await
            .map_err(infra)?
            .unwrap_or_else(|| GeneralParameters::defaults("tiptop", "", today())))
    }

    async fn dto(&self, p: &GeneralParameters) -> Result<GeneralParametersDto, AppError> {
        let name = self
            .currencies
            .get_currency(&p.base_currency)
            .await
            .map_err(infra)?
            .and_then(|c| c.attrs.name);
        Ok(GeneralParametersDto {
            accounting_periods: p.accounting_periods.get(),
            base_currency: p.base_currency.as_str().to_owned(),
            base_currency_name: name,
            rate_basis: p.rate_basis.code().to_owned(),
            auto_item_number: p.auto_item_number,
            doc_code_length: i16::from(p.doc_code_length),
            doc_serial_length: i16::from(p.doc_serial_length),
            created_by: p.created_by.clone(),
            created_dept: p.created_dept.clone(),
            created_on: p.created_on,
            updated_by: p.updated_by.clone(),
            updated_on: p.updated_on,
            version: p.stamp.version,
            created_at: p.stamp.created_at,
            updated_at: p.stamp.updated_at,
        })
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
