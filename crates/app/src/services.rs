//! 組裝各作業的 use case：同一份資料存取實作（PostgreSQL 或記憶體）＋權限檢查（ADR 0006 §4）

use std::sync::Arc;

use crate::ports::RateFeed;

use crate::change_log::HistoryReader;
use crate::{
    AccountingPeriodService, BasicCodeService, ChangeHistoryService, CurrencyService,
    ExchangeRateService, GeneralParameterService, ItemGroupService, ItemUnitConversionService,
    MenuService, ProductClassService, ProjectService, TaxTypeService, UnitConversionService,
    UnitService,
};
use crate::{
    Authorizer, ClassificationCodeService, DataDictionaryService, DepartmentService,
    EmergencyService, EmployeeService, ItemService, LegacyCatalogService, PasswordResetService,
    PermissionAuthorizer, PermissionClassService, PlantService, ProgramPermissionService,
    ProgramService, SignInService, UserPermissionService, UserService,
    ports::{
        AccountPolicyRepository, AccountingPeriodRepository, AuditLog, BasicCodeRepository,
        ChangeLogRepository, ClassificationCodeRepository, CredentialRepository,
        CurrencyRepository, DepartmentRepository, DictionaryRepository, EmployeeRepository,
        ExchangeRateRepository, ItemGroupRepository, ItemRepository, ItemUnitConversionRepository,
        LegacyCatalogRepository, MasterDataRepository, MenuRepository,
        PasswordResetTokenRepository, PermissionClassRepository, PlantRepository,
        ProductClassRepository, ProgramPermissionRepository, ProgramRepository, ProjectRepository,
        SchemaCatalog, SystemParameterRepository, TaxTypeRepository, UnitConversionRepository,
        UnitRepository, UserPermissionRepository, UserRepository,
    },
};

/// 實作全部 repository 的資料存取（ttrs_infra::PgStore、memory::InMemoryStore）
pub trait Store:
    ItemRepository
    + MasterDataRepository
    + ProgramRepository
    + SchemaCatalog
    + DictionaryRepository
    + LegacyCatalogRepository
    + UserRepository
    + PermissionClassRepository
    + PlantRepository
    + ProgramPermissionRepository
    + UserPermissionRepository
    + DepartmentRepository
    + EmployeeRepository
    + CredentialRepository
    + AccountPolicyRepository
    + PasswordResetTokenRepository
    + AuditLog
    + ClassificationCodeRepository
    + ChangeLogRepository
    + MenuRepository
    + UnitRepository
    + BasicCodeRepository
    + CurrencyRepository
    + ExchangeRateRepository
    + RateFeed
    + SystemParameterRepository
    + TaxTypeRepository
    + ProjectRepository
    + ProductClassRepository
    + ItemGroupRepository
    + AccountingPeriodRepository
    + UnitConversionRepository
    + ItemUnitConversionRepository
    + crate::ports::GithubSource
    + crate::ports::TranslationJobRepository
    + crate::ports::EpubLibraryRepository
    + crate::ports::EncodingNatureRepository
    + crate::ports::EncodingSegmentRepository
    + crate::ports::EncodingYearRepository
    + crate::ports::EncodingClassRepository
    + crate::ports::EpubSharedLibraryRepository
    + crate::ports::GroupCenterRepository
    + crate::ports::DataControlRepository
    + crate::ports::CarrySettingRepository
    + crate::ports::CarryLogRepository
    + crate::ports::BasicCarryRepository
    + crate::ports::ItemCarryRepository
    + crate::ports::WarehouseRepository
    + crate::ports::ItemStorageRepository
    + crate::ports::InventoryParameterRepository
    + crate::ports::DocumentTypeRepository
    + crate::ports::InventoryTransactionRepository
    + crate::ports::InventoryBalanceRepository
    + crate::ports::InventoryPostingRepository
    + 'static
{
}

