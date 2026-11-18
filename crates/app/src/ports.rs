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

/// 料件（工廠層；item_maintenance.feature）
#[async_trait]
pub trait ItemRepository: Send + Sync {
    /// 全部料件，依料號排序
    async fn list_items(&self, plant: &PlantId) -> Result<Vec<Item>, RepoError>;
    async fn get_item(&self, plant: &PlantId, no: &ItemNo) -> Result<Option<Item>, RepoError>;
    /// QBE 查詢（aimi100.4gl:703-730）：符合全部篩選且在資料範圍內（imauser、imagrup）者，
    /// 依料號（位元組順序）排序，回傳指定範圍與總筆數；品名、規格比對任一語系（IQ3）
    async fn query_items(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        scope: &DataScope,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Item>, u64), RepoError>;
    /// 料號重複回 `Conflict`
    async fn insert_item(&self, plant: &PlantId, item: &Item) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`（ADR 0012 §4）
    async fn update_item(&self, plant: &PlantId, item: &Item) -> Result<(), RepoError>;
    /// 連同多語言品名一併刪除；不存在回 false
    async fn delete_item(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, RepoError>;
    /// 是否已有庫存異動（TipTop: tlf_file）
    async fn has_transactions(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, RepoError>;
    /// 其他語系的品名、規格
    async fn item_texts(
        &self,
        plant: &PlantId,
        no: &ItemNo,
    ) -> Result<Vec<ttrs_domain::item::ItemText>, RepoError>;
    /// 新增或取代一個語系的品名、規格；兩者皆空白時刪除該語系
    async fn set_item_text(
        &self,
        plant: &PlantId,
        no: &ItemNo,
        text: &ttrs_domain::item::ItemText,
    ) -> Result<(), RepoError>;
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

/// 資料異動記錄（change_log，ADR 0012 §2）；寫入由儲存端完成（PostgreSQL 觸發器、記憶體後端），這裡只查詢
#[async_trait]
pub trait ChangeLogRepository: Send + Sync {
    /// 某筆資料的異動記錄，依發生順序：`tables` 中鍵值包含 `key` 的記錄（主檔與其子表一併列出，CH2）。
    /// `plant` 為 None 時查共用 schema
    async fn history(
        &self,
        plant: Option<&PlantId>,
        tables: &[&str],
        key: &std::collections::BTreeMap<String, String>,
    ) -> Result<Vec<crate::change_log::ChangeRecord>, RepoError>;
}

/// 選單目錄（zm_file，p_zm）；側欄樹狀選單也由此組成（cl_create_4sm）
#[async_trait]
pub trait MenuRepository: Send + Sync {
    /// 全部單身（組成樹狀、檢查迴圈與使用中；資料量小）
    async fn all_menu_items(&self) -> Result<Vec<ttrs_domain::menu::MenuItem>, RepoError>;
    /// 某目錄的單身，依序號排列
    async fn menu_items(&self, menu: &str) -> Result<Vec<ttrs_domain::menu::MenuItem>, RepoError>;
    /// 新增單身；序號或程式在同一目錄重複回 `Conflict`
    async fn insert_menu_item(&self, item: &ttrs_domain::menu::MenuItem) -> Result<(), RepoError>;
    /// 移除單身；回傳是否有刪到
    async fn remove_menu_item(&self, menu: &str, program: &str) -> Result<bool, RepoError>;
    /// 依 `items` 更新目錄內各程式的序號（上移、下移、重新編號），同一交易內完成
    async fn set_menu_seqs(
        &self,
        menu: &str,
        items: &[ttrs_domain::menu::MenuItem],
    ) -> Result<(), RepoError>;
    /// 刪除目錄的全部單身（p_zm.4gl:604-636）
    async fn clear_menu(&self, menu: &str) -> Result<(), RepoError>;
}

/// 基本代碼（區域別 gea、國別 geb、地區 geo、交運方式 ged、報表抬頭 azh）；共用 schema（RC2、SM2、RH3）
#[async_trait]
pub trait BasicCodeRepository: Send + Sync {
    /// 該種類的全部代碼，依代號（位元組順序）排序
    async fn list_basic_codes(
        &self,
        kind: ttrs_domain::basic_code::BasicCodeKind,
    ) -> Result<Vec<ttrs_domain::basic_code::BasicCode>, RepoError>;
    async fn get_basic_code(
        &self,
        kind: ttrs_domain::basic_code::BasicCodeKind,
        code: &ttrs_domain::basic_code::BasicCodeValue,
    ) -> Result<Option<ttrs_domain::basic_code::BasicCode>, RepoError>;
    /// 代號重複回 `Conflict`；同時寫入其他語系名稱
    async fn insert_basic_code(
        &self,
        code: &ttrs_domain::basic_code::BasicCode,
    ) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`；其他語系名稱整批取代
    async fn update_basic_code(
        &self,
        code: &ttrs_domain::basic_code::BasicCode,
    ) -> Result<(), RepoError>;
    /// 已被下層代碼引用時回 `InUse`（RC4）；回傳是否有刪到
    async fn delete_basic_code(
        &self,
        kind: ttrs_domain::basic_code::BasicCodeKind,
        code: &ttrs_domain::basic_code::BasicCodeValue,
    ) -> Result<bool, RepoError>;
}

