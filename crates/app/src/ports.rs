//! Repository 介面。實作在 ttrs-infra（PostgreSQL）與 memory 模組（測試）。

use async_trait::async_trait;
use ttrs_domain::{
    authorization::DataScope,
    department::{Department, DeptCode},
    dictionary::{ColumnText, TableDictionary, TableText},
    employee::{Employee, EmployeeId},
    item::{Item, ItemNo},
    legacy_catalog::LegacyTable,
    permission::ProgramPermission,
    permission_class::{ClassCode, PermissionClass},
    plant::{Plant, PlantId, PlantSchema},
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
    /// 補充說明，例如報表的條件、格式、筆數（ADR 0011 §4）
    pub detail: Option<String>,
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
    /// 使用此 TipTop 程式代號的程式（ADR 0008 P2，不可重複）
    async fn find_by_tiptop_no(&self, tiptop_no: &str) -> Result<Option<ProgramNo>, RepoError>;
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
    /// QBE 查詢（p_zx.4gl:206-207）：符合全部篩選且在資料範圍內（建立者、建立部門，p_zx.4gl:194-204）者
    /// 依使用者代號（位元組順序）排序，回傳指定範圍與總筆數
    async fn query(
        &self,
        filters: &[QbeFilter],
        scope: &DataScope,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<User>, u64), RepoError>;
    /// 新增使用者與可使用營運中心（同一交易）；代號重複回 Conflict
    async fn insert(&self, user: &User, plants: &[PlantId]) -> Result<(), RepoError>;
    /// 更新使用者並以 `plants` 取代可使用營運中心（同一交易）
    async fn update(&self, user: &User, plants: &[PlantId]) -> Result<(), RepoError>;
    /// 刪除使用者連同可使用營運中心（U9，p_zx.4gl:1246-1249）
    async fn delete(&self, id: &UserId) -> Result<(), RepoError>;
    async fn plants(&self, id: &UserId) -> Result<Vec<PlantId>, RepoError>;
    /// 公司帳號（小寫）目前綁定的使用者
    async fn find_by_idp_username(&self, username: &str) -> Result<Option<UserId>, RepoError>;
    /// 公司帳號識別碼（IdP sub）綁定的使用者（sign_in.feature SQ2）
    async fn find_by_idp_subject(&self, subject: &str) -> Result<Option<UserId>, RepoError>;
    /// 首次登入記下公司帳號識別碼；不更新修改者／修改日
    async fn bind_idp_subject(&self, id: &UserId, subject: &str) -> Result<(), RepoError>;
    /// 切換營運中心後寫回預設營運中心（aoos901.4gl:231，SQ8）；不更新修改者／修改日
    async fn set_default_plant(&self, id: &UserId, plant: &PlantId) -> Result<(), RepoError>;
}

// ---------------------------------------------------------------------------
// 權限類別（TipTop p_zw / zw_file）
// ---------------------------------------------------------------------------

#[async_trait]
pub trait PermissionClassRepository: Send + Sync {
    async fn get(&self, code: &ClassCode) -> Result<Option<PermissionClass>, RepoError>;
    /// 依代號排序（p_zw.4gl:235-237）
    async fn list(&self) -> Result<Vec<PermissionClass>, RepoError>;
    /// QBE 查詢（p_zw.4gl:235-237）：符合全部篩選者依代號（位元組順序）排序，回傳指定範圍與總筆數
    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<PermissionClass>, u64), RepoError>;
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
    /// QBE 查詢（aooi901.4gl:151-152）：符合全部篩選者依代號（位元組順序）排序，回傳指定範圍與總筆數
    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Plant>, u64), RepoError>;
    /// 全部營運中心已使用的資料庫代碼（P13 給號與重複檢查）
    async fn schema_names(&self) -> Result<Vec<PlantSchema>, RepoError>;
    /// 代號或資料庫代碼重複回 Conflict。一併建立工廠 schema 與資料庫角色並套用全部工廠 migration，
    /// 任何一步失敗就全部不建（P14，ADR 0007 §5）
    async fn insert(&self, plant: &Plant) -> Result<(), RepoError>;
    async fn update(&self, plant: &Plant) -> Result<(), RepoError>;
    /// 仍有使用者（預設或可使用營運中心）或工廠 schema 內有任何資料時回 InUse，不做連帶刪除（P10）；
    /// 刪除時一併移除工廠 schema 與資料庫角色（P15）
    async fn delete(&self, id: &PlantId) -> Result<(), RepoError>;
    /// 是否仍被使用（同 delete 回 InUse 的條件），供刪除前決定錯誤訊息的先後（P10 優先於 P17）
    async fn in_use(&self, id: &PlantId) -> Result<bool, RepoError>;
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
    /// 權限類別的 QBE 查詢（p_zy.4gl:194-203）：`header` 篩選權限類別本身；`detail` 有值時只取
    /// 至少有一筆程式權限同時符合全部 `detail` 的權限類別（YQ2）。依代號（位元組順序）排序
    async fn query_classes(
        &self,
        header: &[QbeFilter],
        detail: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<PermissionClass>, u64), RepoError>;
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

// ---------------------------------------------------------------------------
// 部門、員工（TipTop aooi030 gem_file、aooi040 gen_file；工廠層，在營運中心的工廠 schema）
// ---------------------------------------------------------------------------

