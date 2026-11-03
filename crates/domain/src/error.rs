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
    #[error("程式代號只能用小寫英文、數字與連字號，且以英文字母開頭：{0}")]
    ProgramNoInvalid(String),
    #[error("程式類別不可空白")]
    ProgramTypeRequired,
    #[error("TipTop 程式代號已被其他程式使用")]
    ProgramTiptopNoDuplicate,
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

    // 使用者維護（user_maintenance.feature）
    #[error("使用者代號不可空白")]
    UserIdEmpty,
    #[error("使用者代號長度不可超過 {max} 碼")]
    UserIdTooLong { max: usize },
    #[error("使用者已存在")]
    UserAlreadyExists,
    #[error("使用者不存在")]
    UserNotFound,
    #[error("使用者代號不可修改")]
    UserIdImmutable,
    #[error("使用者選單必須是選單程式（模組 MENU）：{0}")]
    UserMenuInvalid(String),
    #[error("非多營運中心使用者只能有一個可使用的營運中心")]
    UserSinglePlant,
    #[error("公司帳號已綁定其他使用者：{0}")]
    IdpAccountInUse(String),

    // 登入（sign_in.feature）
    #[error("此公司帳號尚未在 TTRS 註冊，請洽系統管理員")]
    UserNotRegistered,
    #[error("使用者已停用")]
    UserInactive,
    #[error("使用者已超過有效日期")]
    UserExpired,
    #[error("沒有使用營運中心 {0} 的權限")]
    PlantNotAllowed(String),
    #[error("營運中心 {0} 已停用")]
    PlantInactive(String),
    #[error("電子郵件格式不正確：{0}")]
    UserEmailInvalid(String),

    // 使用者特殊權限維護（user_permission_maintenance.feature）
    #[error("此使用者已設定相同的權限類別或程式")]
    UserPermissionDuplicate,
    #[error("目標使用者已有特殊權限，不可複製")]
    UserHasSpecialPermissions,

    // 程式執行權限維護（program_permission_maintenance.feature）
    #[error("此權限類別已設定該程式的權限")]
    PermissionAlreadyExists,
    #[error("此權限類別沒有該程式的權限")]
    PermissionNotFound,
    #[error("程式沒有提供此功能，不可設定：{0}")]
    PermissionActionNotAllowed(String),
    #[error("資料權限代碼不正確：{0}")]
    PermissionLevelInvalid(String),
    #[error("必須是選單程式（模組 MENU）：{0}")]
    MenuProgramInvalid(String),

    // 營運中心維護（plant_maintenance.feature）
    #[error("營運中心代號長度不可超過 {max} 碼")]
    PlantIdTooLong { max: usize },
    #[error("營運中心已存在")]
    PlantAlreadyExists,
    #[error("營運中心代號不可修改")]
    PlantIdImmutable,
    #[error("營運中心名稱不可空白")]
    PlantNameEmpty,
    #[error("時區必須是有效的 IANA 時區名稱（例：Asia/Taipei）：{0}")]
    PlantTimeZoneInvalid(String),
    #[error("營運中心仍有使用者或業務資料，不可刪除（可改為停用）")]
    PlantInUse,

    // 權限類別維護（permission_class_maintenance.feature）
    #[error("權限類別代號不可空白")]
    PermissionClassCodeEmpty,
    #[error("權限類別代號長度不可超過 {max} 碼")]
    PermissionClassCodeTooLong { max: usize },
    #[error("權限類別已存在")]
    PermissionClassAlreadyExists,
    #[error("權限類別代號不可修改")]
    PermissionClassCodeImmutable,
    #[error("權限說明不可空白")]
    PermissionClassNameEmpty,
    #[error("預設選單必須是選單程式（模組 MENU）：{0}")]
    PermissionClassMenuInvalid(String),
    #[error("閒置控制為「依權限類別」時，閒置處理與閒置秒數必須輸入")]
    IdleSettingRequired,
    #[error("閒置秒數必須介於 1 到 {max} 秒")]
    IdleSecondsInvalid { max: i32 },
    #[error("仍有使用者屬於此權限類別，不可停用或刪除")]
    PermissionClassInUse,
    #[error("此權限類別仍有程式權限，請先刪除程式權限")]
    PermissionClassHasPermissions,
    #[error("權限類別已無效，不可指定給使用者")]
    PermissionClassInactive,

    // TipTop 對照目錄（legacy_catalog.feature）
    #[error("英文名稱不符命名規則（小寫英文開頭、僅小寫英數與底線、最多 63 字、非保留字）：{0}")]
    LegacyNameInvalid(String),
    #[error("英文名稱重複：{0}")]
    LegacyNameDuplicate(String),
    #[error("TipTop 對照目錄沒有資料表 {0}")]
    LegacyTableNotFound(String),

    // 營運中心資料庫代碼＝工廠 schema（plant_maintenance.feature P13，ADR 0007）
    #[error("資料庫代碼須為 ds 加兩位數字（例如 ds01）：{0}")]
    PlantSchemaInvalid(String),
    #[error("資料庫代碼已被其他營運中心使用")]
    PlantSchemaDuplicate,
    #[error("資料庫代碼建立後不可修改")]
    PlantSchemaImmutable,
    #[error("資料庫代碼 ds01–ds99 已用完")]
    PlantSchemaExhausted,

    // 部門維護（aooi030，department_maintenance.feature）
    #[error("部門編號不可空白")]
    DepartmentCodeEmpty,
    #[error("部門編號長度不可超過 {max} 碼")]
    DepartmentCodeTooLong { max: usize },
    #[error("部門簡稱不可空白")]
    DepartmentNameEmpty,
    #[error("部門已存在")]
    DepartmentAlreadyExists,
    #[error("部門不存在")]
    DepartmentNotFound,
    #[error("部門已無效")]
    DepartmentInactive,
    #[error("部門編號建立後不可修改")]
    DepartmentCodeImmutable,
    #[error("費用類別只能是 M 管理、P 製造、R 研發、S 銷售：{0}")]
    DepartmentExpenseTypeInvalid(String),
    #[error("管理類別只能是 1 成本中心、2 利潤中心、3 其他：{0}")]
    DepartmentControlTypeInvalid(String),
    #[error("對應成本中心須為有效的部門：{0}")]
    DepartmentCostCenterInvalid(String),
    #[error("部門仍有員工，不可刪除")]
    DepartmentInUse,

    // 員工維護（aooi040，employee_maintenance.feature）
    #[error("員工編號不可空白")]
    EmployeeIdEmpty,
    #[error("員工編號長度不可超過 {max} 碼")]
    EmployeeIdTooLong { max: usize },
    #[error("員工姓名不可空白")]
    EmployeeNameEmpty,
    #[error("員工已存在")]
    EmployeeAlreadyExists,
    #[error("員工不存在")]
    EmployeeNotFound,
    #[error("員工編號建立後不可修改")]
    EmployeeIdImmutable,
    #[error("E-mail 格式不正確：{0}")]
    EmployeeEmailInvalid(String),

    // 權限檢查（authorization.feature）
    #[error("{}", .reason.message(.program, .action))]
    PermissionDenied {
        program: String,
        action: String,
        reason: crate::authorization::DenyReason,
    },
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
            Self::ProgramNoInvalid(_) => "PROGRAM_NO_INVALID",
            Self::ProgramTypeRequired => "PROGRAM_TYPE_REQUIRED",
            Self::ProgramTiptopNoDuplicate => "PROGRAM_TIPTOP_NO_DUPLICATE",
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
            Self::UserIdEmpty => "USER_ID_EMPTY",
            Self::UserIdTooLong { .. } => "USER_ID_TOO_LONG",
            Self::UserAlreadyExists => "USER_ALREADY_EXISTS",
            Self::UserNotFound => "USER_NOT_FOUND",
            Self::UserIdImmutable => "USER_ID_IMMUTABLE",
            Self::UserMenuInvalid(_) => "USER_MENU_INVALID",
            Self::UserSinglePlant => "USER_SINGLE_PLANT",
            Self::IdpAccountInUse(_) => "IDP_ACCOUNT_IN_USE",
            Self::UserNotRegistered => "USER_NOT_REGISTERED",
            Self::UserInactive => "USER_INACTIVE",
            Self::UserExpired => "USER_EXPIRED",
            Self::PlantNotAllowed(_) => "PLANT_NOT_ALLOWED",
            Self::PlantInactive(_) => "PLANT_INACTIVE",
            Self::UserEmailInvalid(_) => "USER_EMAIL_INVALID",
            Self::UserPermissionDuplicate => "USER_PERMISSION_DUPLICATE",
            Self::UserHasSpecialPermissions => "USER_HAS_SPECIAL_PERMISSIONS",
            Self::PermissionAlreadyExists => "PERMISSION_ALREADY_EXISTS",
            Self::PermissionNotFound => "PERMISSION_NOT_FOUND",
            Self::PermissionActionNotAllowed(_) => "PERMISSION_ACTION_NOT_ALLOWED",
            Self::PermissionLevelInvalid(_) => "PERMISSION_LEVEL_INVALID",
            Self::MenuProgramInvalid(_) => "MENU_PROGRAM_INVALID",
            Self::PlantIdTooLong { .. } => "PLANT_ID_TOO_LONG",
            Self::PlantAlreadyExists => "PLANT_ALREADY_EXISTS",
            Self::PlantIdImmutable => "PLANT_ID_IMMUTABLE",
            Self::PlantNameEmpty => "PLANT_NAME_EMPTY",
            Self::PlantTimeZoneInvalid(_) => "PLANT_TIME_ZONE_INVALID",
            Self::PlantInUse => "PLANT_IN_USE",
            Self::PermissionClassCodeEmpty => "PERMISSION_CLASS_CODE_EMPTY",
            Self::PermissionClassCodeTooLong { .. } => "PERMISSION_CLASS_CODE_TOO_LONG",
            Self::PermissionClassAlreadyExists => "PERMISSION_CLASS_ALREADY_EXISTS",
            Self::PermissionClassCodeImmutable => "PERMISSION_CLASS_CODE_IMMUTABLE",
            Self::PermissionClassNameEmpty => "PERMISSION_CLASS_NAME_EMPTY",
            Self::PermissionClassMenuInvalid(_) => "PERMISSION_CLASS_MENU_INVALID",
            Self::IdleSettingRequired => "IDLE_SETTING_REQUIRED",
            Self::IdleSecondsInvalid { .. } => "IDLE_SECONDS_INVALID",
            Self::PermissionClassInUse => "PERMISSION_CLASS_IN_USE",
            Self::PermissionClassHasPermissions => "PERMISSION_CLASS_HAS_PERMISSIONS",
            Self::PermissionClassInactive => "PERMISSION_CLASS_INACTIVE",
            Self::LegacyNameInvalid(_) => "LEGACY_NAME_INVALID",
            Self::LegacyNameDuplicate(_) => "LEGACY_NAME_DUPLICATE",
            Self::LegacyTableNotFound(_) => "LEGACY_TABLE_NOT_FOUND",
            Self::PlantSchemaInvalid(_) => "PLANT_SCHEMA_INVALID",
            Self::PlantSchemaDuplicate => "PLANT_SCHEMA_DUPLICATE",
            Self::PlantSchemaImmutable => "PLANT_SCHEMA_IMMUTABLE",
            Self::PlantSchemaExhausted => "PLANT_SCHEMA_EXHAUSTED",
            Self::DepartmentCodeEmpty => "DEPARTMENT_CODE_EMPTY",
            Self::DepartmentCodeTooLong { .. } => "DEPARTMENT_CODE_TOO_LONG",
            Self::DepartmentNameEmpty => "DEPARTMENT_NAME_EMPTY",
            Self::DepartmentAlreadyExists => "DEPARTMENT_ALREADY_EXISTS",
            Self::DepartmentNotFound => "DEPARTMENT_NOT_FOUND",
            Self::DepartmentInactive => "DEPARTMENT_INACTIVE",
            Self::DepartmentCodeImmutable => "DEPARTMENT_CODE_IMMUTABLE",
            Self::DepartmentExpenseTypeInvalid(_) => "DEPARTMENT_EXPENSE_TYPE_INVALID",
            Self::DepartmentControlTypeInvalid(_) => "DEPARTMENT_CONTROL_TYPE_INVALID",
            Self::DepartmentCostCenterInvalid(_) => "DEPARTMENT_COST_CENTER_INVALID",
            Self::DepartmentInUse => "DEPARTMENT_IN_USE",
            Self::EmployeeIdEmpty => "EMPLOYEE_ID_EMPTY",
            Self::EmployeeIdTooLong { .. } => "EMPLOYEE_ID_TOO_LONG",
            Self::EmployeeNameEmpty => "EMPLOYEE_NAME_EMPTY",
            Self::EmployeeAlreadyExists => "EMPLOYEE_ALREADY_EXISTS",
            Self::EmployeeNotFound => "EMPLOYEE_NOT_FOUND",
            Self::EmployeeIdImmutable => "EMPLOYEE_ID_IMMUTABLE",
            Self::EmployeeEmailInvalid(_) => "EMPLOYEE_EMAIL_INVALID",
            Self::PermissionDenied { .. } => "PERMISSION_DENIED",
        }
    }
}
