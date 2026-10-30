//! TipTop 對照目錄 use case（TipTop .sch → 英文命名的唯讀對照）
//! 規格：specs/features/azz/legacy_catalog.feature（C1–C8）

use std::sync::Arc;

use ttrs_domain::{
    DomainError,
    legacy_catalog::{self, LegacyColumn, LegacyTable},
};
use ttrs_shared::{LegacyColumnDto, LegacyImportResultDto, LegacyTableDto, LegacyTableSummaryDto};

use crate::{AppError, error::RepoError, ports::LegacyCatalogRepository};

#[derive(Clone)]
pub struct LegacyCatalogService {
    repo: Arc<dyn LegacyCatalogRepository>,
}

impl LegacyCatalogService {
    pub fn new(repo: Arc<dyn LegacyCatalogRepository>) -> Self {
        Self { repo }
    }

    /// 匯入（整批取代，C2）；名稱不合法或重複時整批不寫入（C6）
    pub async fn import(
        &self,
        tables: Vec<LegacyTableDto>,
    ) -> Result<LegacyImportResultDto, AppError> {
        let tables: Vec<LegacyTable> = tables.into_iter().map(to_domain).collect();
        legacy_catalog::validate(&tables)?;
        self.repo.replace_all(&tables).await.map_err(infra)?;
        Ok(LegacyImportResultDto {
            tables: count(tables.len()),
            columns: count(tables.iter().map(|t| t.columns.len()).sum()),
        })
    }

    /// 依舊表代碼、英文名稱或說明查詢（規則見 legacy_catalog::matches）
    pub async fn search(&self, query: &str) -> Result<Vec<LegacyTableSummaryDto>, AppError> {
        let list = self.repo.list_tables().await.map_err(infra)?;
        Ok(list
            .into_iter()
            .filter(|t| legacy_catalog::matches(query, &t.legacy_name, &t.name, &t.title))
            .map(|t| LegacyTableSummaryDto {
                legacy_name: t.legacy_name,
                name: t.name,
                title: t.title,
                module: t.module,
                column_count: t.column_count,
            })
            .collect())
    }

    pub async fn get(&self, legacy_name: &str) -> Result<LegacyTableDto, AppError> {
        self.repo
            .get(legacy_name)
            .await
            .map_err(infra)?
            .map(to_dto)
            .ok_or_else(|| DomainError::LegacyTableNotFound(legacy_name.to_owned()).into())
    }

    /// 已移植資料表（英文表名）對應的 TipTop 舊表；沒有對照時為 None（C1）
    pub async fn for_table(&self, table: &str) -> Result<Option<LegacyTableDto>, AppError> {
        Ok(self
            .repo
            .find_by_name(table)
            .await
            .map_err(infra)?
            .map(to_dto))
    }
}

fn infra(e: RepoError) -> AppError {
    AppError::Infra(e.to_string())
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn to_domain(t: LegacyTableDto) -> LegacyTable {
    LegacyTable {
        legacy_name: t.legacy_name.trim().to_owned(),
        name: t.name.trim().to_owned(),
        title: t.title,
        purpose: t.purpose.filter(|s| !s.trim().is_empty()),
        module: t.module,
        columns: t
            .columns
            .into_iter()
            .map(|c| LegacyColumn {
                legacy_name: c.legacy_name.trim().to_owned(),
                name: c.name.trim().to_owned(),
                data_type: c.data_type,
                primary_key: c.primary_key,
                description: c.description,
                notes: c.notes.filter(|s| !s.trim().is_empty()),
                unused: c.unused,
                verify: c.verify,
            })
            .collect(),
    }
}

fn to_dto(t: LegacyTable) -> LegacyTableDto {
    LegacyTableDto {
        legacy_name: t.legacy_name,
        name: t.name,
        title: t.title,
        purpose: t.purpose,
        module: t.module,
        columns: t
            .columns
            .into_iter()
            .map(|c| LegacyColumnDto {
                legacy_name: c.legacy_name,
                name: c.name,
                data_type: c.data_type,
                primary_key: c.primary_key,
                description: c.description,
                notes: c.notes,
                unused: c.unused,
                verify: c.verify,
            })
            .collect(),
    }
}
