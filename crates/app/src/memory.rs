//! 記憶體版 repository，行為需與 PostgreSQL 版一致（含唯一鍵衝突）。

use chrono::{DateTime, NaiveDate, Utc};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex},
};

use crate::ports::RateFeed;
use async_trait::async_trait;
use ttrs_domain::{
    accounting_period::{AccountingYear, Period},
    authorization::DataScope,
    basic_code::{BasicCode, BasicCodeKind, BasicCodeValue},
    currency::{Currency, CurrencyCode},
    department::{Department, DeptCode},
    dictionary::{ColumnText, TableDictionary, TableText},
    employee::{Employee, EmployeeId},
    exchange_rate::{
        DailyRate, MonthlyRate, RatePeriod, RateRecord, Rates,
        feed::{BankRateType, HistoryQuote, Quote},
    },
    general_parameter::GeneralParameters,
    item::{Item, ItemNo, ItemText},
    item_group::{ItemGroup, ItemGroupCode},
    lang::Lang,
    legacy_catalog::LegacyTable,
    menu::MenuItem,
    permission::{PermissionAttrs, ProgramPermission, parse_actions},
    permission_class::{ClassCode, ClassIdleControl, PermissionClass, PermissionClassAttrs},
    plant::{DEFAULT_TIME_ZONE, Plant, PlantAttrs, PlantId, PlantSchema},
    product_class::{ProductClass, ProductClassCode},
    program::{ModuleCode, OtherSettings, Program, ProgramAttrs, ProgramNo},
    project::{Project, ProjectCode},
    qbe::QbeFilter,
    record::Stamp,
    tax_type::{TaxKey, TaxType},
    unit::{Unit, UnitAttrs, UnitCode},
    unit_conversion::{Conversion, ItemUnitConversion, UnitConversion},
    user::{User, UserId},
    user_permission::{SpecialTarget, UserPermission},
};

use crate::{
    change_log::{self, ChangeRecord, Row, text},
    error::RepoError,
    ports::{
        AccountPolicy, AccountPolicyRepository, AccountingPeriodRepository, AuditEntry,
        BasicCodeRepository, ChangeLogRepository, Credential, CredentialRepository,
        CurrencyRepository, DepartmentRepository, DictionaryRepository, EmployeeRepository,
        ExchangeRateRepository, GithubFile, GithubRepo, GithubSource, ItemGroupRepository,
        ItemRepository, ItemUnitConversionRepository, LegacyCatalogRepository, LegacyTableSummary,
        MasterDataRepository, MenuRepository, MigrationSource, PermissionClassRepository,
        PermissionListItem, PlantRepository, ProductClassRepository, ProgramName,
        ProgramPermissionRepository, ProgramRepository, ProjectRepository, SchemaCatalog,
        SystemParameterRepository, TableSchema, TaxTypeRepository, UnitConversionRepository,
        UnitRepository, UserPermissionItem, UserPermissionRepository, UserRepository,
    },
    query,
};
use crate::{
    mail::{Mail, Mailer},
    ports::{
        AuditLog, JobRecord, PasswordResetTokenRepository, ResetToken, TranslateRequest,
        TranslateResponse, TranslationEngine, TranslationJobRepository,
    },
    ports::{ClassificationCode, ClassificationCodeRepository, Code, CodeKind},
};

#[derive(Default)]
struct State {
    /// 使用者（zx_file）與可使用營運中心（zxy_file）
    users: BTreeMap<UserId, User>,
    user_plants: BTreeMap<UserId, Vec<PlantId>>,
    /// Web 密碼（ADR 0009）；使用者刪除時一併刪除
    credentials: BTreeMap<UserId, Credential>,
    /// 連線版本（user_credential.session_version，ADR 0010 R6）
    session_versions: BTreeMap<UserId, u32>,
    /// 權杖雜湊 → 重設權杖（password_reset_token）
    reset_tokens: BTreeMap<String, ResetToken>,
    account_policy: AccountPolicy,
    /// (使用者, 項次) → 使用者特殊權限（zxw_file）
    user_permissions: BTreeMap<(UserId, i32), UserPermission>,
    /// TipTop 對照目錄（舊表代碼 → 對照）
    legacy: BTreeMap<String, LegacyTable>,
    /// 營運中心（azp_file）
    plants: BTreeMap<PlantId, Plant>,
    /// 已建立的工廠 schema（PostgreSQL 版為 schema＋資料庫角色＋工廠 migration，ADR 0007）
    plant_schemas: HashSet<String>,
    /// 部門（gem_file）、員工（gen_file）：工廠層，以營運中心區分
    departments: BTreeMap<(PlantId, DeptCode), Department>,
    /// (營運中心, 碼類別, 代號) → 分類碼（azf_file，工廠層）
    classification_codes: BTreeMap<(PlantId, String, String), ClassificationCode>,
    employees: BTreeMap<(PlantId, EmployeeId), Employee>,
    /// (營運中心, 分類碼) → 產品分類（oba_file，工廠層）
    product_classes: BTreeMap<(PlantId, String), ProductClass>,
    /// (營運中心, 分群碼) → 料件分群（imz_file，工廠層）
    item_groups: BTreeMap<(PlantId, String), ItemGroup>,
    /// 資料異動記錄（change_log，ADR 0012）：營運中心（共用層為 None）＋記錄
    changes: Vec<(Option<PlantId>, ChangeRecord)>,
    /// 單位（gfe_file，共用）
    units: BTreeMap<String, Unit>,
    /// 編碼性質（geh_file；encoding_nature.feature）
    encoding_natures: BTreeMap<String, ttrs_domain::encoding_nature::EncodingNature>,
    /// 編碼獨立段（gej_file、gek_file；encoding_segment.feature）
    encoding_segments: BTreeMap<String, ttrs_domain::encoding_segment::EncodingSegment>,
    /// 集團中心（geu_file）與資料中心的資料類別（gev_file；group_center.feature）
    group_centers: BTreeMap<String, ttrs_domain::group_center::GroupCenter>,
    /// 資料控管原則的受控營運中心（gev03＝N）與欄位條件（gey_file；data_control.feature），鍵為（資料類別, 營運中心）
    data_controls: BTreeMap<(String, String), ttrs_domain::data_control::ControlledPlant>,
    /// 資料中心拋轉設定（gew_file、gez_file；carry_setting.feature），鍵為（資料中心, 資料類別）
    carry_settings: BTreeMap<(String, String), ttrs_domain::carry_setting::CarrySetting>,
    /// 拋轉紀錄（gex_file；basic_data_carry.feature）
    carry_logs: Vec<ttrs_domain::basic_carry::CarryLogEntry>,
    /// 編碼年對照（gff_file；encoding_year.feature），鍵為（年, 長度）
    encoding_years: BTreeMap<(i32, i16), ttrs_domain::encoding_year::EncodingYear>,
    /// 編碼類別（gei_file、gel_file；encoding_class.feature）
    encoding_classes: BTreeMap<String, ttrs_domain::encoding_class::EncodingClass>,
    /// (種類, 代號) → 基本代碼（gea、geb、geo、ged、azh，共用）
    basic_codes: BTreeMap<(BasicCodeKind, String), BasicCode>,
    /// 幣別（azi_file，共用）
    currencies: BTreeMap<String, Currency>,
    /// (幣別, 年月) → 每月匯率（azj_file，共用）
    monthly_rates: BTreeMap<(String, RatePeriod), MonthlyRate>,
    /// (幣別, 日期) → 每日匯率（azk_file，共用）
    daily_rates: BTreeMap<(String, NaiveDate), DailyRate>,
    /// 整體管理系統參數（aza_file；aoos010）；None 表示同 migration 起始資料（12 期、TWD、每日）
    general: Option<GeneralParameters>,
    /// 會計年度（azm_file，共用；accounting_periods.feature）
    accounting_years: BTreeMap<i32, AccountingYear>,
    /// 一般單位換算（smc_file，共用）：(來源, 目的) → 換算
    unit_conversions: BTreeMap<(String, String), UnitConversion>,
    /// 料件單位換算（smd_file，工廠層）：(營運中心, 料號, 來源, 目的) → 換算
    item_unit_conversions: BTreeMap<(PlantId, String, String, String), ItemUnitConversion>,
    /// 匯率自動匯入的來源（BDD 設定）；None 表示無法取得（IM8）
    bank_quotes: Option<Vec<Quote>>,
    /// GitHub 假來源（BDD；github_epub_search.feature）：repo、各 repo 的檔案（路徑、大小、內容）、查詢次數是否用完
    github_repos: Vec<GithubRepo>,
    github_files: BTreeMap<String, Vec<(String, u64, Vec<u8>)>>,
    github_rate_limited: bool,
    /// GitHub 依檔名搜尋（epub_github_filename.feature）：是否設定 token、程式碼搜尋是否被限流
    github_token: bool,
    github_code_limited: bool,
    /// 我的 GitHub 書庫（epub_github_library.feature；表 epub_github_library）
    epub_library: Vec<crate::ports::EpubLibraryEntry>,
    /// 共用 GitHub 書庫（epub_github_shared_library.feature；表 epub_github_shared_library）
    epub_shared_library: Vec<crate::ports::EpubSharedLibraryEntry>,
    /// 翻譯工作（translation_job；epub_translate.feature）
    translation_jobs: BTreeMap<String, JobRecord>,
    customs_current: Option<(NaiveDate, NaiveDate, Vec<Quote>)>,
    customs_history: Option<Vec<HistoryQuote>>,
    /// 稅別（gec_file，共用）
    tax_types: BTreeMap<TaxKey, TaxType>,
    /// (營運中心, 專案編號) → 專案（gja_file，工廠層）
    projects: BTreeMap<(PlantId, String), Project>,
    items: BTreeMap<(PlantId, ItemNo), Item>,
    /// 料件的其他語系品名、規格（item_text）
    item_texts: BTreeMap<(PlantId, ItemNo, String), ItemText>,
    transactions: HashSet<(PlantId, ItemNo)>,
    // Program Maintenance
    modules: HashSet<String>,
    /// 權限類別（zw_file）
    permission_classes: BTreeMap<ClassCode, PermissionClass>,
    /// (選單代號, 程式代號)（zm_file）
    /// 選單單身（zm_file）：目錄、序號、子項程式
    menu_items: Vec<MenuItem>,
    /// p_query 查詢定義代號（zai_file）
    query_definitions: HashSet<String>,
    programs: BTreeMap<ProgramNo, Program>,
    /// (程式代號, 語系) → (名稱, 報表抬頭)（gaz_file）
    names: BTreeMap<(String, Lang), (String, Option<String>)>,
    /// (權限類別, 程式代號)（zy_file）
    /// (權限類別, 程式代號) → 程式執行權限（zy_file）
    permissions: BTreeMap<(String, String), ProgramPermission>,
    audit: Vec<AuditEntry>,
    // 資料字典：資料庫結構與 migration 由測試提供（記憶體版無法讀取真正的資料庫）
    schema: BTreeMap<String, TableSchema>,
    migrations: Vec<MigrationSource>,
    dictionaries: BTreeMap<String, TableDictionary>,
    /// (資料表, 語系) → 表說明
    table_texts: BTreeMap<(String, String), TableText>,
    /// (資料表, 欄位, 語系) → 欄位說明
    column_texts: BTreeMap<(String, String, String), ColumnText>,
}

#[derive(Clone, Default)]
pub struct InMemoryStore(Arc<Mutex<State>>);

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 預設種子資料，與 migrations/0002_seed.sql 對齊
    pub fn seeded() -> Self {
        let s = Self::new();
        s.add_plant("TW01");
        s.add_unit("PCS");
        s.add_unit("KG");
        for m in ["AIM", "AGL", "AZZ", "AOO"] {
            s.add_module(m);
        }
        s.add_permission_class("ADMIN");
        // 同 PostgreSQL：ADMIN 由早期 migration 建立，加上確認欄位時既有資料視為已確認（ADR 0013 SA9-3）
        if let Ok(admin) = ClassCode::parse("ADMIN")
            && let Some(c) = s.lock().permission_classes.get_mut(&admin)
        {
            c.confirmation = ttrs_domain::confirmation::Confirmation::confirmed(None, None);
        }
        s.add_ported_programs(Some("ADMIN"));
        s
    }

    /// 起始目錄（同 migrations/0026，menu_maintenance.feature MN14）：目錄程式、名稱與單身
    pub fn add_default_menus(&self) {
        self.add_module("MENU");
        let today = chrono::Local::now().date_naive();
        let menus = [
            (
                "menu-main",
                ["主選單", "Main Menu", "主选单"],
                vec!["menu-aim", "menu-aoo", "menu-azz"],
            ),
            (
                "menu-aim",
                ["庫存管理", "Inventory", "库存管理"],
                vec!["item-group-maintenance", "item-maintenance"],
            ),
            (
                "menu-aoo",
                ["整體管理", "General Management", "整体管理"],
                vec![
                    "plant-maintenance",
                    "department-maintenance",
                    "employee-maintenance",
                    "classification-code-maintenance",
                    "unit-maintenance",
                    "product-classification-maintenance",
                    "currency-maintenance",
                    "monthly-exchange-rate-maintenance",
                    "daily-exchange-rate-maintenance",
                    "daily-exchange-rate-copy",
                    "tax-type-maintenance",
                    "project-maintenance",
                    "region-maintenance",
                    "country-maintenance",
                    "area-maintenance",
                    "shipping-method-maintenance",
                    "report-title-maintenance",
                    "general-management-parameters",
                    "accounting-period-maintenance",
                    "unit-conversion-maintenance",
                    "item-unit-conversion-maintenance",
                    "encoding-nature-maintenance",
                    "encoding-segment-maintenance",
                    "encoding-year-maintenance",
                    "encoding-class-maintenance",
                ],
            ),
            (
                "menu-azz",
                ["系統管理", "System Admin", "系统管理"],
                vec![
                    "program-maintenance",
                    "data-dictionary",
                    "user-maintenance",
                    "user-permission-maintenance",
                    "permission-class-maintenance",
                    "program-permission-maintenance",
                    "menu-maintenance",
                    "epub-to-text",
                    "github-epub-search",
                ],
            ),
        ];
        {
            let mut s = self.lock();
            for (no, names, _) in &menus {
                let pno = ProgramNo::parse(no).expect("程式代號");
                let module = ModuleCode::parse("MENU").expect("模組");
                let program = Program::new(
                    pno.clone(),
                    module,
                    ProgramAttrs::default(),
                    "tiptop",
                    "",
                    today,
                )
                .expect("程式");
                s.programs.entry(pno).or_insert(program);
                for (lang, name) in ["zh-TW", "en", "zh-CN"].iter().zip(names) {
                    let lang = Lang::parse(lang).expect("語系");
                    s.names
                        .entry(((*no).to_owned(), lang))
                        .or_insert(((*name).to_owned(), None));
                }
            }
        }
        for (no, _, children) in &menus {
            for child in children {
                self.add_menu_item(no, child);
            }
        }
        // 已移植作業的名稱（同 migrations/0026）；已有名稱或程式未登錄時略過
        let names: [(&str, [&str; 3]); 36] = [
            (
                "encoding-class-maintenance",
                ["編碼類別維護", "Encoding Classes", "编码类别维护"],
            ),
            (
                "encoding-year-maintenance",
                ["編碼年對照維護", "Encoding Year Codes", "编码年对照维护"],
            ),
            (
                "encoding-segment-maintenance",
                ["編碼獨立段維護", "Encoding Segments", "编码独立段维护"],
            ),
            (
                "encoding-nature-maintenance",
                ["編碼性質維護", "Encoding Natures", "编码性质维护"],
            ),
            (
                "monthly-exchange-rate-maintenance",
                ["每月匯率維護", "Monthly Exchange Rates", "每月汇率维护"],
            ),
            (
                "daily-exchange-rate-maintenance",
                ["每日匯率維護", "Daily Exchange Rates", "每日汇率维护"],
            ),
            (
                "daily-exchange-rate-copy",
                ["每日匯率複製", "Daily Exchange Rate Copy", "每日汇率复制"],
            ),
            ("project-maintenance", ["專案維護", "Projects", "专案维护"]),
            ("epub-to-text", ["EPUB 轉換", "EPUB Converter", "EPUB 转换"]),
            (
                "github-epub-search",
                ["EPUB 搜尋", "EPUB Search", "EPUB 搜索"],
            ),
            (
                "unit-conversion-maintenance",
                ["單位換算維護", "Unit Conversions", "单位换算维护"],
            ),
            (
                "item-unit-conversion-maintenance",
                [
                    "料件單位換算維護",
                    "Item Unit Conversions",
                    "料件单位换算维护",
                ],
            ),
            (
                "accounting-period-maintenance",
                ["會計期間維護", "Accounting Periods", "会计期间维护"],
            ),
            (
                "general-management-parameters",
                [
                    "整體管理系統參數",
                    "General Management Parameters",
                    "整体管理系统参数",
                ],
            ),
            (
                "tax-type-maintenance",
                ["稅別維護", "Tax Types", "税别维护"],
            ),
            (
                "currency-maintenance",
                ["幣別維護", "Currencies", "币别维护"],
            ),
            ("unit-maintenance", ["單位維護", "Units", "单位维护"]),
            (
                "region-maintenance",
                ["區域別維護", "Regions", "区域别维护"],
            ),
            ("country-maintenance", ["國別維護", "Countries", "国别维护"]),
            ("area-maintenance", ["地區維護", "Areas", "地区维护"]),
            (
                "shipping-method-maintenance",
                ["交運方式維護", "Shipping Methods", "交运方式维护"],
            ),
            (
                "report-title-maintenance",
                ["報表抬頭維護", "Report Titles", "报表抬头维护"],
            ),
            (
                "product-classification-maintenance",
                ["產品分類維護", "Product Classifications", "产品分类维护"],
            ),
            (
                "item-group-maintenance",
                ["料件分群維護", "Item Groups", "料件分群维护"],
            ),
            (
                "menu-maintenance",
                ["選單維護", "Menu Maintenance", "选单维护"],
            ),
            (
                "item-maintenance",
                ["料件基本資料", "Item Master", "料件基本资料"],
            ),
            (
                "plant-maintenance",
                ["營運中心維護", "Plants", "营运中心维护"],
            ),
            (
                "department-maintenance",
                ["部門維護", "Departments", "部门维护"],
            ),
            (
                "employee-maintenance",
                ["員工維護", "Employees", "员工维护"],
            ),
            (
                "classification-code-maintenance",
                ["分類碼維護", "Classification Codes", "分类码维护"],
            ),
            (
                "program-maintenance",
                ["程式維護", "Program Maintenance", "程序维护"],
            ),
            (
                "data-dictionary",
                ["資料字典維護", "Data Dictionary", "数据字典维护"],
            ),
            (
                "user-maintenance",
                ["使用者維護", "User Maintenance", "用户维护"],
            ),
            (
                "user-permission-maintenance",
                ["使用者特殊權限", "User Special Permissions", "用户特殊权限"],
            ),
            (
                "permission-class-maintenance",
                ["權限類別維護", "Permission Classes", "权限类别维护"],
            ),
            (
                "program-permission-maintenance",
                ["程式執行權限", "Program Permissions", "程序执行权限"],
            ),
        ];
        let mut s = self.lock();
        for (no, texts) in names {
            if !s.programs.keys().any(|p| p.as_str() == no) {
                continue;
            }
            for (lang, name) in ["zh-TW", "en", "zh-CN"].iter().zip(texts) {
                let lang = Lang::parse(lang).expect("語系");
                s.names
                    .entry((no.to_owned(), lang))
                    .or_insert((name.to_owned(), None));
            }
        }
    }

    /// 登錄 TTRS 已移植的作業（AQ11，同 migrations/0015）；`grant_to` 有值時給該權限類別全部功能、資料權限 0／0／0、單身 0。
    /// 已存在的程式不變
    pub fn add_ported_programs(&self, grant_to: Option<&str>) {
        let today = chrono::Local::now().date_naive();
        let mut s = self.lock();
        for (no, tiptop, module, actions) in crate::authorization::PORTED_PROGRAMS {
            let no = ProgramNo::parse(no).expect("程式代號");
            let attrs = ProgramAttrs {
                program_type: Some("I".to_owned()),
                // TTRS 新增的作業（如 epub-to-text）沒有 TipTop 代號
                tiptop_program_no: Some((*tiptop).to_owned()).filter(|t| !t.is_empty()),
                other: OtherSettings {
                    default_actions: Some((*actions).to_owned()),
                    ..OtherSettings::default()
                },
                ..ProgramAttrs::default()
            };
            let module = ModuleCode::parse(module).expect("模組");
            let program =
                Program::new(no.clone(), module, attrs, "tiptop", "", today).expect("程式");
            s.programs.entry(no.clone()).or_insert(program);
            if let Some(class) = grant_to {
                let class = ClassCode::parse(class).expect("權限類別代號");
                let attrs = PermissionAttrs::defaults(&parse_actions([*actions]));
                let p = ProgramPermission::new(class, no, attrs, "tiptop", "", today);
                s.permissions.entry(perm_key(&p)).or_insert(p);
            }
        }
    }

    /// 測試用：使用者可使用的營運中心數
    pub fn user_plant_count(&self, id: &UserId) -> usize {
        self.lock().user_plants.get(id).map_or(0, Vec::len)
    }

    /// 營運中心（azp_file）：名稱同代號、資料庫代碼為下一個未使用的號碼，其餘為資料庫預設值
    pub fn add_plant(&self, plant: &str) {
        let id = PlantId::parse(plant).expect("營運中心代號");
        let mut s = self.lock();
        let used: Vec<PlantSchema> = s.plants.values().map(|p| p.schema.clone()).collect();
        let schema = PlantSchema::next_free(&used).expect("資料庫代碼");
        s.plant_schemas.insert(schema.as_str().to_owned());
        let p = Plant {
            stamp: Stamp::default(),
            schema,
            attrs: PlantAttrs {
                name: id.as_str().to_owned(),
                time_zone: DEFAULT_TIME_ZONE.to_owned(),
                address: None,
                phone: None,
                fax: None,
            },
            active: true,
            // 與 PostgreSQL 直接寫入相同：欄位預設未確認
            confirmation: Default::default(),
            created_by: String::new(),
            created_dept: String::new(),
            created_on: chrono::Local::now().date_naive(),
            updated_by: None,
            updated_on: None,
            id: id.clone(),
        };
        s.plants.insert(id, p);
    }

    /// 測試用：工廠 schema 是否已建立
    pub fn plant_schema_exists(&self, schema: &str) -> bool {
        self.lock().plant_schemas.contains(schema)
    }

    /// 可使用營運中心加入 ALL（zxy03＝ALL，sign_in.feature SQ5）
    pub fn set_user_all_plants(&self, user: &str) {
        let id = UserId::parse(user).expect("使用者代號");
        let mut s = self.lock();
        let plants = s.user_plants.entry(id).or_default();
        if !plants.iter().any(PlantId::is_all) {
            plants.push(PlantId::all());
        }
    }

    /// 直接設定有效日期（不經使用者維護，資料有效不變）
    pub fn set_user_valid_until(&self, user: &str, date: Option<NaiveDate>) {
        let id = UserId::parse(user).expect("使用者代號");
        if let Some(u) = self.lock().users.get_mut(&id) {
            u.attrs.valid_until = date;
        }
    }

    /// 帳號稽核原則（gbt_file）
    pub fn set_account_policy(&self, policy: AccountPolicy) {
        self.lock().account_policy = policy;
    }

    /// 使用者目前的密碼狀態（BDD 驗證用）
    pub fn credential_of(&self, user: &str) -> Option<Credential> {
        let id = UserId::parse(user).expect("使用者代號");
        self.lock().credentials.get(&id).cloned()
    }

    /// 測試用：已存的重設權杖雜湊
    pub fn reset_token_hashes(&self) -> Vec<String> {
        self.lock().reset_tokens.keys().cloned().collect()
    }

    /// 測試用：重設權杖的建立與到期時間往前推（模擬時間經過）
    pub fn age_reset_tokens(&self, minutes: i64) {
        let d = chrono::Duration::minutes(minutes);
        for t in self.lock().reset_tokens.values_mut() {
            t.created_at -= d;
            t.expires_at -= d;
        }
    }

    /// 直接設定密碼更新日（模擬先前變更的密碼）
    pub fn set_password_changed_on(&self, user: &str, date: NaiveDate) {
        let id = UserId::parse(user).expect("使用者代號");
        if let Some(c) = self.lock().credentials.get_mut(&id) {
            c.changed_on = Some(date);
        }
    }

    /// 測試資料：單位（代碼轉大寫、說明同代碼，同早期種子資料）
    pub fn add_unit(&self, unit: &str) {
        let code = UnitCode::parse(&unit.to_uppercase()).expect("單位");
        let attrs = UnitAttrs::new(Some(code.as_str().to_owned()), None).expect("單位");
        let today = chrono::Local::now().date_naive();
        let u = Unit::new(code.clone(), attrs, "tiptop", "", today);
        self.lock()
            .units
            .entry(code.as_str().to_owned())
            .or_insert(u);
    }

    pub fn add_transaction(&self, plant: &PlantId, no: &ItemNo) {
        self.lock().transactions.insert((plant.clone(), no.clone()));
    }

    /// 模組（gao_file）
    pub fn add_module(&self, module: &str) {
        self.lock().modules.insert(module.to_uppercase());
    }

    /// 權限類別（zw_file）：說明同代號、其餘為資料庫預設值（同 migrations/0011 的欄位預設）
    pub fn add_permission_class(&self, class: &str) {
        let code = ClassCode::parse(class).expect("權限類別代號");
        let pc = PermissionClass {
            stamp: Stamp::default(),
            attrs: PermissionClassAttrs {
                name: class.to_string(),
                default_menu: None,
                idle_control: ClassIdleControl::Off,
                idle_action: None,
                idle_seconds: None,
            },
            active: true,
            confirmation: Default::default(),
            created_by: String::new(),
            created_dept: String::new(),
            created_on: chrono::Local::now().date_naive(),
            updated_by: None,
            updated_on: None,
            code: code.clone(),
        };
        self.lock().permission_classes.insert(code, pc);
    }

    /// 測試用：使用者的特殊權限筆數
    pub fn user_permission_count(&self, user: &str) -> usize {
        self.lock()
            .user_permissions
            .keys()
            .filter(|(u, _)| u.as_str() == user)
            .count()
    }

    /// 測試用：權限類別的程式權限筆數
    pub fn class_permission_count(&self, class: &str) -> usize {
        self.lock()
            .permissions
            .keys()
            .filter(|(c, _)| c == class)
            .count()
    }

    /// 測試資料：加入選單單身，序號接在最後（與 PostgreSQL 種子資料相同）
    pub fn add_menu_item(&self, menu: &str, program: &str) {
        let mut s = self.lock();
        if s.menu_items
            .iter()
            .any(|i| i.menu == menu && i.program == program)
        {
            return;
        }
        let seq = s
            .menu_items
            .iter()
            .filter(|i| i.menu == menu)
            .map(|i| i.seq)
            .max()
            .unwrap_or(0)
            + ttrs_domain::menu::SEQ_STEP;
        s.menu_items.push(MenuItem {
            menu: menu.to_owned(),
            seq,
            program: program.to_owned(),
        });
    }

    pub fn add_query_definition(&self, query: &str) {
        self.lock().query_definitions.insert(query.to_string());
    }

    /// 測試用：登錄一張資料表的實際結構（PostgreSQL 版由系統目錄讀取）
    pub fn add_table_schema(&self, table: TableSchema) {
        self.lock().schema.insert(table.name.clone(), table);
    }

    /// 測試用：登錄 migration 內容（PostgreSQL 版取自 sqlx 內嵌的 migrations）
    pub fn add_migration(&self, migration: MigrationSource) {
        self.lock().migrations.push(migration);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().expect("in-memory store poisoned")
    }
}

