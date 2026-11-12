use thiserror::Error;

/// 基本代碼的錯誤代碼：種類前綴＋後綴（如 REGION_NOT_FOUND），皆為字面值
macro_rules! kind_code {
    ($kind:expr, $suffix:literal) => {
        match $kind {
            crate::basic_code::BasicCodeKind::Region => concat!("REGION", $suffix),
            crate::basic_code::BasicCodeKind::Country => concat!("COUNTRY", $suffix),
            crate::basic_code::BasicCodeKind::Area => concat!("AREA", $suffix),
            crate::basic_code::BasicCodeKind::ShippingMethod => concat!("SHIPPING_METHOD", $suffix),
            crate::basic_code::BasicCodeKind::ReportTitle => concat!("REPORT_TITLE", $suffix),
        }
    };
}

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
    #[error("料件已有庫存異動資料，不可刪除或作廢")]
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
    #[error("可使用營運中心須包含預設營運中心")]
    UserDefaultPlantRequired,
    #[error("公司帳號已綁定其他使用者：{0}")]
    IdpAccountInUse(String),

    // 登入（sign_in.feature）
    #[error("尚未登入或連線已失效，請重新登入")]
    NotSignedIn,
    #[error("使用者代號或密碼錯誤")]
    SignInFailed,
    #[error("密碼試誤次數已達上限，帳號已鎖定，請洽系統管理員解除")]
    UserLocked,
    #[error("請先變更密碼再登入")]
    PasswordChangeRequired,
    #[error("舊密碼不正確")]
    OldPasswordIncorrect,
    #[error("新密碼與確認密碼不一致")]
    PasswordConfirmMismatch,
    #[error("密碼至少要 {min} 個字")]
    PasswordTooShort { min: usize },
    #[error("密碼最多 {max} 個字")]
    PasswordTooLong { max: usize },
    #[error("密碼不可包含使用者代號")]
    PasswordContainsUserId,
    #[error("密碼必須包含數字")]
    PasswordNeedsDigit,
    #[error("密碼必須包含符號")]
    PasswordNeedsSymbol,
    #[error("新密碼不可與舊密碼相同")]
    PasswordSameAsOld,

    // 忘記密碼與緊急重設（password_reset.feature、emergency_password_reset.feature，ADR 0010）
    #[error("重設連結無效或已過期，請重新申請")]
    PasswordResetTokenInvalid,
    #[error("尚未設定寄信伺服器，無法自助重設密碼，請洽系統管理員")]
    PasswordResetUnavailable,
    #[error("至少要保留一位有效、可修改使用者資料的管理員")]
    LastUserAdmin,
    #[error("沒有可使用的營運中心，請洽系統管理員")]
    NoUsablePlant,

    // 分類碼（classification_code.feature，aooi300）
    #[error("不支援的碼類別：{0}")]
    CodeKindInvalid(String),
    #[error("代號不可空白")]
    CodeEmpty,
    #[error("代號最多 {max} 個字")]
    CodeTooLong { max: usize },
    #[error("此類別已有相同代號")]
    CodeAlreadyExists,
    #[error("分類碼不存在")]
    CodeNotFound,
    #[error("分類碼已被使用，不可刪除；請改為停用")]
    CodeInUse,

    // 報表（ADR 0011）
    #[error("資料超過 {limit} 筆，請縮小條件後再列印")]
    ReportTooLarge { limit: u64 },

    // 異動記錄（ADR 0012 §4）
    #[error("資料已被他人修改，請重新查詢")]
    RecordChanged,
    #[error("修改、刪除、啟用或停用時須帶版本號")]
    VersionRequired,

    // 確認功能鈕（ADR 0013 SA9）
    #[error("資料已確認")]
    RecordAlreadyConfirmed,
    #[error("資料尚未確認，不能取消確認")]
    RecordNotConfirmed,
    #[error("已確認的資料不可刪除，請先取消確認")]
    RecordConfirmedNotDeletable,

    // 選單（p_zm / zm_file；menu_maintenance.feature）
    #[error("目錄須為已登錄、模組 MENU 的程式")]
    MenuNotFound,
    #[error("目錄中沒有此程式")]
    MenuItemNotFound,
    #[error("序號須大於 0")]
    MenuSeqInvalid,
    #[error("此目錄已有相同序號")]
    MenuSeqDuplicate,
    #[error("此目錄已有這支程式")]
    MenuItemDuplicate,
    #[error("不可把目錄本身或上層目錄加入單身")]
    MenuCycle,
    #[error("目錄仍被使用者、權限類別或其他目錄使用，不可刪除")]
    MenuInUse,
    #[error("新目錄已有單身")]
    MenuAlreadyHasItems,
    #[error("此目錄還有單身，請先移除它的單身")]
    MenuNotEmpty,

    // 基本代碼（區域別、國別、地區、交運方式、報表抬頭；basic_code.rs）
    #[error("{0}代號不可空白")]
    BasicCodeEmpty(crate::basic_code::BasicCodeKind),
    #[error("{kind}代號長度不可超過 {max} 碼")]
    BasicCodeTooLong {
        kind: crate::basic_code::BasicCodeKind,
        max: usize,
    },
    #[error("{0}已存在")]
    BasicCodeAlreadyExists(crate::basic_code::BasicCodeKind),
    #[error("{0}不存在")]
    BasicCodeNotFound(crate::basic_code::BasicCodeKind),
    #[error("{0}已停用")]
    BasicCodeInactive(crate::basic_code::BasicCodeKind),
    #[error("{0}已被使用，不可刪除；請改為停用")]
    BasicCodeInUse(crate::basic_code::BasicCodeKind),

    // 專案（aooi200 / gja_file；project_maintenance.feature）
    #[error("專案編號不可空白")]
    ProjectCodeEmpty,
    #[error("專案編號長度不可超過 {max} 碼")]
    ProjectCodeTooLong { max: usize },
    #[error("專案結束日期不可早於起始日期")]
    ProjectDateRangeInvalid,
    #[error("專案已存在")]
    ProjectAlreadyExists,
    #[error("專案不存在")]
    ProjectNotFound,
    #[error("專案已被使用，不可刪除；請改為停用")]
    ProjectInUse,

    // 稅別（aooi150 / gec_file；tax_type_maintenance.feature）
    #[error("稅別碼不可空白")]
    TaxCodeEmpty,
    #[error("稅別碼長度不可超過 {max} 碼")]
    TaxCodeTooLong { max: usize },
    #[error("進／銷項只能為 1（進項）或 2（銷項）：{0}")]
    TaxDirectionInvalid(String),
    #[error("發票聯數只能為 0、2、3、4、X：{0}")]
    TaxInvoiceCopiesInvalid(String),
    #[error("課稅別只能為 1（應稅）、2（零稅率）、3（免稅）：{0}")]
    TaxClassInvalid(String),
    #[error("稅率須在 0 到 100 之間，最多 4 位小數")]
    TaxRateInvalid,
    #[error("稅率不為 0 時會計科目必填")]
    TaxAccountRequired,
    #[error("媒體申報格式不符合進銷項（進項 21–29、銷項 31–38，或 XX 不申報）：{0}")]
    TaxFilingFormatInvalid(String),
    #[error("二聯式發票的單價須含稅（申報格式 28 除外）")]
    TaxTwoCopyMustIncludeTax,
    #[error("稅別已存在")]
    TaxTypeAlreadyExists,
    #[error("稅別不存在")]
    TaxTypeNotFound,
    #[error("稅別已被使用，不可刪除；請改為停用")]
    TaxTypeInUse,

    // 幣別（aooi050 / azi_file；currency_maintenance.feature）
    #[error("幣別不可空白")]
    CurrencyCodeEmpty,
    #[error("幣別長度不可超過 {max} 碼")]
    CurrencyCodeTooLong { max: usize },
    #[error("{field}須在 0 到 {max} 之間")]
    CurrencyDecimalsInvalid { field: &'static str, max: i16 },
    #[error("計算方式只能為 1（乘）或 2（除）：{0}")]
    CurrencyRateMethodInvalid(String),
    #[error("幣別已存在")]
    CurrencyAlreadyExists,
    #[error("幣別不存在")]
    CurrencyNotFound,
    #[error("幣別已被使用，不可刪除；請改為停用")]
    CurrencyInUse,
    #[error("幣別已停用")]
    CurrencyInactive,

    // 匯率（aooi060 / azj_file、aooi070 / azk_file；exchange_rate_*.feature）
    #[error("該幣別此期間的匯率已存在")]
    ExchangeRateAlreadyExists,
    #[error("找不到匯率")]
    ExchangeRateNotFound,
    #[error("年月格式錯誤，須為 YYYYMM（月份 01–12）")]
    ExchangeRatePeriodInvalid,
    #[error("匯率不可小於 0：{0}")]
    ExchangeRateNegative(&'static str),
    #[error("匯率已停用，請先啟用再修改")]
    ExchangeRateInactive,
    #[error("補到日不可晚於今天")]
    ExchangeRateCopyDateInvalid,
    #[error("今天已有每日匯率，請確認是否覆寫")]
    ExchangeRateImportOverwriteRequired,
    #[error("{0}的資料格式不正確（可能是網站驗證頁），請改用上傳檔案匯入")]
    ExchangeRateSourceInvalid(String),
    #[error("無法取得{0}：{1}")]
    ExchangeRateSourceUnavailable(String, String),
    #[error("台銀牌告種類只能是 cash（現金）或 spot（即期）：{0}")]
    ExchangeRateKindInvalid(String),
    // ----- 整體管理系統參數（aoos010；general_parameters.feature） -----
    #[error("會計期間只能是 12 或 13 期：{0}")]
    AccountingPeriodsInvalid(i16),
    #[error("匯率採用只能是每月（monthly）或每日（daily）：{0}")]
    RateBasisInvalid(String),
    #[error("已有每月或每日匯率，不可變更本國幣別")]
    BaseCurrencyInUse,
    // ----- 會計期間（aoos020；accounting_periods.feature） -----
    #[error("會計年度已存在")]
    AccountingYearAlreadyExists,
    #[error("找不到會計年度")]
    AccountingYearNotFound,
    #[error("期數為 {0} 期：各期須從第 1 期起連續且每期都要有（12 期制不可有第 13 期）")]
    AccountingPeriodCountInvalid(i16),
    #[error("第 {0} 期的截止日不可早於起始日")]
    AccountingPeriodDateInvalid(i16),
    #[error("第 {0} 期的季別須為 1–4 且不可小於前一期")]
    AccountingQuarterInvalid(i16),
    #[error("期間日期須在該年度或下一年度內")]
    AccountingPeriodYearInvalid,
    #[error("第 {0} 期的起始日須為前一期截止日的隔天")]
    AccountingPeriodsNotContinuous(i16),
    #[error("與其他會計年度的期間重疊")]
    AccountingYearOverlap,
    #[error("已有會計年度資料，不可變更會計期間的期數")]
    AccountingPeriodsInUse,
    #[error("日期不在任何會計期間，或該年度沒有此期")]
    AccountingPeriodNotFound,
    #[error("查詢的日期區間不正確（迄日不可早於起日、最多 10 年）")]
    AccountingPeriodDateRangeInvalid,
    // ----- 單位換算（aooi102、aooi103；unit_conversion.feature） -----
    #[error("這組單位已有換算（不分方向）")]
    UnitConversionAlreadyExists,
    #[error("找不到單位換算")]
    UnitConversionNotFound,
    #[error("換算數量須大於 0，最多 3 位小數")]
    UnitConversionQuantityInvalid,
    #[error("來源與目的單位不可相同")]
    UnitConversionSameUnit,
    #[error("單位已停用")]
    UnitInactive,
    // ----- EPUB 轉 TXT（epub_to_text.feature） -----
    #[error("不是有效的 EPUB 檔案：{0}")]
    EpubInvalid(String),
    #[error("這本 EPUB 有加密（DRM），無法轉換")]
    EpubEncrypted,
    #[error("檔案超過 {max_mb} MB 上限")]
    EpubTooLarge { max_mb: usize },
    // ----- GitHub EPUB 搜尋（github_epub_search.feature） -----
    #[error("GitHub 查詢次數已用完，請於 {reset} 後再試")]
    GithubRateLimited { reset: String },
    #[error("無法連線 GitHub：{0}")]
    GithubUnavailable(String),
    #[error("GitHub 上找不到：{0}")]
    GithubNotFound(String),

    // 單位（aooi101 / gfe_file；unit_maintenance.feature）
    #[error("單位不可空白")]
    UnitCodeEmpty,
    #[error("單位長度不可超過 {max} 碼")]
    UnitCodeTooLong { max: usize },
    #[error("小數位數須在 0 到 {max} 之間")]
    UnitDecimalsInvalid { max: i16 },
    #[error("單位已存在")]
    UnitAlreadyExists,
    #[error("單位已被使用，不可刪除；請改為停用")]
    UnitInUse,

    // 產品分類（axmi110 / oba_file；product_classification.feature）
    #[error("產品分類碼須為 1 到 {max} 碼")]
    ProductClassCodeInvalid { max: usize },
    #[error("產品分類已存在")]
    ProductClassAlreadyExists,
    #[error("產品分類不存在")]
    ProductClassNotFound,
    #[error("成本中心須為有效、管理類別為成本中心或利潤中心的部門")]
    ProductClassCostCenterInvalid,
    #[error("期數不可小於 0")]
    ProductClassPeriodsInvalid,
    #[error("產品分類已被使用，不可刪除；請改為停用")]
    ProductClassInUse,

    // 料件分群（aimi110 / imz_file；item_group.feature）
    #[error("料件分群碼須為 1 到 {max} 碼")]
    ItemGroupCodeInvalid { max: usize },
    #[error("料件分群已存在")]
    ItemGroupAlreadyExists,
    #[error("料件分群不存在")]
    ItemGroupNotFound,
    #[error("來源碼不正確：{0}")]
    ItemGroupSourceInvalid(String),
    #[error("ABC 碼須為 A、B 或 C：{0}")]
    ItemGroupAbcInvalid(String),
    #[error("補貨策略碼須為 0 到 5：{0}")]
    ItemGroupReplenishInvalid(String),
    #[error("數量與天數不可小於 0")]
    ItemGroupQuantityInvalid,
    #[error("倉管員須為本營運中心有效的員工")]
    ItemGroupStockKeeperInvalid,
    #[error("料件分群已被料件使用，不可刪除；請改為停用")]
    ItemGroupInUse,

    // 料件基本資料（aimi100 / ima_file；item_maintenance.feature）
    #[error("MISC 開頭的料件須先建立料件 MISC")]
    ItemMiscRequired,
    #[error("料件分群已停用，不可選用")]
    ItemGroupInactive,
    #[error("庫存單位不可空白")]
    ItemStockUnitRequired,
    #[error("產品分類不可空白")]
    ItemProductClassRequired,
    #[error("來源碼不正確：{0}")]
    ItemSourceInvalid(String),
    #[error("ABC 碼須為 A、B 或 C：{0}")]
    ItemAbcInvalid(String),
    #[error("補貨策略碼須為 0 到 5：{0}")]
    ItemReplenishInvalid(String),
    #[error("補貨策略為再訂購點或期間採購時，來源碼須為 M、S、P、V 或 Z")]
    ItemReplenishSourceMismatch,
    #[error("供應商管制須為 0 到 3：{0}")]
    ItemSupplierControlInvalid(String),
    #[error("產品週期狀態須為 1 到 6：{0}")]
    ItemLifeCycleInvalid(String),
    #[error("經濟訂購量、平均訂購量須大於 0")]
    ItemOrderQtyInvalid,
    #[error("數量與天數不可小於 0")]
    ItemQuantityInvalid,
    #[error("倉管員須為本營運中心有效的員工")]
    ItemStockKeeperInvalid,
    #[error("申請部門須為本營運中心有效的部門")]
    ItemApplyDeptInvalid,
    #[error("料件已無效，請先恢復")]
    ItemInactive,
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
    #[error("至少要保留一個有效的營運中心")]
    PlantLastActive,

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
            Self::UserDefaultPlantRequired => "USER_DEFAULT_PLANT_REQUIRED",
            Self::IdpAccountInUse(_) => "IDP_ACCOUNT_IN_USE",
            Self::NotSignedIn => "NOT_SIGNED_IN",
            Self::SignInFailed => "SIGN_IN_FAILED",
            Self::UserLocked => "USER_LOCKED",
            Self::PasswordChangeRequired => "PASSWORD_CHANGE_REQUIRED",
            Self::OldPasswordIncorrect => "OLD_PASSWORD_INCORRECT",
            Self::PasswordConfirmMismatch => "PASSWORD_CONFIRM_MISMATCH",
            Self::PasswordTooShort { .. } => "PASSWORD_TOO_SHORT",
            Self::PasswordTooLong { .. } => "PASSWORD_TOO_LONG",
            Self::PasswordContainsUserId => "PASSWORD_CONTAINS_USER_ID",
            Self::PasswordNeedsDigit => "PASSWORD_NEEDS_DIGIT",
            Self::PasswordNeedsSymbol => "PASSWORD_NEEDS_SYMBOL",
            Self::PasswordSameAsOld => "PASSWORD_SAME_AS_OLD",
            Self::PasswordResetTokenInvalid => "PASSWORD_RESET_TOKEN_INVALID",
            Self::PasswordResetUnavailable => "PASSWORD_RESET_UNAVAILABLE",
            Self::LastUserAdmin => "LAST_USER_ADMIN",
            Self::NoUsablePlant => "NO_USABLE_PLANT",
            Self::CodeKindInvalid(_) => "CODE_KIND_INVALID",
            Self::CodeEmpty => "CODE_EMPTY",
            Self::CodeTooLong { .. } => "CODE_TOO_LONG",
            Self::CodeAlreadyExists => "CODE_ALREADY_EXISTS",
            Self::CodeNotFound => "CODE_NOT_FOUND",
            Self::CodeInUse => "CODE_IN_USE",
            Self::ReportTooLarge { .. } => "REPORT_TOO_LARGE",
            Self::RecordChanged => "RECORD_CHANGED",
            Self::VersionRequired => "VERSION_REQUIRED",
            Self::RecordAlreadyConfirmed => "RECORD_ALREADY_CONFIRMED",
            Self::RecordNotConfirmed => "RECORD_NOT_CONFIRMED",
            Self::RecordConfirmedNotDeletable => "RECORD_CONFIRMED_NOT_DELETABLE",
            Self::MenuNotFound => "MENU_NOT_FOUND",
            Self::MenuItemNotFound => "MENU_ITEM_NOT_FOUND",
            Self::MenuSeqInvalid => "MENU_SEQ_INVALID",
            Self::MenuSeqDuplicate => "MENU_SEQ_DUPLICATE",
            Self::MenuItemDuplicate => "MENU_ITEM_DUPLICATE",
            Self::MenuCycle => "MENU_CYCLE",
            Self::MenuInUse => "MENU_IN_USE",
            Self::MenuAlreadyHasItems => "MENU_ALREADY_HAS_ITEMS",
            Self::MenuNotEmpty => "MENU_NOT_EMPTY",
            Self::BasicCodeEmpty(k) => kind_code!(*k, "_CODE_EMPTY"),
            Self::BasicCodeTooLong { kind, .. } => kind_code!(*kind, "_CODE_TOO_LONG"),
            Self::BasicCodeAlreadyExists(k) => kind_code!(*k, "_ALREADY_EXISTS"),
            Self::BasicCodeNotFound(k) => kind_code!(*k, "_NOT_FOUND"),
            Self::BasicCodeInactive(k) => kind_code!(*k, "_INACTIVE"),
            Self::BasicCodeInUse(k) => kind_code!(*k, "_IN_USE"),
            Self::ProjectCodeEmpty => "PROJECT_CODE_EMPTY",
            Self::ProjectCodeTooLong { .. } => "PROJECT_CODE_TOO_LONG",
            Self::ProjectDateRangeInvalid => "PROJECT_DATE_RANGE_INVALID",
            Self::ProjectAlreadyExists => "PROJECT_ALREADY_EXISTS",
            Self::ProjectNotFound => "PROJECT_NOT_FOUND",
            Self::ProjectInUse => "PROJECT_IN_USE",
            Self::TaxCodeEmpty => "TAX_CODE_EMPTY",
            Self::TaxCodeTooLong { .. } => "TAX_CODE_TOO_LONG",
            Self::TaxDirectionInvalid(_) => "TAX_DIRECTION_INVALID",
            Self::TaxInvoiceCopiesInvalid(_) => "TAX_INVOICE_COPIES_INVALID",
            Self::TaxClassInvalid(_) => "TAX_CLASS_INVALID",
            Self::TaxRateInvalid => "TAX_RATE_INVALID",
            Self::TaxAccountRequired => "TAX_ACCOUNT_REQUIRED",
            Self::TaxFilingFormatInvalid(_) => "TAX_FILING_FORMAT_INVALID",
            Self::TaxTwoCopyMustIncludeTax => "TAX_TWO_COPY_MUST_INCLUDE_TAX",
            Self::TaxTypeAlreadyExists => "TAX_TYPE_ALREADY_EXISTS",
            Self::TaxTypeNotFound => "TAX_TYPE_NOT_FOUND",
            Self::TaxTypeInUse => "TAX_TYPE_IN_USE",
            Self::CurrencyCodeEmpty => "CURRENCY_CODE_EMPTY",
            Self::CurrencyCodeTooLong { .. } => "CURRENCY_CODE_TOO_LONG",
            Self::CurrencyDecimalsInvalid { .. } => "CURRENCY_DECIMALS_INVALID",
            Self::CurrencyRateMethodInvalid(_) => "CURRENCY_RATE_METHOD_INVALID",
            Self::CurrencyAlreadyExists => "CURRENCY_ALREADY_EXISTS",
            Self::CurrencyNotFound => "CURRENCY_NOT_FOUND",
            Self::CurrencyInUse => "CURRENCY_IN_USE",
            Self::CurrencyInactive => "CURRENCY_INACTIVE",
            Self::ExchangeRateAlreadyExists => "EXCHANGE_RATE_ALREADY_EXISTS",
            Self::ExchangeRateNotFound => "EXCHANGE_RATE_NOT_FOUND",
            Self::ExchangeRatePeriodInvalid => "EXCHANGE_RATE_PERIOD_INVALID",
            Self::ExchangeRateNegative(_) => "EXCHANGE_RATE_NEGATIVE",
            Self::ExchangeRateInactive => "EXCHANGE_RATE_INACTIVE",
            Self::ExchangeRateCopyDateInvalid => "EXCHANGE_RATE_COPY_DATE_INVALID",
            Self::ExchangeRateImportOverwriteRequired => "EXCHANGE_RATE_IMPORT_OVERWRITE_REQUIRED",
            Self::ExchangeRateSourceInvalid(_) => "EXCHANGE_RATE_SOURCE_INVALID",
            Self::ExchangeRateSourceUnavailable(..) => "EXCHANGE_RATE_SOURCE_UNAVAILABLE",
            Self::ExchangeRateKindInvalid(_) => "EXCHANGE_RATE_KIND_INVALID",
            Self::AccountingPeriodsInvalid(_) => "ACCOUNTING_PERIODS_INVALID",
            Self::RateBasisInvalid(_) => "RATE_BASIS_INVALID",
            Self::BaseCurrencyInUse => "BASE_CURRENCY_IN_USE",
            Self::AccountingYearAlreadyExists => "ACCOUNTING_YEAR_ALREADY_EXISTS",
            Self::AccountingYearNotFound => "ACCOUNTING_YEAR_NOT_FOUND",
            Self::AccountingPeriodCountInvalid(_) => "ACCOUNTING_PERIOD_COUNT_INVALID",
            Self::AccountingPeriodDateInvalid(_) => "ACCOUNTING_PERIOD_DATE_INVALID",
            Self::AccountingQuarterInvalid(_) => "ACCOUNTING_QUARTER_INVALID",
            Self::AccountingPeriodYearInvalid => "ACCOUNTING_PERIOD_YEAR_INVALID",
            Self::AccountingPeriodsNotContinuous(_) => "ACCOUNTING_PERIODS_NOT_CONTINUOUS",
            Self::AccountingYearOverlap => "ACCOUNTING_YEAR_OVERLAP",
            Self::AccountingPeriodsInUse => "ACCOUNTING_PERIODS_IN_USE",
            Self::AccountingPeriodNotFound => "ACCOUNTING_PERIOD_NOT_FOUND",
            Self::AccountingPeriodDateRangeInvalid => "ACCOUNTING_PERIOD_DATE_RANGE_INVALID",
            Self::UnitConversionAlreadyExists => "UNIT_CONVERSION_ALREADY_EXISTS",
            Self::UnitConversionNotFound => "UNIT_CONVERSION_NOT_FOUND",
            Self::UnitConversionQuantityInvalid => "UNIT_CONVERSION_QUANTITY_INVALID",
            Self::UnitConversionSameUnit => "UNIT_CONVERSION_SAME_UNIT",
            Self::UnitInactive => "UNIT_INACTIVE",
            Self::EpubInvalid(_) => "EPUB_INVALID",
            Self::EpubEncrypted => "EPUB_ENCRYPTED",
            Self::EpubTooLarge { .. } => "EPUB_TOO_LARGE",
            Self::GithubRateLimited { .. } => "GITHUB_RATE_LIMITED",
            Self::GithubUnavailable(_) => "GITHUB_UNAVAILABLE",
            Self::GithubNotFound(_) => "GITHUB_NOT_FOUND",
            Self::UnitCodeEmpty => "UNIT_CODE_EMPTY",
            Self::UnitCodeTooLong { .. } => "UNIT_CODE_TOO_LONG",
            Self::UnitDecimalsInvalid { .. } => "UNIT_DECIMALS_INVALID",
            Self::UnitAlreadyExists => "UNIT_ALREADY_EXISTS",
            Self::UnitInUse => "UNIT_IN_USE",
            Self::ProductClassCodeInvalid { .. } => "PRODUCT_CLASS_CODE_INVALID",
            Self::ProductClassAlreadyExists => "PRODUCT_CLASS_ALREADY_EXISTS",
            Self::ProductClassNotFound => "PRODUCT_CLASS_NOT_FOUND",
            Self::ProductClassCostCenterInvalid => "PRODUCT_CLASS_COST_CENTER_INVALID",
            Self::ProductClassPeriodsInvalid => "PRODUCT_CLASS_PERIODS_INVALID",
            Self::ProductClassInUse => "PRODUCT_CLASS_IN_USE",
            Self::ItemGroupCodeInvalid { .. } => "ITEM_GROUP_CODE_INVALID",
            Self::ItemGroupAlreadyExists => "ITEM_GROUP_ALREADY_EXISTS",
            Self::ItemGroupNotFound => "ITEM_GROUP_NOT_FOUND",
            Self::ItemGroupSourceInvalid(_) => "ITEM_GROUP_SOURCE_INVALID",
            Self::ItemGroupAbcInvalid(_) => "ITEM_GROUP_ABC_INVALID",
            Self::ItemGroupReplenishInvalid(_) => "ITEM_GROUP_REPLENISH_INVALID",
            Self::ItemGroupQuantityInvalid => "ITEM_GROUP_QUANTITY_INVALID",
            Self::ItemGroupStockKeeperInvalid => "ITEM_GROUP_STOCK_KEEPER_INVALID",
            Self::ItemGroupInUse => "ITEM_GROUP_IN_USE",
            Self::ItemMiscRequired => "ITEM_MISC_REQUIRED",
            Self::ItemGroupInactive => "ITEM_GROUP_INACTIVE",
            Self::ItemStockUnitRequired => "ITEM_STOCK_UNIT_REQUIRED",
            Self::ItemProductClassRequired => "ITEM_PRODUCT_CLASS_REQUIRED",
            Self::ItemSourceInvalid(_) => "ITEM_SOURCE_INVALID",
            Self::ItemAbcInvalid(_) => "ITEM_ABC_INVALID",
            Self::ItemReplenishInvalid(_) => "ITEM_REPLENISH_INVALID",
            Self::ItemReplenishSourceMismatch => "ITEM_REPLENISH_SOURCE_MISMATCH",
            Self::ItemSupplierControlInvalid(_) => "ITEM_SUPPLIER_CONTROL_INVALID",
            Self::ItemLifeCycleInvalid(_) => "ITEM_LIFE_CYCLE_INVALID",
            Self::ItemOrderQtyInvalid => "ITEM_ORDER_QTY_INVALID",
            Self::ItemQuantityInvalid => "ITEM_QUANTITY_INVALID",
            Self::ItemStockKeeperInvalid => "ITEM_STOCK_KEEPER_INVALID",
            Self::ItemApplyDeptInvalid => "ITEM_APPLY_DEPT_INVALID",
            Self::ItemInactive => "ITEM_INACTIVE",
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
            Self::PlantLastActive => "PLANT_LAST_ACTIVE",
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
