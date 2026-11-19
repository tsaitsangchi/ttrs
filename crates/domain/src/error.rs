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
    // ----- EPUB 轉 PDF／簡轉繁（epub_to_pdf.feature） -----
    #[error("輸出格式不正確：{0}（請用 txt 或 pdf）")]
    EpubFormatInvalid(String),
    #[error("伺服器找不到可用的中文字型，無法產生 PDF（請設定 TTRS_PDF_FONT）")]
    PdfFontMissing,
    #[error("產生 PDF 失敗：{0}")]
    PdfGenerationFailed(String),
    // ----- EPUB 英文翻譯成繁體中文（epub_translate.feature） -----
    #[error("這本書不是英文，不需要也不能用英文翻譯")]
    EpubNotEnglish,
    #[error("翻譯字數超過上限（{max_chars} 字元）")]
    TranslateLimitExceeded { max_chars: usize },
    #[error("目前已有進行中的翻譯工作，請等它完成再提交")]
    TranslateBusy,
    #[error("翻譯服務尚未設定好：{0}")]
    TranslateNotConfigured(String),
    #[error("訂閱用量已達上限，請於 {reset} 後再試；已翻譯的部分會保留，重新提交可續翻")]
    TranslateQuotaExhausted { reset: String },
    #[error("翻譯失敗：{0}")]
    TranslateFailed(String),
    #[error("翻譯會把書的內容送到 Claude，請先勾選同意")]
    TranslateConsentRequired,
    #[error("找不到這個翻譯工作（可能已過期）")]
    TranslateJobNotFound,
    // ----- EPUB 多來源搜尋（epub_web_search.feature） -----
    #[error("找不到來源「{0}」或這本書")]
    EpubBookNotFound(String),
    #[error("來源「{0}」暫時無法使用")]
    EpubSourceUnavailable(String),
    // ----- 以網址下載 EPUB（epub_url_download.feature） -----
    #[error("這個網址不允許連線（只允許 http／https，且不可是內部網路位址）")]
    UrlNotAllowed,
    #[error("該網站的 robots.txt 不允許讀取這個網頁")]
    UrlRobotsDisallowed,
    #[error("無法連到這個網址（連線失敗、逾時、轉址過多或對方回應錯誤）")]
    UrlUnreachable,
    // ----- GitHub EPUB 搜尋（github_epub_search.feature） -----
    #[error("GitHub 查詢次數已用完，請於 {reset} 後再試")]
    GithubRateLimited { reset: String },
    #[error("無法連線 GitHub：{0}")]
    GithubUnavailable(String),
    #[error("GitHub 上找不到：{0}")]
    GithubNotFound(String),
    // ----- 編碼性質（aooi401 / geh_file；encoding_nature.feature） -----
    #[error("編碼性質不可空白")]
    EncodingNatureCodeEmpty,
    #[error("編碼性質長度不可超過 {max} 碼")]
    EncodingNatureCodeTooLong { max: usize },
    #[error("類別只能是 1 到 6（料件編號、客戶編號、供應商編號、固定資產編號、批號、序號）")]
    EncodingNatureCategoryInvalid,
    #[error("編碼性質已存在")]
    EncodingNatureAlreadyExists,
    #[error("編碼性質不存在")]
    EncodingNatureNotFound,
    #[error("編碼性質已被編碼類別使用，不可刪除或變更類別；請改為停用")]
    EncodingNatureInUse,
    // ----- 編碼類別（aooi402 / gei_file、gel_file；encoding_class.feature） -----
    #[error("編碼類別不可空白")]
    EncodingClassCodeEmpty,
    #[error("編碼類別長度不可超過 {max} 碼")]
    EncodingClassCodeTooLong { max: usize },
    #[error("名稱不可空白")]
    EncodingClassNameEmpty,
    #[error("性質必須是存在且有效的編碼性質")]
    EncodingClassNatureInvalid,
    #[error("編碼類別已存在")]
    EncodingClassAlreadyExists,
    #[error("編碼類別不存在")]
    EncodingClassNotFound,
    #[error("編碼類別已被使用，不可刪除；請改為停用")]
    EncodingClassInUse,
    #[error("編碼類別已停用，請先啟用再修改")]
    EncodingClassInactive,
    #[error("型態只能是 1 到 9")]
    EncodingClassTypeInvalid,
    #[error(
        "這個型態不能用在此性質：年、季、月、週、日、欄位值只限批號、序號；批號、序號不能用獨立段"
    )]
    EncodingClassTypeNotAllowed,
    #[error("型態 4（欄位值）尚未開放，批號／序號管理移植時再提供")]
    EncodingClassTypeNotSupported,
    #[error("長度必須是 1 到 {max}")]
    EncodingClassSegmentLengthInvalid { max: i16 },
    #[error("季、月、週、日的長度固定為 2")]
    EncodingClassDateLengthInvalid,
    #[error("編碼年對照沒有 {length} 碼的資料，請先在編碼年對照維護建立")]
    EncodingClassYearLengthNotDefined { length: i16 },
    #[error("固定值不可空白")]
    EncodingClassFixedEmpty,
    #[error("固定值不可含 * % _ ? 與空白")]
    EncodingClassFixedInvalidChar,
    #[error("固定值必須是 {length} 碼（等於這一段的長度）")]
    EncodingClassFixedLengthMismatch { length: i16 },
    #[error("獨立段必須是存在且有效的獨立段")]
    EncodingClassSegmentInvalid,
    #[error("每個編碼類別只能有一段流水號")]
    EncodingClassSerialDuplicate,
    #[error("段次必須是正整數")]
    EncodingClassLineInvalid,
    #[error("段次重複")]
    EncodingClassLineDuplicate,
    #[error("找不到這個段次")]
    EncodingClassLineNotFound,
    // ----- 集團中心（aooi600 / geu_file、gev_file；group_center.feature） -----
    #[error("集團中心代碼不可空白")]
    GroupCenterCodeEmpty,
    #[error("集團中心代碼長度不可超過 {max} 碼")]
    GroupCenterCodeTooLong { max: usize },
    #[error(
        "類別只能是 1 到 7（資料中心、預測中心、訂單中心、採購中心、信用控管中心、帳務中心、資金中心）"
    )]
    GroupCenterCategoryInvalid,
    #[error("集團中心已存在")]
    GroupCenterAlreadyExists,
    #[error("集團中心不存在")]
    GroupCenterNotFound,
    #[error("集團中心已有拋轉紀錄，不可刪除；請改為停用")]
    GroupCenterInUse,
    #[error("這個資料中心已有資料類別設定，不可改成資料中心以外的類別")]
    GroupCenterHasDataTypes,
    #[error("只有有效的資料中心（類別 1）才能設定資料類別與拋轉設定")]
    GroupCenterNotDataCenter,
    #[error(
        "資料類別只能是 0 到 8（基本資料、料件、BOM、ECN、客戶、供應商、會計科目、價格、匯率）"
    )]
    DataCenterTypeInvalid,
    #[error("這個資料類別與營運中心已屬於某個資料中心")]
    DataCenterTypeAlreadyExists,
    #[error("找不到這個資料類別與營運中心")]
    DataCenterTypeNotFound,
    // ----- 資料控管原則（aooi601 / gev_file、gey_file；data_control.feature） -----
    #[error("這個資料類別與營運中心已在資料控管清單中（資料中心或受控營運中心）")]
    DataControlPlantAlreadyExists,
    #[error("找不到這個受控營運中心；資料中心的營運中心請到集團中心維護修改")]
    DataControlPlantNotFound,
    #[error("資料類別 0 基本資料不可維護資料控管原則")]
    DataControlTypeNotMaintainable,
    #[error("這個資料類別還沒有有效的資料中心，請先在集團中心維護設定")]
    DataControlNoDataCenter,
    #[error("資料中心的營運中心不需設定欄位條件，只有受控營運中心可以設定")]
    DataControlRuleNotAllowed,
    #[error("這個欄位已有條件")]
    DataControlRuleAlreadyExists,
    #[error("找不到這個欄位條件")]
    DataControlRuleNotFound,
    #[error("這個資料類別不能設定此欄位")]
    DataControlFieldInvalid,
    #[error("條件只能是 1 到 11")]
    DataControlConditionInvalid,
    #[error(
        "值必填、最多 255 字，多個值以 ; 分隔，不可含 ' \" ) + > < = *；大於、小於類條件只能一個值"
    )]
    DataControlValueInvalid,
    #[error("{field} 的值 {value} 不符合本營運中心的資料控管條件")]
    DataControlFieldRejected { field: String, value: String },
    // ----- 資料中心拋轉設定（aooi602 / gew_file、gez_file；carry_setting.feature） -----
    #[error("這個資料中心與資料類別已有拋轉設定")]
    CarrySettingAlreadyExists,
    #[error("找不到這筆拋轉設定")]
    CarrySettingNotFound,
    #[error("這個資料中心沒有負責此資料類別，請先在集團中心維護設定")]
    CarrySettingNoDataType,
    #[error("已有拋轉紀錄，不可刪除；不想拋轉請改拋轉方式")]
    CarrySettingInUse,
    #[error("拋轉方式只能是 1 自動拋轉、2 詢問後拋轉、3 手動拋轉")]
    CarryModeInvalid,
    #[error("這個營運中心已在拋轉清單中")]
    CarryTargetAlreadyExists,
    #[error("找不到這個拋轉營運中心")]
    CarryTargetNotFound,
    #[error("資料來源營運中心不需要拋轉給自己")]
    CarryTargetIsSource,
    #[error("通知人員必須是電子郵件，多個以 ; 分隔，最多 1500 字")]
    CarryNotifyInvalid,
    #[error("這個資料類別不能用此欄位設定拋轉條件")]
    CarryFieldInvalid,
    #[error("這個欄位已有拋轉條件")]
    CarryConditionAlreadyExists,
    #[error("找不到這個拋轉條件")]
    CarryConditionNotFound,
    #[error("這個欄位已有預設值")]
    CarryDefaultAlreadyExists,
    #[error("找不到這個預設值")]
    CarryDefaultNotFound,
    #[error("這個欄位不能設定預設值")]
    CarryDefaultFieldInvalid,
    #[error("預設值必填，最多 255 字")]
    CarryDefaultValueInvalid,
    #[error("基本資料不可設定拋轉條件、預設值，也不可修改子公司可刪除")]
    CarryBasicDataNotAllowed,
    // ----- 基本資料拋轉（aoop603 / gex_file；basic_data_carry.feature） -----
    #[error("目前營運中心不是任何有效資料中心的基本資料來源營運中心，不能執行基本資料拋轉")]
    BasicCarryNotSource,
    #[error("這個資料中心的基本資料沒有拋轉營運中心，請先在資料中心拋轉設定維護")]
    BasicCarryNoTarget,
    #[error("請至少選擇一項資料與一個拋轉營運中心")]
    BasicCarryNothingSelected,
    #[error("拋轉方式不是手動拋轉，會拋給全部拋轉營運中心，不能另外挑選")]
    BasicCarryTargetsFixed,
    #[error("不能拋轉的資料項目：{0}")]
    BasicCarryItemInvalid(String),
    #[error("{item} {key} 的 {field} {value} 在拋轉營運中心不存在")]
    BasicCarryReferenceMissing {
        item: String,
        key: String,
        field: String,
        value: String,
    },
    // ----- 料件拋轉（aimi100 carry；item_carry.feature） -----
    #[error("只有有效且已確認的料件可以拋轉")]
    ItemCarryNotConfirmed,
    #[error("目前營運中心不是任何有效資料中心的料件來源營運中心，不能拋轉")]
    ItemCarryNotSource,
    #[error("這是資料中心拋轉來的料件，本營運中心不可修改")]
    ItemCarriedNotEditable,
    #[error("這是資料中心拋轉來的料件，本營運中心不可刪除")]
    ItemCarriedNotDeletable,
    // ----- 倉庫儲位（aimi201、aimi200 / imd_file、ime_file；warehouse.feature） -----
    #[error("倉庫別不可空白")]
    WarehouseCodeEmpty,
    #[error("倉庫別長度不可超過 {max} 碼")]
    WarehouseCodeTooLong { max: usize },
    #[error("倉庫已存在")]
    WarehouseAlreadyExists,
    #[error("倉庫不存在或已停用")]
    WarehouseNotFound,
    #[error("倉庫已停用，請先啟用")]
    WarehouseInactive,
    #[error("倉庫已被料件或料件分群使用，不可刪除；請改為停用")]
    WarehouseInUse,
    #[error("倉儲類別只能是 S（一般倉）或 W（在製品倉）")]
    WarehouseClassInvalid,
    #[error("不可用倉儲必定為 MRP 不可用")]
    WarehouseMrpRequiresUsable,
    #[error("優先順序不可小於 0")]
    WarehousePriorityInvalid,
    #[error("成本中心必須是有效的成本中心或利潤中心部門")]
    WarehouseCostCenterInvalid,
    #[error("儲位不可空白")]
    WarehouseLocationCodeEmpty,
    #[error("儲位長度不可超過 {max} 碼")]
    WarehouseLocationCodeTooLong { max: usize },
    #[error("這個倉庫已有此儲位")]
    WarehouseLocationAlreadyExists,
    #[error("儲位不存在")]
    WarehouseLocationNotFound,
    #[error("儲位已被料件或料件分群使用，不可刪除")]
    WarehouseLocationInUse,
    #[error("會計科目方式為 2（使用儲位科目）時，儲位科目必填")]
    WarehouseLocationAccountRequired,
    #[error("會計科目方式只能是 0、1、2")]
    WarehouseAccountMethodInvalid,
    // ----- 料件預設倉儲（aimi202 / imf_file；item_storage.feature） -----
    #[error("這個料件已有此倉庫儲位")]
    ItemStorageAlreadyExists,
    #[error("找不到這筆料件預設倉儲")]
    ItemStorageNotFound,
    #[error("最高存量不可小於 0，最多 3 位小數")]
    ItemStorageMaxQtyInvalid,
    #[error("目的料件已有預設倉儲，不可複製")]
    ItemStorageCopyTargetNotEmpty,
    // ----- 庫存參數（asms230 / sma_file；inventory_parameters.feature） -----
    #[error("料件存放位置管制只能是 1、2、3")]
    InventoryStorageControlInvalid,
    #[error("週轉率須介於 0 到 100、最多 4 位小數，且快速移動不可小於平速移動")]
    InventoryTurnoverInvalid,
    #[error("庫存帳務基礎日期只能是 1（單據日期）或 2（產生日期）")]
    InventoryPostingDateBasisInvalid,
    // ----- 料件自動編號（aimi100 / aza28；item_auto_number.feature） -----
    #[error("整體管理系統參數未開啟料件編號自動編碼")]
    ItemAutoNumberDisabled,
    #[error("這個編碼類別不能用來取料號：須是有效、性質為料件編號的類別")]
    EncodingClassNotForItem,
    // ----- 自動編號（aooi400 / s_auno；encoding_rule.feature） -----
    #[error("請為每個獨立段挑選一個有效的編碼編號")]
    EncodingRuleValueRequired,
    #[error("流水號已用完，請加長流水號或改用其他編碼類別")]
    EncodingRuleSerialExhausted,
    #[error("編碼年對照沒有 {year} 年的資料，請先在編碼年對照維護建立")]
    EncodingRuleYearCodeMissing { year: i32 },
    #[error("今天不在任何會計期間內，無法取得季別；請先在會計期間維護建立")]
    EncodingRulePeriodMissing,
    // ----- 編碼獨立段（aooi403 / gej_file、gek_file；encoding_segment.feature） -----
    #[error("獨立段編號不可空白")]
    EncodingSegmentCodeEmpty,
    #[error("獨立段編號長度不可超過 {max} 碼")]
    EncodingSegmentCodeTooLong { max: usize },
    #[error("長度必須是 1 到 {max}")]
    EncodingSegmentLengthInvalid { max: i16 },
    #[error("獨立段已存在")]
    EncodingSegmentAlreadyExists,
    #[error("獨立段不存在")]
    EncodingSegmentNotFound,
    #[error("獨立段已被編碼類別使用，不可刪除；請改為停用")]
    EncodingSegmentInUse,
    #[error("獨立段已停用，請先啟用再修改")]
    EncodingSegmentInactive,
    #[error("已有編碼編號時不可修改長度；要改長度請先刪除單身")]
    EncodingSegmentLengthHasValues,
    #[error("編碼編號不可空白")]
    EncodingSegmentValueEmpty,
    #[error("編碼編號長度必須是 {length} 碼（等於獨立段的長度）")]
    EncodingSegmentValueLengthMismatch { length: i16 },
    #[error("編碼編號不可含 * % _ ? 與空白")]
    EncodingSegmentValueInvalidChar,
    #[error("同一獨立段內編碼編號不可重複")]
    EncodingSegmentValueDuplicate,
    #[error("項次必須是正整數")]
    EncodingSegmentLineInvalid,
    #[error("項次重複")]
    EncodingSegmentLineDuplicate,
    #[error("找不到這個項次")]
    EncodingSegmentLineNotFound,
    // ----- 編碼年對照（aooi404 / gff_file；encoding_year.feature） -----
    #[error("年必須是 1 到 {max}")]
    EncodingYearInvalid { max: i32 },
    #[error("編碼編號不可空白")]
    EncodingYearCodeEmpty,
    #[error("編碼編號長度不可超過 {max} 碼")]
    EncodingYearCodeTooLong { max: usize },
    #[error("編碼編號不可含 * % _ ? 與空白")]
    EncodingYearCodeInvalidChar,
    #[error("這一年已有相同長度的編碼編號")]
    EncodingYearAlreadyExists,
    #[error("編碼年對照不存在")]
    EncodingYearNotFound,
    #[error("修改時編碼編號必須維持 {length} 碼；要改長度請刪除後重新新增")]
    EncodingYearLengthChanged { length: i16 },
    // ----- 我的 GitHub 書庫（epub_github_library.feature） -----
    #[error("「{0}」已在你的 GitHub 書庫")]
    EpubLibraryDuplicate(String),
    #[error("GitHub 書庫最多 {max} 個 repo")]
    EpubLibraryFull { max: usize },

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
            Self::EpubFormatInvalid(_) => "EPUB_FORMAT_INVALID",
            Self::PdfFontMissing => "PDF_FONT_MISSING",
            Self::PdfGenerationFailed(_) => "PDF_GENERATION_FAILED",
            Self::EpubNotEnglish => "EPUB_NOT_ENGLISH",
            Self::TranslateLimitExceeded { .. } => "TRANSLATE_LIMIT_EXCEEDED",
            Self::TranslateBusy => "TRANSLATE_BUSY",
            Self::TranslateNotConfigured(_) => "TRANSLATE_NOT_CONFIGURED",
            Self::TranslateQuotaExhausted { .. } => "TRANSLATE_QUOTA_EXHAUSTED",
            Self::TranslateFailed(_) => "TRANSLATE_FAILED",
            Self::TranslateConsentRequired => "TRANSLATE_CONSENT_REQUIRED",
            Self::TranslateJobNotFound => "TRANSLATE_JOB_NOT_FOUND",
            Self::EpubBookNotFound(_) => "EPUB_BOOK_NOT_FOUND",
            Self::EpubSourceUnavailable(_) => "EPUB_SOURCE_UNAVAILABLE",
            Self::UrlNotAllowed => "URL_NOT_ALLOWED",
            Self::UrlRobotsDisallowed => "URL_ROBOTS_DISALLOWED",
            Self::UrlUnreachable => "URL_UNREACHABLE",
            Self::GithubRateLimited { .. } => "GITHUB_RATE_LIMITED",
            Self::GithubUnavailable(_) => "GITHUB_UNAVAILABLE",
            Self::GithubNotFound(_) => "GITHUB_NOT_FOUND",
            Self::EncodingNatureCodeEmpty => "ENCODING_NATURE_CODE_EMPTY",
            Self::EncodingNatureCodeTooLong { .. } => "ENCODING_NATURE_CODE_TOO_LONG",
            Self::EncodingNatureCategoryInvalid => "ENCODING_NATURE_CATEGORY_INVALID",
            Self::EncodingNatureAlreadyExists => "ENCODING_NATURE_ALREADY_EXISTS",
            Self::EncodingNatureNotFound => "ENCODING_NATURE_NOT_FOUND",
            Self::EncodingNatureInUse => "ENCODING_NATURE_IN_USE",
            Self::EncodingClassCodeEmpty => "ENCODING_CLASS_CODE_EMPTY",
            Self::EncodingClassCodeTooLong { .. } => "ENCODING_CLASS_CODE_TOO_LONG",
            Self::EncodingClassNameEmpty => "ENCODING_CLASS_NAME_EMPTY",
            Self::EncodingClassNatureInvalid => "ENCODING_CLASS_NATURE_INVALID",
            Self::EncodingClassAlreadyExists => "ENCODING_CLASS_ALREADY_EXISTS",
            Self::EncodingClassNotFound => "ENCODING_CLASS_NOT_FOUND",
            Self::EncodingClassInUse => "ENCODING_CLASS_IN_USE",
            Self::EncodingClassInactive => "ENCODING_CLASS_INACTIVE",
            Self::EncodingClassTypeInvalid => "ENCODING_CLASS_TYPE_INVALID",
            Self::EncodingClassTypeNotAllowed => "ENCODING_CLASS_TYPE_NOT_ALLOWED",
            Self::EncodingClassTypeNotSupported => "ENCODING_CLASS_TYPE_NOT_SUPPORTED",
            Self::EncodingClassSegmentLengthInvalid { .. } => {
                "ENCODING_CLASS_SEGMENT_LENGTH_INVALID"
            }
            Self::EncodingClassDateLengthInvalid => "ENCODING_CLASS_DATE_LENGTH_INVALID",
            Self::EncodingClassYearLengthNotDefined { .. } => {
                "ENCODING_CLASS_YEAR_LENGTH_NOT_DEFINED"
            }
            Self::EncodingClassFixedEmpty => "ENCODING_CLASS_FIXED_EMPTY",
            Self::EncodingClassFixedInvalidChar => "ENCODING_CLASS_FIXED_INVALID_CHAR",
            Self::EncodingClassFixedLengthMismatch { .. } => "ENCODING_CLASS_FIXED_LENGTH_MISMATCH",
            Self::EncodingClassSegmentInvalid => "ENCODING_CLASS_SEGMENT_INVALID",
            Self::EncodingClassSerialDuplicate => "ENCODING_CLASS_SERIAL_DUPLICATE",
            Self::EncodingClassLineInvalid => "ENCODING_CLASS_LINE_INVALID",
            Self::EncodingClassLineDuplicate => "ENCODING_CLASS_LINE_DUPLICATE",
            Self::EncodingClassLineNotFound => "ENCODING_CLASS_LINE_NOT_FOUND",
            Self::GroupCenterCodeEmpty => "GROUP_CENTER_CODE_EMPTY",
            Self::GroupCenterCodeTooLong { .. } => "GROUP_CENTER_CODE_TOO_LONG",
            Self::GroupCenterCategoryInvalid => "GROUP_CENTER_CATEGORY_INVALID",
            Self::GroupCenterAlreadyExists => "GROUP_CENTER_ALREADY_EXISTS",
            Self::GroupCenterNotFound => "GROUP_CENTER_NOT_FOUND",
            Self::GroupCenterInUse => "GROUP_CENTER_IN_USE",
            Self::GroupCenterHasDataTypes => "GROUP_CENTER_HAS_DATA_TYPES",
            Self::GroupCenterNotDataCenter => "GROUP_CENTER_NOT_DATA_CENTER",
            Self::DataCenterTypeInvalid => "DATA_CENTER_TYPE_INVALID",
            Self::DataCenterTypeAlreadyExists => "DATA_CENTER_TYPE_ALREADY_EXISTS",
            Self::DataCenterTypeNotFound => "DATA_CENTER_TYPE_NOT_FOUND",
            Self::DataControlPlantAlreadyExists => "DATA_CONTROL_PLANT_ALREADY_EXISTS",
            Self::DataControlPlantNotFound => "DATA_CONTROL_PLANT_NOT_FOUND",
            Self::DataControlTypeNotMaintainable => "DATA_CONTROL_TYPE_NOT_MAINTAINABLE",
            Self::DataControlNoDataCenter => "DATA_CONTROL_NO_DATA_CENTER",
            Self::DataControlRuleNotAllowed => "DATA_CONTROL_RULE_NOT_ALLOWED",
            Self::DataControlRuleAlreadyExists => "DATA_CONTROL_RULE_ALREADY_EXISTS",
            Self::DataControlRuleNotFound => "DATA_CONTROL_RULE_NOT_FOUND",
            Self::DataControlFieldInvalid => "DATA_CONTROL_FIELD_INVALID",
            Self::DataControlConditionInvalid => "DATA_CONTROL_CONDITION_INVALID",
            Self::DataControlValueInvalid => "DATA_CONTROL_VALUE_INVALID",
            Self::DataControlFieldRejected { .. } => "DATA_CONTROL_FIELD_REJECTED",
            Self::CarrySettingAlreadyExists => "CARRY_SETTING_ALREADY_EXISTS",
            Self::CarrySettingNotFound => "CARRY_SETTING_NOT_FOUND",
            Self::CarrySettingNoDataType => "CARRY_SETTING_NO_DATA_TYPE",
            Self::CarrySettingInUse => "CARRY_SETTING_IN_USE",
            Self::CarryModeInvalid => "CARRY_MODE_INVALID",
            Self::CarryTargetAlreadyExists => "CARRY_TARGET_ALREADY_EXISTS",
            Self::CarryTargetNotFound => "CARRY_TARGET_NOT_FOUND",
            Self::CarryTargetIsSource => "CARRY_TARGET_IS_SOURCE",
            Self::CarryNotifyInvalid => "CARRY_NOTIFY_INVALID",
            Self::CarryFieldInvalid => "CARRY_FIELD_INVALID",
            Self::CarryConditionAlreadyExists => "CARRY_CONDITION_ALREADY_EXISTS",
            Self::CarryConditionNotFound => "CARRY_CONDITION_NOT_FOUND",
            Self::CarryDefaultAlreadyExists => "CARRY_DEFAULT_ALREADY_EXISTS",
            Self::CarryDefaultNotFound => "CARRY_DEFAULT_NOT_FOUND",
            Self::CarryDefaultFieldInvalid => "CARRY_DEFAULT_FIELD_INVALID",
            Self::CarryDefaultValueInvalid => "CARRY_DEFAULT_VALUE_INVALID",
            Self::CarryBasicDataNotAllowed => "CARRY_BASIC_DATA_NOT_ALLOWED",
            Self::BasicCarryNotSource => "BASIC_CARRY_NOT_SOURCE",
            Self::BasicCarryNoTarget => "BASIC_CARRY_NO_TARGET",
            Self::BasicCarryNothingSelected => "BASIC_CARRY_NOTHING_SELECTED",
            Self::BasicCarryTargetsFixed => "BASIC_CARRY_TARGETS_FIXED",
            Self::BasicCarryItemInvalid(_) => "BASIC_CARRY_ITEM_INVALID",
            Self::BasicCarryReferenceMissing { .. } => "BASIC_CARRY_REFERENCE_MISSING",
            Self::ItemCarryNotConfirmed => "ITEM_CARRY_NOT_CONFIRMED",
            Self::ItemCarryNotSource => "ITEM_CARRY_NOT_SOURCE",
            Self::ItemCarriedNotEditable => "ITEM_CARRIED_NOT_EDITABLE",
            Self::ItemCarriedNotDeletable => "ITEM_CARRIED_NOT_DELETABLE",
            Self::WarehouseCodeEmpty => "WAREHOUSE_CODE_EMPTY",
            Self::WarehouseCodeTooLong { .. } => "WAREHOUSE_CODE_TOO_LONG",
            Self::WarehouseAlreadyExists => "WAREHOUSE_ALREADY_EXISTS",
            Self::WarehouseNotFound => "WAREHOUSE_NOT_FOUND",
            Self::WarehouseInactive => "WAREHOUSE_INACTIVE",
            Self::WarehouseInUse => "WAREHOUSE_IN_USE",
            Self::WarehouseClassInvalid => "WAREHOUSE_CLASS_INVALID",
            Self::WarehouseMrpRequiresUsable => "WAREHOUSE_MRP_REQUIRES_USABLE",
            Self::WarehousePriorityInvalid => "WAREHOUSE_PRIORITY_INVALID",
            Self::WarehouseCostCenterInvalid => "WAREHOUSE_COST_CENTER_INVALID",
            Self::WarehouseLocationCodeEmpty => "WAREHOUSE_LOCATION_CODE_EMPTY",
            Self::WarehouseLocationCodeTooLong { .. } => "WAREHOUSE_LOCATION_CODE_TOO_LONG",
            Self::WarehouseLocationAlreadyExists => "WAREHOUSE_LOCATION_ALREADY_EXISTS",
            Self::WarehouseLocationNotFound => "WAREHOUSE_LOCATION_NOT_FOUND",
            Self::WarehouseLocationInUse => "WAREHOUSE_LOCATION_IN_USE",
            Self::WarehouseLocationAccountRequired => "WAREHOUSE_LOCATION_ACCOUNT_REQUIRED",
            Self::WarehouseAccountMethodInvalid => "WAREHOUSE_ACCOUNT_METHOD_INVALID",
            Self::ItemStorageAlreadyExists => "ITEM_STORAGE_ALREADY_EXISTS",
            Self::ItemStorageNotFound => "ITEM_STORAGE_NOT_FOUND",
            Self::ItemStorageMaxQtyInvalid => "ITEM_STORAGE_MAX_QTY_INVALID",
            Self::ItemStorageCopyTargetNotEmpty => "ITEM_STORAGE_COPY_TARGET_NOT_EMPTY",
            Self::InventoryStorageControlInvalid => "INVENTORY_STORAGE_CONTROL_INVALID",
            Self::InventoryTurnoverInvalid => "INVENTORY_TURNOVER_INVALID",
            Self::InventoryPostingDateBasisInvalid => "INVENTORY_POSTING_DATE_BASIS_INVALID",
            Self::ItemAutoNumberDisabled => "ITEM_AUTO_NUMBER_DISABLED",
            Self::EncodingClassNotForItem => "ENCODING_CLASS_NOT_FOR_ITEM",
            Self::EncodingRuleValueRequired => "ENCODING_RULE_VALUE_REQUIRED",
            Self::EncodingRuleSerialExhausted => "ENCODING_RULE_SERIAL_EXHAUSTED",
            Self::EncodingRuleYearCodeMissing { .. } => "ENCODING_RULE_YEAR_CODE_MISSING",
            Self::EncodingRulePeriodMissing => "ENCODING_RULE_PERIOD_MISSING",
            Self::EncodingSegmentCodeEmpty => "ENCODING_SEGMENT_CODE_EMPTY",
            Self::EncodingSegmentCodeTooLong { .. } => "ENCODING_SEGMENT_CODE_TOO_LONG",
            Self::EncodingSegmentLengthInvalid { .. } => "ENCODING_SEGMENT_LENGTH_INVALID",
            Self::EncodingSegmentAlreadyExists => "ENCODING_SEGMENT_ALREADY_EXISTS",
            Self::EncodingSegmentNotFound => "ENCODING_SEGMENT_NOT_FOUND",
            Self::EncodingSegmentInUse => "ENCODING_SEGMENT_IN_USE",
            Self::EncodingSegmentInactive => "ENCODING_SEGMENT_INACTIVE",
            Self::EncodingSegmentLengthHasValues => "ENCODING_SEGMENT_LENGTH_HAS_VALUES",
            Self::EncodingSegmentValueEmpty => "ENCODING_SEGMENT_VALUE_EMPTY",
            Self::EncodingSegmentValueLengthMismatch { .. } => {
                "ENCODING_SEGMENT_VALUE_LENGTH_MISMATCH"
            }
            Self::EncodingSegmentValueInvalidChar => "ENCODING_SEGMENT_VALUE_INVALID_CHAR",
            Self::EncodingSegmentValueDuplicate => "ENCODING_SEGMENT_VALUE_DUPLICATE",
            Self::EncodingSegmentLineInvalid => "ENCODING_SEGMENT_LINE_INVALID",
            Self::EncodingSegmentLineDuplicate => "ENCODING_SEGMENT_LINE_DUPLICATE",
            Self::EncodingSegmentLineNotFound => "ENCODING_SEGMENT_LINE_NOT_FOUND",
            Self::EncodingYearInvalid { .. } => "ENCODING_YEAR_INVALID",
            Self::EncodingYearCodeEmpty => "ENCODING_YEAR_CODE_EMPTY",
            Self::EncodingYearCodeTooLong { .. } => "ENCODING_YEAR_CODE_TOO_LONG",
            Self::EncodingYearCodeInvalidChar => "ENCODING_YEAR_CODE_INVALID_CHAR",
            Self::EncodingYearAlreadyExists => "ENCODING_YEAR_ALREADY_EXISTS",
            Self::EncodingYearNotFound => "ENCODING_YEAR_NOT_FOUND",
            Self::EncodingYearLengthChanged { .. } => "ENCODING_YEAR_LENGTH_CHANGED",
            Self::EpubLibraryDuplicate(_) => "EPUB_LIBRARY_DUPLICATE",
            Self::EpubLibraryFull { .. } => "EPUB_LIBRARY_FULL",
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
