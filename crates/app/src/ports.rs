//! Repository 介面。實作在 ttrs-infra（PostgreSQL）與 memory 模組（測試）。

use async_trait::async_trait;
use ttrs_domain::{
    dictionary::{ColumnText, TableDictionary, TableText},
    item::{Item, ItemNo},
    legacy_catalog::LegacyTable,
    permission::ProgramPermission,
    permission_class::{ClassCode, PermissionClass},
    plant::{Plant, PlantId},
    program::{ModuleCode, Program, ProgramNo},
    qbe::QbeFilter,
    user::{User, UserId},
    user_permission::UserPermission,
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
    /// 新增程式與名稱；`grant_to` 有值時同時建立該程式權限。代號重複回傳 `Conflict`
    async fn insert(
        &self,
        program: &Program,
        names: &[ProgramName],
        grant_to: Option<&ProgramPermission>,
    ) -> Result<(), RepoError>;
    async fn update(&self, program: &Program) -> Result<(), RepoError>;
    /// 修改代號，連動名稱、權限、選單。新代號重複回傳 `Conflict`
    async fn change_no(&self, old: &ProgramNo, new: &ProgramNo) -> Result<(), RepoError>;
    /// 刪除程式並連帶刪除名稱、權限、選單，同時寫入異動紀錄
    async fn delete(&self, no: &ProgramNo, audit: &AuditEntry) -> Result<(), RepoError>;
    async fn names(&self, no: &ProgramNo) -> Result<Vec<ProgramName>, RepoError>;
    /// 新增或更新某語系的名稱與報表抬頭
    async fn set_name(&self, no: &ProgramNo, name: &ProgramName) -> Result<(), RepoError>;
    /// 授權（已存在則不變；TipTop p_zz_gen 會重設 zy03，新系統不覆寫已調整的權限）
    async fn grant(&self, permission: &ProgramPermission) -> Result<(), RepoError>;
    async fn permission_classes(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError>;
    async fn menus_containing(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError>;
    /// 選單直接包含的程式（zm_file，依程式代號排序；子選單為模組 MENU 的程式）
    async fn menu_children(&self, menu: &str) -> Result<Vec<Program>, RepoError>;
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
    /// 外鍵參照的主鍵/唯一限制名稱（TipTop p_zta 限制欄位 rzad07 Ref. Constraint Name）
    pub ref_constraint: Option<String>,
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

/// TipTop 對照目錄清單的一列
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyTableSummary {
    pub legacy_name: String,
    pub name: String,
    pub title: String,
    pub module: String,
    pub column_count: u32,
}

/// TipTop 對照目錄（legacy_catalog.feature）
#[async_trait]
pub trait LegacyCatalogRepository: Send + Sync {
    /// 以新內容整批取代（全有或全無；C2 可重複匯入）
    async fn replace_all(&self, tables: &[LegacyTable]) -> Result<(), RepoError>;
    /// 所有資料表（不含欄位），依舊表代碼排序
    async fn list_tables(&self) -> Result<Vec<LegacyTableSummary>, RepoError>;
    /// 依舊表代碼取得（含欄位，依 .sch 順序）
    async fn get(&self, legacy_name: &str) -> Result<Option<LegacyTable>, RepoError>;
    /// 依英文表名取得（含欄位）；用於已移植的資料表（C1、C4）
    async fn find_by_name(&self, name: &str) -> Result<Option<LegacyTable>, RepoError>;
}

/// 使用者（TipTop: zx_file）與可使用營運中心（zxy_file）— user_maintenance.feature
#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn get(&self, id: &UserId) -> Result<Option<User>, RepoError>;
    /// 依使用者代號排序
    async fn list(&self) -> Result<Vec<User>, RepoError>;
    /// 新增使用者與可使用營運中心（同一交易）；代號重複回 Conflict
    async fn insert(&self, user: &User, plants: &[PlantId]) -> Result<(), RepoError>;
    /// 更新使用者並以 `plants` 取代可使用營運中心（同一交易）
    async fn update(&self, user: &User, plants: &[PlantId]) -> Result<(), RepoError>;
    /// 刪除使用者連同可使用營運中心（U9，p_zx.4gl:1246-1249）
    async fn delete(&self, id: &UserId) -> Result<(), RepoError>;
    async fn plants(&self, id: &UserId) -> Result<Vec<PlantId>, RepoError>;
    /// 公司帳號（小寫）目前綁定的使用者
    async fn find_by_idp_username(&self, username: &str) -> Result<Option<UserId>, RepoError>;
}

// ---------------------------------------------------------------------------
// 權限類別（TipTop p_zw / zw_file）
// ---------------------------------------------------------------------------

#[async_trait]
pub trait PermissionClassRepository: Send + Sync {
    async fn get(&self, code: &ClassCode) -> Result<Option<PermissionClass>, RepoError>;
    /// 依代號排序（p_zw.4gl:235-237）
    async fn list(&self) -> Result<Vec<PermissionClass>, RepoError>;
    /// 代號重複回 Conflict
    async fn insert(&self, class: &PermissionClass) -> Result<(), RepoError>;
    async fn update(&self, class: &PermissionClass) -> Result<(), RepoError>;
    async fn delete(&self, code: &ClassCode) -> Result<(), RepoError>;
    /// 使用此權限類別的使用者數：使用者維護的權限類別（zx04）或特殊權限的權限類別列（zxw03 = 1，X7）
    async fn user_count(&self, code: &ClassCode) -> Result<u64, RepoError>;
    /// 此權限類別的程式權限筆數（zy_file）
    async fn permission_count(&self, code: &ClassCode) -> Result<u64, RepoError>;
}

// ---------------------------------------------------------------------------
// 營運中心（TipTop aooi901 / azp_file）
// ---------------------------------------------------------------------------

#[async_trait]
pub trait PlantRepository: Send + Sync {
    async fn get(&self, id: &PlantId) -> Result<Option<Plant>, RepoError>;
    /// 依代號排序（aooi901.4gl:151-152）
    async fn list(&self) -> Result<Vec<Plant>, RepoError>;
    /// 代號重複回 Conflict
    async fn insert(&self, plant: &Plant) -> Result<(), RepoError>;
    async fn update(&self, plant: &Plant) -> Result<(), RepoError>;
    /// 仍有使用者（預設或可使用營運中心）或業務資料參照時回 InUse，不做連帶刪除（P10）
    async fn delete(&self, id: &PlantId) -> Result<(), RepoError>;
}

// ---------------------------------------------------------------------------
// 程式執行權限（TipTop p_zy / zy_file）
// ---------------------------------------------------------------------------

/// 權限清單的一列：權限＋程式的基本執行功能與名稱（畫面顯示用）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionListItem {
    pub permission: ProgramPermission,
    /// 程式的可執行功能（zz04），可勾選的範圍
    pub default_actions: Option<String>,
    pub names: Vec<ProgramName>,
}

#[async_trait]
pub trait ProgramPermissionRepository: Send + Sync {
    async fn get(
        &self,
        class: &ClassCode,
        program: &ProgramNo,
    ) -> Result<Option<ProgramPermission>, RepoError>;
    /// 依程式代號排序（p_zy.4gl:948-952）
    async fn list(&self, class: &ClassCode) -> Result<Vec<PermissionListItem>, RepoError>;
    /// 同一權限類別＋程式重複回 Conflict
    async fn insert(&self, permission: &ProgramPermission) -> Result<(), RepoError>;
    async fn update(&self, permission: &ProgramPermission) -> Result<(), RepoError>;
    /// 回傳是否有刪到資料
    async fn delete(&self, class: &ClassCode, program: &ProgramNo) -> Result<bool, RepoError>;
    /// 刪除權限類別的全部權限，回傳筆數
    async fn delete_all(&self, class: &ClassCode) -> Result<u64, RepoError>;
}

// ---------------------------------------------------------------------------
// 使用者特殊權限（TipTop p_zxw / zxw_file）
// ---------------------------------------------------------------------------

/// 特殊權限清單的一列：特殊權限＋對象的顯示資料
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserPermissionItem {
    pub permission: UserPermission,
    /// 權限類別列：權限說明（zw02）
    pub class_name: Option<String>,
    /// 程式列：程式的可執行功能（zz04）與名稱（gaz_file）
    pub default_actions: Option<String>,
    pub names: Vec<ProgramName>,
}

#[async_trait]
pub trait UserPermissionRepository: Send + Sync {
    /// 依項次排序
    async fn list(&self, user: &UserId) -> Result<Vec<UserPermissionItem>, RepoError>;
    /// 項次重複，或同一使用者重複同一權限類別或程式，回 Conflict（X3）
    async fn insert(&self, permission: &UserPermission) -> Result<(), RepoError>;
    async fn update(&self, permission: &UserPermission) -> Result<(), RepoError>;
    async fn delete(&self, user: &UserId, line_no: i32) -> Result<bool, RepoError>;
    async fn delete_all(&self, user: &UserId) -> Result<u64, RepoError>;
}