/// 專案（gja_file，aooi200）；工廠層（PJ2）
#[async_trait]
pub trait ProjectRepository: Send + Sync {
    /// 該營運中心的全部專案，依編號排序；營運中心不存在時為空
    async fn list_projects(
        &self,
        plant: &ttrs_domain::plant::PlantId,
    ) -> Result<Vec<ttrs_domain::project::Project>, RepoError>;
    async fn get_project(
        &self,
        plant: &ttrs_domain::plant::PlantId,
        code: &ttrs_domain::project::ProjectCode,
    ) -> Result<Option<ttrs_domain::project::Project>, RepoError>;
    /// 編號重複回 `Conflict`
    async fn insert_project(
        &self,
        plant: &ttrs_domain::plant::PlantId,
        p: &ttrs_domain::project::Project,
    ) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`
    async fn update_project(
        &self,
        plant: &ttrs_domain::plant::PlantId,
        p: &ttrs_domain::project::Project,
    ) -> Result<(), RepoError>;
    /// 已被單據引用時回 `InUse`（PJ4）；回傳是否有刪到
    async fn delete_project(
        &self,
        plant: &ttrs_domain::plant::PlantId,
        code: &ttrs_domain::project::ProjectCode,
    ) -> Result<bool, RepoError>;
}

/// 稅別（gec_file，aooi150）；共用 schema（TX2）。鍵值為稅別碼＋進／銷項
#[async_trait]
pub trait TaxTypeRepository: Send + Sync {
    /// 全部稅別，依稅別碼、進銷項排序
    async fn list_tax_types(&self) -> Result<Vec<ttrs_domain::tax_type::TaxType>, RepoError>;
    async fn get_tax_type(
        &self,
        key: &ttrs_domain::tax_type::TaxKey,
    ) -> Result<Option<ttrs_domain::tax_type::TaxType>, RepoError>;
    /// 鍵值重複回 `Conflict`
    async fn insert_tax_type(&self, t: &ttrs_domain::tax_type::TaxType) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`
    async fn update_tax_type(&self, t: &ttrs_domain::tax_type::TaxType) -> Result<(), RepoError>;
    /// 已被引用時回 `InUse`（TX7）；回傳是否有刪到
    async fn delete_tax_type(&self, key: &ttrs_domain::tax_type::TaxKey)
    -> Result<bool, RepoError>;
}

/// 幣別（azi_file，aooi050）；共用 schema（CY2）
#[async_trait]
pub trait CurrencyRepository: Send + Sync {
    /// 全部幣別，依代碼排序
    async fn list_currencies(&self) -> Result<Vec<ttrs_domain::currency::Currency>, RepoError>;
    async fn get_currency(
        &self,
        code: &ttrs_domain::currency::CurrencyCode,
    ) -> Result<Option<ttrs_domain::currency::Currency>, RepoError>;
    /// 代碼重複回 `Conflict`
    async fn insert_currency(&self, c: &ttrs_domain::currency::Currency) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`
    async fn update_currency(&self, c: &ttrs_domain::currency::Currency) -> Result<(), RepoError>;
    /// 已被引用時回 `InUse`（CY6）；回傳是否有刪到
    async fn delete_currency(
        &self,
        code: &ttrs_domain::currency::CurrencyCode,
    ) -> Result<bool, RepoError>;
}

/// 匯率（azj_file 每月、azk_file 每日；aooi060、aooi070、aoop010）；共用 schema（ER1）
#[async_trait]
pub trait ExchangeRateRepository: Send + Sync {
    /// 全部每月匯率，依幣別、年月排序
    async fn list_monthly_rates(
        &self,
    ) -> Result<Vec<ttrs_domain::exchange_rate::MonthlyRate>, RepoError>;
    async fn get_monthly_rate(
        &self,
        currency: &ttrs_domain::currency::CurrencyCode,
        period: ttrs_domain::exchange_rate::RatePeriod,
    ) -> Result<Option<ttrs_domain::exchange_rate::MonthlyRate>, RepoError>;
    /// 鍵值重複回 `Conflict`
    async fn insert_monthly_rate(
        &self,
        r: &ttrs_domain::exchange_rate::MonthlyRate,
    ) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`
    async fn update_monthly_rate(
        &self,
        r: &ttrs_domain::exchange_rate::MonthlyRate,
    ) -> Result<(), RepoError>;
    /// 回傳是否有刪到
    async fn delete_monthly_rate(
        &self,
        currency: &ttrs_domain::currency::CurrencyCode,
        period: ttrs_domain::exchange_rate::RatePeriod,
    ) -> Result<bool, RepoError>;