#[async_trait]
impl ItemRepository for InMemoryStore {
    async fn list_items(&self, plant: &PlantId) -> Result<Vec<Item>, RepoError> {
        Ok(self
            .lock()
            .items
            .iter()
            .filter(|((p, _), _)| p == plant)
            .map(|(_, i)| i.clone())
            .collect())
    }

    async fn get_item(&self, plant: &PlantId, no: &ItemNo) -> Result<Option<Item>, RepoError> {
        Ok(self.lock().items.get(&(plant.clone(), no.clone())).cloned())
    }

    async fn query_items(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        scope: &DataScope,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Item>, u64), RepoError> {
        let s = self.lock();
        let hits = s
            .items
            .iter()
            .filter(|((p, _), i)| p == plant && scope.includes(&i.created_by, &i.created_dept))
            .filter(|((_, no), i)| {
                let texts: Vec<ItemText> = s
                    .item_texts
                    .iter()
                    .filter(|((p, n, _), _)| p == plant && n == no)
                    .map(|(_, t)| t.clone())
                    .collect();
                query::matches_all(filters, |f| query::item::value(i, &texts, f))
            })
            .map(|(_, i)| i.clone())
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert_item(&self, plant: &PlantId, item: &Item) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (plant.clone(), item.no.clone());
        if s.items.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut item = item.clone();
        item.stamp = new_stamp();
        s.log_change(
            Some(plant),
            "item",
            item_key(&item),
            None,
            Some(item_row(&item)),
            &[],
        );
        s.items.insert(key, item);
        Ok(())
    }

    async fn update_item(&self, plant: &PlantId, item: &Item) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (plant.clone(), item.no.clone());
        let Some(old) = s.items.get(&key).cloned() else {
            return Ok(());
        };
        let mut new = item.clone();
        new.stamp = bumped(&old.stamp, &item.stamp)?;
        s.log_change(
            Some(plant),
            "item",
            item_key(&new),
            Some(item_row(&old)),
            Some(item_row(&new)),
            &[],
        );
        s.items.insert(key, new);
        Ok(())
    }

    async fn delete_item(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s.items.remove(&(plant.clone(), no.clone())) else {
            return Ok(false);
        };
        s.item_texts.retain(|(p, n, _), _| !(p == plant && n == no));
        s.log_change(
            Some(plant),
            "item",
            item_key(&old),
            Some(item_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }

    async fn has_transactions(&self, plant: &PlantId, no: &ItemNo) -> Result<bool, RepoError> {
        Ok(self
            .lock()
            .transactions
            .contains(&(plant.clone(), no.clone())))
    }

    async fn item_texts(&self, plant: &PlantId, no: &ItemNo) -> Result<Vec<ItemText>, RepoError> {
        Ok(self
            .lock()
            .item_texts
            .iter()
            .filter(|((p, n, _), _)| p == plant && n == no)
            .map(|(_, t)| t.clone())
            .collect())
    }

    async fn set_item_text(
        &self,
        plant: &PlantId,
        no: &ItemNo,
        text: &ItemText,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (plant.clone(), no.clone(), text.lang.as_str().to_owned());
        if text.name.is_none() && text.spec.is_none() {
            s.item_texts.remove(&key);
        } else {
            s.item_texts.insert(key, text.clone());
        }
        Ok(())
    }
}

#[async_trait]
impl MasterDataRepository for InMemoryStore {
    async fn plant_active(&self, plant: &PlantId) -> Result<bool, RepoError> {
        Ok(self.lock().plants.get(plant).is_some_and(|p| p.active))
    }

    async fn unit_exists(&self, unit: &str) -> Result<bool, RepoError> {
        // 早期料件以大寫比對（同 PostgreSQL 版）
        Ok(self.lock().units.contains_key(&unit.to_uppercase()))
    }
}

/// 依序結果取指定範圍，並回傳總筆數
fn page_of<T>(hits: Vec<T>, offset: u64, limit: u64) -> (Vec<T>, u64) {
    let total = hits.len() as u64;
    let page = hits
        .into_iter()
        .skip(usize::try_from(offset).unwrap_or(usize::MAX))
        .take(usize::try_from(limit).unwrap_or(usize::MAX))
        .collect();
    (page, total)
}

#[async_trait]
impl ProgramRepository for InMemoryStore {
    async fn get(&self, no: &ProgramNo) -> Result<Option<Program>, RepoError> {
        Ok(self.lock().programs.get(no).cloned())
    }

    async fn list(&self) -> Result<Vec<Program>, RepoError> {
        Ok(self.lock().programs.values().cloned().collect())
    }

    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Program>, u64), RepoError> {
        let s = self.lock();
        // BTreeMap 依 ProgramNo（String）排序＝位元組順序，與 PostgreSQL 的 COLLATE "C" 一致
        let hits = s
            .programs
            .values()
            .filter(|p| {
                let names: Vec<ProgramName> = s
                    .names
                    .iter()
                    .filter(|((n, _), _)| n == p.program_no.as_str())
                    .map(|((_, lang), (name, report_title))| ProgramName {
                        lang: lang.clone(),
                        name: name.clone(),
                        report_title: report_title.clone(),
                    })
                    .collect();
                crate::program_query::matches_all(p, &names, filters)
            })
            .cloned()
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert(
        &self,
        program: &Program,
        names: &[ProgramName],
        grant_to: Option<&ProgramPermission>,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.programs.contains_key(&program.program_no) {
            return Err(RepoError::Conflict);
        }
        let no = program.program_no.as_str().to_owned();
        let mut program = program.clone();
        program.stamp = new_stamp();
        s.log_change(
            None,
            "program",
            program_key(&program),
            None,
            Some(program_row(&program)),
            &[],
        );
        s.programs.insert(program.program_no.clone(), program);
        for n in names {
            s.names.insert(
                (no.clone(), n.lang.clone()),
                (n.name.clone(), n.report_title.clone()),
            );
        }
        if let Some(p) = grant_to {
            s.permissions.insert(perm_key(p), p.clone());
        }
        Ok(())
    }

    async fn update(&self, program: &Program) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.programs.get(&program.program_no).cloned() else {
            return Ok(());
        };
        let mut p = program.clone();
        p.stamp = bumped(&old.stamp, &program.stamp)?;
        s.log_change(
            None,
            "program",
            program_key(&p),
            Some(program_row(&old)),
            Some(program_row(&p)),
            &[],
        );
        s.programs.insert(p.program_no.clone(), p);
        Ok(())
    }

    async fn change_no(&self, old: &ProgramNo, new: &ProgramNo) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.programs.contains_key(new) {
            return Err(RepoError::Conflict);
        }
        let Some(mut p) = s.programs.remove(old) else {
            return Ok(());
        };
        let before = program_row(&p);
        p.program_no = new.clone();
        p.stamp = bumped(&p.stamp, &p.stamp)?;
        s.log_change(
            None,
            "program",
            program_key(&p),
            Some(before),
            Some(program_row(&p)),
            &[],
        );
        s.programs.insert(new.clone(), p);
        let (old, new) = (old.as_str(), new.as_str());
        s.names = std::mem::take(&mut s.names)
            .into_iter()
            .map(|((no, lang), name)| ((rename(no, old, new), lang), name))
            .collect();
        let renamed = ProgramNo::parse(new).expect("程式代號");
        s.permissions = std::mem::take(&mut s.permissions)
            .into_iter()
            .map(|((class, no), mut p)| {
                if no == old {
                    p.program = renamed.clone();
                }
                ((class, rename(no, old, new)), p)
            })
            .collect();
        for i in &mut s.menu_items {
            if i.program == old {
                i.program = new.to_owned();
            }
        }
        // 特殊權限的程式列（同 PostgreSQL 的 ON UPDATE CASCADE）
        for up in s.user_permissions.values_mut() {
            if let SpecialTarget::Program { program, .. } = &mut up.target
                && program.as_str() == old
            {
                *program = renamed.clone();
            }
        }
        Ok(())
    }

    async fn delete(&self, no: &ProgramNo, audit: &AuditEntry) -> Result<(), RepoError> {
        let mut s = self.lock();
        if let Some(old) = s.programs.remove(no) {
            s.log_change(
                None,
                "program",
                program_key(&old),
                Some(program_row(&old)),
                None,
                &[],
            );
        }
        let no = no.as_str();
        s.names.retain(|(n, _), _| n != no);
        s.permissions.retain(|(_, n), _| n != no);
        s.menu_items.retain(|i| i.program != no);
        s.user_permissions
            .retain(|_, up| up.target.program().is_none_or(|p| p.as_str() != no));
        s.audit.push(audit.clone());
        Ok(())
    }

    async fn names(&self, no: &ProgramNo) -> Result<Vec<ProgramName>, RepoError> {
        Ok(self
            .lock()
            .names
            .iter()
            .filter(|((n, _), _)| n == no.as_str())
            .map(|((_, lang), (name, report_title))| ProgramName {
                lang: lang.clone(),
                name: name.clone(),
                report_title: report_title.clone(),
            })
            .collect())
    }

    async fn set_name(&self, no: &ProgramNo, name: &ProgramName) -> Result<(), RepoError> {
        self.lock().names.insert(
            (no.as_str().to_owned(), name.lang.clone()),
            (name.name.clone(), name.report_title.clone()),
        );
        Ok(())
    }

    async fn grant(&self, permission: &ProgramPermission) -> Result<(), RepoError> {
        self.lock()
            .permissions
            .entry(perm_key(permission))
            .or_insert_with(|| permission.clone());
        Ok(())
    }

    async fn permission_classes(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError> {
        let mut v: Vec<String> = self
            .lock()
            .permissions
            .keys()
            .filter(|(_, n)| n == no.as_str())
            .map(|(c, _)| c.clone())
            .collect();
        v.sort();
        Ok(v)
    }

    async fn menus_containing(&self, no: &ProgramNo) -> Result<Vec<String>, RepoError> {
        let mut v: Vec<String> = self
            .lock()
            .menu_items
            .iter()
            .filter(|i| i.program == no.as_str())
            .map(|i| i.menu.clone())
            .collect();
        v.sort();
        Ok(v)
    }

    async fn menu_children(&self, menu: &str) -> Result<Vec<Program>, RepoError> {
        let s = self.lock();
        // programs 為 BTreeMap，依程式代號排序
        Ok(s.programs
            .values()
            .filter(|p| {
                s.menu_items
                    .iter()
                    .any(|i| i.menu == menu && i.program == p.program_no.as_str())
            })
            .cloned()
            .collect())
    }

    async fn audit_entries(&self, key: &str) -> Result<Vec<AuditEntry>, RepoError> {
        Ok(self
            .lock()
            .audit
            .iter()
            .filter(|a| a.key == key)
            .cloned()
            .collect())
    }

    async fn module_exists(&self, module: &ModuleCode) -> Result<bool, RepoError> {
        Ok(self.lock().modules.contains(module.as_str()))
    }

    async fn permission_class_exists(&self, class: &str) -> Result<bool, RepoError> {
        Ok(ClassCode::parse(class).is_ok_and(|c| self.lock().permission_classes.contains_key(&c)))
    }

    async fn query_definition_exists(&self, query: &str) -> Result<bool, RepoError> {
        Ok(self.lock().query_definitions.contains(query))
    }

    async fn find_by_tiptop_no(&self, tiptop_no: &str) -> Result<Option<ProgramNo>, RepoError> {
        Ok(self
            .lock()
            .programs
            .values()
            .find(|p| p.tiptop_program_no.as_deref() == Some(tiptop_no))
            .map(|p| p.program_no.clone()))
    }
}

fn perm_key(p: &ProgramPermission) -> (String, String) {
    (p.class.as_str().to_owned(), p.program.as_str().to_owned())
}

fn rename(no: String, old: &str, new: &str) -> String {
    if no == old { new.to_owned() } else { no }
}

#[async_trait]
impl SchemaCatalog for InMemoryStore {
    async fn table_names(&self) -> Result<Vec<String>, RepoError> {
        Ok(self.lock().schema.keys().cloned().collect())
    }

    async fn table(&self, name: &str) -> Result<Option<TableSchema>, RepoError> {
        Ok(self.lock().schema.get(name).cloned())
    }

    async fn migrations(&self) -> Result<Vec<MigrationSource>, RepoError> {
        let mut v = self.lock().migrations.clone();
        v.sort_by_key(|m| m.version);
        Ok(v)
    }
}

#[async_trait]
impl DictionaryRepository for InMemoryStore {
    async fn get(&self, table: &str) -> Result<Option<TableDictionary>, RepoError> {
        Ok(self.lock().dictionaries.get(table).cloned())
    }

    async fn list(&self) -> Result<Vec<TableDictionary>, RepoError> {
        Ok(self.lock().dictionaries.values().cloned().collect())
    }

    async fn insert(&self, dict: &TableDictionary) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.dictionaries.contains_key(&dict.table_name) {
            return Err(RepoError::Conflict);
        }
        let mut d = dict.clone();
        d.stamp = new_stamp();
        s.log_change(
            None,
            "table_dictionary",
            dict_key(&d),
            None,
            Some(dict_row(&d)),
            &[],
        );
        s.dictionaries.insert(d.table_name.clone(), d);
        Ok(())
    }

    async fn update(&self, dict: &TableDictionary) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.dictionaries.get(&dict.table_name).cloned() else {
            return Ok(());
        };
        let mut d = dict.clone();
        d.stamp = bumped(&old.stamp, &dict.stamp)?;
        s.log_change(
            None,
            "table_dictionary",
            dict_key(&d),
            Some(dict_row(&old)),
            Some(dict_row(&d)),
            &[],
        );
        s.dictionaries.insert(d.table_name.clone(), d);
        Ok(())
    }

    async fn delete(&self, table: &str) -> Result<(), RepoError> {
        let mut s = self.lock();
        if let Some(old) = s.dictionaries.remove(table) {
            s.log_change(
                None,
                "table_dictionary",
                dict_key(&old),
                Some(dict_row(&old)),
                None,
                &[],
            );
        }
        s.table_texts.retain(|(t, _), _| t != table);
        s.column_texts.retain(|(t, _, _), _| t != table);
        Ok(())
    }

    async fn set_table_text(&self, table: &str, text: &TableText) -> Result<(), RepoError> {
        self.lock().table_texts.insert(
            (table.to_owned(), text.lang.as_str().to_owned()),
            text.clone(),
        );
        Ok(())
    }

    async fn table_texts(&self, table: &str) -> Result<Vec<TableText>, RepoError> {
        Ok(self
            .lock()
            .table_texts
            .iter()
            .filter(|((t, _), _)| t == table)
            .map(|(_, v)| v.clone())
            .collect())
    }

    async fn set_column_text(&self, table: &str, text: &ColumnText) -> Result<(), RepoError> {
        self.lock().column_texts.insert(
            (
                table.to_owned(),
                text.column_name.clone(),
                text.lang.as_str().to_owned(),
            ),
            text.clone(),
        );
        Ok(())
    }

    async fn column_texts(&self, table: &str) -> Result<Vec<ColumnText>, RepoError> {
        Ok(self
            .lock()
            .column_texts
            .iter()
            .filter(|((t, _, _), _)| t == table)
            .map(|(_, v)| v.clone())
            .collect())
    }

    async fn module_exists(&self, module: &ModuleCode) -> Result<bool, RepoError> {
        Ok(self.lock().modules.contains(module.as_str()))
    }
}

#[async_trait]
impl LegacyCatalogRepository for InMemoryStore {
    async fn replace_all(&self, tables: &[LegacyTable]) -> Result<(), RepoError> {
        self.lock().legacy = tables
            .iter()
            .map(|t| (t.legacy_name.clone(), t.clone()))
            .collect();
        Ok(())
    }

    async fn list_tables(&self) -> Result<Vec<LegacyTableSummary>, RepoError> {
        // BTreeMap 依舊表代碼排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(self
            .lock()
            .legacy
            .values()
            .map(|t| LegacyTableSummary {
                legacy_name: t.legacy_name.clone(),
                name: t.name.clone(),
                title: t.title.clone(),
                module: t.module.clone(),
                column_count: u32::try_from(t.columns.len()).unwrap_or(u32::MAX),
            })
            .collect())
    }

    async fn get(&self, legacy_name: &str) -> Result<Option<LegacyTable>, RepoError> {
        Ok(self.lock().legacy.get(legacy_name).cloned())
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<LegacyTable>, RepoError> {
        Ok(self
            .lock()
            .legacy
            .values()
            .find(|t| t.name == name)
            .cloned())
    }
}

#[async_trait]
impl UserRepository for InMemoryStore {
    async fn get(&self, id: &UserId) -> Result<Option<User>, RepoError> {
        Ok(self.lock().users.get(id).cloned())
    }

    async fn list(&self) -> Result<Vec<User>, RepoError> {
        // BTreeMap 依使用者代號排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(self.lock().users.values().cloned().collect())
    }

    async fn query(
        &self,
        filters: &[QbeFilter],
        scope: &DataScope,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<User>, u64), RepoError> {
        let hits = self
            .lock()
            .users
            .values()
            .filter(|x| scope.includes(&x.created_by, &x.created_dept))
            .filter(|x| query::matches_all(filters, |f| query::user::value(x, f)))
            .cloned()
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert(&self, user: &User, plants: &[PlantId]) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.users.contains_key(&user.user_id) {
            return Err(RepoError::Conflict);
        }
        let mut u = user.clone();
        u.stamp = new_stamp();
        s.log_change(
            None,
            "user_account",
            user_key(&u),
            None,
            Some(user_row(&u, plants)),
            USER_MASKED,
        );
        s.users.insert(u.user_id.clone(), u);
        s.replace_user_plants(&user.user_id, plants);
        Ok(())
    }

    async fn update(&self, user: &User, plants: &[PlantId]) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.users.get(&user.user_id).cloned() else {
            return Ok(());
        };
        let old_plants = s
            .user_plants
            .get(&user.user_id)
            .cloned()
            .unwrap_or_default();
        let mut u = user.clone();
        u.stamp = bumped(&old.stamp, &user.stamp)?;
        s.log_change(
            None,
            "user_account",
            user_key(&u),
            Some(user_row(&old, &old_plants)),
            Some(user_row(&u, plants)),
            USER_MASKED,
        );
        s.users.insert(u.user_id.clone(), u);
        s.replace_user_plants(&user.user_id, plants);
        Ok(())
    }

    async fn delete(&self, id: &UserId) -> Result<(), RepoError> {
        let mut s = self.lock();
        s.replace_user_plants(id, &[]);
        if let Some(old) = s.users.remove(id) {
            s.log_change(
                None,
                "user_account",
                user_key(&old),
                Some(user_row(&old, &[])),
                None,
                USER_MASKED,
            );
        }
        s.user_plants.remove(id);
        s.credentials.remove(id);
        s.session_versions.remove(id);
        s.reset_tokens.retain(|_, t| t.user_id != id.as_str());
        s.user_permissions.retain(|(u, _), _| u != id);
        Ok(())
    }

    async fn plants(&self, id: &UserId) -> Result<Vec<PlantId>, RepoError> {
        // 與 PostgreSQL 一致：依營運中心代號排序
        let mut plants = self.lock().user_plants.get(id).cloned().unwrap_or_default();
        plants.sort();
        Ok(plants)
    }

    async fn find_by_idp_username(&self, username: &str) -> Result<Option<UserId>, RepoError> {
        Ok(self
            .lock()
            .users
            .values()
            .find(|u| {
                u.attrs
                    .idp_username
                    .as_deref()
                    .is_some_and(|a| a.eq_ignore_ascii_case(username))
            })
            .map(|u| u.user_id.clone()))
    }

    async fn find_by_idp_subject(&self, subject: &str) -> Result<Option<UserId>, RepoError> {
        Ok(self
            .lock()
            .users
            .values()
            .find(|u| u.idp_subject.as_deref() == Some(subject))
            .map(|u| u.user_id.clone()))
    }

    async fn bind_idp_subject(&self, id: &UserId, subject: &str) -> Result<(), RepoError> {
        self.lock()
            .touch_user(id, |u| u.idp_subject = Some(subject.to_owned()));
        Ok(())
    }

    async fn set_default_plant(&self, id: &UserId, plant: &PlantId) -> Result<(), RepoError> {
        self.lock()
            .touch_user(id, |u| u.attrs.default_plant = plant.clone());
        Ok(())
    }
}

#[async_trait]
impl PermissionClassRepository for InMemoryStore {
    async fn get(&self, code: &ClassCode) -> Result<Option<PermissionClass>, RepoError> {
        Ok(self.lock().permission_classes.get(code).cloned())
    }