impl<T> Store for T where
    T: ItemRepository
        + MasterDataRepository
        + ProgramRepository
        + SchemaCatalog
        + DictionaryRepository
        + LegacyCatalogRepository
        + UserRepository
        + PermissionClassRepository
        + PlantRepository
        + ProgramPermissionRepository
        + UserPermissionRepository
        + DepartmentRepository
        + EmployeeRepository
        + CredentialRepository
        + AccountPolicyRepository
        + PasswordResetTokenRepository
        + AuditLog
        + ClassificationCodeRepository
        + ChangeLogRepository
        + MenuRepository
        + UnitRepository
        + BasicCodeRepository
        + CurrencyRepository
        + ExchangeRateRepository
        + RateFeed
        + SystemParameterRepository
        + TaxTypeRepository
        + ProjectRepository
        + ProductClassRepository
        + ItemGroupRepository
        + AccountingPeriodRepository
        + UnitConversionRepository
        + ItemUnitConversionRepository
        + crate::ports::GithubSource
        + crate::ports::TranslationJobRepository
        + crate::ports::EpubLibraryRepository
        + crate::ports::EncodingNatureRepository
        + crate::ports::EncodingSegmentRepository
        + crate::ports::EncodingYearRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingYearRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingSegmentRepository
        + crate::ports::EncodingYearRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingYearRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingNatureRepository
        + crate::ports::EncodingSegmentRepository
        + crate::ports::EncodingYearRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingYearRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingSegmentRepository
        + crate::ports::EncodingYearRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingYearRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EncodingClassRepository
        + crate::ports::EpubSharedLibraryRepository
        + crate::ports::GroupCenterRepository
        + crate::ports::DataControlRepository
        + crate::ports::CarrySettingRepository
        + crate::ports::CarryLogRepository
        + crate::ports::BasicCarryRepository
        + crate::ports::ItemCarryRepository
        + crate::ports::WarehouseRepository
        + crate::ports::ItemStorageRepository
        + crate::ports::InventoryParameterRepository
        + crate::ports::DocumentTypeRepository
        + crate::ports::InventoryTransactionRepository
        + crate::ports::InventoryBalanceRepository
        + crate::ports::InventoryPostingRepository
        + 'static
{
}

#[derive(Clone)]
pub struct Services {
    pub items: ItemService,
    pub programs: ProgramService,
    pub dictionary: DataDictionaryService,
    pub legacy: LegacyCatalogService,
    pub users: UserService,
    pub classes: PermissionClassService,
    pub plants: PlantService,
    pub permissions: ProgramPermissionService,
    pub special: UserPermissionService,
    pub departments: DepartmentService,
    pub employees: EmployeeService,
    /// 分類碼（aooi300）
    pub classification: ClassificationCodeService,
    pub sign_in: SignInService,
    /// 忘記密碼（ADR 0010 §A）；寄信設定以 [`PasswordResetService::with_mail`] 接上
    pub password_reset: PasswordResetService,
    /// 伺服器端緊急重設（ttrs-admin，ADR 0010 §B）；不提供給 HTTP API
    pub emergency: EmergencyService,
    /// 共用層作業的異動記錄（ADR 0012 §8）
    pub history: ChangeHistoryService,
    /// 選單維護與側欄（p_zm）
    pub menus: MenuService,
    /// 單位維護（aooi101）
    pub units: UnitService,
    pub encoding_natures: crate::EncodingNatureService,
    pub encoding_segments: crate::EncodingSegmentService,
    pub encoding_years: crate::EncodingYearService,
    pub encoding_classes: crate::EncodingClassService,
    /// 集團中心維護（aooi600）
    pub group_centers: crate::GroupCenterService,
    /// 資料控管原則維護（aooi601）
    pub data_controls: crate::DataControlService,
    /// 資料中心拋轉設定（aooi602）
    pub carry_settings: crate::CarrySettingService,
    /// 倉庫儲位維護（aimi201、aimi200）
    pub warehouses: crate::WarehouseService,
    pub item_storages: crate::ItemStorageService,
    pub inventory_parameters: crate::InventoryParameterService,
    pub document_types: crate::DocumentTypeService,
    /// 雜項發料 aimt301、雜項收料 aimt302、雜項報廢 aimt303（三支獨立作業）
    pub misc_issues: crate::MiscIssueService,
    pub misc_receipts: crate::MiscReceiptService,
    pub misc_scraps: crate::MiscScrapService,
    /// 料件拋轉（aimi100 carry；item_carry.feature）
    pub item_carry: crate::ItemCarryService,
    /// 基本資料拋轉（aoop603）與拋轉歷史查詢（aooq604）
    pub basic_carry: crate::BasicCarryService,
    /// 基本代碼：區域別、國別、地區、交運方式、報表抬頭（aooi100、090、110、160、210）
    pub basic_codes: BasicCodeService,
    /// 幣別維護（aooi050）
    pub currencies: CurrencyService,
    /// 匯率：每月匯率（aooi060）、每日匯率（aooi070）、每日匯率複製（aoop010）、匯率取用（s_exrate）
    pub rates: ExchangeRateService,
    /// 整體管理系統參數（aoos010）
    pub general_parameters: GeneralParameterService,
    /// 會計期間維護（aoos020）
    pub accounting_periods: AccountingPeriodService,
    /// 單位換算（aooi102）、料件單位換算（aooi103）
    pub unit_conversions: UnitConversionService,
    pub item_unit_conversions: ItemUnitConversionService,
    /// EPUB 轉 TXT（工具作業）
    pub epub: crate::EpubService,
    /// GitHub EPUB 搜尋（工具作業）
    pub github_epub: crate::GithubEpubService,
    /// EPUB 英文翻譯成繁體中文（epub_translate.feature）
    pub translate: crate::TranslationService,
    /// EPUB 多來源搜尋（epub_web_search.feature）
    pub epub_search: crate::EpubSearchService,
    /// 以網址下載 EPUB（epub_url_download.feature）
    pub epub_url: crate::EpubUrlService,
    /// 稅別維護（aooi150）
    pub taxes: TaxTypeService,
    /// 專案維護（aooi200，工廠層）
    pub projects: ProjectService,
    /// 產品分類維護（axmi110）
    pub product_classes: ProductClassService,
    /// 料件分群維護（aimi110）
    pub item_groups: ItemGroupService,
    pub authorizer: Arc<dyn Authorizer>,
}