    /// 每日匯率，依幣別、日期排序；`currency` 有值時只取該幣別、`from..=to` 有值時只取該期間
    async fn list_daily_rates(
        &self,
        currency: Option<&ttrs_domain::currency::CurrencyCode>,
        from: Option<chrono::NaiveDate>,
        to: Option<chrono::NaiveDate>,
    ) -> Result<Vec<ttrs_domain::exchange_rate::DailyRate>, RepoError>;
    async fn get_daily_rate(
        &self,
        currency: &ttrs_domain::currency::CurrencyCode,
        date: chrono::NaiveDate,
    ) -> Result<Option<ttrs_domain::exchange_rate::DailyRate>, RepoError>;
    /// 鍵值重複回 `Conflict`
    async fn insert_daily_rate(
        &self,
        r: &ttrs_domain::exchange_rate::DailyRate,
    ) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`
    async fn update_daily_rate(
        &self,
        r: &ttrs_domain::exchange_rate::DailyRate,
    ) -> Result<(), RepoError>;
    async fn delete_daily_rate(
        &self,
        currency: &ttrs_domain::currency::CurrencyCode,
        date: chrono::NaiveDate,
    ) -> Result<bool, RepoError>;
    /// 是否已有任何每月或每日匯率（變更本國幣別前檢查，general_parameters.feature GP5）
    async fn has_any_rate(&self) -> Result<bool, RepoError>;
}

/// 匯率自動匯入的來源（台銀牌告、關務署每旬報關匯率；exchange_rate_import.feature IM1）。
/// infra 以 HTTPS 下載，記憶體後端由 BDD 步驟設定。失敗回 EXCHANGE_RATE_SOURCE_UNAVAILABLE／_INVALID
#[async_trait]
pub trait RateFeed: Send + Sync {
    /// 台銀牌告匯率（現金或即期）
    async fn bank_quotes(
        &self,
        kind: ttrs_domain::exchange_rate::feed::BankRateType,
    ) -> Result<Vec<ttrs_domain::exchange_rate::feed::Quote>, ttrs_domain::DomainError>;
    /// 關務署當旬報關匯率：起日、迄日、報價
    async fn customs_current(
        &self,
    ) -> Result<
        (
            chrono::NaiveDate,
            chrono::NaiveDate,
            Vec<ttrs_domain::exchange_rate::feed::Quote>,
        ),
        ttrs_domain::DomainError,
    >;
    /// 關務署歷史報關匯率
    async fn customs_history(
        &self,
    ) -> Result<Vec<ttrs_domain::exchange_rate::feed::HistoryQuote>, ttrs_domain::DomainError>;
}

/// 系統參數（aza_file 整體管理系統參數；aoos010，general_parameters.feature）；共用 schema、只有一筆（GP2）
#[async_trait]
pub trait SystemParameterRepository: Send + Sync {
    /// 本國幣別（aza17）；未設定回 None
    async fn base_currency(&self) -> Result<Option<String>, RepoError>;
    /// 直接設定本國幣別（測試與資料轉入用，不經 use case）
    async fn set_base_currency(&self, code: &str) -> Result<(), RepoError>;
    /// 整體管理系統參數；沒有資料時回 None
    async fn general_parameters(
        &self,
    ) -> Result<Option<ttrs_domain::general_parameter::GeneralParameters>, RepoError>;
    /// 修改唯一的一筆；版本不符回 `Stale`
    async fn update_general_parameters(
        &self,
        p: &ttrs_domain::general_parameter::GeneralParameters,
    ) -> Result<(), RepoError>;
}

/// 單位（gfe_file，aooi101）；共用 schema（UN2）
#[async_trait]
pub trait UnitRepository: Send + Sync {
    /// 全部單位，依代碼排序
    async fn list_units(&self) -> Result<Vec<ttrs_domain::unit::Unit>, RepoError>;
    async fn get_unit(
        &self,
        code: &ttrs_domain::unit::UnitCode,
    ) -> Result<Option<ttrs_domain::unit::Unit>, RepoError>;
    /// 代碼重複回 `Conflict`
    async fn insert_unit(&self, unit: &ttrs_domain::unit::Unit) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`
    async fn update_unit(&self, unit: &ttrs_domain::unit::Unit) -> Result<(), RepoError>;
    /// 已被料件等資料引用時回 `InUse`（UN5）；回傳是否有刪到
    async fn delete_unit(&self, code: &ttrs_domain::unit::UnitCode) -> Result<bool, RepoError>;
}

