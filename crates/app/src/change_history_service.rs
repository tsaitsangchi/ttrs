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
