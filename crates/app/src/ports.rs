//! Repository 介面。實作在 ttrs-infra（PostgreSQL）與 memory 模組（測試）。

use async_trait::async_trait;
use ttrs_domain::{
    dictionary::{ColumnText, TableDictionary, TableText},
    item::{Item, ItemNo},
    plant::PlantId,
    program::{ModuleCode, Program, ProgramNo},
    qbe::QbeFilter,
};

use crate::error::RepoError;

#[async_trait]
pub trait ItemRepository: Send + Sync {
    async fn get(&self, plant: &PlantId, no: &ItemNo) -> Result<Option<Item>, RepoError>;
    async fn list(&self, plant: &PlantId) -> Result<Vec<Item>, RepoError>;
    async fn insert(&self, item: &Item) -> Result<(), RepoError>;
    async fn update(&self, item: &Item) -> Result<(), RepoError>;
    async fn delete(&self, plant: &PlantId, no: &ItemNo) -> Result<(), RepoError>;
    /// 是否已有庫存異動（TipTop: tlf_file）
    async fn has_transactions(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, RepoError>;
}

/// 共用基本資料查詢（營運中心、單位…）
#[async_trait]
pub trait MasterDataRepository: Send + Sync {
    async fn plant_active(&self, plant: &PlantId) -> Result<bool, RepoError>;
    async fn unit_exists(&self, unit: &str) -> Result<bool, RepoError>;
}

/// 程式名稱（TipTop: gaz_file）；定義在 domain，多語言規則共用（ADR 0005）
pub use ttrs_domain::program::ProgramName;

/// 異動紀錄（TipTop: azo_file）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub program: String,
    pub user_id: String,
    pub key: String,
    pub action: String,
}

/// 程式維護（TipTop: zz_file、gaz_file、zy_file、zm_file、azo_file）。
/// 需要連動多張表的方法（新增、改代號、刪除）必須在同一交易內完成。
#[async_trait]
pub trait ProgramRepository: Send + Sync {
    async fn get(&self, no: &ProgramNo) -> Result<Option<Program>, RepoError>;
    async fn list(&self) -> Result<Vec<Program>, RepoError>;
    /// QBE 查詢：符合全部篩選者依程式代號（位元組順序）排序，回傳指定範圍與總筆數
    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Program>, u64), RepoError>;
    /// 新增程式與名稱；`grant_to` 有值時同時授權給該權限類別。代號重複回傳 `Conflict`
    async fn insert(
        &self,
        program: &Program,
        names: &[ProgramName],
        grant_to: Option<&str>,
    ) -> Result<(), RepoError>;
    async fn update(&self, program: &Program) -> Result<(), RepoError>;
    /// 修改代號，連動名稱、權限、選單。新代號重複回傳 `Conflict`
    async fn change_no(&self, old: &ProgramNo, new: &ProgramNo) -> Result<(), RepoError>;
    /// 刪除程式並連帶刪除名稱、權限、選單，同時寫入異動紀錄
    async fn delete(&self, no: &ProgramNo, audit: &AuditEntry) -> Result<(), RepoError>;
    async fn names(&self, no: &ProgramNo) -> Result<Vec<ProgramName>, RepoError>;
    /// 新增或更新某語系的名稱與報表抬頭
    async fn set_name(&self, no: &ProgramNo, name: &ProgramName) -> Result<(), RepoError>;
    /// 授權（已存在則不變，同 p_zz_gen 的 upsert）
    async fn grant(&self, permission_class: &str, no: &ProgramNo) -> Result<(), RepoError>;
    async fn permission_classes(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError>;
    async fn menus_containing(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError>;
    async fn audit_entries(&self, key: &str) -> Result<Vec<AuditEntry>, RepoError>;
    async fn module_exists(&self, module: &ModuleCode) -> Result<bool, RepoError>;
    async fn permission_class_exists(&self, class: &str) -> Result<bool, RepoError>;
    async fn query_definition_exists(&self, query: &str) -> Result<bool, RepoError>;
}

// ---------------------------------------------------------------------------
// 資料字典（TipTop p_zta）
// ---------------------------------------------------------------------------

/// 資料庫實際結構的一張表（唯讀；PostgreSQL 由系統目錄讀取，記憶體版由測試提供）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnSchema>,
    pub indexes: Vec<IndexSchema>,
    pub constraints: Vec<ConstraintSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSchema {
    pub name: String,
    pub unique: bool,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintKind {
    PrimaryKey,
    ForeignKey,
    Unique,
    Check,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintSchema {
    pub name: String,
    pub kind: ConstraintKind,
    pub columns: Vec<String>,
    pub ref_table: Option<String>,
    pub ref_columns: Vec<String>,
    pub check_clause: Option<String>,
}

/// 一個 migration 的內容（版本、說明、SQL），用來推得變更紀錄
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationSource {
    pub version: i64,
    pub description: String,
    pub sql: String,
}

/// 資料庫結構（唯讀）
#[async_trait]
pub trait SchemaCatalog: Send + Sync {
    /// 目前 schema 的資料表名稱（不含 sqlx 的 migration 紀錄表）
    async fn table_names(&self) -> Result<Vec<String>, RepoError>;
    async fn table(&self, name: &str) -> Result<Option<TableSchema>, RepoError>;
    /// 依版本排序的所有 migration
    async fn migrations(&self) -> Result<Vec<MigrationSource>, RepoError>;
}

/// 資料字典（TipTop zta_file、gat_file、gaq_file）
#[async_trait]
pub trait DictionaryRepository: Send + Sync {
    async fn get(&self, table: &str) -> Result<Option<TableDictionary>, RepoError>;
    async fn list(&self) -> Result<Vec<TableDictionary>, RepoError>;
    /// 已存在回傳 `Conflict`
    async fn insert(&self, dict: &TableDictionary) -> Result<(), RepoError>;
    async fn update(&self, dict: &TableDictionary) -> Result<(), RepoError>;
    /// 刪除字典，連同表與欄位的多語言說明（D3）
    async fn delete(&self, table: &str) -> Result<(), RepoError>;
    async fn set_table_text(&self, table: &str, text: &TableText) -> Result<(), RepoError>;
    async fn table_texts(&self, table: &str) -> Result<Vec<TableText>, RepoError>;
    async fn set_column_text(&self, table: &str, text: &ColumnText) -> Result<(), RepoError>;
    async fn column_texts(&self, table: &str) -> Result<Vec<ColumnText>, RepoError>;
    async fn module_exists(&self, module: &ModuleCode) -> Result<bool, RepoError>;
}