/// 產品分類（oba_file，axmi110）；工廠層（PC2）
#[async_trait]
pub trait ProductClassRepository: Send + Sync {
    /// 全部產品分類，依分類碼排序
    async fn list_product_classes(
        &self,
        plant: &PlantId,
    ) -> Result<Vec<ttrs_domain::product_class::ProductClass>, RepoError>;
    async fn get_product_class(
        &self,
        plant: &PlantId,
        code: &ttrs_domain::product_class::ProductClassCode,
    ) -> Result<Option<ttrs_domain::product_class::ProductClass>, RepoError>;
    /// 分類碼重複回 `Conflict`
    async fn insert_product_class(
        &self,
        plant: &PlantId,
        c: &ttrs_domain::product_class::ProductClass,
    ) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`
    async fn update_product_class(
        &self,
        plant: &PlantId,
        c: &ttrs_domain::product_class::ProductClass,
    ) -> Result<(), RepoError>;
    /// 被料件引用時回 `InUse`（PC8）；回傳是否有刪到
    async fn delete_product_class(
        &self,
        plant: &PlantId,
        code: &ttrs_domain::product_class::ProductClassCode,
    ) -> Result<bool, RepoError>;
}

/// 料件分群（imz_file，aimi110）；工廠層（IG2）
#[async_trait]
pub trait ItemGroupRepository: Send + Sync {
    /// 全部料件分群，依分群碼排序
    async fn list_item_groups(
        &self,
        plant: &PlantId,
    ) -> Result<Vec<ttrs_domain::item_group::ItemGroup>, RepoError>;
    async fn get_item_group(
        &self,
        plant: &PlantId,
        code: &ttrs_domain::item_group::ItemGroupCode,
    ) -> Result<Option<ttrs_domain::item_group::ItemGroup>, RepoError>;
    /// QBE 查詢（aimi110.4gl:476-489）：資料範圍 imzuser、imzgrup，依分群碼（位元組順序）排序
    async fn query_item_groups(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        scope: &DataScope,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<ttrs_domain::item_group::ItemGroup>, u64), RepoError>;
    /// 分群碼重複回 `Conflict`
    async fn insert_item_group(
        &self,
        plant: &PlantId,
        g: &ttrs_domain::item_group::ItemGroup,
    ) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`
    async fn update_item_group(
        &self,
        plant: &PlantId,
        g: &ttrs_domain::item_group::ItemGroup,
    ) -> Result<(), RepoError>;
    /// 被料件引用時回 `InUse`（IG7，mfg9045）；回傳是否有刪到
    async fn delete_item_group(
        &self,
        plant: &PlantId,
        code: &ttrs_domain::item_group::ItemGroupCode,
    ) -> Result<bool, RepoError>;
}

/// 會計期間（azm_file；aoos020，accounting_periods.feature）；共用 schema（PD2）
#[async_trait]
pub trait AccountingPeriodRepository: Send + Sync {
    /// 全部會計年度，依年度排序
    async fn list_accounting_years(
        &self,
    ) -> Result<Vec<ttrs_domain::accounting_period::AccountingYear>, RepoError>;
    async fn get_accounting_year(
        &self,
        year: i32,
    ) -> Result<Option<ttrs_domain::accounting_period::AccountingYear>, RepoError>;
    /// 年度重複回 `Conflict`
    async fn insert_accounting_year(
        &self,
        y: &ttrs_domain::accounting_period::AccountingYear,
    ) -> Result<(), RepoError>;
    /// 表頭與各期一起更新；版本不符回 `Stale`
    async fn update_accounting_year(
        &self,
        y: &ttrs_domain::accounting_period::AccountingYear,
    ) -> Result<(), RepoError>;
    /// 回傳是否有刪到
    async fn delete_accounting_year(&self, year: i32) -> Result<bool, RepoError>;
}

/// 一般單位換算（smc_file；aooi102）；共用 schema（UC2）。一組單位只存一筆（UC3）
#[async_trait]
pub trait UnitConversionRepository: Send + Sync {
    /// 全部，依來源、目的單位排序
    async fn list_unit_conversions(
        &self,
    ) -> Result<Vec<ttrs_domain::unit_conversion::UnitConversion>, RepoError>;
    /// 鍵值重複回 `Conflict`
    async fn insert_unit_conversion(
        &self,
        c: &ttrs_domain::unit_conversion::UnitConversion,
    ) -> Result<(), RepoError>;
    /// 以來源、目的單位更新；版本不符回 `Stale`
    async fn update_unit_conversion(
        &self,
        c: &ttrs_domain::unit_conversion::UnitConversion,
    ) -> Result<(), RepoError>;
    async fn delete_unit_conversion(
        &self,
        from: &ttrs_domain::unit::UnitCode,
        to: &ttrs_domain::unit::UnitCode,
    ) -> Result<bool, RepoError>;
}