    async fn list(&self) -> Result<Vec<PermissionClass>, RepoError> {
        // BTreeMap 依代號排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(self.lock().permission_classes.values().cloned().collect())
    }

    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<PermissionClass>, u64), RepoError> {
        let hits = self
            .lock()
            .permission_classes
            .values()
            .filter(|x| query::matches_all(filters, |f| query::permission_class::value(x, f)))
            .cloned()
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert(&self, class: &PermissionClass) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.permission_classes.contains_key(&class.code) {
            return Err(RepoError::Conflict);
        }
        let mut c = class.clone();
        c.stamp = new_stamp();
        s.log_change(
            None,
            "permission_class",
            class_code_key(&c),
            None,
            Some(permission_class_row(&c)),
            &[],
        );
        s.permission_classes.insert(c.code.clone(), c);
        Ok(())
    }

    async fn update(&self, class: &PermissionClass) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.permission_classes.get(&class.code).cloned() else {
            return Ok(());
        };
        let mut c = class.clone();
        c.stamp = bumped(&old.stamp, &class.stamp)?;
        s.log_change(
            None,
            "permission_class",
            class_code_key(&c),
            Some(permission_class_row(&old)),
            Some(permission_class_row(&c)),
            &[],
        );
        s.permission_classes.insert(c.code.clone(), c);
        Ok(())
    }

    async fn delete(&self, code: &ClassCode) -> Result<(), RepoError> {
        let mut s = self.lock();
        if let Some(old) = s.permission_classes.remove(code) {
            s.log_change(
                None,
                "permission_class",
                class_code_key(&old),
                Some(permission_class_row(&old)),
                None,
                &[],
            );
        }
        Ok(())
    }

    async fn user_count(&self, code: &ClassCode) -> Result<u64, RepoError> {
        let s = self.lock();
        let direct = s
            .users
            .values()
            .filter(|u| u.attrs.permission_class == code.as_str())
            .count();
        let special: HashSet<&UserId> = s
            .user_permissions
            .values()
            .filter(|up| up.target.class() == Some(code))
            .map(|up| &up.user_id)
            .collect();
        Ok((direct + special.len()) as u64)
    }

    async fn permission_count(&self, code: &ClassCode) -> Result<u64, RepoError> {
        Ok(self.class_permission_count(code.as_str()) as u64)
    }
}

#[async_trait]
impl PlantRepository for InMemoryStore {
    async fn get(&self, id: &PlantId) -> Result<Option<Plant>, RepoError> {
        Ok(self.lock().plants.get(id).cloned())
    }

    async fn list(&self) -> Result<Vec<Plant>, RepoError> {
        // BTreeMap 依代號排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(self.lock().plants.values().cloned().collect())
    }

    async fn query(
        &self,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Plant>, u64), RepoError> {
        let hits = self
            .lock()
            .plants
            .values()
            .filter(|x| query::matches_all(filters, |f| query::plant::value(x, f)))
            .cloned()
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn schema_names(&self) -> Result<Vec<PlantSchema>, RepoError> {
        Ok(self
            .lock()
            .plants
            .values()
            .map(|p| p.schema.clone())
            .collect())
    }

    async fn insert(&self, plant: &Plant) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.plants.contains_key(&plant.id) || s.plants.values().any(|p| p.schema == plant.schema) {
            return Err(RepoError::Conflict);
        }
        s.plant_schemas.insert(plant.schema.as_str().to_owned());
        let mut p = plant.clone();
        p.stamp = new_stamp();
        s.log_change(None, "plant", plant_key(&p), None, Some(plant_row(&p)), &[]);
        s.plants.insert(p.id.clone(), p);
        Ok(())
    }

    async fn update(&self, plant: &Plant) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.plants.get(&plant.id).cloned() else {
            return Ok(());
        };
        let mut p = plant.clone();
        p.stamp = bumped(&old.stamp, &plant.stamp)?;
        s.log_change(
            None,
            "plant",
            plant_key(&p),
            Some(plant_row(&old)),
            Some(plant_row(&p)),
            &[],
        );
        s.plants.insert(p.id.clone(), p);
        Ok(())
    }

    async fn delete(&self, id: &PlantId) -> Result<(), RepoError> {
        if self.in_use(id).await? {
            return Err(RepoError::InUse);
        }
        let mut s = self.lock();
        if let Some(p) = s.plants.remove(id) {
            s.plant_schemas.remove(p.schema.as_str());
            s.log_change(None, "plant", plant_key(&p), Some(plant_row(&p)), None, &[]);
        }
        Ok(())
    }

    /// 與 PostgreSQL 一致：使用者預設／可使用營運中心（外鍵）、工廠 schema 內有資料（料件、部門、員工）
    async fn in_use(&self, id: &PlantId) -> Result<bool, RepoError> {
        let s = self.lock();
        Ok(s.users.values().any(|u| &u.attrs.default_plant == id)
            || s.user_plants.values().any(|ps| ps.contains(id))
            || s.items.keys().any(|(p, _)| p == id)
            || s.departments.keys().any(|(p, _)| p == id)
            || s.employees.keys().any(|(p, _)| p == id)
            // 資料中心的資料類別設定（外鍵 data_center_type.plant_id，group_center.feature）
            || s.group_centers
                .values()
                .any(|c| c.data_types.iter().any(|t| t.plant == id.as_str()))
            // 資料控管的受控營運中心（外鍵 data_control_plant.plant_id，data_control.feature）
            || s.data_controls.keys().any(|(_, p)| p == id.as_str())
            // 拋轉營運中心（外鍵 carry_target.plant_id，carry_setting.feature）
            || s.carry_settings
                .values()
                .any(|c| c.targets.iter().any(|t| t.plant == id.as_str())))
    }
}

#[async_trait]
impl ProgramPermissionRepository for InMemoryStore {
    async fn get(
        &self,
        class: &ClassCode,
        program: &ProgramNo,
    ) -> Result<Option<ProgramPermission>, RepoError> {
        let key = (class.as_str().to_owned(), program.as_str().to_owned());
        Ok(self.lock().permissions.get(&key).cloned())
    }

    async fn query_classes(
        &self,
        header: &[QbeFilter],
        detail: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<PermissionClass>, u64), RepoError> {
        let s = self.lock();
        let hits = s
            .permission_classes
            .values()
            .filter(|c| {
                query::matches_all(header, |f| query::permission_class::value(c, f))
                    && (detail.is_empty()
                        || s.permissions.values().any(|p| {
                            p.class == c.code
                                && query::matches_all(detail, |f| {
                                    query::program_permission::value(p, f)
                                })
                        }))
            })
            .cloned()
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn list(&self, class: &ClassCode) -> Result<Vec<PermissionListItem>, RepoError> {
        let s = self.lock();
        // BTreeMap 依 (權限類別, 程式代號) 排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(s.permissions
            .iter()
            .filter(|((c, _), _)| c == class.as_str())
            .map(|((_, no), p)| PermissionListItem {
                permission: p.clone(),
                default_actions: s
                    .programs
                    .get(&p.program)
                    .and_then(|prog| prog.other.default_actions.clone()),
                names: s
                    .names
                    .iter()
                    .filter(|((n, _), _)| n == no)
                    .map(|((_, lang), (name, report_title))| ProgramName {
                        lang: lang.clone(),
                        name: name.clone(),
                        report_title: report_title.clone(),
                    })
                    .collect(),
            })
            .collect())
    }