#[async_trait]
pub trait DepartmentRepository: Send + Sync {
    async fn get(&self, plant: &PlantId, code: &DeptCode) -> Result<Option<Department>, RepoError>;
    /// QBE 查詢（aooi030.4gl:955）：依部門編號（位元組順序）排序，回傳指定範圍與總筆數
    async fn query_departments(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Department>, u64), RepoError>;
    /// 部門編號重複回 Conflict；營運中心不存在回 Other
    async fn insert(&self, plant: &PlantId, dept: &Department) -> Result<(), RepoError>;
    async fn update(&self, plant: &PlantId, dept: &Department) -> Result<(), RepoError>;
    /// 還有員工時回 InUse（DQ5）；回傳是否有刪到資料
    async fn delete(&self, plant: &PlantId, code: &DeptCode) -> Result<bool, RepoError>;
}

#[async_trait]
pub trait EmployeeRepository: Send + Sync {
    async fn get(&self, plant: &PlantId, id: &EmployeeId) -> Result<Option<Employee>, RepoError>;
    /// QBE 查詢（aooi040.4gl:692）：依員工編號（位元組順序）排序，回傳指定範圍與總筆數
    async fn query_employees(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Employee>, u64), RepoError>;
    /// 員工編號重複回 Conflict
    async fn insert(&self, plant: &PlantId, employee: &Employee) -> Result<(), RepoError>;
    async fn update(&self, plant: &PlantId, employee: &Employee) -> Result<(), RepoError>;
    /// 回傳是否有刪到資料
    async fn delete(&self, plant: &PlantId, id: &EmployeeId) -> Result<bool, RepoError>;
}

// ---------------------------------------------------------------------------
// Web 密碼與帳號稽核原則（TipTop zx10、zx16、zx17、zx19、zx20 / gbt_file；ADR 0009）
// ---------------------------------------------------------------------------

pub use ttrs_domain::password::{AccountPolicy, Credential};

#[async_trait]
pub trait CredentialRepository: Send + Sync {
    /// 尚未設定密碼的使用者回 None（初始密碼＝使用者代號，W1）
    async fn credential(&self, user: &UserId) -> Result<Option<Credential>, RepoError>;
    /// 新增或取代使用者的密碼狀態；使用者刪除時一併刪除
    async fn save_credential(
        &self,
        user: &UserId,
        credential: &Credential,
    ) -> Result<(), RepoError>;
    /// 連線版本：登入時記下，與目前值不同的連線即失效（ADR 0010 R6）；沒有資料時為 0
    async fn session_version(&self, user: &UserId) -> Result<u32, RepoError>;
    /// 結束使用者所有已登入的連線（連線版本＋1）
    async fn end_sessions(&self, user: &UserId) -> Result<(), RepoError>;
}

#[async_trait]
pub trait AccountPolicyRepository: Send + Sync {
    /// 系統帳號稽核原則（gbt00＝0 那一筆）
    async fn account_policy(&self) -> Result<AccountPolicy, RepoError>;
}

// ---------------------------------------------------------------------------
// 忘記密碼（TTRS 新增；password_reset_token，ADR 0010）
// ---------------------------------------------------------------------------

pub use ttrs_domain::password_reset::ResetToken;

#[async_trait]
pub trait PasswordResetTokenRepository: Send + Sync {
    /// 新增權杖（只存雜湊）；同一使用者先前尚未使用的權杖同時失效（決策 A4）
    async fn issue_reset_token(
        &self,
        token_hash: &str,
        token: &ResetToken,
    ) -> Result<(), RepoError>;
    async fn reset_token(&self, token_hash: &str) -> Result<Option<ResetToken>, RepoError>;
    /// 此時間之後建立的權杖數（申請次數上限，R7）
    async fn reset_requests_since(
        &self,
        user: &UserId,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, RepoError>;
    /// 重設成功：該使用者所有尚未使用的權杖失效
    async fn revoke_reset_tokens(
        &self,
        user: &UserId,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepoError>;
}

/// 寫入異動紀錄（azo_file / audit_log）；讀取用 [`ProgramRepository::audit_entries`]
#[async_trait]
pub trait AuditLog: Send + Sync {
    async fn record(&self, entry: &AuditEntry) -> Result<(), RepoError>;
}

// ---------------------------------------------------------------------------
// 分類碼（TipTop azf_file，aooi300）：工廠層
// ---------------------------------------------------------------------------

pub use ttrs_domain::classification::{ClassificationCode, Code, CodeKind};

#[async_trait]
pub trait ClassificationCodeRepository: Send + Sync {
    /// 該類別的全部分類碼，依代號排序（位元組順序）
    async fn list_codes(
        &self,
        plant: &PlantId,
        kind: CodeKind,
    ) -> Result<Vec<ClassificationCode>, RepoError>;
    async fn get_code(
        &self,
        plant: &PlantId,
        kind: CodeKind,
        code: &Code,
    ) -> Result<Option<ClassificationCode>, RepoError>;
    /// 同一類別內代號重複時回 Conflict
    async fn insert_code(&self, plant: &PlantId, c: &ClassificationCode) -> Result<(), RepoError>;
    async fn update_code(&self, plant: &PlantId, c: &ClassificationCode) -> Result<(), RepoError>;
    /// 只刪除該類別的代號（CC4）；不存在回 false
    async fn delete_code(
        &self,
        plant: &PlantId,
        kind: CodeKind,
        code: &Code,
    ) -> Result<bool, RepoError>;
}