/// 料件單位換算（smd_file；aooi103）；工廠 schema（UC2）
#[async_trait]
pub trait ItemUnitConversionRepository: Send + Sync {
    /// 該營運中心的料件換算；`item` 有值時只取該料件
    async fn list_item_unit_conversions(
        &self,
        plant: &ttrs_domain::plant::PlantId,
        item: Option<&ttrs_domain::item::ItemNo>,
    ) -> Result<Vec<ttrs_domain::unit_conversion::ItemUnitConversion>, RepoError>;
    /// 鍵值重複回 `Conflict`
    async fn insert_item_unit_conversion(
        &self,
        plant: &ttrs_domain::plant::PlantId,
        c: &ttrs_domain::unit_conversion::ItemUnitConversion,
    ) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`
    async fn update_item_unit_conversion(
        &self,
        plant: &ttrs_domain::plant::PlantId,
        c: &ttrs_domain::unit_conversion::ItemUnitConversion,
    ) -> Result<(), RepoError>;
    async fn delete_item_unit_conversion(
        &self,
        plant: &ttrs_domain::plant::PlantId,
        item: &ttrs_domain::item::ItemNo,
        from: &ttrs_domain::unit::UnitCode,
        to: &ttrs_domain::unit::UnitCode,
    ) -> Result<bool, RepoError>;
}

/// GitHub 上的 repo（GET /search/repositories 的一筆）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubRepo {
    pub full_name: String,
    pub description: Option<String>,
    pub license: Option<String>,
    pub stars: u32,
    pub updated_at: Option<String>,
    pub default_branch: String,
}

/// repo 檔案樹的一個檔案
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubFile {
    pub path: String,
    pub size: u64,
}

/// 依檔名搜尋找到的 EPUB 檔（epub_github_filename.feature GF1）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubCodeHit {
    pub full_name: String,
    pub path: String,
    /// 下載用的分支或 commit
    pub git_ref: String,
}

/// GitHub 來源（github_epub_search.feature GE9）：infra 以 HTTPS 呼叫 GitHub，記憶體後端由 BDD 設定。
/// 失敗回 GITHUB_RATE_LIMITED／GITHUB_UNAVAILABLE／GITHUB_NOT_FOUND
#[async_trait]
pub trait GithubSource: Send + Sync {
    /// 關鍵字搜尋 repo：該頁結果與總筆數
    async fn search_repos(
        &self,
        keyword: &str,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<GithubRepo>, u64), ttrs_domain::DomainError>;
    async fn get_repo(&self, full_name: &str) -> Result<GithubRepo, ttrs_domain::DomainError>;
    /// 分支的完整檔案樹：檔案與是否未列完
    async fn tree(
        &self,
        full_name: &str,
        branch: &str,
    ) -> Result<(Vec<GithubFile>, bool), ttrs_domain::DomainError>;
    /// 下載檔案內容
    async fn download(
        &self,
        full_name: &str,
        branch: &str,
        path: &str,
    ) -> Result<Vec<u8>, ttrs_domain::DomainError>;
    /// 有沒有設定 token（GF2）：沒有時不能用程式碼搜尋
    fn has_token(&self) -> bool;
    /// 依檔名搜尋 EPUB 檔（GF1）：`name` 是關鍵字的一種寫法；GitHub 只收錄部分檔案
    async fn search_epub_files(
        &self,
        name: &str,
    ) -> Result<Vec<GithubCodeHit>, ttrs_domain::DomainError>;
}

/// 編碼性質（geh_file；encoding_nature.feature）。共用 schema
#[async_trait]
pub trait EncodingNatureRepository: Send + Sync {
    /// 全部性質，依代碼排序
    async fn list_encoding_natures(
        &self,
    ) -> Result<Vec<ttrs_domain::encoding_nature::EncodingNature>, RepoError>;
    async fn get_encoding_nature(
        &self,
        code: &ttrs_domain::encoding_nature::NatureCode,
    ) -> Result<Option<ttrs_domain::encoding_nature::EncodingNature>, RepoError>;
    /// 代碼重複回 `Conflict`
    async fn insert_encoding_nature(
        &self,
        nature: &ttrs_domain::encoding_nature::EncodingNature,
    ) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`
    async fn update_encoding_nature(
        &self,
        nature: &ttrs_domain::encoding_nature::EncodingNature,
    ) -> Result<(), RepoError>;
    /// 是否已被編碼類別使用（EN4、EC9）
    async fn encoding_nature_in_use(
        &self,
        code: &ttrs_domain::encoding_nature::NatureCode,
    ) -> Result<bool, RepoError>;
    /// 已被編碼類別引用時回 `InUse`（EN6）；回傳是否有刪到
    async fn delete_encoding_nature(
        &self,
        code: &ttrs_domain::encoding_nature::NatureCode,
    ) -> Result<bool, RepoError>;
}

/// 編碼獨立段（gej_file 單頭、gek_file 單身；encoding_segment.feature）。共用 schema；單頭與單身一起存取
#[async_trait]
pub trait EncodingSegmentRepository: Send + Sync {
    /// 全部獨立段（含單身），依編號排序
    async fn list_encoding_segments(
        &self,
    ) -> Result<Vec<ttrs_domain::encoding_segment::EncodingSegment>, RepoError>;
    async fn get_encoding_segment(
        &self,
        code: &ttrs_domain::encoding_segment::SegmentCode,
    ) -> Result<Option<ttrs_domain::encoding_segment::EncodingSegment>, RepoError>;
    /// 單頭與單身一起新增；編號重複回 `Conflict`
    async fn insert_encoding_segment(
        &self,
        segment: &ttrs_domain::encoding_segment::EncodingSegment,
    ) -> Result<(), RepoError>;
    /// 單頭版本不符回 `Stale`；單身只寫入有變動的列（異動記錄只記變動的列）
    async fn update_encoding_segment(
        &self,
        segment: &ttrs_domain::encoding_segment::EncodingSegment,
    ) -> Result<(), RepoError>;
    /// 是否已被編碼類別的獨立段使用（IS6、EC9）
    async fn encoding_segment_in_use(
        &self,
        code: &ttrs_domain::encoding_segment::SegmentCode,
    ) -> Result<bool, RepoError>;
    /// 連同單身刪除；已被編碼類別引用時回 `InUse`（IS6）；回傳是否有刪到
    async fn delete_encoding_segment(
        &self,
        code: &ttrs_domain::encoding_segment::SegmentCode,
    ) -> Result<bool, RepoError>;
}

