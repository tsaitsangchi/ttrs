use thiserror::Error;

/// 業務規則違反。`code()` 是穩定代碼，BDD 與前端都依賴它，不可任意更名。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error("料件編號不可空白")]
    ItemNoEmpty,
    #[error("料件編號長度不可超過 {max} 碼")]
    ItemNoTooLong { max: usize },
    #[error("料件編號含有不允許的字元：{0}")]
    ItemNoInvalidChar(char),
    #[error("品名不可空白")]
    ItemNameEmpty,
    #[error("營運中心代碼不可空白")]
    PlantIdEmpty,
    #[error("料件已存在")]
    ItemAlreadyExists,
    #[error("料件不存在")]
    ItemNotFound,
    #[error("營運中心不存在或未啟用")]
    PlantNotFound,
    #[error("單位不存在")]
    UnitNotFound,
    #[error("料件已有庫存異動資料，不可刪除")]
    ItemHasTransactions,
    #[error("料件狀態為「{from}」，不可執行「{action}」")]
    InvalidItemTransition {
        from: &'static str,
        action: &'static str,
    },

    // Program Maintenance 程式維護
    #[error("程式代號不可空白")]
    ProgramNoEmpty,
    #[error("程式代號長度不可超過 {max} 碼")]
    ProgramNoTooLong { max: usize },
    #[error("「{field}」長度不可超過 {max} 個字")]
    FieldTooLong { field: &'static str, max: usize },
    #[error("程式已存在")]
    ProgramAlreadyExists,
    #[error("程式不存在")]
    ProgramNotFound,
    #[error("所屬模組不可空白")]
    ModuleEmpty,
    #[error("模組不存在")]
    ModuleNotFound,
    #[error("共用程式模組 {0} 不可作為作業程式的所屬模組")]
    ModuleNotAllowed(String),
    #[error("報表程式必須指定報表類型")]
    ReportTypeRequired,
    #[error("選單不是可執行程式，不可設定權限")]
    MenuHasNoPermission,
    #[error("權限類別不存在")]
    PermissionClassNotFound,
    #[error("經由 p_query 執行的程式必須先建立查詢定義")]
    QueryDefinitionNotFound,

    // 資料字典（TipTop p_zta）
    #[error("資料表代碼不可空白")]
    DictTableNameEmpty,
    #[error("資料庫中沒有資料表 {0}（資料表需先由 migration 建立）")]
    DictTableNotInSchema(String),
    #[error("此資料表已有資料字典")]
    DictAlreadyExists,
    #[error("此資料表尚無資料字典")]
    DictNotFound,
    #[error("資料表沒有欄位 {0}")]
    DictColumnNotFound(String),
    #[error("不支援的語系：{0}（支援 zh-TW、en、zh-CN）")]
    LanguageNotSupported(String),

    // QBE 查詢條件
    #[error("查詢條件格式錯誤：{0}")]
    QbeInvalidSyntax(String),
    #[error("查詢條件不是數字：{0}")]
    QbeInvalidNumber(String),
    #[error("查詢條件不是正確的日期（YYYY/MM/DD）：{0}")]
    QbeInvalidDate(String),
    #[error("欄位「{0}」不可做為查詢條件")]
    QbeFieldNotAllowed(String),

    // TipTop 對照目錄（legacy_catalog.feature）
    #[error("英文名稱不符命名規則（小寫英文開頭、僅小寫英數與底線、最多 63 字、非保留字）：{0}")]
    LegacyNameInvalid(String),
    #[error("英文名稱重複：{0}")]
    LegacyNameDuplicate(String),
    #[error("TipTop 對照目錄沒有資料表 {0}")]
    LegacyTableNotFound(String),
}

impl DomainError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ItemNoEmpty => "ITEM_NO_EMPTY",
            Self::ItemNoTooLong { .. } => "ITEM_NO_TOO_LONG",
            Self::ItemNoInvalidChar(_) => "ITEM_NO_INVALID_CHAR",
            Self::ItemNameEmpty => "ITEM_NAME_EMPTY",
            Self::PlantIdEmpty => "PLANT_ID_EMPTY",
            Self::ItemAlreadyExists => "ITEM_ALREADY_EXISTS",
            Self::ItemNotFound => "ITEM_NOT_FOUND",
            Self::PlantNotFound => "PLANT_NOT_FOUND",
            Self::UnitNotFound => "UNIT_NOT_FOUND",
            Self::ItemHasTransactions => "ITEM_HAS_TRANSACTIONS",
            Self::InvalidItemTransition { .. } => "ITEM_INVALID_TRANSITION",
            Self::ProgramNoEmpty => "PROGRAM_NO_EMPTY",
            Self::ProgramNoTooLong { .. } => "PROGRAM_NO_TOO_LONG",
            Self::FieldTooLong { .. } => "FIELD_TOO_LONG",
            Self::ProgramAlreadyExists => "PROGRAM_ALREADY_EXISTS",
            Self::ProgramNotFound => "PROGRAM_NOT_FOUND",
            Self::ModuleEmpty => "MODULE_EMPTY",
            Self::ModuleNotFound => "MODULE_NOT_FOUND",
            Self::ModuleNotAllowed(_) => "MODULE_NOT_ALLOWED",
            Self::ReportTypeRequired => "REPORT_TYPE_REQUIRED",
            Self::MenuHasNoPermission => "MENU_HAS_NO_PERMISSION",
            Self::PermissionClassNotFound => "PERMISSION_CLASS_NOT_FOUND",
            Self::QueryDefinitionNotFound => "QUERY_DEFINITION_NOT_FOUND",
            Self::DictTableNameEmpty => "DICT_TABLE_NAME_EMPTY",
            Self::DictTableNotInSchema(_) => "DICT_TABLE_NOT_IN_SCHEMA",
            Self::DictAlreadyExists => "DICT_ALREADY_EXISTS",
            Self::DictNotFound => "DICT_NOT_FOUND",
            Self::DictColumnNotFound(_) => "DICT_COLUMN_NOT_FOUND",
            Self::LanguageNotSupported(_) => "LANGUAGE_NOT_SUPPORTED",
            Self::QbeInvalidSyntax(_) => "QBE_INVALID_SYNTAX",
            Self::QbeInvalidNumber(_) => "QBE_INVALID_NUMBER",
            Self::QbeInvalidDate(_) => "QBE_INVALID_DATE",
            Self::QbeFieldNotAllowed(_) => "QBE_FIELD_NOT_ALLOWED",
            Self::LegacyNameInvalid(_) => "LEGACY_NAME_INVALID",
            Self::LegacyNameDuplicate(_) => "LEGACY_NAME_DUPLICATE",
            Self::LegacyTableNotFound(_) => "LEGACY_TABLE_NOT_FOUND",
        }
    }
}