    async fn insert(&self, permission: &ProgramPermission) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = perm_key(permission);
        if s.permissions.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut p = permission.clone();
        p.stamp = new_stamp();
        s.log_change(
            None,
            "program_permission",
            perm_change_key(&p),
            None,
            Some(perm_row(&p)),
            &[],
        );
        s.permissions.insert(key, p);
        Ok(())
    }

    async fn update(&self, permission: &ProgramPermission) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = perm_key(permission);
        let Some(old) = s.permissions.get(&key).cloned() else {
            return Ok(());
        };
        let mut p = permission.clone();
        p.stamp = bumped(&old.stamp, &permission.stamp)?;
        s.log_change(
            None,
            "program_permission",
            perm_change_key(&p),
            Some(perm_row(&old)),
            Some(perm_row(&p)),
            &[],
        );
        s.permissions.insert(key, p);
        Ok(())
    }

    async fn delete(&self, class: &ClassCode, program: &ProgramNo) -> Result<bool, RepoError> {
        let key = (class.as_str().to_owned(), program.as_str().to_owned());
        let mut s = self.lock();
        let Some(old) = s.permissions.remove(&key) else {
            return Ok(false);
        };
        s.log_change(
            None,
            "program_permission",
            perm_change_key(&old),
            Some(perm_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }

    async fn delete_all(&self, class: &ClassCode) -> Result<u64, RepoError> {
        let mut s = self.lock();
        let removed: Vec<ProgramPermission> = s
            .permissions
            .iter()
            .filter(|((c, _), _)| c == class.as_str())
            .map(|(_, p)| p.clone())
            .collect();
        s.permissions.retain(|(c, _), _| c != class.as_str());
        for old in &removed {
            s.log_change(
                None,
                "program_permission",
                perm_change_key(old),
                Some(perm_row(old)),
                None,
                &[],
            );
        }
        Ok(removed.len() as u64)
    }
}

#[async_trait]
impl UserPermissionRepository for InMemoryStore {
    async fn list(&self, user: &UserId) -> Result<Vec<UserPermissionItem>, RepoError> {
        let s = self.lock();
        // BTreeMap 依 (使用者, 項次) 排序
        Ok(s.user_permissions
            .iter()
            .filter(|((u, _), _)| u == user)
            .map(|(_, up)| {
                let (class_name, default_actions, names) = match &up.target {
                    SpecialTarget::Class(c) => (
                        s.permission_classes.get(c).map(|pc| pc.attrs.name.clone()),
                        None,
                        Vec::new(),
                    ),
                    SpecialTarget::Program { program, .. } => (
                        None,
                        s.programs
                            .get(program)
                            .and_then(|p| p.other.default_actions.clone()),
                        s.names
                            .iter()
                            .filter(|((n, _), _)| n == program.as_str())
                            .map(|((_, lang), (name, report_title))| ProgramName {
                                lang: lang.clone(),
                                name: name.clone(),
                                report_title: report_title.clone(),
                            })
                            .collect(),
                    ),
                };
                UserPermissionItem {
                    permission: up.clone(),
                    class_name,
                    default_actions,
                    names,
                }
            })
            .collect())
    }

    async fn insert(&self, permission: &UserPermission) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (permission.user_id.clone(), permission.line_no);
        let same_target = s.user_permissions.values().any(|up| {
            up.user_id == permission.user_id
                && ((up.target.class().is_some() && up.target.class() == permission.target.class())
                    || (up.target.program().is_some()
                        && up.target.program() == permission.target.program()))
        });
        if s.user_permissions.contains_key(&key) || same_target {
            return Err(RepoError::Conflict);
        }
        let mut p = permission.clone();
        p.stamp = new_stamp();
        s.log_change(
            None,
            "user_permission",
            uperm_key(&p),
            None,
            Some(uperm_row(&p)),
            &[],
        );
        s.user_permissions.insert(key, p);
        Ok(())
    }

    async fn update(&self, permission: &UserPermission) -> Result<(), RepoError> {
        let key = (permission.user_id.clone(), permission.line_no);
        let mut s = self.lock();
        let Some(old) = s.user_permissions.get(&key).cloned() else {
            return Ok(());
        };
        let mut p = permission.clone();
        p.stamp = bumped(&old.stamp, &permission.stamp)?;
        s.log_change(
            None,
            "user_permission",
            uperm_key(&p),
            Some(uperm_row(&old)),
            Some(uperm_row(&p)),
            &[],
        );
        s.user_permissions.insert(key, p);
        Ok(())
    }

    async fn delete(&self, user: &UserId, line_no: i32) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s.user_permissions.remove(&(user.clone(), line_no)) else {
            return Ok(false);
        };
        s.log_change(
            None,
            "user_permission",
            uperm_key(&old),
            Some(uperm_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }

    async fn delete_all(&self, user: &UserId) -> Result<u64, RepoError> {
        let mut s = self.lock();
        let removed: Vec<UserPermission> = s
            .user_permissions
            .iter()
            .filter(|((u, _), _)| u == user)
            .map(|(_, p)| p.clone())
            .collect();
        s.user_permissions.retain(|(u, _), _| u != user);
        for old in &removed {
            s.log_change(
                None,
                "user_permission",
                uperm_key(old),
                Some(uperm_row(old)),
                None,
                &[],
            );
        }
        Ok(removed.len() as u64)
    }
}

#[async_trait]
impl ClassificationCodeRepository for InMemoryStore {
    async fn list_codes(
        &self,
        plant: &PlantId,
        kind: CodeKind,
    ) -> Result<Vec<ClassificationCode>, RepoError> {
        // BTreeMap 依代號排序（位元組順序，同 PostgreSQL COLLATE "C"）
        Ok(self
            .lock()
            .classification_codes
            .iter()
            .filter(|((p, k, _), _)| p == plant && k == kind.code())
            .map(|(_, c)| c.clone())
            .collect())
    }

    async fn get_code(
        &self,
        plant: &PlantId,
        kind: CodeKind,
        code: &Code,
    ) -> Result<Option<ClassificationCode>, RepoError> {
        Ok(self
            .lock()
            .classification_codes
            .get(&code_key(plant, kind, code))
            .cloned())
    }

    async fn insert_code(&self, plant: &PlantId, c: &ClassificationCode) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.plants.contains_key(plant) {
            return Err(RepoError::Other(format!(
                "營運中心不存在：{}",
                plant.as_str()
            )));
        }
        let key = code_key(plant, c.kind, &c.code);
        if s.classification_codes.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut c = c.clone();
        c.stamp = new_stamp();
        s.log_change(
            Some(plant),
            "classification_code",
            class_key(&c),
            None,
            Some(class_row(&c)),
            &[],
        );
        s.classification_codes.insert(key, c);
        Ok(())
    }

    async fn update_code(&self, plant: &PlantId, c: &ClassificationCode) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = code_key(plant, c.kind, &c.code);
        let Some(old) = s.classification_codes.get(&key).cloned() else {
            return Ok(());
        };
        let mut new = c.clone();
        new.stamp = bumped(&old.stamp, &c.stamp)?;
        s.log_change(
            Some(plant),
            "classification_code",
            class_key(&new),
            Some(class_row(&old)),
            Some(class_row(&new)),
            &[],
        );
        s.classification_codes.insert(key, new);
        Ok(())
    }

    async fn delete_code(
        &self,
        plant: &PlantId,
        kind: CodeKind,
        code: &Code,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s.classification_codes.remove(&code_key(plant, kind, code)) else {
            return Ok(false);
        };
        s.log_change(
            Some(plant),
            "classification_code",
            class_key(&old),
            Some(class_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

fn code_key(plant: &PlantId, kind: CodeKind, code: &Code) -> (PlantId, String, String) {
    (
        plant.clone(),
        kind.code().to_owned(),
        code.as_str().to_owned(),
    )
}

#[async_trait]
impl DepartmentRepository for InMemoryStore {
    async fn get(&self, plant: &PlantId, code: &DeptCode) -> Result<Option<Department>, RepoError> {
        Ok(self
            .lock()
            .departments
            .get(&(plant.clone(), code.clone()))
            .cloned())
    }

    async fn query_departments(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Department>, u64), RepoError> {
        let hits = self
            .lock()
            .departments
            .iter()
            .filter(|((p, _), d)| {
                p == plant && query::matches_all(filters, |f| query::department::value(d, f))
            })
            .map(|(_, d)| d.clone())
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert(&self, plant: &PlantId, dept: &Department) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.plants.contains_key(plant) {
            return Err(RepoError::Other(format!(
                "營運中心不存在：{}",
                plant.as_str()
            )));
        }
        let key = (plant.clone(), dept.code.clone());
        if s.departments.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut d = dept.clone();
        d.stamp = new_stamp();
        s.log_change(
            Some(plant),
            "department",
            dept_key(&d),
            None,
            Some(dept_row(&d)),
            &[],
        );
        s.departments.insert(key, d);
        Ok(())
    }

    async fn update(&self, plant: &PlantId, dept: &Department) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (plant.clone(), dept.code.clone());
        let Some(old) = s.departments.get(&key).cloned() else {
            return Ok(());
        };
        let mut d = dept.clone();
        d.stamp = bumped(&old.stamp, &dept.stamp)?;
        s.log_change(
            Some(plant),
            "department",
            dept_key(&d),
            Some(dept_row(&old)),
            Some(dept_row(&d)),
            &[],
        );
        s.departments.insert(key, d);
        Ok(())
    }

    async fn delete(&self, plant: &PlantId, code: &DeptCode) -> Result<bool, RepoError> {
        let mut s = self.lock();
        // 與 PostgreSQL 的外鍵一致：還有員工時不可刪除
        if s.employees
            .iter()
            .any(|((p, _), e)| p == plant && e.attrs.dept_code.as_ref() == Some(code))
        {
            return Err(RepoError::InUse);
        }
        let Some(old) = s.departments.remove(&(plant.clone(), code.clone())) else {
            return Ok(false);
        };
        s.log_change(
            Some(plant),
            "department",
            dept_key(&old),
            Some(dept_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

#[async_trait]
impl EmployeeRepository for InMemoryStore {
    async fn get(&self, plant: &PlantId, id: &EmployeeId) -> Result<Option<Employee>, RepoError> {
        Ok(self
            .lock()
            .employees
            .get(&(plant.clone(), id.clone()))
            .cloned())
    }

    async fn query_employees(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<Employee>, u64), RepoError> {
        let hits = self
            .lock()
            .employees
            .iter()
            .filter(|((p, _), e)| {
                p == plant && query::matches_all(filters, |f| query::employee::value(e, f))
            })
            .map(|(_, e)| e.clone())
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert(&self, plant: &PlantId, employee: &Employee) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.plants.contains_key(plant) {
            return Err(RepoError::Other(format!(
                "營運中心不存在：{}",
                plant.as_str()
            )));
        }
        let key = (plant.clone(), employee.id.clone());
        if s.employees.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut e = employee.clone();
        e.stamp = new_stamp();
        s.log_change(
            Some(plant),
            "employee",
            employee_key(&e),
            None,
            Some(employee_row(&e)),
            EMPLOYEE_MASKED,
        );
        s.employees.insert(key, e);
        Ok(())
    }

    async fn update(&self, plant: &PlantId, employee: &Employee) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (plant.clone(), employee.id.clone());
        let Some(old) = s.employees.get(&key).cloned() else {
            return Ok(());
        };
        let mut e = employee.clone();
        e.stamp = bumped(&old.stamp, &employee.stamp)?;
        s.log_change(
            Some(plant),
            "employee",
            employee_key(&e),
            Some(employee_row(&old)),
            Some(employee_row(&e)),
            EMPLOYEE_MASKED,
        );
        s.employees.insert(key, e);
        Ok(())
    }

    async fn delete(&self, plant: &PlantId, id: &EmployeeId) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s.employees.remove(&(plant.clone(), id.clone())) else {
            return Ok(false);
        };
        s.log_change(
            Some(plant),
            "employee",
            employee_key(&old),
            Some(employee_row(&old)),
            None,
            EMPLOYEE_MASKED,
        );
        Ok(true)
    }
}

#[async_trait]
impl CredentialRepository for InMemoryStore {
    async fn credential(&self, user: &UserId) -> Result<Option<Credential>, RepoError> {
        Ok(self.lock().credentials.get(user).cloned())
    }

    async fn save_credential(
        &self,
        user: &UserId,
        credential: &Credential,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.users.contains_key(user) {
            return Err(RepoError::Other(format!("使用者不存在：{}", user.as_str())));
        }
        s.credentials.insert(user.clone(), credential.clone());
        Ok(())
    }

    async fn session_version(&self, user: &UserId) -> Result<u32, RepoError> {
        Ok(self.lock().session_versions.get(user).copied().unwrap_or(0))
    }

    async fn end_sessions(&self, user: &UserId) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.users.contains_key(user) {
            return Err(RepoError::Other(format!("使用者不存在：{}", user.as_str())));
        }
        *s.session_versions.entry(user.clone()).or_insert(0) += 1;
        Ok(())
    }
}

#[async_trait]
impl PasswordResetTokenRepository for InMemoryStore {
    async fn issue_reset_token(
        &self,
        token_hash: &str,
        token: &ResetToken,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        for t in s.reset_tokens.values_mut() {
            if t.user_id == token.user_id && t.used_at.is_none() {
                t.used_at = Some(token.created_at);
            }
        }
        if s.reset_tokens.contains_key(token_hash) {
            return Err(RepoError::Conflict);
        }
        s.reset_tokens.insert(token_hash.to_owned(), token.clone());
        Ok(())
    }

    async fn reset_token(&self, token_hash: &str) -> Result<Option<ResetToken>, RepoError> {
        Ok(self.lock().reset_tokens.get(token_hash).cloned())
    }

    async fn reset_requests_since(
        &self,
        user: &UserId,
        since: DateTime<Utc>,
    ) -> Result<u64, RepoError> {
        let n = self
            .lock()
            .reset_tokens
            .values()
            .filter(|t| t.user_id == user.as_str() && t.created_at > since)
            .count();
        Ok(n as u64)
    }

    async fn revoke_reset_tokens(&self, user: &UserId, at: DateTime<Utc>) -> Result<(), RepoError> {
        for t in self.lock().reset_tokens.values_mut() {
            if t.user_id == user.as_str() && t.used_at.is_none() {
                t.used_at = Some(at);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl AuditLog for InMemoryStore {
    async fn record(&self, entry: &AuditEntry) -> Result<(), RepoError> {
        self.lock().audit.push(entry.clone());
        Ok(())
    }
}

/// 記憶體版寄信：信件留在寄件匣，供測試與沒有 SMTP 的本機開發檢視
#[derive(Clone, Default)]
pub struct Outbox(Arc<Mutex<Vec<Mail>>>);

impl Outbox {
    pub fn sent(&self) -> Vec<Mail> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

#[async_trait]
impl Mailer for Outbox {
    async fn send(&self, mail: &Mail) -> Result<(), RepoError> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(mail.clone());
        Ok(())
    }
}

#[async_trait]
impl AccountPolicyRepository for InMemoryStore {
    async fn account_policy(&self) -> Result<AccountPolicy, RepoError> {
        Ok(self.lock().account_policy.clone())
    }
}

// ---------------------------------------------------------------------------
// 資料異動記錄（ADR 0012）：與 PostgreSQL 觸發器 ttrs_change_log／ttrs_bump_version 相同的規則
// ---------------------------------------------------------------------------

/// 員工的個資欄位：只記「有修改」（CH6）
const EMPLOYEE_MASKED: &[&str] = &["email"];

fn new_stamp() -> Stamp {
    Stamp {
        version: 1,
        created_at: Some(Utc::now()),
        updated_at: None,
    }
}

/// 修改：版本須與儲存的相同（樂觀鎖），新版本加 1、記修改時間
fn bumped(stored: &Stamp, loaded: &Stamp) -> Result<Stamp, RepoError> {
    if stored.version != loaded.version {
        return Err(RepoError::Stale);
    }
    Ok(Stamp {
        version: stored.version + 1,
        created_at: stored.created_at,
        updated_at: Some(Utc::now()),
    })
}

fn row(pairs: Vec<(&str, Option<String>)>) -> Row {
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

fn audit_cols(
    active: bool,
    created_by: &str,
    created_dept: &str,
    created_on: NaiveDate,
    updated_by: Option<&str>,
    updated_on: Option<NaiveDate>,
    stamp: &Stamp,
) -> Vec<(&'static str, Option<String>)> {
    vec![
        ("active", text::b(active)),
        ("created_by", text::s(created_by)),
        ("created_dept", text::s(created_dept)),
        ("created_on", text::date(created_on)),
        ("updated_by", text::opt(updated_by)),
        ("updated_on", text::opt_date(updated_on)),
        ("version", text::int(i64::from(stamp.version))),
    ]
}

fn dept_key(d: &Department) -> Vec<(&'static str, String)> {
    vec![("dept_code", d.code.as_str().to_owned())]
}

fn dept_row(d: &Department) -> Row {
    let a = &d.accounting;
    let mut cols = vec![
        ("dept_code", text::s(d.code.as_str())),
        ("name", text::s(&d.attrs.name)),
        ("full_name", text::opt(d.attrs.full_name.as_deref())),
        ("accounting_dept", text::b(a.accounting_dept)),
        ("expense_type", a.expense_type.map(|e| e.code().to_owned())),
        ("control_type", text::s(a.control_type.code())),
        (
            "cost_center",
            a.cost_center.as_ref().map(|c| c.as_str().to_owned()),
        ),
        ("confirmed", text::b(d.confirmation.confirmed)),
        ("confirmed_by", text::opt(d.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        d.active,
        &d.created_by,
        &d.created_dept,
        d.created_on,
        d.updated_by.as_deref(),
        d.updated_on,
        &d.stamp,
    ));
    row(cols)
}

fn employee_key(e: &Employee) -> Vec<(&'static str, String)> {
    vec![("employee_id", e.id.as_str().to_owned())]
}

fn employee_row(e: &Employee) -> Row {
    let a = &e.attrs;
    let mut cols = vec![
        ("employee_id", text::s(e.id.as_str())),
        ("name", text::s(&a.name)),
        (
            "dept_code",
            a.dept_code.as_ref().map(|d| d.as_str().to_owned()),
        ),
        ("title", text::opt(a.title.as_deref())),
        ("extension", text::opt(a.extension.as_deref())),
        ("email", text::opt(a.email.as_deref())),
        ("confirmed", text::b(e.confirmation.confirmed)),
        ("confirmed_by", text::opt(e.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        e.active,
        &e.created_by,
        &e.created_dept,
        e.created_on,
        e.updated_by.as_deref(),
        e.updated_on,
        &e.stamp,
    ));
    row(cols)
}

fn class_key(c: &ClassificationCode) -> Vec<(&'static str, String)> {
    vec![
        ("kind", c.kind.code().to_owned()),
        ("code", c.code.as_str().to_owned()),
    ]
}

fn class_row(c: &ClassificationCode) -> Row {
    let a = &c.attrs;
    let mut cols = vec![
        ("kind", text::s(c.kind.code())),
        ("code", text::s(c.code.as_str())),
        ("description", text::opt(a.description.as_deref())),
        ("variance_account", text::opt(a.variance_account.as_deref())),
        (
            "variance_account2",
            text::opt(a.variance_account2.as_deref()),
        ),
        ("cost_nature", a.cost_nature.map(|n| n.to_string())),
        ("confirmed", text::b(c.confirmation.confirmed)),
        ("confirmed_by", text::opt(c.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        c.active,
        &c.created_by,
        &c.created_dept,
        c.created_on,
        c.updated_by.as_deref(),
        c.updated_on,
        &c.stamp,
    ));
    row(cols)
}

impl State {
    /// 寫一筆異動記錄；操作情境取自 change_log::current()，沒有時記為 db（直接修改資料）
    fn log_change(
        &mut self,
        plant: Option<&PlantId>,
        table: &str,
        key: Vec<(&str, String)>,
        before: Option<Row>,
        after: Option<Row>,
        masked: &[&str],
    ) {
        let ctx = change_log::current();
        let changed = change_log::changed_fields(before.as_ref(), after.as_ref());
        let masked_row = |r: Option<Row>| {
            r.map(|mut r| {
                change_log::mask(&mut r, masked);
                r
            })
        };
        let operation = match (&before, &after) {
            (None, _) => "insert".to_owned(),
            (_, None) => "delete".to_owned(),
            _ => ctx
                .as_ref()
                .and_then(|c| c.operation.clone())
                .unwrap_or_else(|| "update".to_owned()),
        };
        let version = after
            .as_ref()
            .or(before.as_ref())
            .and_then(|r| r.get("version").cloned().flatten())
            .and_then(|v| v.parse().ok());
        let change_id = i64::try_from(self.changes.len()).unwrap_or(i64::MAX - 1) + 1;
        let record = ChangeRecord {
            change_id,
            occurred_at: Utc::now(),
            table_name: table.to_owned(),
            record_key: key.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
            operation,
            before: masked_row(before),
            after: masked_row(after),
            changed_fields: changed,
            version,
            user_id: ctx
                .as_ref()
                .map_or_else(|| "db:memory".to_owned(), |c| c.user_id.clone()),
            user_dept: ctx.as_ref().map(|c| c.user_dept.clone()),
            actor_kind: ctx.as_ref().map_or("db", |c| c.actor_kind).to_owned(),
            on_behalf_of: ctx.as_ref().and_then(|c| c.on_behalf_of.clone()),
            program: ctx.as_ref().map(|c| c.program.clone()),
            action: ctx.as_ref().map(|c| c.action.clone()),
            request_id: ctx.as_ref().map(|c| c.request.request_id.clone()),
            client_ip: ctx.as_ref().and_then(|c| c.request.client_ip.clone()),
            user_agent: ctx.as_ref().and_then(|c| c.request.user_agent.clone()),
            reason: ctx.as_ref().and_then(|c| c.reason.clone()),
        };
        self.changes.push((plant.cloned(), record));
    }
}

#[async_trait]
impl ChangeLogRepository for InMemoryStore {
    async fn history(
        &self,
        plant: Option<&PlantId>,
        tables: &[&str],
        key: &BTreeMap<String, String>,
    ) -> Result<Vec<ChangeRecord>, RepoError> {
        Ok(self
            .lock()
            .changes
            .iter()
            .filter(|(p, r)| {
                p.as_ref() == plant
                    && tables.contains(&r.table_name.as_str())
                    && key.iter().all(|(k, v)| r.record_key.get(k) == Some(v))
            })
            .map(|(_, r)| r.clone())
            .collect())
    }
}

// ----- 第二階段：共用層的表（與 migrations/0025 的觸發器一致）。
// 記憶體後端記錄主檔與可使用營運中心；程式名稱、資料字典說明與連帶刪除的子表只在 PostgreSQL 記錄。

/// 使用者的個資欄位：只記「有修改」（CH6）
const USER_MASKED: &[&str] = &["email"];

/// text[] 欄位的文字表示（與 PostgreSQL jsonb_each_text 相同：JSON 陣列）
fn text_array(items: &[String]) -> Option<String> {
    let quoted: Vec<String> = items
        .iter()
        .map(|i| format!("\"{}\"", i.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    Some(format!("[{}]", quoted.join(", ")))
}

fn stamp_cols(
    created_by: &str,
    created_dept: &str,
    created_on: NaiveDate,
    updated_by: Option<&str>,
    updated_on: Option<NaiveDate>,
    stamp: &Stamp,
) -> Vec<(&'static str, Option<String>)> {
    vec![
        ("created_by", text::s(created_by)),
        ("created_dept", text::s(created_dept)),
        ("created_on", text::date(created_on)),
        ("updated_by", text::opt(updated_by)),
        ("updated_on", text::opt_date(updated_on)),
        ("version", text::int(i64::from(stamp.version))),
    ]
}

fn program_key(p: &Program) -> Vec<(&'static str, String)> {
    vec![("program_no", p.program_no.as_str().to_owned())]
}

/// 程式資料：主要欄位（其餘執行設定欄位只在 PostgreSQL 的記錄中）
fn program_row(p: &Program) -> Row {
    row(vec![
        ("program_no", text::s(p.program_no.as_str())),
        ("module_code", text::s(p.module.as_str())),
        ("program_type", text::opt(p.program_type.as_deref())),
        (
            "default_actions",
            text::opt(p.other.default_actions.as_deref()),
        ),
        ("print_conditions", text::b(p.other.print_conditions)),
        (
            "tiptop_program_no",
            text::opt(p.tiptop_program_no.as_deref()),
        ),
        ("created_by", text::s(&p.created_by)),
        ("created_dept", text::s(&p.created_dept)),
        ("updated_by", text::opt(p.updated_by.as_deref())),
        ("updated_on", text::date(p.updated_on)),
        ("version", text::int(i64::from(p.stamp.version))),
    ])
}

fn dict_key(d: &TableDictionary) -> Vec<(&'static str, String)> {
    vec![("table_name", d.table_name.clone())]
}

fn dict_row(d: &TableDictionary) -> Row {
    row(vec![
        ("table_name", text::s(&d.table_name)),
        ("module_code", text::s(d.module.as_str())),
        ("table_type", d.table_type.map(|t| t.code().to_owned())),
        ("upstream", text::opt(d.upstream.as_deref())),
        ("downstream", text::opt(d.downstream.as_deref())),
        ("created_by", text::s(&d.created_by)),
        ("created_on", text::date(d.created_on)),
        ("version", text::int(i64::from(d.stamp.version))),
    ])
}

fn user_key(u: &User) -> Vec<(&'static str, String)> {
    vec![("user_id", u.user_id.as_str().to_owned())]
}

fn user_row(u: &User, plants: &[PlantId]) -> Row {
    let a = &u.attrs;
    let mut cols = vec![
        ("user_id", text::s(u.user_id.as_str())),
        ("name", text::opt(a.name.as_deref())),
        ("dept_code", text::opt(a.dept_code.as_deref())),
        ("class_code", text::s(&a.permission_class)),
        ("menu_program", text::s(&a.menu)),
        ("multi_plant", text::b(a.multi_plant)),
        ("default_plant", text::s(a.default_plant.as_str())),
        ("email", text::opt(a.email.as_deref())),
        ("idp_username", text::opt(a.idp_username.as_deref())),
        ("idp_subject", text::opt(u.idp_subject.as_deref())),
        ("lang", text::s(a.lang.as_str())),
        ("valid_until", text::opt_date(a.valid_until)),
        ("active", text::b(u.active)),
        ("confirmed", text::b(u.confirmation.confirmed)),
        ("confirmed_by", text::opt(u.confirmation.by.as_deref())),
        ("all_plants", text::b(plants.iter().any(PlantId::is_all))),
    ];
    cols.extend(stamp_cols(
        &u.created_by,
        &u.created_dept,
        u.created_on,
        u.updated_by.as_deref(),
        u.updated_on,
        &u.stamp,
    ));
    row(cols)
}

fn class_code_key(c: &PermissionClass) -> Vec<(&'static str, String)> {
    vec![("class_code", c.code.as_str().to_owned())]
}

fn permission_class_row(c: &PermissionClass) -> Row {
    let a = &c.attrs;
    let mut cols = vec![
        ("class_code", text::s(c.code.as_str())),
        ("name", text::s(&a.name)),
        ("default_menu", text::opt(a.default_menu.as_deref())),
        ("idle_control", text::s(a.idle_control.code())),
        ("idle_action", a.idle_action.map(|x| x.code().to_owned())),
        ("idle_seconds", a.idle_seconds.map(|n| n.to_string())),
        ("active", text::b(c.active)),
        ("confirmed", text::b(c.confirmation.confirmed)),
        ("confirmed_by", text::opt(c.confirmation.by.as_deref())),
    ];
    cols.extend(stamp_cols(
        &c.created_by,
        &c.created_dept,
        c.created_on,
        c.updated_by.as_deref(),
        c.updated_on,
        &c.stamp,
    ));
    row(cols)
}

fn plant_key(p: &Plant) -> Vec<(&'static str, String)> {
    vec![("plant_id", p.id.as_str().to_owned())]
}

fn plant_row(p: &Plant) -> Row {
    let a = &p.attrs;
    let mut cols = vec![
        ("plant_id", text::s(p.id.as_str())),
        ("name", text::s(&a.name)),
        ("schema_name", text::s(p.schema.as_str())),
        ("time_zone", text::s(&a.time_zone)),
        ("address", text::opt(a.address.as_deref())),
        ("phone", text::opt(a.phone.as_deref())),
        ("fax", text::opt(a.fax.as_deref())),
        ("active", text::b(p.active)),
        ("confirmed", text::b(p.confirmation.confirmed)),
        ("confirmed_by", text::opt(p.confirmation.by.as_deref())),
    ];
    cols.extend(stamp_cols(
        &p.created_by,
        &p.created_dept,
        p.created_on,
        p.updated_by.as_deref(),
        p.updated_on,
        &p.stamp,
    ));
    row(cols)
}

fn perm_change_key(p: &ProgramPermission) -> Vec<(&'static str, String)> {
    vec![
        ("class_code", p.class.as_str().to_owned()),
        ("program_no", p.program.as_str().to_owned()),
    ]
}

fn perm_row(p: &ProgramPermission) -> Row {
    let a = &p.attrs;
    let mut cols = vec![
        ("class_code", text::s(p.class.as_str())),
        ("program_no", text::s(p.program.as_str())),
        ("actions", text_array(&a.actions)),
        ("other_user_data", text::s(a.other_user.code())),
        ("other_dept_data", text::s(a.other_dept.code())),
        ("detail_rows", text::s(a.detail.code())),
    ];
    cols.extend(stamp_cols(
        &p.created_by,
        &p.created_dept,
        p.created_on,
        p.updated_by.as_deref(),
        p.updated_on,
        &p.stamp,
    ));
    row(cols)
}

fn uperm_key(p: &UserPermission) -> Vec<(&'static str, String)> {
    vec![
        ("line_no", p.line_no.to_string()),
        ("user_id", p.user_id.as_str().to_owned()),
    ]
}

fn uperm_row(p: &UserPermission) -> Row {
    let program = match &p.target {
        SpecialTarget::Program { attrs, .. } => Some(attrs),
        SpecialTarget::Class(_) => None,
    };
    let mut cols = vec![
        ("user_id", text::s(p.user_id.as_str())),
        ("line_no", text::int(i64::from(p.line_no))),
        ("kind", text::s(p.target.kind_code())),
        (
            "class_code",
            p.target.class().map(|c| c.as_str().to_owned()),
        ),
        (
            "program_no",
            p.target.program().map(|n| n.as_str().to_owned()),
        ),
        ("actions", program.and_then(|a| text_array(&a.actions))),
        (
            "other_user_data",
            program.map(|a| a.other_user.code().to_owned()),
        ),
        (
            "other_dept_data",
            program.map(|a| a.other_dept.code().to_owned()),
        ),
        ("detail_rows", program.map(|a| a.detail.code().to_owned())),
    ];
    cols.extend(stamp_cols(
        &p.created_by,
        &p.created_dept,
        p.created_on,
        p.updated_by.as_deref(),
        p.updated_on,
        &p.stamp,
    ));
    row(cols)
}

impl State {
    /// 可使用營運中心（user_plant）：只刪除移除的、新增增加的（與 PostgreSQL 一致）；ALL 記在 user_account.all_plants
    fn replace_user_plants(&mut self, user: &UserId, plants: &[PlantId]) {
        let old = self.user_plants.get(user).cloned().unwrap_or_default();
        let real = |p: &&PlantId| !p.is_all();
        for p in old.iter().filter(real).filter(|p| !plants.contains(p)) {
            let r = row(vec![
                ("user_id", text::s(user.as_str())),
                ("plant_id", text::s(p.as_str())),
            ]);
            let key = vec![
                ("plant_id", p.as_str().to_owned()),
                ("user_id", user.as_str().to_owned()),
            ];
            self.log_change(None, "user_plant", key, Some(r), None, &[]);
        }
        for p in plants.iter().filter(real).filter(|p| !old.contains(p)) {
            let r = row(vec![
                ("user_id", text::s(user.as_str())),
                ("plant_id", text::s(p.as_str())),
            ]);
            let key = vec![
                ("plant_id", p.as_str().to_owned()),
                ("user_id", user.as_str().to_owned()),
            ];
            self.log_change(None, "user_plant", key, None, Some(r), &[]);
        }
        self.user_plants.insert(user.clone(), plants.to_vec());
    }

    /// 單一欄位的使用者更新（綁定公司帳號、預設營運中心）：版本加 1 並記錄
    fn touch_user(&mut self, id: &UserId, change: impl FnOnce(&mut User)) {
        let Some(old) = self.users.get(id).cloned() else {
            return;
        };
        let plants = self.user_plants.get(id).cloned().unwrap_or_default();
        let mut u = old.clone();
        change(&mut u);
        if let Ok(stamp) = bumped(&old.stamp, &old.stamp) {
            u.stamp = stamp;
        }
        self.log_change(
            None,
            "user_account",
            user_key(&u),
            Some(user_row(&old, &plants)),
            Some(user_row(&u, &plants)),
            USER_MASKED,
        );
        self.users.insert(id.clone(), u);
    }
}

// ----- 選單（zm_file；menu_maintenance.feature） -----

#[async_trait]
impl MenuRepository for InMemoryStore {
    async fn all_menu_items(&self) -> Result<Vec<MenuItem>, RepoError> {
        Ok(self.lock().menu_items.clone())
    }

    async fn menu_items(&self, menu: &str) -> Result<Vec<MenuItem>, RepoError> {
        let mut v: Vec<MenuItem> = self
            .lock()
            .menu_items
            .iter()
            .filter(|i| i.menu == menu)
            .cloned()
            .collect();
        v.sort_by_key(|i| i.seq);
        Ok(v)
    }

    async fn insert_menu_item(&self, item: &MenuItem) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.menu_items
            .iter()
            .any(|i| i.menu == item.menu && (i.seq == item.seq || i.program == item.program))
        {
            return Err(RepoError::Conflict);
        }
        s.menu_items.push(item.clone());
        Ok(())
    }

    async fn remove_menu_item(&self, menu: &str, program: &str) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let before = s.menu_items.len();
        s.menu_items
            .retain(|i| !(i.menu == menu && i.program == program));
        Ok(s.menu_items.len() != before)
    }

    async fn set_menu_seqs(&self, menu: &str, items: &[MenuItem]) -> Result<(), RepoError> {
        let mut s = self.lock();
        for i in s.menu_items.iter_mut().filter(|i| i.menu == menu) {
            if let Some(n) = items.iter().find(|n| n.program == i.program) {
                i.seq = n.seq;
            }
        }
        Ok(())
    }

    async fn clear_menu(&self, menu: &str) -> Result<(), RepoError> {
        self.lock().menu_items.retain(|i| i.menu != menu);
        Ok(())
    }
}

// ----- 單位（gfe_file；unit_maintenance.feature） -----

#[async_trait]
impl UnitRepository for InMemoryStore {
    async fn list_units(&self) -> Result<Vec<Unit>, RepoError> {
        Ok(self.lock().units.values().cloned().collect())
    }

    async fn get_unit(&self, code: &UnitCode) -> Result<Option<Unit>, RepoError> {
        Ok(self.lock().units.get(code.as_str()).cloned())
    }

    async fn insert_unit(&self, unit: &Unit) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.units.contains_key(unit.code.as_str()) {
            return Err(RepoError::Conflict);
        }
        let mut u = unit.clone();
        u.stamp = new_stamp();
        s.log_change(None, "unit", unit_key(&u), None, Some(unit_row(&u)), &[]);
        s.units.insert(u.code.as_str().to_owned(), u);
        Ok(())
    }

    async fn update_unit(&self, unit: &Unit) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.units.get(unit.code.as_str()).cloned() else {
            return Ok(());
        };
        let mut u = unit.clone();
        u.stamp = bumped(&old.stamp, &unit.stamp)?;
        s.log_change(
            None,
            "unit",
            unit_key(&u),
            Some(unit_row(&old)),
            Some(unit_row(&u)),
            &[],
        );
        s.units.insert(u.code.as_str().to_owned(), u);
        Ok(())
    }

    async fn delete_unit(&self, code: &UnitCode) -> Result<bool, RepoError> {
        let mut s = self.lock();
        // 與 PostgreSQL 的外鍵一致：料件、料件分群引用時不可刪除（UN5）
        let used = s
            .items
            .values()
            .any(|i| i.attrs.units().any(|u| u == code.as_str()))
            || s.item_groups
                .values()
                .any(|g| g.attrs.units().any(|u| u == code.as_str()));
        if used {
            return Err(RepoError::InUse);
        }
        let Some(old) = s.units.remove(code.as_str()) else {
            return Ok(false);
        };
        s.log_change(
            None,
            "unit",
            unit_key(&old),
            Some(unit_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

fn unit_key(u: &Unit) -> Vec<(&'static str, String)> {
    vec![("unit_code", u.code.as_str().to_owned())]
}

fn unit_row(u: &Unit) -> Row {
    let mut cols = vec![
        ("unit_code", text::s(u.code.as_str())),
        ("name", text::opt(u.attrs.name.as_deref())),
        ("decimals", text::int(i64::from(u.attrs.decimals))),
        ("confirmed", text::b(u.confirmation.confirmed)),
        ("confirmed_by", text::opt(u.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        u.active,
        &u.created_by,
        &u.created_dept,
        u.created_on,
        u.updated_by.as_deref(),
        u.updated_on,
        &u.stamp,
    ));
    row(cols)
}

// ----- 編碼性質（geh_file；encoding_nature.feature） -----

fn nature_key(n: &ttrs_domain::encoding_nature::EncodingNature) -> Vec<(&'static str, String)> {
    vec![("nature_code", n.code.as_str().to_owned())]
}

fn nature_row(n: &ttrs_domain::encoding_nature::EncodingNature) -> Row {
    let mut cols = vec![
        ("nature_code", text::s(n.code.as_str())),
        ("name", text::opt(n.attrs.name.as_deref())),
        ("category", text::s(n.attrs.category.code())),
        ("confirmed", text::b(n.confirmation.confirmed)),
        ("confirmed_by", text::opt(n.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        n.active,
        &n.created_by,
        &n.created_dept,
        n.created_on,
        n.updated_by.as_deref(),
        n.updated_on,
        &n.stamp,
    ));
    row(cols)
}

#[async_trait]
impl crate::ports::EncodingNatureRepository for InMemoryStore {
    async fn list_encoding_natures(
        &self,
    ) -> Result<Vec<ttrs_domain::encoding_nature::EncodingNature>, RepoError> {
        Ok(self.lock().encoding_natures.values().cloned().collect())
    }

    async fn get_encoding_nature(
        &self,
        code: &ttrs_domain::encoding_nature::NatureCode,
    ) -> Result<Option<ttrs_domain::encoding_nature::EncodingNature>, RepoError> {
        Ok(self.lock().encoding_natures.get(code.as_str()).cloned())
    }

    async fn insert_encoding_nature(
        &self,
        nature: &ttrs_domain::encoding_nature::EncodingNature,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.encoding_natures.contains_key(nature.code.as_str()) {
            return Err(RepoError::Conflict);
        }
        let mut n = nature.clone();
        n.stamp = new_stamp();
        s.log_change(
            None,
            "encoding_nature",
            nature_key(&n),
            None,
            Some(nature_row(&n)),
            &[],
        );
        s.encoding_natures.insert(n.code.as_str().to_owned(), n);
        Ok(())
    }

    async fn update_encoding_nature(
        &self,
        nature: &ttrs_domain::encoding_nature::EncodingNature,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.encoding_natures.get(nature.code.as_str()).cloned() else {
            return Ok(());
        };
        let mut n = nature.clone();
        n.stamp = bumped(&old.stamp, &nature.stamp)?;
        s.log_change(
            None,
            "encoding_nature",
            nature_key(&n),
            Some(nature_row(&old)),
            Some(nature_row(&n)),
            &[],
        );
        s.encoding_natures.insert(n.code.as_str().to_owned(), n);
        Ok(())
    }

    async fn encoding_nature_in_use(
        &self,
        code: &ttrs_domain::encoding_nature::NatureCode,
    ) -> Result<bool, RepoError> {
        Ok(self
            .lock()
            .encoding_classes
            .values()
            .any(|c| c.attrs.nature == code.as_str()))
    }

    async fn delete_encoding_nature(
        &self,
        code: &ttrs_domain::encoding_nature::NatureCode,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        // 與 PostgreSQL 的外鍵一致：被編碼類別使用時不可刪除（EN6、EC9）
        if s.encoding_classes
            .values()
            .any(|c| c.attrs.nature == code.as_str())
        {
            return Err(RepoError::InUse);
        }
        let Some(old) = s.encoding_natures.remove(code.as_str()) else {
            return Ok(false);
        };
        s.log_change(
            None,
            "encoding_nature",
            nature_key(&old),
            Some(nature_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

// ----- 編碼獨立段（gej_file、gek_file；encoding_segment.feature） -----

fn segment_key(s: &ttrs_domain::encoding_segment::EncodingSegment) -> Vec<(&'static str, String)> {
    vec![("segment_code", s.code.as_str().to_owned())]
}

fn segment_row(s: &ttrs_domain::encoding_segment::EncodingSegment) -> Row {
    let mut cols = vec![
        ("segment_code", text::s(s.code.as_str())),
        ("name", text::opt(s.attrs.name.as_deref())),
        ("length", text::int(i64::from(s.attrs.length))),
        ("confirmed", text::b(s.confirmation.confirmed)),
        ("confirmed_by", text::opt(s.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        s.active,
        &s.created_by,
        &s.created_dept,
        s.created_on,
        s.updated_by.as_deref(),
        s.updated_on,
        &s.stamp,
    ));
    row(cols)
}

fn segment_value_key(
    code: &str,
    v: &ttrs_domain::encoding_segment::SegmentValue,
) -> Vec<(&'static str, String)> {
    vec![
        ("segment_code", code.to_owned()),
        ("line_no", v.line.to_string()),
    ]
}

fn segment_value_row(code: &str, v: &ttrs_domain::encoding_segment::SegmentValue) -> Row {
    row(vec![
        ("segment_code", text::s(code)),
        ("line_no", text::int(i64::from(v.line))),
        ("code_value", text::s(&v.value)),
        ("name", text::opt(v.name.as_deref())),
    ])
}

impl State {
    /// 單身的異動記錄：只記有變動的列（與 PostgreSQL 觸發器相同）
    fn log_segment_values(
        &mut self,
        code: &str,
        old: &[ttrs_domain::encoding_segment::SegmentValue],
        new: &[ttrs_domain::encoding_segment::SegmentValue],
    ) {
        for o in old.iter().filter(|o| !new.iter().any(|n| n.line == o.line)) {
            self.log_change(
                None,
                "encoding_segment_code",
                segment_value_key(code, o),
                Some(segment_value_row(code, o)),
                None,
                &[],
            );
        }
        for n in new {
            let before = old.iter().find(|o| o.line == n.line);
            if before != Some(n) {
                self.log_change(
                    None,
                    "encoding_segment_code",
                    segment_value_key(code, n),
                    before.map(|b| segment_value_row(code, b)),
                    Some(segment_value_row(code, n)),
                    &[],
                );
            }
        }
    }
}

#[async_trait]
impl crate::ports::EncodingSegmentRepository for InMemoryStore {
    async fn list_encoding_segments(
        &self,
    ) -> Result<Vec<ttrs_domain::encoding_segment::EncodingSegment>, RepoError> {
        Ok(self.lock().encoding_segments.values().cloned().collect())
    }

    async fn get_encoding_segment(
        &self,
        code: &ttrs_domain::encoding_segment::SegmentCode,
    ) -> Result<Option<ttrs_domain::encoding_segment::EncodingSegment>, RepoError> {
        Ok(self.lock().encoding_segments.get(code.as_str()).cloned())
    }

    async fn insert_encoding_segment(
        &self,
        segment: &ttrs_domain::encoding_segment::EncodingSegment,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.encoding_segments.contains_key(segment.code.as_str()) {
            return Err(RepoError::Conflict);
        }
        let mut n = segment.clone();
        n.stamp = new_stamp();
        s.log_change(
            None,
            "encoding_segment",
            segment_key(&n),
            None,
            Some(segment_row(&n)),
            &[],
        );
        s.log_segment_values(n.code.as_str(), &[], &n.values);
        s.encoding_segments.insert(n.code.as_str().to_owned(), n);
        Ok(())
    }

    async fn update_encoding_segment(
        &self,
        segment: &ttrs_domain::encoding_segment::EncodingSegment,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.encoding_segments.get(segment.code.as_str()).cloned() else {
            return Ok(());
        };
        let mut n = segment.clone();
        n.stamp = bumped(&old.stamp, &segment.stamp)?;
        s.log_change(
            None,
            "encoding_segment",
            segment_key(&n),
            Some(segment_row(&old)),
            Some(segment_row(&n)),
            &[],
        );
        s.log_segment_values(n.code.as_str(), &old.values, &n.values);
        s.encoding_segments.insert(n.code.as_str().to_owned(), n);
        Ok(())
    }

    async fn encoding_segment_in_use(
        &self,
        code: &ttrs_domain::encoding_segment::SegmentCode,
    ) -> Result<bool, RepoError> {
        Ok(segment_used(&self.lock(), code.as_str()))
    }

    async fn delete_encoding_segment(
        &self,
        code: &ttrs_domain::encoding_segment::SegmentCode,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        // 與 PostgreSQL 的外鍵一致：被編碼類別的獨立段使用時不可刪除（IS6、EC9）
        if segment_used(&s, code.as_str()) {
            return Err(RepoError::InUse);
        }
        let Some(old) = s.encoding_segments.remove(code.as_str()) else {
            return Ok(false);
        };
        s.log_segment_values(old.code.as_str(), &old.values, &[]);
        s.log_change(
            None,
            "encoding_segment",
            segment_key(&old),
            Some(segment_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

/// 獨立段是否被編碼類別的型態 2 使用
fn segment_used(s: &State, code: &str) -> bool {
    s.encoding_classes.values().any(|c| {
        c.segments.iter().any(|g| {
            g.kind == ttrs_domain::encoding_class::SegmentType::Independent
                && g.value.as_deref() == Some(code)
        })
    })
}

// ----- 編碼類別（gei_file、gel_file；encoding_class.feature） -----

fn enc_class_key(c: &ttrs_domain::encoding_class::EncodingClass) -> Vec<(&'static str, String)> {
    vec![("class_code", c.code.as_str().to_owned())]
}

fn enc_class_row(c: &ttrs_domain::encoding_class::EncodingClass) -> Row {
    let mut cols = vec![
        ("class_code", text::s(c.code.as_str())),
        ("name", text::s(&c.attrs.name)),
        ("nature_code", text::s(&c.attrs.nature)),
        ("confirmed", text::b(c.confirmation.confirmed)),
        ("confirmed_by", text::opt(c.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        c.active,
        &c.created_by,
        &c.created_dept,
        c.created_on,
        c.updated_by.as_deref(),
        c.updated_on,
        &c.stamp,
    ));
    row(cols)
}

fn class_segment_key(
    code: &str,
    g: &ttrs_domain::encoding_class::ClassSegment,
) -> Vec<(&'static str, String)> {
    vec![
        ("class_code", code.to_owned()),
        ("seq_no", g.no.to_string()),
    ]
}

/// 單身一段的欄位：固定值與獨立段分兩欄（獨立段有外鍵）
fn class_segment_row(code: &str, g: &ttrs_domain::encoding_class::ClassSegment) -> Row {
    let independent = g.kind == ttrs_domain::encoding_class::SegmentType::Independent;
    row(vec![
        ("class_code", text::s(code)),
        ("seq_no", text::int(i64::from(g.no))),
        ("description", text::opt(g.description.as_deref())),
        ("length", text::int(i64::from(g.length))),
        ("segment_type", text::s(g.kind.code())),
        (
            "fixed_value",
            text::opt(g.value.as_deref().filter(|_| !independent)),
        ),
        (
            "independent_segment",
            text::opt(g.value.as_deref().filter(|_| independent)),
        ),
        ("show_name", text::b(g.show_name)),
    ])
}

impl State {
    /// 單身的異動記錄：只記有變動的段（與 PostgreSQL 觸發器相同）
    fn log_class_segments(
        &mut self,
        code: &str,
        old: &[ttrs_domain::encoding_class::ClassSegment],
        new: &[ttrs_domain::encoding_class::ClassSegment],
    ) {
        for o in old.iter().filter(|o| !new.iter().any(|n| n.no == o.no)) {
            self.log_change(
                None,
                "encoding_class_segment",
                class_segment_key(code, o),
                Some(class_segment_row(code, o)),
                None,
                &[],
            );
        }
        for n in new {
            let before = old.iter().find(|o| o.no == n.no);
            if before != Some(n) {
                self.log_change(
                    None,
                    "encoding_class_segment",
                    class_segment_key(code, n),
                    before.map(|b| class_segment_row(code, b)),
                    Some(class_segment_row(code, n)),
                    &[],
                );
            }
        }
    }
}

#[async_trait]
impl crate::ports::EncodingClassRepository for InMemoryStore {
    async fn list_encoding_classes(
        &self,
    ) -> Result<Vec<ttrs_domain::encoding_class::EncodingClass>, RepoError> {
        Ok(self.lock().encoding_classes.values().cloned().collect())
    }

    async fn get_encoding_class(
        &self,
        code: &ttrs_domain::encoding_class::ClassCode,
    ) -> Result<Option<ttrs_domain::encoding_class::EncodingClass>, RepoError> {
        Ok(self.lock().encoding_classes.get(code.as_str()).cloned())
    }

    async fn insert_encoding_class(
        &self,
        class: &ttrs_domain::encoding_class::EncodingClass,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.encoding_classes.contains_key(class.code.as_str()) {
            return Err(RepoError::Conflict);
        }
        let mut n = class.clone();
        n.stamp = new_stamp();
        s.log_change(
            None,
            "encoding_class",
            enc_class_key(&n),
            None,
            Some(enc_class_row(&n)),
            &[],
        );
        s.log_class_segments(n.code.as_str(), &[], &n.segments);
        s.encoding_classes.insert(n.code.as_str().to_owned(), n);
        Ok(())
    }

    async fn update_encoding_class(
        &self,
        class: &ttrs_domain::encoding_class::EncodingClass,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.encoding_classes.get(class.code.as_str()).cloned() else {
            return Ok(());
        };
        let mut n = class.clone();
        n.stamp = bumped(&old.stamp, &class.stamp)?;
        s.log_change(
            None,
            "encoding_class",
            enc_class_key(&n),
            Some(enc_class_row(&old)),
            Some(enc_class_row(&n)),
            &[],
        );
        s.log_class_segments(n.code.as_str(), &old.segments, &n.segments);
        s.encoding_classes.insert(n.code.as_str().to_owned(), n);
        Ok(())
    }

    async fn delete_encoding_class(
        &self,
        code: &ttrs_domain::encoding_class::ClassCode,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s.encoding_classes.remove(code.as_str()) else {
            return Ok(false);
        };
        s.log_class_segments(old.code.as_str(), &old.segments, &[]);
        s.log_change(
            None,
            "encoding_class",
            enc_class_key(&old),
            Some(enc_class_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

// ----- 編碼年對照（gff_file；encoding_year.feature） -----

fn year_code_key(y: &ttrs_domain::encoding_year::EncodingYear) -> Vec<(&'static str, String)> {
    vec![
        ("year", y.year.to_string()),
        ("length", y.length.to_string()),
    ]
}

fn year_code_row(y: &ttrs_domain::encoding_year::EncodingYear) -> Row {
    let mut cols = vec![
        ("year", text::int(i64::from(y.year))),
        ("length", text::int(i64::from(y.length))),
        ("code_value", text::s(&y.code_value)),
        ("confirmed", text::b(y.confirmation.confirmed)),
        ("confirmed_by", text::opt(y.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        y.active,
        &y.created_by,
        &y.created_dept,
        y.created_on,
        y.updated_by.as_deref(),
        y.updated_on,
        &y.stamp,
    ));
    row(cols)
}

#[async_trait]
impl crate::ports::EncodingYearRepository for InMemoryStore {
    async fn list_encoding_years(
        &self,
    ) -> Result<Vec<ttrs_domain::encoding_year::EncodingYear>, RepoError> {
        Ok(self.lock().encoding_years.values().cloned().collect())
    }

    async fn get_encoding_year(
        &self,
        year: i32,
        length: i16,
    ) -> Result<Option<ttrs_domain::encoding_year::EncodingYear>, RepoError> {
        Ok(self.lock().encoding_years.get(&(year, length)).cloned())
    }

    async fn insert_encoding_year(
        &self,
        y: &ttrs_domain::encoding_year::EncodingYear,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.encoding_years.contains_key(&(y.year, y.length)) {
            return Err(RepoError::Conflict);
        }
        let mut n = y.clone();
        n.stamp = new_stamp();
        s.log_change(
            None,
            "encoding_year_code",
            year_code_key(&n),
            None,
            Some(year_code_row(&n)),
            &[],
        );
        s.encoding_years.insert((n.year, n.length), n);
        Ok(())
    }

    async fn update_encoding_year(
        &self,
        y: &ttrs_domain::encoding_year::EncodingYear,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.encoding_years.get(&(y.year, y.length)).cloned() else {
            return Ok(());
        };
        let mut n = y.clone();
        n.stamp = bumped(&old.stamp, &y.stamp)?;
        s.log_change(
            None,
            "encoding_year_code",
            year_code_key(&n),
            Some(year_code_row(&old)),
            Some(year_code_row(&n)),
            &[],
        );
        s.encoding_years.insert((n.year, n.length), n);
        Ok(())
    }

    async fn delete_encoding_year(&self, year: i32, length: i16) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s.encoding_years.remove(&(year, length)) else {
            return Ok(false);
        };
        s.log_change(
            None,
            "encoding_year_code",
            year_code_key(&old),
            Some(year_code_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

// ----- 專案（gja_file；project_maintenance.feature；工廠層） -----

#[async_trait]
impl ProjectRepository for InMemoryStore {
    async fn list_projects(&self, plant: &PlantId) -> Result<Vec<Project>, RepoError> {
        Ok(self
            .lock()
            .projects
            .iter()
            .filter(|((p, _), _)| p == plant)
            .map(|(_, x)| x.clone())
            .collect())
    }

    async fn get_project(
        &self,
        plant: &PlantId,
        code: &ProjectCode,
    ) -> Result<Option<Project>, RepoError> {
        Ok(self
            .lock()
            .projects
            .get(&(plant.clone(), code.as_str().to_owned()))
            .cloned())
    }

    async fn insert_project(&self, plant: &PlantId, x: &Project) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.plants.contains_key(plant) {
            return Err(RepoError::Other(format!(
                "營運中心不存在：{}",
                plant.as_str()
            )));
        }
        let key = (plant.clone(), x.code.as_str().to_owned());
        if s.projects.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut x = x.clone();
        x.stamp = new_stamp();
        s.log_change(
            Some(plant),
            "project",
            project_key(&x),
            None,
            Some(project_row(&x)),
            &[],
        );
        s.projects.insert(key, x);
        Ok(())
    }

    async fn update_project(&self, plant: &PlantId, x: &Project) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (plant.clone(), x.code.as_str().to_owned());
        let Some(old) = s.projects.get(&key).cloned() else {
            return Ok(());
        };
        let mut new = x.clone();
        new.stamp = bumped(&old.stamp, &x.stamp)?;
        s.log_change(
            Some(plant),
            "project",
            project_key(&new),
            Some(project_row(&old)),
            Some(project_row(&new)),
            &[],
        );
        s.projects.insert(key, new);
        Ok(())
    }

    async fn delete_project(&self, plant: &PlantId, code: &ProjectCode) -> Result<bool, RepoError> {
        let mut s = self.lock();
        // 引用專案的單據移植後再加入檢查（PJ4）
        let Some(old) = s
            .projects
            .remove(&(plant.clone(), code.as_str().to_owned()))
        else {
            return Ok(false);
        };
        s.log_change(
            Some(plant),
            "project",
            project_key(&old),
            Some(project_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

fn project_key(x: &Project) -> Vec<(&'static str, String)> {
    vec![("project_code", x.code.as_str().to_owned())]
}

fn project_row(x: &Project) -> Row {
    let a = &x.attrs;
    let mut cols = vec![
        ("project_code", text::s(x.code.as_str())),
        ("name", text::opt(a.name.as_deref())),
        ("start_on", text::opt_date(a.start_on)),
        ("end_on", text::opt_date(a.end_on)),
        ("confirmed", text::b(x.confirmation.confirmed)),
        ("confirmed_by", text::opt(x.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        x.active,
        &x.created_by,
        &x.created_dept,
        x.created_on,
        x.updated_by.as_deref(),
        x.updated_on,
        &x.stamp,
    ));
    row(cols)
}

// ----- 稅別（gec_file；tax_type_maintenance.feature） -----

#[async_trait]
impl TaxTypeRepository for InMemoryStore {
    async fn list_tax_types(&self) -> Result<Vec<TaxType>, RepoError> {
        Ok(self.lock().tax_types.values().cloned().collect())
    }

    async fn get_tax_type(&self, key: &TaxKey) -> Result<Option<TaxType>, RepoError> {
        Ok(self.lock().tax_types.get(key).cloned())
    }

    async fn insert_tax_type(&self, t: &TaxType) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.tax_types.contains_key(&t.key) {
            return Err(RepoError::Conflict);
        }
        let mut t = t.clone();
        t.stamp = new_stamp();
        s.log_change(None, "tax_type", tax_key(&t), None, Some(tax_row(&t)), &[]);
        s.tax_types.insert(t.key.clone(), t);
        Ok(())
    }

    async fn update_tax_type(&self, t: &TaxType) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.tax_types.get(&t.key).cloned() else {
            return Ok(());
        };
        let mut t = t.clone();
        t.stamp = bumped(&old.stamp, &t.stamp)?;
        s.log_change(
            None,
            "tax_type",
            tax_key(&t),
            Some(tax_row(&old)),
            Some(tax_row(&t)),
            &[],
        );
        s.tax_types.insert(t.key.clone(), t);
        Ok(())
    }

    async fn delete_tax_type(&self, key: &TaxKey) -> Result<bool, RepoError> {
        let mut s = self.lock();
        // 引用稅別的作業（客戶、供應商、單據）移植後再加入檢查（TX7）
        let Some(old) = s.tax_types.remove(key) else {
            return Ok(false);
        };
        s.log_change(
            None,
            "tax_type",
            tax_key(&old),
            Some(tax_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

fn tax_key(t: &TaxType) -> Vec<(&'static str, String)> {
    vec![
        ("tax_code", t.key.code.as_str().to_owned()),
        ("direction", t.key.direction.code().to_owned()),
    ]
}

fn tax_row(t: &TaxType) -> Row {
    let a = &t.attrs;
    let mut cols = vec![
        ("tax_code", text::s(t.key.code.as_str())),
        ("direction", text::s(t.key.direction.code())),
        ("name", text::opt(a.name.as_deref())),
        ("account", text::opt(a.account.as_deref())),
        ("account2", text::opt(a.account2.as_deref())),
        (
            "invoice_copies",
            a.invoice_copies.map(|c| c.code().to_owned()),
        ),
        ("tax_class", a.tax_class.map(|c| c.code().to_owned())),
        ("rate", Some(a.rate.to_string())),
        ("include_tax", text::b(a.include_tax)),
        ("filing_format", text::s(&a.filing_format)),
        ("confirmed", text::b(t.confirmation.confirmed)),
        ("confirmed_by", text::opt(t.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        t.active,
        &t.created_by,
        &t.created_dept,
        t.created_on,
        t.updated_by.as_deref(),
        t.updated_on,
        &t.stamp,
    ));
    row(cols)
}

// ----- 幣別（azi_file；currency_maintenance.feature） -----

#[async_trait]
impl CurrencyRepository for InMemoryStore {
    async fn list_currencies(&self) -> Result<Vec<Currency>, RepoError> {
        Ok(self.lock().currencies.values().cloned().collect())
    }

    async fn get_currency(&self, code: &CurrencyCode) -> Result<Option<Currency>, RepoError> {
        Ok(self.lock().currencies.get(code.as_str()).cloned())
    }

    async fn insert_currency(&self, c: &Currency) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.currencies.contains_key(c.code.as_str()) {
            return Err(RepoError::Conflict);
        }
        let mut c = c.clone();
        c.stamp = new_stamp();
        s.log_change(
            None,
            "currency",
            currency_key(&c),
            None,
            Some(currency_row(&c)),
            &[],
        );
        s.currencies.insert(c.code.as_str().to_owned(), c);
        Ok(())
    }

    async fn update_currency(&self, c: &Currency) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.currencies.get(c.code.as_str()).cloned() else {
            return Ok(());
        };
        let mut c = c.clone();
        c.stamp = bumped(&old.stamp, &c.stamp)?;
        s.log_change(
            None,
            "currency",
            currency_key(&c),
            Some(currency_row(&old)),
            Some(currency_row(&c)),
            &[],
        );
        s.currencies.insert(c.code.as_str().to_owned(), c);
        Ok(())
    }

    async fn delete_currency(&self, code: &CurrencyCode) -> Result<bool, RepoError> {
        let mut s = self.lock();
        // 已被匯率引用時不可刪除（CY6；同 PostgreSQL 外鍵）
        let used = s.monthly_rates.keys().any(|(c, _)| c == code.as_str())
            || s.daily_rates.keys().any(|(c, _)| c == code.as_str());
        if used {
            return Err(RepoError::InUse);
        }
        let Some(old) = s.currencies.remove(code.as_str()) else {
            return Ok(false);
        };
        s.log_change(
            None,
            "currency",
            currency_key(&old),
            Some(currency_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

fn currency_key(c: &Currency) -> Vec<(&'static str, String)> {
    vec![("currency_code", c.code.as_str().to_owned())]
}

fn currency_row(c: &Currency) -> Row {
    let a = &c.attrs;
    let mut cols = vec![
        ("currency_code", text::s(c.code.as_str())),
        ("name", text::opt(a.name.as_deref())),
        ("price_decimals", text::int(i64::from(a.price_decimals))),
        ("amount_decimals", text::int(i64::from(a.amount_decimals))),
        ("total_decimals", text::int(i64::from(a.total_decimals))),
        ("rate_decimals", text::int(i64::from(a.rate_decimals))),
        ("rate_method", text::s(a.rate_method.code())),
        ("import_code", text::opt(a.import_code.as_deref())),
        ("confirmed", text::b(c.confirmation.confirmed)),
        ("confirmed_by", text::opt(c.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        c.active,
        &c.created_by,
        &c.created_dept,
        c.created_on,
        c.updated_by.as_deref(),
        c.updated_on,
        &c.stamp,
    ));
    row(cols)
}

// ----- 匯率（azj_file、azk_file；exchange_rate_service.rs） -----

#[async_trait]
impl ExchangeRateRepository for InMemoryStore {
    async fn list_monthly_rates(&self) -> Result<Vec<MonthlyRate>, RepoError> {
        Ok(self.lock().monthly_rates.values().cloned().collect())
    }

    async fn get_monthly_rate(
        &self,
        currency: &CurrencyCode,
        period: RatePeriod,
    ) -> Result<Option<MonthlyRate>, RepoError> {
        Ok(self
            .lock()
            .monthly_rates
            .get(&(currency.as_str().to_owned(), period))
            .cloned())
    }

    async fn insert_monthly_rate(&self, r: &MonthlyRate) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (r.currency.as_str().to_owned(), r.period);
        if s.monthly_rates.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut r = r.clone();
        r.record.stamp = new_stamp();
        s.log_change(
            None,
            "monthly_exchange_rate",
            monthly_key(&r),
            None,
            Some(monthly_row(&r)),
            &[],
        );
        s.monthly_rates.insert(key, r);
        Ok(())
    }

    async fn update_monthly_rate(&self, r: &MonthlyRate) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (r.currency.as_str().to_owned(), r.period);
        let Some(old) = s.monthly_rates.get(&key).cloned() else {
            return Ok(());
        };
        let mut r = r.clone();
        r.record.stamp = bumped(&old.record.stamp, &r.record.stamp)?;
        s.log_change(
            None,
            "monthly_exchange_rate",
            monthly_key(&r),
            Some(monthly_row(&old)),
            Some(monthly_row(&r)),
            &[],
        );
        s.monthly_rates.insert(key, r);
        Ok(())
    }

    async fn delete_monthly_rate(
        &self,
        currency: &CurrencyCode,
        period: RatePeriod,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s
            .monthly_rates
            .remove(&(currency.as_str().to_owned(), period))
        else {
            return Ok(false);
        };
        s.log_change(
            None,
            "monthly_exchange_rate",
            monthly_key(&old),
            Some(monthly_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }

    async fn list_daily_rates(
        &self,
        currency: Option<&CurrencyCode>,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
    ) -> Result<Vec<DailyRate>, RepoError> {
        Ok(self
            .lock()
            .daily_rates
            .values()
            .filter(|r| currency.is_none_or(|c| c == &r.currency))
            .filter(|r| from.is_none_or(|d| r.date >= d))
            .filter(|r| to.is_none_or(|d| r.date <= d))
            .cloned()
            .collect())
    }

    async fn get_daily_rate(
        &self,
        currency: &CurrencyCode,
        date: NaiveDate,
    ) -> Result<Option<DailyRate>, RepoError> {
        Ok(self
            .lock()
            .daily_rates
            .get(&(currency.as_str().to_owned(), date))
            .cloned())
    }

    async fn insert_daily_rate(&self, r: &DailyRate) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (r.currency.as_str().to_owned(), r.date);
        if s.daily_rates.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut r = r.clone();
        r.record.stamp = new_stamp();
        s.log_change(
            None,
            "daily_exchange_rate",
            daily_key(&r),
            None,
            Some(daily_row(&r)),
            &[],
        );
        s.daily_rates.insert(key, r);
        Ok(())
    }

    async fn update_daily_rate(&self, r: &DailyRate) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (r.currency.as_str().to_owned(), r.date);
        let Some(old) = s.daily_rates.get(&key).cloned() else {
            return Ok(());
        };
        let mut r = r.clone();
        r.record.stamp = bumped(&old.record.stamp, &r.record.stamp)?;
        s.log_change(
            None,
            "daily_exchange_rate",
            daily_key(&r),
            Some(daily_row(&old)),
            Some(daily_row(&r)),
            &[],
        );
        s.daily_rates.insert(key, r);
        Ok(())
    }

    async fn delete_daily_rate(
        &self,
        currency: &CurrencyCode,
        date: NaiveDate,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s.daily_rates.remove(&(currency.as_str().to_owned(), date)) else {
            return Ok(false);
        };
        s.log_change(
            None,
            "daily_exchange_rate",
            daily_key(&old),
            Some(daily_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }

    async fn has_any_rate(&self) -> Result<bool, RepoError> {
        let s = self.lock();
        Ok(!s.monthly_rates.is_empty() || !s.daily_rates.is_empty())
    }
}

#[async_trait]
impl SystemParameterRepository for InMemoryStore {
    async fn base_currency(&self) -> Result<Option<String>, RepoError> {
        Ok(Some(
            self.lock().general().base_currency.as_str().to_owned(),
        ))
    }

    async fn set_base_currency(&self, code: &str) -> Result<(), RepoError> {
        let code = CurrencyCode::parse(code).map_err(|e| RepoError::Other(e.to_string()))?;
        let mut s = self.lock();
        let mut p = s.general();
        p.base_currency = code;
        s.general = Some(p);
        Ok(())
    }

    async fn general_parameters(&self) -> Result<Option<GeneralParameters>, RepoError> {
        Ok(Some(self.lock().general()))
    }

    async fn update_general_parameters(&self, p: &GeneralParameters) -> Result<(), RepoError> {
        let mut s = self.lock();
        let old = s.general();
        let mut p = p.clone();
        p.stamp = bumped(&old.stamp, &p.stamp)?;
        s.log_change(
            None,
            "system_parameter",
            vec![("parameter_key", "0".to_owned())],
            Some(general_row(&old)),
            Some(general_row(&p)),
            &[],
        );
        s.general = Some(p);
        Ok(())
    }
}

impl State {
    /// 目前的整體管理系統參數；沒有時同 migration 0045／0047 的起始資料
    fn general(&self) -> GeneralParameters {
        self.general
            .clone()
            .unwrap_or_else(|| GeneralParameters::defaults("tiptop", "", Utc::now().date_naive()))
    }
}

/// 異動記錄的欄位（同 system_parameter 表的欄位）
fn general_row(p: &GeneralParameters) -> Row {
    row(vec![
        ("parameter_key", text::s("0")),
        (
            "accounting_periods",
            text::int(i64::from(p.accounting_periods.get())),
        ),
        ("base_currency", text::s(p.base_currency.as_str())),
        ("rate_basis", text::s(p.rate_basis.code())),
        ("auto_item_number", text::b(p.auto_item_number)),
        ("created_by", text::s(&p.created_by)),
        ("created_dept", text::s(&p.created_dept)),
        ("created_on", text::date(p.created_on)),
        ("updated_by", text::opt(p.updated_by.as_deref())),
        ("updated_on", text::opt_date(p.updated_on)),
        ("version", text::int(i64::from(p.stamp.version))),
    ])
}

impl InMemoryStore {
    /// BDD：台銀牌告報價；None 表示無法取得
    pub fn set_bank_quotes(&self, quotes: Option<Vec<Quote>>) {
        self.lock().bank_quotes = quotes;
    }

    /// BDD：關務署當旬檔；None 表示無法取得
    pub fn set_customs_current(&self, current: Option<(NaiveDate, NaiveDate, Vec<Quote>)>) {
        self.lock().customs_current = current;
    }

    /// BDD：關務署歷史檔（加到既有資料後面）
    pub fn add_customs_history(&self, lines: Vec<HistoryQuote>) {
        self.lock()
            .customs_history
            .get_or_insert_with(Vec::new)
            .extend(lines);
    }
}

fn feed_unavailable(what: &str) -> ttrs_domain::DomainError {
    ttrs_domain::DomainError::ExchangeRateSourceUnavailable(
        what.to_owned(),
        "記憶體資料未設定".to_owned(),
    )
}

#[async_trait]
impl RateFeed for InMemoryStore {
    async fn bank_quotes(
        &self,
        _kind: BankRateType,
    ) -> Result<Vec<Quote>, ttrs_domain::DomainError> {
        self.lock()
            .bank_quotes
            .clone()
            .ok_or_else(|| feed_unavailable("台銀牌告匯率"))
    }

    async fn customs_current(
        &self,
    ) -> Result<(NaiveDate, NaiveDate, Vec<Quote>), ttrs_domain::DomainError> {
        self.lock()
            .customs_current
            .clone()
            .ok_or_else(|| feed_unavailable("關務署當旬報關匯率"))
    }

    async fn customs_history(&self) -> Result<Vec<HistoryQuote>, ttrs_domain::DomainError> {
        self.lock()
            .customs_history
            .clone()
            .ok_or_else(|| feed_unavailable("關務署歷史報關匯率"))
    }
}

fn monthly_key(r: &MonthlyRate) -> Vec<(&'static str, String)> {
    vec![
        ("currency_code", r.currency.as_str().to_owned()),
        ("period", r.period.to_string()),
    ]
}

fn daily_key(r: &DailyRate) -> Vec<(&'static str, String)> {
    vec![
        ("currency_code", r.currency.as_str().to_owned()),
        ("rate_date", r.date.to_string()),
    ]
}

fn rate_cols(r: &Rates) -> Vec<(&'static str, Option<String>)> {
    let d = |v: rust_decimal::Decimal| text::s(&v.normalize().to_string());
    vec![
        ("bank_buy", d(r.bank_buy)),
        ("bank_sell", d(r.bank_sell)),
        ("bank_mid", d(r.bank_mid)),
        ("customs_period", d(r.customs_period)),
        ("customs_buy", d(r.customs_buy)),
        ("customs_sell", d(r.customs_sell)),
    ]
}

fn record_cols(x: &RateRecord) -> Vec<(&'static str, Option<String>)> {
    let mut cols = vec![
        ("confirmed", text::b(x.confirmation.confirmed)),
        ("confirmed_by", text::opt(x.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        x.active,
        &x.created_by,
        &x.created_dept,
        x.created_on,
        x.updated_by.as_deref(),
        x.updated_on,
        &x.stamp,
    ));
    cols
}

fn monthly_row(r: &MonthlyRate) -> Row {
    let mut cols = vec![
        ("currency_code", text::s(r.currency.as_str())),
        ("period", text::s(&r.period.to_string())),
    ];
    cols.extend(rate_cols(&r.rates));
    cols.push((
        "revaluation",
        text::s(&r.revaluation.normalize().to_string()),
    ));
    cols.extend(record_cols(&r.record));
    row(cols)
}

fn daily_row(r: &DailyRate) -> Row {
    let mut cols = vec![
        ("currency_code", text::s(r.currency.as_str())),
        ("rate_date", text::date(r.date)),
    ];
    cols.extend(rate_cols(&r.rates));
    cols.extend(record_cols(&r.record));
    row(cols)
}

// ----- 基本代碼（gea、geb、geo、ged、azh；basic_code_service.rs） -----

#[async_trait]
impl BasicCodeRepository for InMemoryStore {
    async fn list_basic_codes(&self, kind: BasicCodeKind) -> Result<Vec<BasicCode>, RepoError> {
        Ok(self
            .lock()
            .basic_codes
            .range((kind, String::new())..)
            .take_while(|((k, _), _)| *k == kind)
            .map(|(_, c)| c.clone())
            .collect())
    }

    async fn get_basic_code(
        &self,
        kind: BasicCodeKind,
        code: &BasicCodeValue,
    ) -> Result<Option<BasicCode>, RepoError> {
        Ok(self
            .lock()
            .basic_codes
            .get(&(kind, code.as_str().to_owned()))
            .cloned())
    }

    async fn insert_basic_code(&self, code: &BasicCode) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (code.kind, code.code.as_str().to_owned());
        if s.basic_codes.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        // 與 PostgreSQL 的外鍵一致：隸屬的上層代碼須存在
        if let (Some(pk), Some(p)) = (code.kind.parent(), &code.attrs.parent)
            && !s.basic_codes.contains_key(&(pk, p.clone()))
        {
            return Err(RepoError::Other(format!("上層代碼不存在：{p}")));
        }
        let mut c = code.clone();
        c.stamp = new_stamp();
        let (table, ..) = crate::basic_code_service::storage(c.kind);
        s.log_change(None, table, basic_key(&c), None, Some(basic_row(&c)), &[]);
        s.basic_codes.insert(key, c);
        Ok(())
    }

    async fn update_basic_code(&self, code: &BasicCode) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (code.kind, code.code.as_str().to_owned());
        let Some(old) = s.basic_codes.get(&key).cloned() else {
            return Ok(());
        };
        let mut c = code.clone();
        c.stamp = bumped(&old.stamp, &code.stamp)?;
        let (table, ..) = crate::basic_code_service::storage(c.kind);
        s.log_change(
            None,
            table,
            basic_key(&c),
            Some(basic_row(&old)),
            Some(basic_row(&c)),
            &[],
        );
        s.basic_codes.insert(key, c);
        Ok(())
    }

    async fn delete_basic_code(
        &self,
        kind: BasicCodeKind,
        code: &BasicCodeValue,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        // 與 PostgreSQL 的外鍵一致：被下層代碼引用時不可刪除（RC4）
        if let Some(child) = kind.child() {
            let used = s
                .basic_codes
                .iter()
                .any(|((k, _), c)| *k == child && c.attrs.parent.as_deref() == Some(code.as_str()));
            if used {
                return Err(RepoError::InUse);
            }
        }
        let Some(old) = s.basic_codes.remove(&(kind, code.as_str().to_owned())) else {
            return Ok(false);
        };
        let (table, ..) = crate::basic_code_service::storage(kind);
        s.log_change(
            None,
            table,
            basic_key(&old),
            Some(basic_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

fn basic_key(c: &BasicCode) -> Vec<(&'static str, String)> {
    let (_, key, _) = crate::basic_code_service::storage(c.kind);
    vec![(key, c.code.as_str().to_owned())]
}

fn basic_row(c: &BasicCode) -> Row {
    let (_, key, parent) = crate::basic_code_service::storage(c.kind);
    let mut cols = vec![
        (key, text::s(c.code.as_str())),
        ("name", text::opt(c.attrs.name.as_deref())),
        ("confirmed", text::b(c.confirmation.confirmed)),
        ("confirmed_by", text::opt(c.confirmation.by.as_deref())),
    ];
    if let Some(parent) = parent {
        cols.push((parent, text::opt(c.attrs.parent.as_deref())));
    }
    cols.extend(audit_cols(
        c.active,
        &c.created_by,
        &c.created_dept,
        c.created_on,
        c.updated_by.as_deref(),
        c.updated_on,
        &c.stamp,
    ));
    row(cols)
}

// ----- 產品分類（oba_file；product_classification.feature） -----

#[async_trait]
impl ProductClassRepository for InMemoryStore {
    async fn list_product_classes(&self, plant: &PlantId) -> Result<Vec<ProductClass>, RepoError> {
        Ok(self
            .lock()
            .product_classes
            .iter()
            .filter(|((p, _), _)| p == plant)
            .map(|(_, c)| c.clone())
            .collect())
    }

    async fn get_product_class(
        &self,
        plant: &PlantId,
        code: &ProductClassCode,
    ) -> Result<Option<ProductClass>, RepoError> {
        Ok(self
            .lock()
            .product_classes
            .get(&(plant.clone(), code.as_str().to_owned()))
            .cloned())
    }

    async fn insert_product_class(
        &self,
        plant: &PlantId,
        c: &ProductClass,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.plants.contains_key(plant) {
            return Err(RepoError::Other(format!(
                "營運中心不存在：{}",
                plant.as_str()
            )));
        }
        let key = (plant.clone(), c.code.as_str().to_owned());
        if s.product_classes.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut c = c.clone();
        c.stamp = new_stamp();
        s.log_change(
            Some(plant),
            "product_class",
            pclass_key(&c),
            None,
            Some(pclass_row(&c)),
            &[],
        );
        s.product_classes.insert(key, c);
        Ok(())
    }

    async fn update_product_class(
        &self,
        plant: &PlantId,
        c: &ProductClass,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (plant.clone(), c.code.as_str().to_owned());
        let Some(old) = s.product_classes.get(&key).cloned() else {
            return Ok(());
        };
        let mut new = c.clone();
        new.stamp = bumped(&old.stamp, &c.stamp)?;
        s.log_change(
            Some(plant),
            "product_class",
            pclass_key(&new),
            Some(pclass_row(&old)),
            Some(pclass_row(&new)),
            &[],
        );
        s.product_classes.insert(key, new);
        Ok(())
    }

    async fn delete_product_class(
        &self,
        plant: &PlantId,
        code: &ProductClassCode,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        // 與 PostgreSQL 的外鍵一致：料件、料件分群的產品分類引用時不可刪除（PC8）
        let used = s.items.iter().any(|((p, _), i)| {
            p == plant && i.attrs.product_class.as_deref() == Some(code.as_str())
        }) || s.item_groups.iter().any(|((p, _), g)| {
            p == plant && g.attrs.product_class.as_deref() == Some(code.as_str())
        });
        if used {
            return Err(RepoError::InUse);
        }
        let Some(old) = s
            .product_classes
            .remove(&(plant.clone(), code.as_str().to_owned()))
        else {
            return Ok(false);
        };
        s.log_change(
            Some(plant),
            "product_class",
            pclass_key(&old),
            Some(pclass_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

fn pclass_key(c: &ProductClass) -> Vec<(&'static str, String)> {
    vec![("class_code", c.code.as_str().to_owned())]
}

fn pclass_row(c: &ProductClass) -> Row {
    let a = &c.attrs;
    let mut cols = vec![
        ("class_code", text::s(c.code.as_str())),
        ("name", text::opt(a.name.as_deref())),
        ("period_format", text::opt(a.period_format.as_deref())),
        ("periods", a.periods.map(|p| p.to_string())),
        (
            "cost_center",
            a.cost_center.as_ref().map(|d| d.as_str().to_owned()),
        ),
        ("revenue_account", text::opt(a.revenue_account.as_deref())),
        ("revenue_account2", text::opt(a.revenue_account2.as_deref())),
        ("confirmed", text::b(c.confirmation.confirmed)),
        ("confirmed_by", text::opt(c.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        c.active,
        &c.created_by,
        &c.created_dept,
        c.created_on,
        c.updated_by.as_deref(),
        c.updated_on,
        &c.stamp,
    ));
    row(cols)
}

// ----- 料件分群（imz_file；item_group.feature） -----

#[async_trait]
impl ItemGroupRepository for InMemoryStore {
    async fn list_item_groups(&self, plant: &PlantId) -> Result<Vec<ItemGroup>, RepoError> {
        Ok(self
            .lock()
            .item_groups
            .iter()
            .filter(|((p, _), _)| p == plant)
            .map(|(_, g)| g.clone())
            .collect())
    }

    async fn get_item_group(
        &self,
        plant: &PlantId,
        code: &ItemGroupCode,
    ) -> Result<Option<ItemGroup>, RepoError> {
        Ok(self
            .lock()
            .item_groups
            .get(&(plant.clone(), code.as_str().to_owned()))
            .cloned())
    }

    async fn query_item_groups(
        &self,
        plant: &PlantId,
        filters: &[QbeFilter],
        scope: &DataScope,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<ItemGroup>, u64), RepoError> {
        let hits = self
            .lock()
            .item_groups
            .iter()
            .filter(|((p, _), g)| {
                p == plant
                    && scope.includes(&g.created_by, &g.created_dept)
                    && query::matches_all(filters, |f| query::item_group::value(g, f))
            })
            .map(|(_, g)| g.clone())
            .collect();
        Ok(page_of(hits, offset, limit))
    }

    async fn insert_item_group(&self, plant: &PlantId, g: &ItemGroup) -> Result<(), RepoError> {
        let mut s = self.lock();
        if !s.plants.contains_key(plant) {
            return Err(RepoError::Other(format!(
                "營運中心不存在：{}",
                plant.as_str()
            )));
        }
        let key = (plant.clone(), g.code.as_str().to_owned());
        if s.item_groups.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut g = g.clone();
        g.stamp = new_stamp();
        s.log_change(
            Some(plant),
            "item_group",
            group_key(&g),
            None,
            Some(group_row(&g)),
            &[],
        );
        s.item_groups.insert(key, g);
        Ok(())
    }

    async fn update_item_group(&self, plant: &PlantId, g: &ItemGroup) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (plant.clone(), g.code.as_str().to_owned());
        let Some(old) = s.item_groups.get(&key).cloned() else {
            return Ok(());
        };
        let mut new = g.clone();
        new.stamp = bumped(&old.stamp, &g.stamp)?;
        s.log_change(
            Some(plant),
            "item_group",
            group_key(&new),
            Some(group_row(&old)),
            Some(group_row(&new)),
            &[],
        );
        s.item_groups.insert(key, new);
        Ok(())
    }

    async fn delete_item_group(
        &self,
        plant: &PlantId,
        code: &ItemGroupCode,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        // 與 PostgreSQL 的外鍵一致：料件的分群碼（ima06）引用時不可刪除（IG7）
        if s.items
            .iter()
            .any(|((p, _), i)| p == plant && i.attrs.group_code.as_deref() == Some(code.as_str()))
        {
            return Err(RepoError::InUse);
        }
        let Some(old) = s
            .item_groups
            .remove(&(plant.clone(), code.as_str().to_owned()))
        else {
            return Ok(false);
        };
        s.log_change(
            Some(plant),
            "item_group",
            group_key(&old),
            Some(group_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

fn item_key(i: &Item) -> Vec<(&'static str, String)> {
    vec![("item_no", i.no.as_str().to_owned())]
}

/// 料件的主要欄位（其餘欄位只在 PostgreSQL 的異動記錄中）
fn item_row(i: &Item) -> Row {
    let a = &i.attrs;
    let mut cols = vec![
        ("item_no", text::s(i.no.as_str())),
        ("name", text::opt(a.name.as_deref())),
        ("spec", text::opt(a.spec.as_deref())),
        ("group_code", text::opt(a.group_code.as_deref())),
        ("source_code", text::opt(a.source_code.as_deref())),
        ("stock_unit", text::opt(a.stock_unit.as_deref())),
        ("product_class", text::opt(a.product_class.as_deref())),
        ("safety_stock", a.safety_stock.map(|d| d.to_string())),
        ("active_code", text::s(i.active_code.code())),
        ("status_code", text::s(i.status_code.code())),
        ("order_invalid", Some(i.order_invalid.to_string())),
    ];
    cols.extend(audit_cols(
        i.active(),
        &i.created_by,
        &i.created_dept,
        i.created_on,
        i.updated_by.as_deref(),
        i.updated_on,
        &i.stamp,
    ));
    row(cols)
}

fn group_key(g: &ItemGroup) -> Vec<(&'static str, String)> {
    vec![("group_code", g.code.as_str().to_owned())]
}

/// 料件分群的主要欄位（其餘欄位只在 PostgreSQL 的異動記錄中）
fn group_row(g: &ItemGroup) -> Row {
    let a = &g.attrs;
    let mut cols = vec![
        ("group_code", text::s(g.code.as_str())),
        ("description", text::opt(a.description.as_deref())),
        ("source_code", text::opt(a.source_code.as_deref())),
        ("stock_unit", text::opt(a.stock_unit.as_deref())),
        ("abc_code", text::opt(a.abc_code.as_deref())),
        ("replenish_policy", text::opt(a.replenish_policy.as_deref())),
        ("safety_stock", a.safety_stock.map(|d| d.to_string())),
        ("stock_keeper", text::opt(a.stock_keeper.as_deref())),
        ("confirmed", Some(g.confirmation.confirmed.to_string())),
        ("confirmed_by", text::opt(g.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        g.active,
        &g.created_by,
        &g.created_dept,
        g.created_on,
        g.updated_by.as_deref(),
        g.updated_on,
        &g.stamp,
    ));
    row(cols)
}

// ----- 會計期間（azm_file；accounting_period_service.rs） -----

#[async_trait]
impl AccountingPeriodRepository for InMemoryStore {
    async fn list_accounting_years(&self) -> Result<Vec<AccountingYear>, RepoError> {
        Ok(self.lock().accounting_years.values().cloned().collect())
    }

    async fn get_accounting_year(&self, year: i32) -> Result<Option<AccountingYear>, RepoError> {
        Ok(self.lock().accounting_years.get(&year).cloned())
    }

    async fn insert_accounting_year(&self, y: &AccountingYear) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.accounting_years.contains_key(&y.year) {
            return Err(RepoError::Conflict);
        }
        let mut y = y.clone();
        y.stamp = new_stamp();
        s.log_change(
            None,
            "accounting_year",
            year_key(y.year),
            None,
            Some(year_row(&y)),
            &[],
        );
        for p in &y.periods {
            s.log_change(
                None,
                "accounting_period",
                period_key(y.year, p),
                None,
                Some(period_row(y.year, p)),
                &[],
            );
        }
        s.accounting_years.insert(y.year, y);
        Ok(())
    }

    async fn update_accounting_year(&self, y: &AccountingYear) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.accounting_years.get(&y.year).cloned() else {
            return Ok(());
        };
        let mut y = y.clone();
        y.stamp = bumped(&old.stamp, &y.stamp)?;
        s.log_change(
            None,
            "accounting_year",
            year_key(y.year),
            Some(year_row(&old)),
            Some(year_row(&y)),
            &[],
        );
        for p in &y.periods {
            let before = old.periods.iter().find(|o| o.no == p.no);
            if before != Some(p) {
                s.log_change(
                    None,
                    "accounting_period",
                    period_key(y.year, p),
                    before.map(|b| period_row(y.year, b)),
                    Some(period_row(y.year, p)),
                    &[],
                );
            }
        }
        s.accounting_years.insert(y.year, y);
        Ok(())
    }

    async fn delete_accounting_year(&self, year: i32) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s.accounting_years.remove(&year) else {
            return Ok(false);
        };
        for p in &old.periods {
            s.log_change(
                None,
                "accounting_period",
                period_key(year, p),
                Some(period_row(year, p)),
                None,
                &[],
            );
        }
        s.log_change(
            None,
            "accounting_year",
            year_key(year),
            Some(year_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

fn year_key(year: i32) -> Vec<(&'static str, String)> {
    vec![("year", year.to_string())]
}

fn period_key(year: i32, p: &Period) -> Vec<(&'static str, String)> {
    vec![("year", year.to_string()), ("period_no", p.no.to_string())]
}

fn year_row(y: &AccountingYear) -> Row {
    let mut cols = vec![
        ("year", text::int(i64::from(y.year))),
        ("period_count", text::int(i64::from(y.period_count))),
        ("confirmed", text::b(y.confirmation.confirmed)),
        ("confirmed_by", text::opt(y.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        true,
        &y.created_by,
        &y.created_dept,
        y.created_on,
        y.updated_by.as_deref(),
        y.updated_on,
        &y.stamp,
    ));
    // 會計年度沒有資料有效碼（PD9）
    cols.retain(|(k, _)| *k != "active");
    row(cols)
}

fn period_row(year: i32, p: &Period) -> Row {
    row(vec![
        ("year", text::int(i64::from(year))),
        ("period_no", text::int(i64::from(p.no))),
        ("start_date", text::date(p.start)),
        ("end_date", text::date(p.end)),
        ("quarter", text::int(i64::from(p.quarter))),
    ])
}

// ----- 單位換算（smc_file、smd_file；unit_conversion_service.rs） -----

fn conv_cols(c: &Conversion) -> Vec<(&'static str, Option<String>)> {
    vec![
        ("from_unit", text::s(c.from_unit.as_str())),
        ("to_unit", text::s(c.to_unit.as_str())),
        (
            "from_quantity",
            Some(c.from_quantity.normalize().to_string()),
        ),
        ("to_quantity", Some(c.to_quantity.normalize().to_string())),
        ("note", text::opt(c.note.as_deref())),
    ]
}

fn unit_conv_key(c: &Conversion) -> Vec<(&'static str, String)> {
    vec![
        ("from_unit", c.from_unit.as_str().to_owned()),
        ("to_unit", c.to_unit.as_str().to_owned()),
    ]
}

fn unit_conv_row(c: &UnitConversion) -> Row {
    let mut cols = conv_cols(&c.conversion);
    cols.push(("confirmed", text::b(c.confirmation.confirmed)));
    cols.push(("confirmed_by", text::opt(c.confirmation.by.as_deref())));
    cols.extend(audit_cols(
        c.active,
        &c.created_by,
        &c.created_dept,
        c.created_on,
        c.updated_by.as_deref(),
        c.updated_on,
        &c.stamp,
    ));
    row(cols)
}

fn item_conv_key(c: &ItemUnitConversion) -> Vec<(&'static str, String)> {
    let mut k = vec![("item_no", c.item.as_str().to_owned())];
    k.extend(unit_conv_key(&c.conversion));
    k
}

fn item_conv_row(c: &ItemUnitConversion) -> Row {
    let mut cols = vec![("item_no", text::s(c.item.as_str()))];
    cols.extend(conv_cols(&c.conversion));
    cols.extend(audit_cols(
        c.active,
        &c.created_by,
        &c.created_dept,
        c.created_on,
        c.updated_by.as_deref(),
        c.updated_on,
        &c.stamp,
    ));
    row(cols)
}

fn pair(c: &Conversion) -> (String, String) {
    (
        c.from_unit.as_str().to_owned(),
        c.to_unit.as_str().to_owned(),
    )
}

#[async_trait]
impl UnitConversionRepository for InMemoryStore {
    async fn list_unit_conversions(&self) -> Result<Vec<UnitConversion>, RepoError> {
        Ok(self.lock().unit_conversions.values().cloned().collect())
    }

    async fn insert_unit_conversion(&self, c: &UnitConversion) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = pair(&c.conversion);
        if s.unit_conversions.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut c = c.clone();
        c.stamp = new_stamp();
        s.log_change(
            None,
            "unit_conversion",
            unit_conv_key(&c.conversion),
            None,
            Some(unit_conv_row(&c)),
            &[],
        );
        s.unit_conversions.insert(key, c);
        Ok(())
    }

    async fn update_unit_conversion(&self, c: &UnitConversion) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = pair(&c.conversion);
        let Some(old) = s.unit_conversions.get(&key).cloned() else {
            return Ok(());
        };
        let mut c = c.clone();
        c.stamp = bumped(&old.stamp, &c.stamp)?;
        s.log_change(
            None,
            "unit_conversion",
            unit_conv_key(&c.conversion),
            Some(unit_conv_row(&old)),
            Some(unit_conv_row(&c)),
            &[],
        );
        s.unit_conversions.insert(key, c);
        Ok(())
    }

    async fn delete_unit_conversion(
        &self,
        from: &UnitCode,
        to: &UnitCode,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s
            .unit_conversions
            .remove(&(from.as_str().to_owned(), to.as_str().to_owned()))
        else {
            return Ok(false);
        };
        s.log_change(
            None,
            "unit_conversion",
            unit_conv_key(&old.conversion),
            Some(unit_conv_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

#[async_trait]
impl ItemUnitConversionRepository for InMemoryStore {
    async fn list_item_unit_conversions(
        &self,
        plant: &PlantId,
        item: Option<&ItemNo>,
    ) -> Result<Vec<ItemUnitConversion>, RepoError> {
        Ok(self
            .lock()
            .item_unit_conversions
            .iter()
            .filter(|((p, i, _, _), _)| p == plant && item.is_none_or(|x| x.as_str() == i))
            .map(|(_, c)| c.clone())
            .collect())
    }

    async fn insert_item_unit_conversion(
        &self,
        plant: &PlantId,
        c: &ItemUnitConversion,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let (f, t) = pair(&c.conversion);
        let key = (plant.clone(), c.item.as_str().to_owned(), f, t);
        if s.item_unit_conversions.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut c = c.clone();
        c.stamp = new_stamp();
        s.log_change(
            Some(plant),
            "item_unit_conversion",
            item_conv_key(&c),
            None,
            Some(item_conv_row(&c)),
            &[],
        );
        s.item_unit_conversions.insert(key, c);
        Ok(())
    }

    async fn update_item_unit_conversion(
        &self,
        plant: &PlantId,
        c: &ItemUnitConversion,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let (f, t) = pair(&c.conversion);
        let key = (plant.clone(), c.item.as_str().to_owned(), f, t);
        let Some(old) = s.item_unit_conversions.get(&key).cloned() else {
            return Ok(());
        };
        let mut c = c.clone();
        c.stamp = bumped(&old.stamp, &c.stamp)?;
        s.log_change(
            Some(plant),
            "item_unit_conversion",
            item_conv_key(&c),
            Some(item_conv_row(&old)),
            Some(item_conv_row(&c)),
            &[],
        );
        s.item_unit_conversions.insert(key, c);
        Ok(())
    }

    async fn delete_item_unit_conversion(
        &self,
        plant: &PlantId,
        item: &ItemNo,
        from: &UnitCode,
        to: &UnitCode,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let key = (
            plant.clone(),
            item.as_str().to_owned(),
            from.as_str().to_owned(),
            to.as_str().to_owned(),
        );
        let Some(old) = s.item_unit_conversions.remove(&key) else {
            return Ok(false);
        };
        s.log_change(
            Some(plant),
            "item_unit_conversion",
            item_conv_key(&old),
            Some(item_conv_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

// ----- GitHub 假來源（github_epub_search.feature GE8） -----

impl InMemoryStore {
    /// BDD：GitHub 上的 repo
    pub fn add_github_repo(&self, repo: GithubRepo) {
        self.lock().github_repos.push(repo);
    }

    /// BDD：repo 的一個檔案（路徑、大小、內容）
    pub fn add_github_file(&self, repo: &str, path: &str, size: u64, content: Vec<u8>) {
        let mut s = self.lock();
        let files = s.github_files.entry(repo.to_owned()).or_default();
        files.retain(|(p, _, _)| p != path);
        files.push((path.to_owned(), size, content));
    }

    /// BDD：查詢次數用完
    pub fn set_github_rate_limited(&self, v: bool) {
        self.lock().github_rate_limited = v;
    }

    /// BDD：是否設定 GitHub token（GF2）
    pub fn set_github_token(&self, v: bool) {
        self.lock().github_token = v;
    }

    /// BDD：程式碼搜尋被限流（GF4）
    pub fn set_github_code_limited(&self, v: bool) {
        self.lock().github_code_limited = v;
    }
}

fn github_check(s: &State) -> Result<(), ttrs_domain::DomainError> {
    if s.github_rate_limited {
        return Err(ttrs_domain::DomainError::GithubRateLimited {
            reset: "2026-11-12 12:00".to_owned(),
        });
    }
    Ok(())
}

#[async_trait]
impl GithubSource for InMemoryStore {
    /// 假來源不比對關鍵字：依星數由多到少回傳全部
    async fn search_repos(
        &self,
        _keyword: &str,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<GithubRepo>, u64), ttrs_domain::DomainError> {
        let s = self.lock();
        github_check(&s)?;
        let mut all = s.github_repos.clone();
        all.sort_by_key(|a| std::cmp::Reverse(a.stars));
        let total = all.len() as u64;
        let skip = usize::try_from((page.max(1) - 1) * per_page).unwrap_or(usize::MAX);
        Ok((
            all.into_iter()
                .skip(skip)
                .take(usize::try_from(per_page).unwrap_or(usize::MAX))
                .collect(),
            total,
        ))
    }

    async fn get_repo(&self, full_name: &str) -> Result<GithubRepo, ttrs_domain::DomainError> {
        let s = self.lock();
        github_check(&s)?;
        s.github_repos
            .iter()
            .find(|r| r.full_name == full_name)
            .cloned()
            .ok_or_else(|| ttrs_domain::DomainError::GithubNotFound(full_name.to_owned()))
    }

    async fn tree(
        &self,
        full_name: &str,
        _branch: &str,
    ) -> Result<(Vec<GithubFile>, bool), ttrs_domain::DomainError> {
        let s = self.lock();
        github_check(&s)?;
        Ok((
            s.github_files
                .get(full_name)
                .map(|fs| {
                    fs.iter()
                        .map(|(path, size, _)| GithubFile {
                            path: path.clone(),
                            size: *size,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            false,
        ))
    }

    async fn download(
        &self,
        full_name: &str,
        _branch: &str,
        path: &str,
    ) -> Result<Vec<u8>, ttrs_domain::DomainError> {
        let s = self.lock();
        github_check(&s)?;
        s.github_files
            .get(full_name)
            .and_then(|fs| fs.iter().find(|(p, _, _)| p == path))
            .map(|(_, _, c)| c.clone())
            .ok_or_else(|| ttrs_domain::DomainError::GithubNotFound(format!("{full_name}/{path}")))
    }

    fn has_token(&self) -> bool {
        self.lock().github_token
    }

    /// 假的程式碼搜尋：檔名（不含資料夾）包含 `name` 的 .epub 檔
    async fn search_epub_files(
        &self,
        name: &str,
    ) -> Result<Vec<crate::ports::GithubCodeHit>, ttrs_domain::DomainError> {
        let s = self.lock();
        if s.github_code_limited {
            return Err(ttrs_domain::DomainError::GithubRateLimited {
                reset: "2026-11-16 12:00".to_owned(),
            });
        }
        let mut out = Vec::new();
        for (repo, files) in &s.github_files {
            for (path, _, _) in files {
                let file = path.rsplit('/').next().unwrap_or(path);
                if file.to_lowercase().ends_with(".epub") && file.contains(name) {
                    out.push(crate::ports::GithubCodeHit {
                        full_name: repo.clone(),
                        path: path.clone(),
                        git_ref: "main".to_owned(),
                    });
                }
            }
        }
        Ok(out)
    }
}

// ----- EPUB 英文翻譯（epub_translate.feature） -----

use ttrs_domain::DomainError;

use crate::ports::{EpubBook, EpubSource, HttpHop, WebClient};

fn epub_library_row(e: &crate::ports::EpubLibraryEntry) -> Row {
    row(vec![
        ("user_id", text::s(&e.user_id)),
        ("repo", text::s(&e.repo)),
        ("version", text::int(i64::from(e.version))),
        ("created_at", Some(e.created_at.to_rfc3339())),
        ("updated_at", None),
    ])
}

#[async_trait]
impl crate::ports::EpubLibraryRepository for InMemoryStore {
    async fn list_epub_library(
        &self,
        user_id: &str,
    ) -> Result<Vec<crate::ports::EpubLibraryEntry>, RepoError> {
        let mut v: Vec<_> = self
            .lock()
            .epub_library
            .iter()
            .filter(|e| e.user_id == user_id)
            .cloned()
            .collect();
        v.sort_by(|a, b| (a.created_at, &a.repo).cmp(&(b.created_at, &b.repo)));
        Ok(v)
    }

    async fn insert_epub_library(&self, user_id: &str, repo: &str) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.epub_library
            .iter()
            .any(|e| e.user_id == user_id && e.repo == repo)
        {
            return Err(RepoError::Conflict);
        }
        let e = crate::ports::EpubLibraryEntry {
            user_id: user_id.to_owned(),
            repo: repo.to_owned(),
            created_at: Utc::now(),
            version: 1,
        };
        s.log_change(
            None,
            "epub_github_library",
            vec![("user_id", e.user_id.clone()), ("repo", e.repo.clone())],
            None,
            Some(epub_library_row(&e)),
            &[],
        );
        s.epub_library.push(e);
        Ok(())
    }

    async fn delete_epub_library(
        &self,
        user_id: &str,
        repo: &str,
        version: i32,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(i) = s
            .epub_library
            .iter()
            .position(|e| e.user_id == user_id && e.repo == repo)
        else {
            return Ok(false);
        };
        if s.epub_library[i].version != version {
            return Err(RepoError::Stale);
        }
        let e = s.epub_library.remove(i);
        s.log_change(
            None,
            "epub_github_library",
            vec![("user_id", e.user_id.clone()), ("repo", e.repo.clone())],
            Some(epub_library_row(&e)),
            None,
            &[],
        );
        Ok(true)
    }
}

fn epub_shared_row(e: &crate::ports::EpubSharedLibraryEntry) -> Row {
    row(vec![
        ("repo", text::s(&e.repo)),
        ("created_by", text::s(&e.created_by)),
        ("created_dept", text::s(&e.created_dept)),
        ("version", text::int(i64::from(e.version))),
        ("created_at", Some(e.created_at.to_rfc3339())),
        ("updated_at", None),
    ])
}

#[async_trait]
impl crate::ports::EpubSharedLibraryRepository for InMemoryStore {
    async fn list_epub_shared_library(
        &self,
    ) -> Result<Vec<crate::ports::EpubSharedLibraryEntry>, RepoError> {
        let mut v = self.lock().epub_shared_library.clone();
        v.sort_by(|a, b| (a.created_at, &a.repo).cmp(&(b.created_at, &b.repo)));
        Ok(v)
    }

    async fn insert_epub_shared_library(
        &self,
        repo: &str,
        created_by: &str,
        created_dept: &str,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.epub_shared_library.iter().any(|e| e.repo == repo) {
            return Err(RepoError::Conflict);
        }
        let e = crate::ports::EpubSharedLibraryEntry {
            repo: repo.to_owned(),
            created_by: created_by.to_owned(),
            created_dept: created_dept.to_owned(),
            created_at: Utc::now(),
            version: 1,
        };
        s.log_change(
            None,
            "epub_github_shared_library",
            vec![("repo", e.repo.clone())],
            None,
            Some(epub_shared_row(&e)),
            &[],
        );
        s.epub_shared_library.push(e);
        Ok(())
    }

    async fn delete_epub_shared_library(
        &self,
        repo: &str,
        version: i32,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(i) = s.epub_shared_library.iter().position(|e| e.repo == repo) else {
            return Ok(false);
        };
        if s.epub_shared_library[i].version != version {
            return Err(RepoError::Stale);
        }
        let e = s.epub_shared_library.remove(i);
        s.log_change(
            None,
            "epub_github_shared_library",
            vec![("repo", e.repo.clone())],
            Some(epub_shared_row(&e)),
            None,
            &[],
        );
        Ok(true)
    }
}

#[async_trait]
impl TranslationJobRepository for InMemoryStore {
    async fn create(&self, job: &JobRecord) -> Result<(), RepoError> {
        self.lock()
            .translation_jobs
            .insert(job.job_id.clone(), job.clone());
        Ok(())
    }

    async fn get(&self, job_id: &str) -> Result<Option<JobRecord>, RepoError> {
        Ok(self.lock().translation_jobs.get(job_id).cloned())
    }

    async fn save(&self, job: &JobRecord) -> Result<(), RepoError> {
        self.lock()
            .translation_jobs
            .insert(job.job_id.clone(), job.clone());
        Ok(())
    }

    async fn find_resumable(
        &self,
        user_id: &str,
        source_sha256: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<JobRecord>, RepoError> {
        Ok(self
            .lock()
            .translation_jobs
            .values()
            .filter(|j| {
                j.user_id == user_id
                    && j.source_sha256 == source_sha256
                    && j.status == "failed"
                    && j.expires_at > now
            })
            .max_by_key(|j| j.created_at)
            .cloned())
    }

    async fn has_active(&self, since: DateTime<Utc>) -> Result<bool, RepoError> {
        Ok(self
            .lock()
            .translation_jobs
            .values()
            .any(|j| matches!(j.status.as_str(), "queued" | "running") && j.updated_at > since))
    }

    async fn purge_expired(&self, now: DateTime<Utc>) -> Result<u64, RepoError> {
        let mut st = self.lock();
        let before = st.translation_jobs.len();
        st.translation_jobs.retain(|_, j| j.expires_at > now);
        Ok((before - st.translation_jobs.len()) as u64)
    }
}

/// 假翻譯引擎（BDD；不連外網）：把每一段加上「【譯】」前綴，可設定失敗、額度用完等情境，並記錄每次呼叫
#[derive(Clone, Default)]
pub struct FakeTranslator(Arc<Mutex<FakeTranslatorState>>);

/// 假引擎收到的一次呼叫
#[derive(Debug, Clone)]
pub struct TranslateCall {
    /// 這次送出的段落總字元數
    pub chars: usize,
    /// 這次帶入的人名地名對照表
    pub glossary: Vec<(String, String)>,
}

#[derive(Default)]
struct FakeTranslatorState {
    calls: Vec<TranslateCall>,
    /// 前幾次呼叫失敗
    fail_first: usize,
    always_fail: bool,
    /// 第幾次呼叫回報訂閱用量已達上限（從 1 起算）
    quota_at: Option<usize>,
    not_configured: bool,
    /// 每批回報的人名
    name: Option<(String, String)>,
}

impl FakeTranslator {
    fn state(&self) -> std::sync::MutexGuard<'_, FakeTranslatorState> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn set_fail_first(&self, n: usize) {
        self.state().fail_first = n;
    }

    pub fn set_always_fail(&self) {
        self.state().always_fail = true;
    }

    pub fn set_quota_at(&self, call: Option<usize>) {
        self.state().quota_at = call;
    }

    pub fn set_not_configured(&self) {
        self.state().not_configured = true;
    }

    pub fn set_name(&self, english: &str, chinese: &str) {
        self.state().name = Some((english.to_owned(), chinese.to_owned()));
    }

    pub fn calls(&self) -> Vec<TranslateCall> {
        self.state().calls.clone()
    }

    /// 清掉呼叫紀錄與情境（模擬引擎恢復）
    pub fn recover(&self) {
        let mut s = self.state();
        s.quota_at = None;
        s.fail_first = 0;
        s.always_fail = false;
    }
}

#[async_trait]
impl TranslationEngine for FakeTranslator {
    fn name(&self) -> String {
        "fake".to_owned()
    }

    async fn check(&self) -> Result<(), DomainError> {
        if self.state().not_configured {
            return Err(DomainError::TranslateNotConfigured(
                "假引擎未設定".to_owned(),
            ));
        }
        Ok(())
    }

    async fn translate(&self, req: &TranslateRequest) -> Result<TranslateResponse, DomainError> {
        self.check().await?;
        let mut s = self.state();
        s.calls.push(TranslateCall {
            chars: req.lines.iter().map(|l| l.chars().count()).sum(),
            glossary: req.glossary.clone(),
        });
        let n = s.calls.len();
        if s.always_fail || n <= s.fail_first {
            return Err(DomainError::TranslateFailed("假引擎失敗".to_owned()));
        }
        if s.quota_at == Some(n) {
            return Err(DomainError::TranslateQuotaExhausted {
                reset: "稍後".to_owned(),
            });
        }
        Ok(TranslateResponse {
            lines: req.lines.iter().map(|l| format!("【譯】{l}")).collect(),
            names: s.name.clone().into_iter().collect(),
            input_tokens: 10,
            output_tokens: 20,
        })
    }
}

// ----- EPUB 多來源搜尋的假來源（epub_web_search.feature；BDD，不連外網） -----

/// 假來源：BDD 依表格放入書，可設定失敗與檔案大小
#[derive(Clone)]
pub struct FakeEpubSource(Arc<Mutex<FakeSourceState>>);

struct FakeSourceState {
    name: String,
    /// (書, 標籤, EPUB 位元組, 檔案大小)
    books: Vec<(EpubBook, String, Vec<u8>, u64)>,
    failing: bool,
}

impl FakeEpubSource {
    pub fn new(name: &str) -> Self {
        Self(Arc::new(Mutex::new(FakeSourceState {
            name: name.to_owned(),
            books: Vec::new(),
            failing: false,
        })))
    }

    fn state(&self) -> std::sync::MutexGuard<'_, FakeSourceState> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn add_book(&self, book: EpubBook, tag: &str, bytes: Vec<u8>) {
        let size = bytes.len() as u64;
        self.state().books.push((book, tag.to_owned(), bytes, size));
    }

    pub fn set_failing(&self, failing: bool) {
        self.state().failing = failing;
    }

    /// 設定某本書回報的檔案大小（不必真的放那麼大的資料）
    pub fn set_size(&self, title: &str, size: u64) {
        for b in &mut self.state().books {
            if b.0.title == title {
                b.3 = size;
            }
        }
    }
}

#[async_trait]
impl EpubSource for FakeEpubSource {
    fn name(&self) -> String {
        self.state().name.clone()
    }

    async fn search(
        &self,
        keywords: &[String],
        _open_only: bool,
        limit: usize,
    ) -> Result<Vec<EpubBook>, DomainError> {
        let s = self.state();
        if s.failing {
            return Err(DomainError::EpubSourceUnavailable(s.name.clone()));
        }
        Ok(s.books
            .iter()
            .filter(|(b, tag, _, _)| {
                keywords
                    .iter()
                    .any(|k| b.title.contains(k.as_str()) || tag.contains(k.as_str()))
            })
            .map(|(b, ..)| b.clone())
            .take(limit)
            .collect())
    }

    async fn download(&self, id: &str, max_bytes: u64) -> Result<Vec<u8>, DomainError> {
        let s = self.state();
        let (_, _, bytes, size) = s
            .books
            .iter()
            .find(|(b, ..)| b.id == id)
            .ok_or_else(|| DomainError::EpubBookNotFound(id.to_owned()))?;
        if *size > max_bytes {
            return Err(DomainError::EpubTooLarge {
                max_mb: usize::try_from(max_bytes / 1024 / 1024).unwrap_or(0),
            });
        }
        Ok(bytes.clone())
    }
}

// ----- 以網址下載 EPUB 的假網路（epub_url_download.feature；BDD，不連外網、位址解析也是假的） -----

#[derive(Clone, Default)]
pub struct FakeWeb(Arc<Mutex<FakeWebState>>);

#[derive(Default)]
struct FakeWebState {
    pages: BTreeMap<String, FakePage>,
    dns: BTreeMap<String, Vec<std::net::IpAddr>>,
    /// 實際發出的請求（網址，連到的位址）
    requests: Vec<(String, std::net::IpAddr)>,
}

enum FakePage {
    Body {
        content_type: String,
        body: Vec<u8>,
        /// 假裝的內容大小（測試大檔，不必真的放那麼大的資料）
        declared: Option<u64>,
    },
    Redirect(String),
}

impl FakeWeb {
    fn state(&self) -> std::sync::MutexGuard<'_, FakeWebState> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn add_body(&self, url: &str, content_type: &str, body: Vec<u8>) {
        self.state().pages.insert(
            url.to_owned(),
            FakePage::Body {
                content_type: content_type.to_owned(),
                body,
                declared: None,
            },
        );
    }

    pub fn add_epub(&self, url: &str, bytes: Vec<u8>) {
        self.add_body(url, "application/epub+zip", bytes);
    }

    pub fn add_html(&self, url: &str, html: &str) {
        self.add_body(url, "text/html; charset=utf-8", html.as_bytes().to_vec());
    }

    pub fn add_redirect(&self, url: &str, to: &str) {
        self.state()
            .pages
            .insert(url.to_owned(), FakePage::Redirect(to.to_owned()));
    }

    /// 內容有 `size` 位元組（只記大小）
    pub fn add_sized(&self, url: &str, size: u64) {
        self.state().pages.insert(
            url.to_owned(),
            FakePage::Body {
                content_type: "application/epub+zip".to_owned(),
                body: b"PK".to_vec(),
                declared: Some(size),
            },
        );
    }

    pub fn set_dns(&self, host: &str, ip: &str) {
        if let Ok(ip) = ip.parse() {
            self.state().dns.insert(host.to_owned(), vec![ip]);
        }
    }

    pub fn requests(&self) -> Vec<(String, std::net::IpAddr)> {
        self.state().requests.clone()
    }
}

#[async_trait]
impl WebClient for FakeWeb {
    async fn resolve(&self, host: &str) -> Result<Vec<std::net::IpAddr>, DomainError> {
        Ok(self
            .state()
            .dns
            .get(host)
            .cloned()
            // 沒有設定的網域一律解析到一個公開位址
            .unwrap_or_else(|| vec![std::net::IpAddr::from([93, 184, 216, 34])]))
    }

    async fn get(
        &self,
        url: &str,
        ip: std::net::IpAddr,
        max_bytes: u64,
    ) -> Result<HttpHop, DomainError> {
        let mut s = self.state();
        s.requests.push((url.to_owned(), ip));
        match s.pages.get(url) {
            None => Ok(HttpHop {
                status: 404,
                ..HttpHop::default()
            }),
            Some(FakePage::Redirect(to)) => Ok(HttpHop {
                status: 302,
                location: Some(to.clone()),
                ..HttpHop::default()
            }),
            Some(FakePage::Body {
                content_type,
                body,
                declared,
            }) => {
                let size = declared.unwrap_or(body.len() as u64);
                if size > max_bytes {
                    return Err(DomainError::EpubTooLarge {
                        max_mb: usize::try_from(max_bytes / 1024 / 1024).unwrap_or(0),
                    });
                }
                Ok(HttpHop {
                    status: 200,
                    location: None,
                    content_type: Some(content_type.clone()),
                    body: body.clone(),
                })
            }
        }
    }
}

// ----- 集團中心（geu_file、gev_file；group_center.feature） -----

fn group_center_key(c: &ttrs_domain::group_center::GroupCenter) -> Vec<(&'static str, String)> {
    vec![("center_code", c.code.as_str().to_owned())]
}

fn group_center_row(c: &ttrs_domain::group_center::GroupCenter) -> Row {
    let mut cols = vec![
        ("center_code", text::s(c.code.as_str())),
        ("name", text::opt(c.name.as_deref())),
        ("category", text::s(&c.category.code())),
        ("confirmed", text::b(c.confirmation.confirmed)),
        ("confirmed_by", text::opt(c.confirmation.by.as_deref())),
    ];
    cols.extend(audit_cols(
        c.active,
        &c.created_by,
        &c.created_dept,
        c.created_on,
        c.updated_by.as_deref(),
        c.updated_on,
        &c.stamp,
    ));
    row(cols)
}

fn data_center_type_key(
    code: &str,
    t: &ttrs_domain::group_center::DataCenterType,
) -> Vec<(&'static str, String)> {
    vec![
        ("center_code", code.to_owned()),
        ("data_type", t.data_type.code()),
        ("plant_id", t.plant.clone()),
    ]
}

fn data_center_type_row(code: &str, t: &ttrs_domain::group_center::DataCenterType) -> Row {
    row(vec![
        ("center_code", text::s(code)),
        ("data_type", text::s(&t.data_type.code())),
        ("plant_id", text::s(&t.plant)),
    ])
}

#[async_trait]
impl crate::ports::GroupCenterRepository for InMemoryStore {
    async fn list_group_centers(
        &self,
    ) -> Result<Vec<ttrs_domain::group_center::GroupCenter>, RepoError> {
        Ok(self.lock().group_centers.values().cloned().collect())
    }

    async fn get_group_center(
        &self,
        code: &ttrs_domain::group_center::GroupCenterCode,
    ) -> Result<Option<ttrs_domain::group_center::GroupCenter>, RepoError> {
        Ok(self.lock().group_centers.get(code.as_str()).cloned())
    }

    async fn insert_group_center(
        &self,
        center: &ttrs_domain::group_center::GroupCenter,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        if s.group_centers.contains_key(center.code.as_str()) {
            return Err(RepoError::Conflict);
        }
        let mut n = center.clone();
        n.stamp = new_stamp();
        n.data_types.clear();
        s.log_change(
            None,
            "group_center",
            group_center_key(&n),
            None,
            Some(group_center_row(&n)),
            &[],
        );
        s.group_centers.insert(n.code.as_str().to_owned(), n);
        Ok(())
    }

    async fn update_group_center(
        &self,
        center: &ttrs_domain::group_center::GroupCenter,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let Some(old) = s.group_centers.get(center.code.as_str()).cloned() else {
            return Ok(());
        };
        let mut n = center.clone();
        n.stamp = bumped(&old.stamp, &center.stamp)?;
        // 資料類別另以 add／remove 維護，這裡保留現有的設定
        n.data_types = old.data_types.clone();
        s.log_change(
            None,
            "group_center",
            group_center_key(&n),
            Some(group_center_row(&old)),
            Some(group_center_row(&n)),
            &[],
        );
        s.group_centers.insert(n.code.as_str().to_owned(), n);
        Ok(())
    }

    async fn add_data_center_type(
        &self,
        code: &ttrs_domain::group_center::GroupCenterCode,
        t: &ttrs_domain::group_center::DataCenterType,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        // 與 PostgreSQL 一致：全集團只能屬於一個資料中心（GC5），也不可已是受控營運中心（DC2）
        if s.group_centers.values().any(|c| c.data_types.contains(t))
            || s.data_controls
                .contains_key(&(t.data_type.code(), t.plant.clone()))
        {
            return Err(RepoError::Conflict);
        }
        let Some(c) = s.group_centers.get_mut(code.as_str()) else {
            return Err(RepoError::Other(format!(
                "集團中心不存在：{}",
                code.as_str()
            )));
        };
        c.data_types.push(t.clone());
        c.data_types.sort();
        s.log_change(
            None,
            "data_center_type",
            data_center_type_key(code.as_str(), t),
            None,
            Some(data_center_type_row(code.as_str(), t)),
            &[],
        );
        Ok(())
    }

    async fn remove_data_center_type(
        &self,
        code: &ttrs_domain::group_center::GroupCenterCode,
        t: &ttrs_domain::group_center::DataCenterType,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(c) = s.group_centers.get_mut(code.as_str()) else {
            return Ok(false);
        };
        let before = c.data_types.len();
        c.data_types.retain(|x| x != t);
        if c.data_types.len() == before {
            return Ok(false);
        }
        s.log_change(
            None,
            "data_center_type",
            data_center_type_key(code.as_str(), t),
            Some(data_center_type_row(code.as_str(), t)),
            None,
            &[],
        );
        Ok(true)
    }

    async fn delete_group_center(
        &self,
        code: &ttrs_domain::group_center::GroupCenterCode,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s.group_centers.remove(code.as_str()) else {
            return Ok(false);
        };
        // 與 PostgreSQL 的 ON DELETE CASCADE 一致：資料類別設定一起刪除並記錄異動
        for t in &old.data_types {
            s.log_change(
                None,
                "data_center_type",
                data_center_type_key(code.as_str(), t),
                Some(data_center_type_row(code.as_str(), t)),
                None,
                &[],
            );
        }
        s.log_change(
            None,
            "group_center",
            group_center_key(&old),
            Some(group_center_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

// ----- 資料控管原則（gev_file gev03＝N、gey_file；data_control.feature） -----

fn controlled_key(p: &ttrs_domain::data_control::ControlledPlant) -> Vec<(&'static str, String)> {
    vec![
        ("data_type", p.data_type.code()),
        ("plant_id", p.plant.clone()),
    ]
}

fn controlled_row(p: &ttrs_domain::data_control::ControlledPlant) -> Row {
    let mut cols = vec![
        ("data_type", text::s(&p.data_type.code())),
        ("plant_id", text::s(&p.plant)),
    ];
    cols.extend(audit_cols(
        true,
        &p.created_by,
        &p.created_dept,
        p.created_on,
        p.updated_by.as_deref(),
        p.updated_on,
        &p.stamp,
    ));
    // 受控營運中心沒有資料有效碼（DC9）
    cols.retain(|(k, _)| *k != "active");
    row(cols)
}

fn rule_key(
    p: &ttrs_domain::data_control::ControlledPlant,
    r: &ttrs_domain::data_control::FieldRule,
) -> Vec<(&'static str, String)> {
    vec![
        ("data_type", p.data_type.code()),
        ("plant_id", p.plant.clone()),
        ("field_code", r.field.clone()),
    ]
}

fn rule_row(
    p: &ttrs_domain::data_control::ControlledPlant,
    r: &ttrs_domain::data_control::FieldRule,
) -> Row {
    row(vec![
        ("data_type", text::s(&p.data_type.code())),
        ("plant_id", text::s(&p.plant)),
        ("field_code", text::s(&r.field)),
        ("condition", text::s(&r.condition.code())),
        ("field_values", text::s(&r.values)),
    ])
}

impl State {
    /// 欄位條件的異動記錄：只記有變動的列（與 PostgreSQL 觸發器相同）
    fn log_rules(
        &mut self,
        p: &ttrs_domain::data_control::ControlledPlant,
        old: &[ttrs_domain::data_control::FieldRule],
        new: &[ttrs_domain::data_control::FieldRule],
    ) {
        for o in old
            .iter()
            .filter(|o| !new.iter().any(|n| n.field == o.field))
        {
            self.log_change(
                None,
                "data_control_rule",
                rule_key(p, o),
                Some(rule_row(p, o)),
                None,
                &[],
            );
        }
        for n in new {
            let before = old.iter().find(|o| o.field == n.field);
            if before != Some(n) {
                self.log_change(
                    None,
                    "data_control_rule",
                    rule_key(p, n),
                    before.map(|b| rule_row(p, b)),
                    Some(rule_row(p, n)),
                    &[],
                );
            }
        }
    }
}

#[async_trait]
impl crate::ports::DataControlRepository for InMemoryStore {
    async fn list_controlled_plants(
        &self,
    ) -> Result<Vec<ttrs_domain::data_control::ControlledPlant>, RepoError> {
        Ok(self.lock().data_controls.values().cloned().collect())
    }

    async fn get_controlled_plant(
        &self,
        data_type: ttrs_domain::group_center::DataType,
        plant: &str,
    ) -> Result<Option<ttrs_domain::data_control::ControlledPlant>, RepoError> {
        Ok(self
            .lock()
            .data_controls
            .get(&(data_type.code(), plant.to_owned()))
            .cloned())
    }

    async fn insert_controlled_plant(
        &self,
        p: &ttrs_domain::data_control::ControlledPlant,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (p.data_type.code(), p.plant.clone());
        // 與 PostgreSQL 一致：資料類別＋營運中心不可已是資料中心或受控營運中心（DC2）
        let is_center = s.group_centers.values().any(|c| {
            c.data_types
                .iter()
                .any(|t| t.data_type == p.data_type && t.plant == p.plant)
        });
        if is_center || s.data_controls.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut n = p.clone();
        n.stamp = new_stamp();
        s.log_change(
            None,
            "data_control_plant",
            controlled_key(&n),
            None,
            Some(controlled_row(&n)),
            &[],
        );
        s.log_rules(&n, &[], &n.rules);
        s.data_controls.insert(key, n);
        Ok(())
    }

    async fn update_controlled_plant(
        &self,
        p: &ttrs_domain::data_control::ControlledPlant,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (p.data_type.code(), p.plant.clone());
        let Some(old) = s.data_controls.get(&key).cloned() else {
            return Ok(());
        };
        let mut n = p.clone();
        n.stamp = bumped(&old.stamp, &p.stamp)?;
        // 與 PostgreSQL 的順序一致：先寫欄位條件，再更新營運中心列
        s.log_rules(&n, &old.rules, &n.rules);
        s.log_change(
            None,
            "data_control_plant",
            controlled_key(&n),
            Some(controlled_row(&old)),
            Some(controlled_row(&n)),
            &[],
        );
        s.data_controls.insert(key, n);
        Ok(())
    }

    async fn delete_controlled_plant(
        &self,
        data_type: ttrs_domain::group_center::DataType,
        plant: &str,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s
            .data_controls
            .remove(&(data_type.code(), plant.to_owned()))
        else {
            return Ok(false);
        };
        s.log_rules(&old, &old.rules, &[]);
        s.log_change(
            None,
            "data_control_plant",
            controlled_key(&old),
            Some(controlled_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

// ----- 資料中心拋轉設定（gew_file、gez_file；carry_setting.feature） -----

type CarryKey = Vec<(&'static str, String)>;

fn carry_key(c: &ttrs_domain::carry_setting::CarrySetting) -> CarryKey {
    vec![
        ("center_code", c.center.as_str().to_owned()),
        ("data_type", c.data_type.code()),
    ]
}

fn carry_row(c: &ttrs_domain::carry_setting::CarrySetting) -> Row {
    let mut cols = vec![
        ("center_code", text::s(c.center.as_str())),
        ("data_type", text::s(&c.data_type.code())),
        ("mode", text::s(&c.mode.code())),
    ];
    cols.extend(audit_cols(
        true,
        &c.created_by,
        &c.created_dept,
        c.created_on,
        c.updated_by.as_deref(),
        c.updated_on,
        &c.stamp,
    ));
    // 拋轉設定沒有資料有效碼（CS9）
    cols.retain(|(k, _)| *k != "active");
    row(cols)
}

fn carry_child_key(
    c: &ttrs_domain::carry_setting::CarrySetting,
    plant: &str,
    field: Option<&str>,
) -> CarryKey {
    let mut k = carry_key(c);
    k.push(("plant_id", plant.to_owned()));
    if let Some(f) = field {
        k.push(("field_code", f.to_owned()));
    }
    k
}

fn carry_target_row(
    c: &ttrs_domain::carry_setting::CarrySetting,
    t: &ttrs_domain::carry_setting::CarryTarget,
) -> Row {
    row(vec![
        ("center_code", text::s(c.center.as_str())),
        ("data_type", text::s(&c.data_type.code())),
        ("plant_id", text::s(&t.plant)),
        ("deletable", text::b(t.deletable)),
        ("update_existing", text::b(t.update_existing)),
        ("notify", text::opt(t.notify.as_deref())),
    ])
}

fn carry_condition_row(
    c: &ttrs_domain::carry_setting::CarrySetting,
    plant: &str,
    r: &ttrs_domain::data_control::FieldRule,
) -> Row {
    row(vec![
        ("center_code", text::s(c.center.as_str())),
        ("data_type", text::s(&c.data_type.code())),
        ("plant_id", text::s(plant)),
        ("field_code", text::s(&r.field)),
        ("condition", text::s(&r.condition.code())),
        ("field_values", text::s(&r.values)),
    ])
}

fn carry_default_row(
    c: &ttrs_domain::carry_setting::CarrySetting,
    plant: &str,
    d: &ttrs_domain::carry_setting::CarryDefault,
) -> Row {
    row(vec![
        ("center_code", text::s(c.center.as_str())),
        ("data_type", text::s(&c.data_type.code())),
        ("plant_id", text::s(plant)),
        ("field_code", text::s(&d.field)),
        ("field_value", text::s(&d.value)),
    ])
}

impl State {
    /// 單身（拋轉營運中心、條件、預設值）的異動記錄：只記有變動的列，刪除時先子後母、新增時先母後子（與 PostgreSQL 相同）
    fn log_carry_children(
        &mut self,
        c: &ttrs_domain::carry_setting::CarrySetting,
        old: &[ttrs_domain::carry_setting::CarryTarget],
        new: &[ttrs_domain::carry_setting::CarryTarget],
    ) {
        let empty = ttrs_domain::carry_setting::CarryTarget::new("");
        // 被刪除的拋轉營運中心：先刪條件、預設值，再刪營運中心
        for o in old
            .iter()
            .filter(|o| !new.iter().any(|n| n.plant == o.plant))
        {
            self.log_carry_rows(c, o, &empty);
            self.log_change(
                None,
                "carry_target",
                carry_child_key(c, &o.plant, None),
                Some(carry_target_row(c, o)),
                None,
                &[],
            );
        }
        for n in new {
            let before = old.iter().find(|o| o.plant == n.plant);
            let changed = before.is_none_or(|b| {
                (b.deletable, b.update_existing, &b.notify)
                    != (n.deletable, n.update_existing, &n.notify)
            });
            if changed {
                self.log_change(
                    None,
                    "carry_target",
                    carry_child_key(c, &n.plant, None),
                    before.map(|b| carry_target_row(c, b)),
                    Some(carry_target_row(c, n)),
                    &[],
                );
            }
            let base = before
                .cloned()
                .unwrap_or_else(|| ttrs_domain::carry_setting::CarryTarget::new(&n.plant));
            self.log_carry_rows(c, &base, n);
        }
    }

    fn log_carry_rows(
        &mut self,
        c: &ttrs_domain::carry_setting::CarrySetting,
        old: &ttrs_domain::carry_setting::CarryTarget,
        new: &ttrs_domain::carry_setting::CarryTarget,
    ) {
        let plant = if new.plant.is_empty() {
            &old.plant
        } else {
            &new.plant
        };
        for o in old
            .conditions
            .iter()
            .filter(|o| !new.conditions.iter().any(|n| n.field == o.field))
        {
            self.log_change(
                None,
                "carry_condition",
                carry_child_key(c, plant, Some(&o.field)),
                Some(carry_condition_row(c, plant, o)),
                None,
                &[],
            );
        }
        for n in &new.conditions {
            let before = old.conditions.iter().find(|o| o.field == n.field);
            if before != Some(n) {
                self.log_change(
                    None,
                    "carry_condition",
                    carry_child_key(c, plant, Some(&n.field)),
                    before.map(|b| carry_condition_row(c, plant, b)),
                    Some(carry_condition_row(c, plant, n)),
                    &[],
                );
            }
        }
        for o in old
            .defaults
            .iter()
            .filter(|o| !new.defaults.iter().any(|n| n.field == o.field))
        {
            self.log_change(
                None,
                "carry_default",
                carry_child_key(c, plant, Some(&o.field)),
                Some(carry_default_row(c, plant, o)),
                None,
                &[],
            );
        }
        for n in &new.defaults {
            let before = old.defaults.iter().find(|o| o.field == n.field);
            if before != Some(n) {
                self.log_change(
                    None,
                    "carry_default",
                    carry_child_key(c, plant, Some(&n.field)),
                    before.map(|b| carry_default_row(c, plant, b)),
                    Some(carry_default_row(c, plant, n)),
                    &[],
                );
            }
        }
    }
}

#[async_trait]
impl crate::ports::CarrySettingRepository for InMemoryStore {
    async fn list_carry_settings(
        &self,
    ) -> Result<Vec<ttrs_domain::carry_setting::CarrySetting>, RepoError> {
        Ok(self.lock().carry_settings.values().cloned().collect())
    }

    async fn get_carry_setting(
        &self,
        center: &ttrs_domain::group_center::GroupCenterCode,
        data_type: ttrs_domain::group_center::DataType,
    ) -> Result<Option<ttrs_domain::carry_setting::CarrySetting>, RepoError> {
        Ok(self
            .lock()
            .carry_settings
            .get(&(center.as_str().to_owned(), data_type.code()))
            .cloned())
    }

    async fn insert_carry_setting(
        &self,
        c: &ttrs_domain::carry_setting::CarrySetting,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (c.center.as_str().to_owned(), c.data_type.code());
        if s.carry_settings.contains_key(&key) {
            return Err(RepoError::Conflict);
        }
        let mut n = c.clone();
        n.stamp = new_stamp();
        s.log_change(
            None,
            "carry_setting",
            carry_key(&n),
            None,
            Some(carry_row(&n)),
            &[],
        );
        s.log_carry_children(&n, &[], &n.targets);
        s.carry_settings.insert(key, n);
        Ok(())
    }

    async fn update_carry_setting(
        &self,
        c: &ttrs_domain::carry_setting::CarrySetting,
    ) -> Result<(), RepoError> {
        let mut s = self.lock();
        let key = (c.center.as_str().to_owned(), c.data_type.code());
        let Some(old) = s.carry_settings.get(&key).cloned() else {
            return Ok(());
        };
        let mut n = c.clone();
        n.stamp = bumped(&old.stamp, &c.stamp)?;
        // 與 PostgreSQL 的順序一致：先寫單身，再更新單頭
        s.log_carry_children(&n, &old.targets, &n.targets);
        s.log_change(
            None,
            "carry_setting",
            carry_key(&n),
            Some(carry_row(&old)),
            Some(carry_row(&n)),
            &[],
        );
        s.carry_settings.insert(key, n);
        Ok(())
    }

    async fn delete_carry_setting(
        &self,
        center: &ttrs_domain::group_center::GroupCenterCode,
        data_type: ttrs_domain::group_center::DataType,
    ) -> Result<bool, RepoError> {
        let mut s = self.lock();
        let Some(old) = s
            .carry_settings
            .remove(&(center.as_str().to_owned(), data_type.code()))
        else {
            return Ok(false);
        };
        s.log_carry_children(&old, &old.targets, &[]);
        s.log_change(
            None,
            "carry_setting",
            carry_key(&old),
            Some(carry_row(&old)),
            None,
            &[],
        );
        Ok(true)
    }
}

// ----- 基本資料拋轉與拋轉紀錄（aoop603、gex_file；basic_data_carry.feature） -----

#[async_trait]
impl crate::ports::CarryLogRepository for InMemoryStore {
    async fn append_carry_logs(
        &self,
        entries: &[ttrs_domain::basic_carry::CarryLogEntry],
    ) -> Result<(), RepoError> {
        self.lock().carry_logs.extend(entries.iter().cloned());
        Ok(())
    }

    async fn list_carry_logs(
        &self,
    ) -> Result<Vec<ttrs_domain::basic_carry::CarryLogEntry>, RepoError> {
        let mut v = self.lock().carry_logs.clone();
        v.sort_by_key(|l| std::cmp::Reverse(l.carried_at));
        Ok(v)
    }

    async fn carry_log_exists(
        &self,
        center: &ttrs_domain::group_center::GroupCenterCode,
        data_type: Option<ttrs_domain::group_center::DataType>,
    ) -> Result<bool, RepoError> {
        Ok(self
            .lock()
            .carry_logs
            .iter()
            .any(|l| &l.center == center && data_type.is_none_or(|t| l.data_type == t)))
    }
}

#[async_trait]
impl crate::ports::BasicCarryRepository for InMemoryStore {
    /// 記憶體後端：use case 已先檢查參照，依序寫入（與 PostgreSQL 的單一交易結果相同）
    async fn apply_basic_carry(
        &self,
        target: &PlantId,
        batch: &crate::ports::BasicCarryBatch,
    ) -> Result<(), RepoError> {
        use crate::ports::{
            CarryWrite, ClassificationCodeRepository, DepartmentRepository, EmployeeRepository,
            ItemGroupRepository, ProductClassRepository,
        };
        for w in &batch.departments {
            match w {
                CarryWrite::Insert(d) => DepartmentRepository::insert(self, target, d).await?,
                CarryWrite::Update(d) => DepartmentRepository::update(self, target, d).await?,
            }
        }
        for w in &batch.employees {
            match w {
                CarryWrite::Insert(e) => EmployeeRepository::insert(self, target, e).await?,
                CarryWrite::Update(e) => EmployeeRepository::update(self, target, e).await?,
            }
        }
        for w in &batch.codes {
            match w {
                CarryWrite::Insert(c) => self.insert_code(target, c).await?,
                CarryWrite::Update(c) => self.update_code(target, c).await?,
            }
        }
        for w in &batch.product_classes {
            match w {
                CarryWrite::Insert(c) => self.insert_product_class(target, c).await?,
                CarryWrite::Update(c) => self.update_product_class(target, c).await?,
            }
        }
        for w in &batch.item_groups {
            match w {
                CarryWrite::Insert(g) => self.insert_item_group(target, g).await?,
                CarryWrite::Update(g) => self.update_item_group(target, g).await?,
            }
        }
        Ok(())
    }
}
