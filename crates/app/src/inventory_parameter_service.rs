//! Inventory Parameters 庫存參數 use case（TipTop: asms230 / sma_file）
//! 規格：specs/features/aim/inventory_parameters.feature（IP1–IP10）
//!
//! 每個營運中心一筆（IP2）。TipTop 只有「修改」；依 ADR 0013 保留查詢（開啟畫面）、修改、異動記錄、列印、匯出 Excel，
//! 其餘標準功能鈕列為例外（IP9）。沒有資料時顯示預設值，第一次修改時先以預設值建立再修改。

use std::sync::Arc;

use chrono::NaiveDate;
use ttrs_domain::{
    DomainError,
    inventory_parameter::{
        InventoryParameters, InventorySettings, PostingDateBasis, StorageControl,
    },
    plant::PlantId,
};
use ttrs_shared::{ChangeDto, InventoryParametersDto, InventoryParametersInput};

use crate::{
    Actor, AppError, Authorizer,
    change_log::HistoryReader,
    error::RepoError,
    list_report::{ListReporter, ListSpec},
    ports::InventoryParameterRepository,
    report::{self, ReportData, ReportFormat},
};

/// 權限檢查用的程式代號
pub const PROGRAM: &str = "inventory-parameters";

/// 列印、匯出 Excel：一列一個參數（IP9）
const REPORT: ListSpec = ListSpec {
    program: PROGRAM,
    title: "inventory-params-report-title",
    columns: &[
        ("parameter", "inventory-params-col-parameter"),
        ("value", "inventory-params-col-value"),
    ],
};

#[derive(Clone)]
pub struct InventoryParameterService {
    authz: Arc<dyn Authorizer>,
    params: Arc<dyn InventoryParameterRepository>,
    history: HistoryReader,
    reports: ListReporter,
}

impl InventoryParameterService {
    pub fn new(
        authz: Arc<dyn Authorizer>,
        params: Arc<dyn InventoryParameterRepository>,
        history: HistoryReader,
        reports: ListReporter,
    ) -> Self {
        Self {
            authz,
            params,
            history,
            reports,
        }
    }

    /// 開啟畫面（asms230.4gl:60-80）；沒有資料時為預設值、版本 0
    pub async fn get(
        &self,
        actor: &Actor,
        plant: &str,
    ) -> Result<InventoryParametersDto, AppError> {
        self.authz.authorize(actor, PROGRAM, "query").await?;
        let plant = PlantId::parse(plant)?;
        Ok(dto(&self.load(&plant).await?))
    }

    /// 修改（asms230.4gl:112-265）：sma42 1／2／3（IP4）、週轉率（IP5）、帳務基礎日期 1／2（IP6）；比對版本（ADR 0012 §4）
    pub async fn update(
        &self,
        actor: &Actor,
        plant: &str,
        input: InventoryParametersInput,
    ) -> Result<InventoryParametersDto, AppError> {
        crate::change_log::run(actor, PROGRAM, "modify", async {
            self.authz.authorize(actor, PROGRAM, "modify").await?;
            let plant = PlantId::parse(plant)?;
            let settings = settings_of(&input)?;
            settings.validate()?;
            let mut p = self.load(&plant).await?;
            p.stamp.check(input.version)?;
            if p.stamp.version == 0 {
                // 第一次修改：先以預設值建立（IP2）
                let first = InventoryParameters::defaults(&actor.user_id, &actor.dept, today());
                self.params
                    .insert_inventory_parameters(&plant, &first)
                    .await
                    .map_err(|e| match e {
                        RepoError::Conflict => DomainError::RecordChanged.into(),
                        e => infra(e),
                    })?;
                p = self.load(&plant).await?;
            }
            p.update(settings, &actor.user_id, today())?;
            self.params
                .update_inventory_parameters(&plant, &p)
                .await
                .map_err(infra)?;
            Ok(dto(&self.load(&plant).await?))
        })
        .await
    }

    /// 異動記錄（ADR 0012 §8）
    pub async fn history(
        &self,
        actor: &Actor,
        plant: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        self.authz.authorize(actor, PROGRAM, "history").await?;
        let plant = PlantId::parse(plant)?;
        self.history
            .read(
                Some(&plant),
                &["manufacturing_parameter"],
                &[("parameter_key", "0")],
                lang,
            )
            .await
    }