impl Services {
    /// 依權限資料檢查（zy_file、zxw_file）
    pub fn new<S: Store>(store: Arc<S>) -> Self {
        let authorizer = Arc::new(PermissionAuthorizer::new(store.clone(), store.clone()));
        Self::with_authorizer(store, authorizer)
    }

    pub fn with_authorizer<S: Store>(store: Arc<S>, authorizer: Arc<dyn Authorizer>) -> Self {
        let a = authorizer.clone();
        // 判斷使用者維護管理員一律依權限資料（R14），與請求的權限檢查分開
        let admins: Arc<dyn Authorizer> =
            Arc::new(PermissionAuthorizer::new(store.clone(), store.clone()));
        let history = HistoryReader::new(store.clone(), store.clone());
        // 列印、匯出 Excel 的共用組裝（ADR 0013 SA3）
        let reports =
            crate::list_report::ListReporter::new(store.clone(), store.clone(), store.clone());
        let item_carry = crate::ItemCarryService::new(
            a.clone(),
            crate::ItemCarryRepos {
                centers: store.clone(),
                carries: store.clone(),
                logs: store.clone(),
                writer: store.clone(),
                items: store.clone(),
                conversions: store.clone(),
                groups: store.clone(),
                classes: store.clone(),
                codes: store.clone(),
                employees: store.clone(),
                depts: store.clone(),
            },
        );
        let item_repos = crate::item_service::ItemRepos {
            items: store.clone(),
            groups: store.clone(),
            units: store.clone(),
            classes: store.clone(),
            codes: store.clone(),
            employees: store.clone(),
            departments: store.clone(),
            master: store.clone(),
            data_control: store.clone(),
            warehouses: store.clone(),
            reports: reports.clone(),
        };
        Self {
            items: ItemService::new(a.clone(), item_repos.clone(), history.clone())
                .with_carry(item_carry.clone()),
            item_carry,
            warehouses: crate::WarehouseService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
                reports.clone(),
            ),
            item_storages: crate::ItemStorageService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
                reports.clone(),
            ),
            inventory_parameters: crate::InventoryParameterService::new(
                a.clone(),
                store.clone(),
                history.clone(),
                reports.clone(),
            ),
            document_types: crate::DocumentTypeService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
                reports.clone(),
            ),
            misc_issues: crate::MiscIssueService::new(crate::misc_issue_service::MiscIssueRepos {
                authz: a.clone(),
                txns: store.clone(),
                balances: store.clone(),
                docs: store.clone(),
                params: store.clone(),
                inventory_params: store.clone(),
                periods: store.clone(),
                users: store.clone(),
                depts: store.clone(),
                employees: store.clone(),
                items: store.clone(),
                warehouses: store.clone(),
                codes: store.clone(),
                posting: store.clone(),
                history: history.clone(),
                reports: reports.clone(),
            }),
            misc_receipts: crate::MiscReceiptService::new(
                crate::misc_receipt_service::MiscReceiptRepos {
                    authz: a.clone(),
                    txns: store.clone(),
                    balances: store.clone(),
                    docs: store.clone(),
                    params: store.clone(),
                    inventory_params: store.clone(),
                    periods: store.clone(),
                    users: store.clone(),
                    depts: store.clone(),
                    employees: store.clone(),
                    items: store.clone(),
                    warehouses: store.clone(),
                    codes: store.clone(),
                    posting: store.clone(),
                    item_storages: store.clone(),
                    history: history.clone(),
                    reports: reports.clone(),
                },
            ),
            misc_scraps: crate::MiscScrapService::new(crate::misc_scrap_service::MiscScrapRepos {
                authz: a.clone(),
                txns: store.clone(),
                balances: store.clone(),
                docs: store.clone(),
                params: store.clone(),
                inventory_params: store.clone(),
                periods: store.clone(),
                users: store.clone(),
                depts: store.clone(),
                employees: store.clone(),
                items: store.clone(),
                warehouses: store.clone(),
                codes: store.clone(),
                posting: store.clone(),
                history: history.clone(),
                reports: reports.clone(),
            }),
            programs: ProgramService::new(a.clone(), store.clone(), reports.clone()),
            dictionary: DataDictionaryService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                reports.clone(),
            ),
            legacy: LegacyCatalogService::new(a.clone(), store.clone()),
            users: UserService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                admins,
                reports.clone(),
            ),
            classes: PermissionClassService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                reports.clone(),
            ),
            plants: PlantService::new(a.clone(), store.clone(), reports.clone()),
            permissions: ProgramPermissionService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                reports.clone(),
            ),
            departments: DepartmentService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
            ),
            sign_in: SignInService::new(store.clone(), store.clone(), store.clone(), store.clone()),
            password_reset: PasswordResetService::new(
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
            ),
            emergency: EmergencyService::new(store.clone(), store.clone(), store.clone()),
            employees: EmployeeService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
                reports.clone(),
            ),
            classification: ClassificationCodeService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
                reports.clone(),
            ),
            history: ChangeHistoryService::new(a.clone(), history.clone()),
            units: UnitService::new(a.clone(), store.clone(), reports.clone()),
            encoding_natures: crate::EncodingNatureService::new(
                a.clone(),
                store.clone(),
                reports.clone(),
            ),
            encoding_segments: crate::EncodingSegmentService::new(
                a.clone(),
                store.clone(),
                reports.clone(),
            ),
            encoding_years: crate::EncodingYearService::new(
                a.clone(),
                store.clone(),
                reports.clone(),
            ),
            basic_carry: crate::BasicCarryService::new(
                a.clone(),
                crate::BasicCarryRepos {
                    centers: store.clone(),
                    carries: store.clone(),
                    logs: store.clone(),
                    writer: store.clone(),
                    depts: store.clone(),
                    employees: store.clone(),
                    codes: store.clone(),
                    classes: store.clone(),
                    groups: store.clone(),
                },
                reports.clone(),
            ),
            carry_settings: crate::CarrySettingService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                reports.clone(),
            ),
            data_controls: crate::DataControlService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                reports.clone(),
            ),
            group_centers: crate::GroupCenterService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                reports.clone(),
            ),
            encoding_classes: crate::EncodingClassService::new(
                a.clone(),
                crate::EncodingClassRepos {
                    classes: store.clone(),
                    natures: store.clone(),
                    segments: store.clone(),
                    years: store.clone(),
                    items: store.clone(),
                    periods: store.clone(),
                    params: store.clone(),
                },
                reports.clone(),
            ),
            basic_codes: BasicCodeService::new(a.clone(), store.clone(), reports.clone()),
            currencies: CurrencyService::new(a.clone(), store.clone(), reports.clone()),
            rates: ExchangeRateService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                reports.clone(),
            ),
            general_parameters: GeneralParameterService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                reports.clone(),
            )
            .with_doc_types(store.clone())
            .with_inventory_transactions(store.clone()),
            accounting_periods: AccountingPeriodService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                reports.clone(),
            ),
            unit_conversions: UnitConversionService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                reports.clone(),
            ),
            epub: crate::EpubService::new(a.clone(), store.clone()),
            github_epub: crate::GithubEpubService::new(a.clone(), store.clone(), store.clone()),
            epub_search: crate::EpubSearchService::new(a.clone(), store.clone())
                .with_library(store.clone(), store.clone())
                .with_shared_library(store.clone()),
            epub_url: crate::EpubUrlService::new(
                a.clone(),
                store.clone(),
                Arc::new(crate::epub_url_service::NoWeb),
            ),
            translate: crate::TranslationService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                Arc::new(crate::NoTranslator),
            ),
            item_unit_conversions: ItemUnitConversionService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
                reports.clone(),
            ),
            taxes: TaxTypeService::new(a.clone(), store.clone(), reports.clone()),
            projects: ProjectService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
                reports.clone(),
            ),
            product_classes: ProductClassService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                history.clone(),
                reports.clone(),
            ),
            item_groups: ItemGroupService::new(a.clone(), item_repos, history.clone()),
            menus: MenuService::new(
                a.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
                store.clone(),
            ),
            special: UserPermissionService::new(
                a,
                store.clone(),
                store.clone(),
                store.clone(),
                store,
                reports,
            ),
            authorizer,
        }
    }
}
