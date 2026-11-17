//! 共用層維護作業的「異動記錄」查詢（docs/adr/0012-change-history.md §8；change_history.feature CH2）。
//! 依程式代號對應資料表：主檔與其子表的異動一併列出（例如使用者與可使用營運中心）。
//! 工廠層作業（部門、員工、分類碼）在各自的 service 查詢。

use std::sync::Arc;

use ttrs_domain::DomainError;
use ttrs_shared::ChangeDto;

use crate::{Actor, AppError, Authorizer, change_log::HistoryReader};

/// （程式代號, 資料表（主檔在前）, 鍵值欄位）
const TARGETS: &[(&str, &[&str], &str)] = &[
    (
        "user-maintenance",
        &["user_account", "user_plant"],
        "user_id",
    ),
    (
        "program-maintenance",
        &["program", "program_name"],
        "program_no",
    ),
    (
        "permission-class-maintenance",
        &["permission_class"],
        "class_code",
    ),
    (
        "program-permission-maintenance",
        &["program_permission"],
        "class_code",
    ),
    (
        "user-permission-maintenance",
        &["user_permission"],
        "user_id",
    ),
    ("plant-maintenance", &["plant"], "plant_id"),
    ("menu-maintenance", &["menu_item"], "menu_no"),
    ("unit-maintenance", &["unit"], "unit_code"),
    (
        "encoding-nature-maintenance",
        &["encoding_nature"],
        "nature_code",
    ),
    // 單位換算以來源單位查詢
    (
        "unit-conversion-maintenance",
        &["unit_conversion"],
        "from_unit",
    ),
    ("currency-maintenance", &["currency"], "currency_code"),
    // 會計期間：表頭與各期一併列出
    (
        "accounting-period-maintenance",
        &["accounting_year", "accounting_period"],
        "year",
    ),
    // 整體管理系統參數只有一筆，鍵值固定 0
    (
        "general-management-parameters",
        &["system_parameter"],
        "parameter_key",
    ),
    // 匯率以幣別查詢（各年月、各日期一併列出）
    (
        "monthly-exchange-rate-maintenance",
        &["monthly_exchange_rate"],
        "currency_code",
    ),
    (
        "daily-exchange-rate-maintenance",
        &["daily_exchange_rate"],
        "currency_code",
    ),
    // 稅別以稅別碼查詢（進項與銷項一併列出）
    ("tax-type-maintenance", &["tax_type"], "tax_code"),
    ("region-maintenance", &["region"], "region_code"),
    ("country-maintenance", &["country"], "country_code"),
    ("area-maintenance", &["area"], "area_code"),
    (
        "shipping-method-maintenance",
        &["shipping_method"],
        "method_code",
    ),
    (
        "report-title-maintenance",
        &["report_title", "report_title_text"],
        "title_code",
    ),
    (
        "data-dictionary",
        &[
            "table_dictionary",
            "table_dictionary_text",
            "column_dictionary_text",
        ],
        "table_name",
    ),
];

#[derive(Clone)]
pub struct ChangeHistoryService {
    authz: Arc<dyn Authorizer>,
    reader: HistoryReader,
}

impl ChangeHistoryService {
    pub fn new(authz: Arc<dyn Authorizer>, reader: HistoryReader) -> Self {
        Self { authz, reader }
    }

    /// 作業 `program` 中鍵值為 `key` 的資料的異動記錄（功能 history）
    pub async fn history(
        &self,
        actor: &Actor,
        program: &str,
        key: &str,
        lang: Option<&str>,
    ) -> Result<Vec<ChangeDto>, AppError> {
        let Some((program, tables, column)) = TARGETS.iter().find(|(p, _, _)| *p == program.trim())
        else {
            return Err(DomainError::ProgramNotFound.into());
        };
        self.authz.authorize(actor, program, "history").await?;
        self.reader
            .read(None, tables, &[(column, key.trim())], lang)
            .await
    }
}