    /// 列印、匯出 Excel（IP9）：全部參數與值
    pub async fn report(
        &self,
        actor: &Actor,
        plant: &str,
        format: ReportFormat,
        lang: &ttrs_domain::lang::Lang,
    ) -> Result<ReportData, AppError> {
        self.authz
            .authorize(actor, PROGRAM, format.action())
            .await?;
        let plant = PlantId::parse(plant)?;
        let v = self.load(&plant).await?.settings;
        let t = |id: &str| report::text(lang, id);
        let yn = |b: bool| t(if b { "report-yes" } else { "report-no" });
        let rows = vec![
            vec![
                t("inventory-params-storage-control"),
                t(&format!(
                    "inventory-params-storage-control-{}",
                    v.storage_control.code()
                )),
            ],
            vec![
                t("inventory-params-fast-turnover"),
                v.fast_turnover.normalize().to_string(),
            ],
            vec![
                t("inventory-params-average-turnover"),
                v.average_turnover.normalize().to_string(),
            ],
            vec![
                t("inventory-params-posting-date-basis"),
                t(&format!(
                    "inventory-params-posting-date-basis-{}",
                    v.posting_date_basis.code()
                )),
            ],
            vec![
                t("inventory-params-misc-receipt-new-location"),
                yn(v.misc_receipt_new_location),
            ],
            vec![
                t("inventory-params-stock-query-window"),
                yn(v.stock_query_window),
            ],
            vec![
                t("inventory-params-plant-transfer-same-no"),
                yn(v.plant_transfer_same_no),
            ],
            vec![
                t("inventory-params-warehouse-transfer-same-no"),
                yn(v.warehouse_transfer_same_no),
            ],
            vec![t("inventory-params-misc-issue-dept"), yn(v.misc_issue_dept)],
            vec![
                t("inventory-params-misc-receipt-dept"),
                yn(v.misc_receipt_dept),
            ],
            vec![
                t("inventory-params-add-storage-freely"),
                yn(v.add_storage_freely),
            ],
            vec![
                t("inventory-params-current-period"),
                match (v.current_year, v.current_period) {
                    (Some(y), Some(p)) => format!("{y}/{p:02}"),
                    _ => String::new(),
                },
            ],
            vec![
                t("inventory-params-closing-date"),
                v.closing_date.map(|d| d.to_string()).unwrap_or_default(),
            ],
        ];
        self.reports
            .build(actor, Some(&plant), &REPORT, rows, &[], format, lang)
            .await
    }

    /// 目前的參數；沒有資料時為預設值、版本 0（尚未建立）
    async fn load(&self, plant: &PlantId) -> Result<InventoryParameters, AppError> {
        Ok(
            match self
                .params
                .inventory_parameters(plant)
                .await
                .map_err(infra)?
            {
                Some(p) => p,
                None => {
                    let mut p = InventoryParameters::defaults("tiptop", "", today());
                    p.stamp.version = 0;
                    p
                }
            },
        )
    }
}

fn settings_of(i: &InventoryParametersInput) -> Result<InventorySettings, DomainError> {
    Ok(InventorySettings {
        storage_control: StorageControl::parse(&i.storage_control)?,
        fast_turnover: i.fast_turnover,
        average_turnover: i.average_turnover,
        posting_date_basis: PostingDateBasis::parse(&i.posting_date_basis)?,
        misc_receipt_new_location: i.misc_receipt_new_location,
        stock_query_window: i.stock_query_window,
        plant_transfer_same_no: i.plant_transfer_same_no,
        warehouse_transfer_same_no: i.warehouse_transfer_same_no,
        misc_issue_dept: i.misc_issue_dept,
        misc_receipt_dept: i.misc_receipt_dept,
        add_storage_freely: i.add_storage_freely,
        current_year: i.current_year,
        current_period: i.current_period,
        closing_date: i.closing_date,
    })
}

fn dto(p: &InventoryParameters) -> InventoryParametersDto {
    let v = &p.settings;
    InventoryParametersDto {
        settings: InventoryParametersInput {
            storage_control: v.storage_control.code().to_owned(),
            fast_turnover: v.fast_turnover.normalize(),
            average_turnover: v.average_turnover.normalize(),
            posting_date_basis: v.posting_date_basis.code().to_owned(),
            misc_receipt_new_location: v.misc_receipt_new_location,
            stock_query_window: v.stock_query_window,
            plant_transfer_same_no: v.plant_transfer_same_no,
            warehouse_transfer_same_no: v.warehouse_transfer_same_no,
            misc_issue_dept: v.misc_issue_dept,
            misc_receipt_dept: v.misc_receipt_dept,
            add_storage_freely: v.add_storage_freely,
            current_year: v.current_year,
            current_period: v.current_period,
            closing_date: v.closing_date,
            version: Some(p.stamp.version),
        },
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