/// 資料控管原則的受控營運中心（gev_file 中 gev03＝N 的列）與欄位條件（gey_file；data_control.feature）。共用 schema（DC2）
#[async_trait]
pub trait DataControlRepository: Send + Sync {
    /// 全部受控營運中心（含欄位條件）
    async fn list_controlled_plants(
        &self,
    ) -> Result<Vec<ttrs_domain::data_control::ControlledPlant>, RepoError>;
    async fn get_controlled_plant(
        &self,
        data_type: ttrs_domain::group_center::DataType,
        plant: &str,
    ) -> Result<Option<ttrs_domain::data_control::ControlledPlant>, RepoError>;
    /// 連同欄位條件新增；同一資料類別＋營運中心已是資料中心或受控營運中心時回 `Conflict`（DC2）
    async fn insert_controlled_plant(
        &self,
        plant: &ttrs_domain::data_control::ControlledPlant,
    ) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`；欄位條件只寫入有變動的列
    async fn update_controlled_plant(
        &self,
        plant: &ttrs_domain::data_control::ControlledPlant,
    ) -> Result<(), RepoError>;
    /// 連同欄位條件刪除；回傳是否有刪到
    async fn delete_controlled_plant(
        &self,
        data_type: ttrs_domain::group_center::DataType,
        plant: &str,
    ) -> Result<bool, RepoError>;
}

/// 集團中心（geu_file）與資料中心的資料類別（gev_file 中 gev03＝Y 的列；group_center.feature）。共用 schema（GC2）
#[async_trait]
pub trait GroupCenterRepository: Send + Sync {
    /// 全部集團中心（含資料類別設定）
    async fn list_group_centers(
        &self,
    ) -> Result<Vec<ttrs_domain::group_center::GroupCenter>, RepoError>;
    async fn get_group_center(
        &self,
        code: &ttrs_domain::group_center::GroupCenterCode,
    ) -> Result<Option<ttrs_domain::group_center::GroupCenter>, RepoError>;
    /// 只新增集團中心本身；代碼重複回 `Conflict`
    async fn insert_group_center(
        &self,
        center: &ttrs_domain::group_center::GroupCenter,
    ) -> Result<(), RepoError>;
    /// 只更新集團中心本身（不含資料類別）；版本不符回 `Stale`
    async fn update_group_center(
        &self,
        center: &ttrs_domain::group_center::GroupCenter,
    ) -> Result<(), RepoError>;
    /// 新增一個資料類別；同一資料類別＋營運中心已屬於任何資料中心、或已是受控營運中心時回 `Conflict`（GC5、DC2）
    async fn add_data_center_type(
        &self,
        code: &ttrs_domain::group_center::GroupCenterCode,
        row: &ttrs_domain::group_center::DataCenterType,
    ) -> Result<(), RepoError>;
    /// 刪除一個資料類別；回傳是否有刪到（GC6；拋轉設定移植後一併刪除同一資料類別＋營運中心的設定）
    async fn remove_data_center_type(
        &self,
        code: &ttrs_domain::group_center::GroupCenterCode,
        row: &ttrs_domain::group_center::DataCenterType,
    ) -> Result<bool, RepoError>;
    /// 連同資料類別設定一起刪除（GC8）；回傳是否有刪到
    async fn delete_group_center(
        &self,
        code: &ttrs_domain::group_center::GroupCenterCode,
    ) -> Result<bool, RepoError>;
}

/// 編碼年對照（gff_file；encoding_year.feature）。共用 schema；主鍵為年＋長度
#[async_trait]
pub trait EncodingYearRepository: Send + Sync {
    /// 全部對照，依年、長度排序
    async fn list_encoding_years(
        &self,
    ) -> Result<Vec<ttrs_domain::encoding_year::EncodingYear>, RepoError>;
    async fn get_encoding_year(
        &self,
        year: i32,
        length: i16,
    ) -> Result<Option<ttrs_domain::encoding_year::EncodingYear>, RepoError>;
    /// 年＋長度重複回 `Conflict`
    async fn insert_encoding_year(
        &self,
        y: &ttrs_domain::encoding_year::EncodingYear,
    ) -> Result<(), RepoError>;
    /// 版本不符回 `Stale`
    async fn update_encoding_year(
        &self,
        y: &ttrs_domain::encoding_year::EncodingYear,
    ) -> Result<(), RepoError>;
    /// 回傳是否有刪到
    async fn delete_encoding_year(&self, year: i32, length: i16) -> Result<bool, RepoError>;
}

/// 編碼類別（gei_file 單頭、gel_file 單身；encoding_class.feature）。共用 schema；單頭與單身一起存取
#[async_trait]
pub trait EncodingClassRepository: Send + Sync {
    /// 全部類別（含單身），依代碼排序
    async fn list_encoding_classes(
        &self,
    ) -> Result<Vec<ttrs_domain::encoding_class::EncodingClass>, RepoError>;
    async fn get_encoding_class(
        &self,
        code: &ttrs_domain::encoding_class::ClassCode,
    ) -> Result<Option<ttrs_domain::encoding_class::EncodingClass>, RepoError>;
    /// 單頭與單身一起新增；代碼重複回 `Conflict`
    async fn insert_encoding_class(
        &self,
        class: &ttrs_domain::encoding_class::EncodingClass,
    ) -> Result<(), RepoError>;
    /// 單頭版本不符回 `Stale`；單身只寫入有變動的段
    async fn update_encoding_class(
        &self,
        class: &ttrs_domain::encoding_class::EncodingClass,
    ) -> Result<(), RepoError>;
    /// 連同單身刪除；回傳是否有刪到
    async fn delete_encoding_class(
        &self,
        code: &ttrs_domain::encoding_class::ClassCode,
    ) -> Result<bool, RepoError>;
}

/// 我的 GitHub 書庫的一筆（epub_github_library.feature ML1、ML6；表 epub_github_library）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpubLibraryEntry {
    pub user_id: String,
    /// 擁有者/repo（GitHub 回傳的正式名稱）
    pub repo: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub version: i32,
}

/// 我的 GitHub 書庫（epub_github_library.feature）：每位使用者自己的 repo 清單
#[async_trait]
pub trait EpubLibraryRepository: Send + Sync {
    /// 該使用者的清單，依加入時間、repo 排序
    async fn list_epub_library(&self, user_id: &str) -> Result<Vec<EpubLibraryEntry>, RepoError>;
    /// 重複回 `Conflict`
    async fn insert_epub_library(&self, user_id: &str, repo: &str) -> Result<(), RepoError>;
    /// 刪除：版本不符回 `Stale`；不存在回 `Ok(false)`
    async fn delete_epub_library(
        &self,
        user_id: &str,
        repo: &str,
        version: i32,
    ) -> Result<bool, RepoError>;
}

/// 共用 GitHub 書庫的一筆（epub_github_shared_library.feature SL1、SL6；表 epub_github_shared_library）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpubSharedLibraryEntry {
    /// 擁有者/repo（GitHub 回傳的正式名稱）
    pub repo: String,
    pub created_by: String,
    pub created_dept: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub version: i32,
}

/// 共用 GitHub 書庫（epub_github_shared_library.feature）：全公司一份 repo 清單
#[async_trait]
pub trait EpubSharedLibraryRepository: Send + Sync {
    /// 依加入時間、repo 排序
    async fn list_epub_shared_library(&self) -> Result<Vec<EpubSharedLibraryEntry>, RepoError>;
    /// 重複回 `Conflict`
    async fn insert_epub_shared_library(
        &self,
        repo: &str,
        created_by: &str,
        created_dept: &str,
    ) -> Result<(), RepoError>;
    /// 刪除：版本不符回 `Stale`；不存在回 `Ok(false)`
    async fn delete_epub_shared_library(&self, repo: &str, version: i32)
    -> Result<bool, RepoError>;
}

// ---------------------------------------------------------------------------
// EPUB 英文翻譯成繁體中文（TTRS 新增，TipTop 無對應；epub_translate.feature）
// ---------------------------------------------------------------------------

/// 一批要翻譯的段落與已知的人名地名譯名（EL5）
#[derive(Debug, Clone, Default)]
pub struct TranslateRequest {
    pub lines: Vec<String>,
    /// (英文, 中文)：前面批次累積的人名、地名對照表
    pub glossary: Vec<(String, String)>,
}

/// 翻譯結果：與請求同樣段數的譯文、新發現的人名地名，以及用量（token 數，不是金額，EL13）
#[derive(Debug, Clone, Default)]
pub struct TranslateResponse {
    pub lines: Vec<String>,
    pub names: Vec<(String, String)>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// 翻譯引擎（EL2）：infra 以 Claude Code `claude -p` 實作；BDD 用記憶體假引擎。
/// 失敗回 TRANSLATE_NOT_CONFIGURED（沒裝、未登入）、TRANSLATE_QUOTA_EXHAUSTED（訂閱用量用完）、TRANSLATE_FAILED（可重試）
#[async_trait]
pub trait TranslationEngine: Send + Sync {
    /// 引擎名稱（稽核紀錄用）
    fn name(&self) -> String;
    /// 提交前檢查引擎是否可用
    async fn check(&self) -> Result<(), ttrs_domain::DomainError>;
    async fn translate(
        &self,
        req: &TranslateRequest,
    ) -> Result<TranslateResponse, ttrs_domain::DomainError>;
}

/// 翻譯工作（translation_job，只保留 24 小時）
#[derive(Debug, Clone)]
pub struct JobRecord {
    pub job_id: String,
    pub user_id: String,
    pub source_sha256: String,
    pub file_name: String,
    /// txt／pdf
    pub format: String,
    /// queued／running／done／failed
    pub status: String,
    pub total_batches: i32,
    pub done_batches: i32,
    pub total_lines: i32,
    pub done_lines: i32,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    /// 原文、切段與已翻譯的段落（JSON，供續翻）
    pub doc: String,
    pub result: Option<Vec<u8>>,
    pub result_file_name: Option<String>,
    pub result_title: Option<String>,
    pub result_author: Option<String>,
    pub result_chapters: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[async_trait]
pub trait TranslationJobRepository: Send + Sync {
    async fn create(&self, job: &JobRecord) -> Result<(), RepoError>;
    async fn get(&self, job_id: &str) -> Result<Option<JobRecord>, RepoError>;
    /// 整筆覆寫（每批翻完存一次進度）
    async fn save(&self, job: &JobRecord) -> Result<(), RepoError>;
    /// 同一位使用者、同一個檔案內容（sha256）的失敗工作，未過期時可續翻
    async fn find_resumable(
        &self,
        user_id: &str,
        source_sha256: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<JobRecord>, RepoError>;
    /// 是否有進行中（queued／running）且 `since` 之後還有更新的工作（更新太久沒動的視為已中斷）
    async fn has_active(&self, since: chrono::DateTime<chrono::Utc>) -> Result<bool, RepoError>;
    /// 刪除到期的工作，回刪除筆數
    async fn purge_expired(&self, now: chrono::DateTime<chrono::Utc>) -> Result<u64, RepoError>;
}

// ---------------------------------------------------------------------------
// EPUB 多來源搜尋（TTRS 新增，TipTop 無對應；epub_web_search.feature）
// ---------------------------------------------------------------------------

/// 搜尋結果的種類：一本可下載的書，或一個 GitHub repo（按「列出 EPUB」才看得到檔案）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpubBookKind {
    Book,
    Repo,
}

/// 來源回傳的一筆結果（尚未排序、去重）
#[derive(Debug, Clone)]
pub struct EpubBook {
    /// 來源名稱（如 "Internet Archive"）
    pub source: String,
    /// 在來源內的識別（IA identifier、Gutenberg 編號、Wikisource 頁名、GitHub repo 全名）
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    /// 來源標示的語言（zh-TW、chi、en…），不一定準
    pub language: Option<String>,
    /// 授權原文：SPDX、CC 授權網址、「公共領域」；沒有資料為 None
    pub license: Option<String>,
    pub downloads: Option<u64>,
    pub kind: EpubBookKind,
    pub url: Option<String>,
}

/// EPUB 來源（WS2）：infra 實作 Internet Archive、Project Gutenberg、Wikisource、GitHub；BDD 用記憶體假來源。
/// 搜尋失敗回 DomainError（逾時、來源錯誤），由服務隔離，不影響其他來源（WS7）
#[async_trait]
pub trait EpubSource: Send + Sync {
    /// 顯示名稱，也是結果的「來源」
    fn name(&self) -> String;
    /// `keywords` 是同一個關鍵字的繁體、簡體等寫法（任一個符合即可）；`open_only` 時來源可先過濾授權
    async fn search(
        &self,
        keywords: &[String],
        open_only: bool,
        limit: usize,
    ) -> Result<Vec<EpubBook>, ttrs_domain::DomainError>;
    /// 下載 EPUB 位元組；超過 `max_bytes` 回 EPUB_TOO_LARGE，不下載完
    async fn download(&self, id: &str, max_bytes: u64)
    -> Result<Vec<u8>, ttrs_domain::DomainError>;
    /// 給使用者的提示代碼（例如 GITHUB_NO_TOKEN，GF2）；沒有則 None
    fn notice(&self) -> Option<String> {
        None
    }
}

// ---------------------------------------------------------------------------
// 以網址下載 EPUB（TTRS 新增，TipTop 無對應；epub_url_download.feature）
// ---------------------------------------------------------------------------

/// 單次 HTTP 請求（不自動轉址）的回應
#[derive(Debug, Clone, Default)]
pub struct HttpHop {
    pub status: u16,
    /// 3xx 的 Location 標頭
    pub location: Option<String>,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

/// 對外的 HTTP 連線（UD4）：app 層負責檢查網址與位址（SSRF 防護）、處理轉址；
/// 這裡只做「解析網域」與「對已檢查過的位址發一次請求」。infra 以 reqwest 實作；BDD 用記憶體假網路
#[async_trait]
pub trait WebClient: Send + Sync {
    /// 解析網域成 IP（可能多個）
    async fn resolve(&self, host: &str) -> Result<Vec<std::net::IpAddr>, ttrs_domain::DomainError>;
    /// 對 `url` 發一次 GET，連線到已檢查過的 `ip`（避免 DNS 重新綁定），不跟隨轉址、不帶 cookie；
    /// 內容超過 `max_bytes` 回 EPUB_TOO_LARGE；連線失敗、逾時回 URL_UNREACHABLE
    async fn get(
        &self,
        url: &str,
        ip: std::net::IpAddr,
        max_bytes: u64,
    ) -> Result<HttpHop, ttrs_domain::DomainError>;
}
