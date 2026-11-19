# 錯誤訊息：ID 為 error-<錯誤代碼>（DomainError::code()）；zh-TW 一律使用伺服器回傳的訊息
# 語系：en。ID 規則見 docs/adr/0005-i18n.md；三個語系的 ID 必須一致。

error-PROGRAM_NO_EMPTY = Program ID is required.
error-PROGRAM_NO_TOO_LONG = Program ID is too long (max 40).
error-PROGRAM_NO_INVALID = Program ID may contain only lowercase letters, digits and hyphens, and must start with a letter.
error-PROGRAM_TYPE_REQUIRED = Program type is required.
error-PROGRAM_TIPTOP_NO_DUPLICATE = This TipTop program ID is already mapped to another program.
error-PROGRAM_ALREADY_EXISTS = This program ID already exists.
error-PROGRAM_NOT_FOUND = Program not found.
error-MODULE_EMPTY = Module is required.
error-MODULE_NOT_FOUND = Module not found.
error-MODULE_NOT_ALLOWED = Shared library modules cannot own a program.
error-REPORT_TYPE_REQUIRED = Report programs must have a report type.
error-MENU_HAS_NO_PERMISSION = Menus are not executable and cannot have permissions.
error-PERMISSION_CLASS_NOT_FOUND = Permission class not found.
error-QUERY_DEFINITION_NOT_FOUND = Programs run via p_query need a query definition first.
error-FIELD_TOO_LONG = A field exceeds its maximum length.
error-LANGUAGE_NOT_SUPPORTED = Unsupported language (zh-TW, en, zh-CN).
error-QBE_INVALID_SYNTAX = Invalid query condition.
error-QBE_INVALID_NUMBER = Query condition is not a number.
error-QBE_INVALID_DATE = Query condition is not a valid date (YYYY/MM/DD).
error-QBE_FIELD_NOT_ALLOWED = This field cannot be queried.
error-INFRA_ERROR = System error. Please try again later.
error-ITEM_NO_EMPTY = Item no. is required.
error-ITEM_NO_TOO_LONG = Item no. is too long.
error-ITEM_NO_INVALID_CHAR = Item no. contains invalid characters.
error-ITEM_NAME_EMPTY = Item name is required.
error-PLANT_ID_EMPTY = Plant is required.
error-ITEM_ALREADY_EXISTS = This item already exists.
error-ITEM_NOT_FOUND = Item not found.
error-PLANT_NOT_FOUND = Plant not found or inactive.
error-UNIT_NOT_FOUND = Unit not found.
error-ITEM_HAS_TRANSACTIONS = The item has inventory transactions and cannot be deleted.
error-ITEM_INVALID_TRANSITION = This action is not allowed in the item's current status.
error-DICT_TABLE_NAME_EMPTY = Table name is required.
error-DICT_TABLE_NOT_IN_SCHEMA = The table does not exist in the database (create it by a migration first).
error-DICT_ALREADY_EXISTS = This table already has a dictionary.
error-DICT_NOT_FOUND = This table has no dictionary yet.
error-DICT_COLUMN_NOT_FOUND = The table has no such column.
error-LEGACY_NAME_INVALID = Invalid English name (lowercase letter first, only lowercase letters, digits and underscores, up to 63, not a reserved word).
error-LEGACY_NAME_DUPLICATE = Duplicate English name.
error-LEGACY_TABLE_NOT_FOUND = TipTop table not found in the catalog.
error-USER_ID_EMPTY = User code is required.
error-USER_ID_TOO_LONG = User code is too long (max 10).
error-USER_ALREADY_EXISTS = This user code already exists.
error-USER_NOT_FOUND = User not found.
error-USER_ID_IMMUTABLE = User code cannot be changed.
error-USER_MENU_INVALID = User menu must be a menu program (module MENU).
error-USER_SINGLE_PLANT = A single-plant user can have only one plant.
error-USER_DEFAULT_PLANT_REQUIRED = The allowed plants must include the default plant.
error-IDP_ACCOUNT_IN_USE = This company account is already bound to another user.
error-USER_EMAIL_INVALID = Invalid email address.
error-PERMISSION_CLASS_CODE_EMPTY = Permission class code is required.
error-PERMISSION_CLASS_CODE_TOO_LONG = Permission class code is too long (max 10 characters).
error-PERMISSION_CLASS_ALREADY_EXISTS = Permission class already exists.
error-PERMISSION_CLASS_CODE_IMMUTABLE = Permission class code cannot be changed.
error-PERMISSION_CLASS_NAME_EMPTY = Description is required.
error-PERMISSION_CLASS_MENU_INVALID = Default menu must be a menu program (module MENU).
error-IDLE_SETTING_REQUIRED = Idle action and idle seconds are required when idle control is "By permission class".
error-IDLE_SECONDS_INVALID = Idle seconds must be between 1 and 99999.
error-PERMISSION_CLASS_IN_USE = Users still belong to this permission class; it cannot be deactivated or deleted.
error-PERMISSION_CLASS_HAS_PERMISSIONS = This permission class still has program permissions; delete them first.
error-PERMISSION_CLASS_INACTIVE = This permission class is inactive and cannot be assigned to users.
error-PLANT_ID_TOO_LONG = Plant code is too long (max 10 characters).
error-PLANT_ALREADY_EXISTS = Plant already exists.
error-PLANT_ID_IMMUTABLE = Plant code cannot be changed.
error-PLANT_NAME_EMPTY = Plant name is required.
error-PLANT_TIME_ZONE_INVALID = Time zone must be a valid IANA name (e.g. Asia/Taipei).
error-PLANT_IN_USE = The plant still has users or business data and cannot be deleted; deactivate it instead.
error-PLANT_LAST_ACTIVE = At least one plant must stay active.
error-CODE_KIND_INVALID = This code type is not supported.
error-CODE_EMPTY = The code is required.
error-CODE_TOO_LONG = The code is too long.
error-CODE_ALREADY_EXISTS = This code type already has the same code.
error-CODE_NOT_FOUND = The classification code does not exist.
error-CODE_IN_USE = The classification code is in use and cannot be deleted; disable it instead.
error-RECORD_CHANGED = This record was changed by someone else. Please query it again.
error-RECORD_ALREADY_CONFIRMED = This record is already confirmed.
error-RECORD_NOT_CONFIRMED = This record is not confirmed, so it cannot be unconfirmed.
error-RECORD_CONFIRMED_NOT_DELETABLE = A confirmed record cannot be deleted. Unconfirm it first.
error-VERSION_REQUIRED = A version number is required to change this record.
error-MENU_NOT_FOUND = The menu must be a registered program in module MENU.
error-MENU_ITEM_NOT_FOUND = This program is not in the menu.
error-MENU_SEQ_INVALID = The sequence must be greater than 0.
error-MENU_SEQ_DUPLICATE = This sequence is already used in the menu.
error-MENU_ITEM_DUPLICATE = This program is already in the menu.
error-MENU_CYCLE = A menu cannot contain itself or a menu above it.
error-MENU_IN_USE = This menu is still used by a user, a permission class or another menu.
error-MENU_ALREADY_HAS_ITEMS = The new menu already has items.
error-MENU_NOT_EMPTY = This menu still has items. Remove its items first.
error-REPORT_TOO_LARGE = Too many records to print. Please narrow the conditions.
error-PLANT_SCHEMA_INVALID = The database code must be ds followed by two digits (for example ds01).
error-PLANT_SCHEMA_DUPLICATE = The database code is already used by another plant.
error-PLANT_SCHEMA_IMMUTABLE = The database code can't be changed after the plant is created.
error-PLANT_SCHEMA_EXHAUSTED = Database codes ds01–ds99 are all in use.
error-PERMISSION_ALREADY_EXISTS = This class already has a permission for the program.
error-PERMISSION_NOT_FOUND = This class has no permission for the program.
error-PERMISSION_ACTION_NOT_ALLOWED = The program does not offer this action.
error-PERMISSION_LEVEL_INVALID = Invalid data permission code.
error-MENU_PROGRAM_INVALID = Must be a menu program (module MENU).
error-USER_PERMISSION_DUPLICATE = This user already has the same class or program.
error-USER_HAS_SPECIAL_PERMISSIONS = The target user already has special permissions.
error-PERMISSION_DENIED = You don't have permission for this operation. Check your permission settings and the data owner.
error-DEPARTMENT_CODE_EMPTY = Department code is required.
error-DEPARTMENT_CODE_TOO_LONG = Department code is too long.
error-DEPARTMENT_NAME_EMPTY = Department short name is required.
error-DEPARTMENT_ALREADY_EXISTS = The department already exists.
error-DEPARTMENT_NOT_FOUND = The department doesn't exist.
error-DEPARTMENT_INACTIVE = The department is inactive.
error-DEPARTMENT_CODE_IMMUTABLE = The department code can't be changed.
error-DEPARTMENT_EXPENSE_TYPE_INVALID = Expense category must be M, P, R or S.
error-DEPARTMENT_CONTROL_TYPE_INVALID = Control type must be 1, 2 or 3.
error-DEPARTMENT_COST_CENTER_INVALID = The cost center must be an active department.
error-DEPARTMENT_IN_USE = The department still has employees and can't be deleted.
error-EMPLOYEE_ID_EMPTY = Employee ID is required.
error-EMPLOYEE_ID_TOO_LONG = Employee ID is too long.
error-EMPLOYEE_NAME_EMPTY = Employee name is required.
error-EMPLOYEE_ALREADY_EXISTS = The employee already exists.
error-EMPLOYEE_NOT_FOUND = The employee doesn't exist.
error-EMPLOYEE_ID_IMMUTABLE = The employee ID can't be changed.
error-EMPLOYEE_EMAIL_INVALID = The e-mail address is not valid.
error-USER_NOT_REGISTERED = This company account is not registered in TTRS. Please contact the system administrator.
error-USER_INACTIVE = The user is deactivated.
error-USER_EXPIRED = The user has passed the expiry date.
error-PLANT_NOT_ALLOWED = You are not allowed to use this plant.
error-PLANT_INACTIVE = This plant is deactivated.
error-NOT_SIGNED_IN = You are not signed in or your session has expired. Please sign in again.
error-SIGN_IN_FAILED = Incorrect user ID or password.
error-USER_LOCKED = Too many failed attempts. The account is locked; please ask the system administrator to unlock it.
error-PASSWORD_CHANGE_REQUIRED = Please change your password before signing in.
error-OLD_PASSWORD_INCORRECT = The current password is incorrect.
error-PASSWORD_CONFIRM_MISMATCH = The new password and confirmation do not match.
error-PASSWORD_TOO_SHORT = The password is too short.
error-PASSWORD_TOO_LONG = The password is too long.
error-PASSWORD_CONTAINS_USER_ID = The password must not contain the user ID.
error-PASSWORD_NEEDS_DIGIT = The password must contain a digit.
error-PASSWORD_NEEDS_SYMBOL = The password must contain a symbol.
error-PASSWORD_SAME_AS_OLD = The new password must be different from the current one.
error-PASSWORD_RESET_TOKEN_INVALID = The reset link is invalid or has expired. Please request a new one.
error-PASSWORD_RESET_UNAVAILABLE = Password reset by email is not set up. Please contact the system administrator.
error-LAST_USER_ADMIN = At least one active administrator who can modify users must remain.
error-NO_USABLE_PLANT = You have no plant you can use. Please contact the system administrator.

error-UNIT_CODE_EMPTY = Unit is required.
error-UNIT_CODE_TOO_LONG = The unit is too long.
error-UNIT_DECIMALS_INVALID = Decimal places must be between 0 and 6.
error-UNIT_ALREADY_EXISTS = The unit already exists.
error-UNIT_IN_USE = The unit is in use and cannot be deleted; deactivate it instead.
error-PRODUCT_CLASS_CODE_INVALID = The product class code must be 1 to 10 characters.
error-PRODUCT_CLASS_ALREADY_EXISTS = The product class already exists.
error-PRODUCT_CLASS_NOT_FOUND = The product class does not exist.
error-PRODUCT_CLASS_COST_CENTER_INVALID = The cost center must be an active department whose control type is cost center or profit center.
error-PRODUCT_CLASS_PERIODS_INVALID = Periods cannot be less than 0.
error-PRODUCT_CLASS_IN_USE = The product class is in use and cannot be deleted; deactivate it instead.
error-ITEM_GROUP_CODE_INVALID = The item group code must be 1 to 10 characters.
error-ITEM_GROUP_ALREADY_EXISTS = The item group already exists.
error-ITEM_GROUP_NOT_FOUND = The item group does not exist.
error-ITEM_GROUP_SOURCE_INVALID = Invalid source code.
error-ITEM_GROUP_ABC_INVALID = The ABC code must be A, B or C.
error-ITEM_GROUP_REPLENISH_INVALID = The replenishment policy must be 0 to 5.
error-ITEM_GROUP_QUANTITY_INVALID = Quantities and days cannot be less than 0.
error-ITEM_GROUP_STOCK_KEEPER_INVALID = The stock keeper must be an active employee of this plant.
error-ITEM_GROUP_IN_USE = The item group is used by items and cannot be deleted; deactivate it instead.

error-ITEM_MISC_REQUIRED = Items starting with MISC require the item MISC first.
error-ITEM_GROUP_INACTIVE = The item group is inactive.
error-ITEM_STOCK_UNIT_REQUIRED = Stock unit is required.
error-ITEM_PRODUCT_CLASS_REQUIRED = Product class is required.
error-ITEM_SOURCE_INVALID = Invalid source code.
error-ITEM_ABC_INVALID = The ABC code must be A, B or C.
error-ITEM_REPLENISH_INVALID = The replenishment policy must be 0 to 5.
error-ITEM_REPLENISH_SOURCE_MISMATCH = Reorder point or fixed interval purchase requires source code M, S, P, V or Z.
error-ITEM_SUPPLIER_CONTROL_INVALID = Supplier control must be 0 to 3.
error-ITEM_LIFE_CYCLE_INVALID = Life cycle must be 1 to 6.
error-ITEM_ORDER_QTY_INVALID = Economic and average order quantities must be greater than 0.
error-ITEM_QUANTITY_INVALID = Quantities and days cannot be less than 0.
error-ITEM_STOCK_KEEPER_INVALID = The stock keeper must be an active employee of this plant.
error-ITEM_APPLY_DEPT_INVALID = The applying department must be an active department of this plant.
error-ITEM_INACTIVE = The item is invalid. Restore it first.

# 基本代碼：區域別、國別、地區、交運方式、報表抬頭（basic_code.rs）
error-REGION_CODE_EMPTY = Region code is required.
error-REGION_CODE_TOO_LONG = The region code is too long.
error-REGION_ALREADY_EXISTS = Region already exists.
error-REGION_NOT_FOUND = Region not found.
error-REGION_INACTIVE = Region is inactive.
error-REGION_IN_USE = Region is in use and cannot be deleted; deactivate it instead.
error-COUNTRY_CODE_EMPTY = Country code is required.
error-COUNTRY_CODE_TOO_LONG = The country code is too long.
error-COUNTRY_ALREADY_EXISTS = Country already exists.
error-COUNTRY_NOT_FOUND = Country not found.
error-COUNTRY_INACTIVE = Country is inactive.
error-COUNTRY_IN_USE = Country is in use and cannot be deleted; deactivate it instead.
error-AREA_CODE_EMPTY = Area code is required.
error-AREA_CODE_TOO_LONG = The area code is too long.
error-AREA_ALREADY_EXISTS = Area already exists.
error-AREA_NOT_FOUND = Area not found.
error-AREA_INACTIVE = Area is inactive.
error-AREA_IN_USE = Area is in use and cannot be deleted; deactivate it instead.
error-SHIPPING_METHOD_CODE_EMPTY = Shipping method code is required.
error-SHIPPING_METHOD_CODE_TOO_LONG = The shipping method code is too long.
error-SHIPPING_METHOD_ALREADY_EXISTS = Shipping method already exists.
error-SHIPPING_METHOD_NOT_FOUND = Shipping method not found.
error-SHIPPING_METHOD_INACTIVE = Shipping method is inactive.
error-SHIPPING_METHOD_IN_USE = Shipping method is in use and cannot be deleted; deactivate it instead.
error-REPORT_TITLE_CODE_EMPTY = Report title code is required.
error-REPORT_TITLE_CODE_TOO_LONG = The report title code is too long.
error-REPORT_TITLE_ALREADY_EXISTS = Report title already exists.
error-REPORT_TITLE_NOT_FOUND = Report title not found.
error-REPORT_TITLE_INACTIVE = Report title is inactive.
error-REPORT_TITLE_IN_USE = Report title is in use and cannot be deleted; deactivate it instead.

# 幣別（aooi050）
error-CURRENCY_CODE_EMPTY = Currency is required.
error-CURRENCY_CODE_TOO_LONG = The currency code is too long.
error-CURRENCY_DECIMALS_INVALID = Decimal places are out of range (prices, amounts and totals 0–6; exchange rates 0–10).
error-CURRENCY_RATE_METHOD_INVALID = The calculation method must be 1 (multiply) or 2 (divide).
error-CURRENCY_ALREADY_EXISTS = Currency already exists.
error-CURRENCY_NOT_FOUND = Currency not found.
error-CURRENCY_IN_USE = Currency is in use and cannot be deleted; deactivate it instead.
error-CURRENCY_INACTIVE = Currency is inactive.
error-EXCHANGE_RATE_ALREADY_EXISTS = An exchange rate for this currency and period already exists.
error-EXCHANGE_RATE_NOT_FOUND = Exchange rate not found.
error-EXCHANGE_RATE_PERIOD_INVALID = Year/month must be YYYYMM with month 01–12.
error-EXCHANGE_RATE_NEGATIVE = Exchange rates cannot be negative.
error-EXCHANGE_RATE_INACTIVE = The exchange rate is inactive; activate it before editing.
error-EXCHANGE_RATE_COPY_DATE_INVALID = The fill-up date cannot be later than today.
error-EXCHANGE_RATE_IMPORT_OVERWRITE_REQUIRED = Today's daily rates already exist; confirm to overwrite them.
error-EXCHANGE_RATE_SOURCE_INVALID = The rate source returned data in an unexpected format (possibly a website verification page); import an uploaded file instead.
error-EXCHANGE_RATE_SOURCE_UNAVAILABLE = The exchange rate source could not be reached.
error-EXCHANGE_RATE_KIND_INVALID = The Bank of Taiwan rate type must be cash or spot.

# 稅別（aooi150）
error-TAX_CODE_EMPTY = Tax code is required.
error-TAX_CODE_TOO_LONG = The tax code is too long.
error-TAX_DIRECTION_INVALID = Purchase/sales must be 1 (purchase) or 2 (sales).
error-TAX_INVOICE_COPIES_INVALID = Invoice copies must be 0, 2, 3, 4 or X.
error-TAX_CLASS_INVALID = Tax class must be 1 (taxable), 2 (zero-rated) or 3 (exempt).
error-TAX_RATE_INVALID = The tax rate must be between 0 and 100 with up to 4 decimal places.
error-TAX_ACCOUNT_REQUIRED = An account is required when the tax rate is not 0.
error-TAX_FILING_FORMAT_INVALID = The filing format does not match purchase/sales (purchase 21–29, sales 31–38, or XX).
error-TAX_TWO_COPY_MUST_INCLUDE_TAX = Prices on duplicate invoices must include tax (except filing format 28).
error-TAX_TYPE_ALREADY_EXISTS = Tax type already exists.
error-TAX_TYPE_NOT_FOUND = Tax type not found.
error-TAX_TYPE_IN_USE = Tax type is in use and cannot be deleted; deactivate it instead.

# 專案（aooi200）
error-PROJECT_CODE_EMPTY = Project is required.
error-PROJECT_CODE_TOO_LONG = The project code is too long.
error-PROJECT_DATE_RANGE_INVALID = The end date cannot be earlier than the start date.
error-PROJECT_ALREADY_EXISTS = Project already exists.
error-PROJECT_NOT_FOUND = Project not found.
error-PROJECT_IN_USE = Project is in use and cannot be deleted; deactivate it instead.

# 整體管理系統參數（aoos010）
error-ACCOUNTING_PERIODS_INVALID = Accounting periods must be 12 or 13.
error-RATE_BASIS_INVALID = Exchange rates used must be monthly or daily.
error-BASE_CURRENCY_IN_USE = Monthly or daily exchange rates already exist; the home currency cannot be changed.

# 會計期間（aoos020）
error-ACCOUNTING_YEAR_ALREADY_EXISTS = This fiscal year already exists.
error-ACCOUNTING_YEAR_NOT_FOUND = Fiscal year not found.
error-ACCOUNTING_PERIOD_COUNT_INVALID = Every period from 1 to the number of periods is required (no period 13 for 12 periods).
error-ACCOUNTING_PERIOD_DATE_INVALID = A period end date cannot be earlier than its start date.
error-ACCOUNTING_QUARTER_INVALID = Quarters must be 1–4 and must not decrease.
error-ACCOUNTING_PERIOD_YEAR_INVALID = Period dates must fall in the fiscal year or the next year.
error-ACCOUNTING_PERIODS_NOT_CONTINUOUS = Each period must start the day after the previous period ends.
error-ACCOUNTING_YEAR_OVERLAP = The periods overlap another fiscal year.
error-ACCOUNTING_PERIODS_IN_USE = Fiscal years already exist; the number of accounting periods cannot be changed.
error-ACCOUNTING_PERIOD_NOT_FOUND = The date is not in any accounting period, or the year has no such period.
error-ACCOUNTING_PERIOD_DATE_RANGE_INVALID = The date range is invalid (end before start, or longer than 10 years).

# 單位換算（aooi102、aooi103）
error-UNIT_CONVERSION_ALREADY_EXISTS = A conversion between these two units already exists (in either direction).
error-UNIT_CONVERSION_NOT_FOUND = Unit conversion not found.
error-UNIT_CONVERSION_QUANTITY_INVALID = Quantities must be greater than 0 with up to 3 decimals.
error-UNIT_CONVERSION_SAME_UNIT = The from and to units must differ.
error-UNIT_INACTIVE = The unit is inactive.

# EPUB 轉 TXT
error-EPUB_INVALID = The file is not a valid EPUB.
error-EPUB_ENCRYPTED = This EPUB is DRM-protected and cannot be converted.
error-EPUB_TOO_LARGE = The file exceeds the 100 MB limit.
error-EPUB_FORMAT_INVALID = The output format must be txt or pdf.
error-PDF_FONT_MISSING = The server has no CJK font available, so the PDF cannot be created.
error-PDF_GENERATION_FAILED = Failed to create the PDF.
error-EPUB_NOT_ENGLISH = This book is not in English, so it cannot be translated from English.
error-TRANSLATE_LIMIT_EXCEEDED = The book exceeds the translation limit (300,000 characters).
error-TRANSLATE_BUSY = A translation is already running; please wait for it to finish.
error-TRANSLATE_NOT_CONFIGURED = The translation service is not set up (the server needs Claude Code installed and signed in).
error-TRANSLATE_QUOTA_EXHAUSTED = The Claude subscription usage limit has been reached; try again later. Translated parts are kept and resubmitting resumes.
error-TRANSLATE_FAILED = The translation failed; please try again later.
error-TRANSLATE_CONSENT_REQUIRED = Please confirm first: translating sends the book text to Claude.
error-TRANSLATE_JOB_NOT_FOUND = The translation job was not found (it may have expired).
error-EPUB_BOOK_NOT_FOUND = The book was not found (the source may have removed it).
error-EPUB_SOURCE_UNAVAILABLE = This source is temporarily unavailable; please try again later.
error-URL_NOT_ALLOWED = This URL is not allowed (only http/https, and no internal network addresses).
error-URL_ROBOTS_DISALLOWED = The site's robots.txt does not allow reading this page.
error-URL_UNREACHABLE = Cannot reach this URL (connection failed, timed out, too many redirects or the server returned an error).

# GitHub EPUB 搜尋
error-GITHUB_RATE_LIMITED = The GitHub query limit has been reached; try again later.
error-GITHUB_UNAVAILABLE = Cannot connect to GitHub.
error-GITHUB_NOT_FOUND = Not found on GitHub.
error-EPUB_LIBRARY_DUPLICATE = This repo is already in your GitHub library.
error-EPUB_LIBRARY_FULL = Your GitHub library is full (at most 20 repos).

# 編碼性質（aooi401）
error-ENCODING_NATURE_CODE_EMPTY = Encoding nature is required.
error-ENCODING_NATURE_CODE_TOO_LONG = The encoding nature is too long (at most 2 characters).
error-ENCODING_NATURE_CATEGORY_INVALID = Category must be 1 to 6.
error-ENCODING_NATURE_ALREADY_EXISTS = The encoding nature already exists.
error-ENCODING_NATURE_NOT_FOUND = Encoding nature not found.
error-ENCODING_NATURE_IN_USE = The encoding nature is used by an encoding class; deactivate it instead.

# 編碼獨立段（aooi403）
error-ENCODING_SEGMENT_CODE_EMPTY = Segment No. is required.
error-ENCODING_SEGMENT_CODE_TOO_LONG = Segment No. is too long (at most 10 characters).
error-ENCODING_SEGMENT_LENGTH_INVALID = Length must be 1 to 30.
error-ENCODING_SEGMENT_ALREADY_EXISTS = The segment already exists.
error-ENCODING_SEGMENT_NOT_FOUND = Segment not found.
error-ENCODING_SEGMENT_IN_USE = The segment is used by an encoding class; deactivate it instead.
error-ENCODING_SEGMENT_INACTIVE = The segment is inactive; activate it before editing.
error-ENCODING_SEGMENT_LENGTH_HAS_VALUES = The length cannot change while the segment has codes; delete the codes first.
error-ENCODING_SEGMENT_VALUE_EMPTY = Code No. is required.
error-ENCODING_SEGMENT_VALUE_LENGTH_MISMATCH = Code No. must be exactly as long as the segment length.
error-ENCODING_SEGMENT_VALUE_INVALID_CHAR = Code No. cannot contain * % _ ? or spaces.
error-ENCODING_SEGMENT_VALUE_DUPLICATE = Code No. is already used in this segment.
error-ENCODING_SEGMENT_LINE_INVALID = Line No. must be a positive whole number.
error-ENCODING_SEGMENT_LINE_DUPLICATE = Line No. already exists.
error-ENCODING_SEGMENT_LINE_NOT_FOUND = Line not found.

# 編碼年對照（aooi404）
error-ENCODING_YEAR_INVALID = Year must be 1 to 9999.
error-ENCODING_YEAR_CODE_EMPTY = Code No. is required.
error-ENCODING_YEAR_CODE_TOO_LONG = Code No. is too long (at most 10 characters).
error-ENCODING_YEAR_CODE_INVALID_CHAR = Code No. cannot contain * % _ ? or spaces.
error-ENCODING_YEAR_ALREADY_EXISTS = This year already has a code of the same length.
error-ENCODING_YEAR_NOT_FOUND = Year code not found.
error-ENCODING_YEAR_LENGTH_CHANGED = The code must keep the same length; delete and add it again to change the length.

# 編碼類別（aooi402）
error-ENCODING_CLASS_CODE_EMPTY = Encoding class is required.
error-ENCODING_CLASS_CODE_TOO_LONG = Encoding class is too long (at most 10 characters).
error-ENCODING_CLASS_NAME_EMPTY = Name is required.
error-ENCODING_CLASS_NATURE_INVALID = Nature must be an existing, active encoding nature.
error-ENCODING_CLASS_ALREADY_EXISTS = The encoding class already exists.
error-ENCODING_CLASS_NOT_FOUND = Encoding class not found.
error-ENCODING_CLASS_IN_USE = The encoding class is in use; deactivate it instead.
error-ENCODING_CLASS_INACTIVE = The encoding class is inactive; activate it before editing.
error-ENCODING_CLASS_TYPE_INVALID = Type must be 1 to 9.
error-ENCODING_CLASS_TYPE_NOT_ALLOWED = This type cannot be used with this nature: year, quarter, month, week, day and field are for lot and serial numbers only, and lot and serial numbers cannot use independent segments.
error-ENCODING_CLASS_TYPE_NOT_SUPPORTED = Type 4 (field value) is not available yet; it comes with lot and serial number management.
error-ENCODING_CLASS_SEGMENT_LENGTH_INVALID = Length must be 1 to 30.
error-ENCODING_CLASS_DATE_LENGTH_INVALID = Quarter, month, week and day are always 2 characters.
error-ENCODING_CLASS_YEAR_LENGTH_NOT_DEFINED = There is no year code of this length; add it in Encoding Year Codes first.
error-ENCODING_CLASS_FIXED_EMPTY = Fixed value is required.
error-ENCODING_CLASS_FIXED_INVALID_CHAR = Fixed value cannot contain * % _ ? or spaces.
error-ENCODING_CLASS_FIXED_LENGTH_MISMATCH = Fixed value must be exactly as long as the segment length.
error-ENCODING_CLASS_SEGMENT_INVALID = Independent segment must be an existing, active segment.
error-ENCODING_CLASS_SERIAL_DUPLICATE = An encoding class can have only one running-number segment.
error-ENCODING_CLASS_LINE_INVALID = Segment No. must be a positive whole number.
error-ENCODING_CLASS_LINE_DUPLICATE = Segment No. already exists.
error-ENCODING_CLASS_LINE_NOT_FOUND = Segment not found.

# 自動編號（aooi400 / s_auno）
error-ENCODING_RULE_VALUE_REQUIRED = Pick a valid code for each independent segment.
error-ENCODING_RULE_SERIAL_EXHAUSTED = The running number is used up; lengthen it or use another encoding class.
error-ENCODING_RULE_YEAR_CODE_MISSING = There is no year code for this year; add it in Encoding Year Codes first.
error-ENCODING_RULE_PERIOD_MISSING = Today is not in any accounting period, so the quarter is unknown; add the accounting periods first.

# 料件自動編號（aimi100 / aza28）
error-ITEM_AUTO_NUMBER_DISABLED = Item auto-numbering is turned off in the general management parameters.
error-ENCODING_CLASS_NOT_FOR_ITEM = This encoding class cannot be used for item numbers; it must be an active class whose nature is item number.

# 集團中心（aooi600 / geu_file、gev_file）
error-GROUP_CENTER_CODE_EMPTY = The group center code is required.
error-GROUP_CENTER_CODE_TOO_LONG = The group center code can have at most 10 characters.
error-GROUP_CENTER_CATEGORY_INVALID = The category must be 1 to 7 (data, forecast, order, purchase, credit control, account or funds center).
error-GROUP_CENTER_ALREADY_EXISTS = This group center already exists.
error-GROUP_CENTER_NOT_FOUND = This group center does not exist.
error-GROUP_CENTER_IN_USE = This group center has transfer records and cannot be deleted; deactivate it instead.
error-GROUP_CENTER_HAS_DATA_TYPES = This data center has data types, so it cannot be changed to another category.
error-GROUP_CENTER_NOT_DATA_CENTER = Only an active data center (category 1) can have data types and transfer settings.
error-DATA_CENTER_TYPE_INVALID = The data type must be 0 to 8 (basic data, item, BOM, ECN, customer, vendor, account, price or exchange rate).
error-DATA_CENTER_TYPE_ALREADY_EXISTS = This data type and plant already belong to a data center.
error-DATA_CENTER_TYPE_NOT_FOUND = This data type and plant were not found.

# 資料控管原則（aooi601 / gev_file、gey_file）
error-DATA_CONTROL_PLANT_ALREADY_EXISTS = This data type and plant are already on the data control list (as a data center or a controlled plant).
error-DATA_CONTROL_PLANT_NOT_FOUND = This controlled plant was not found. Data center plants are changed in Group Center Maintenance.
error-DATA_CONTROL_TYPE_NOT_MAINTAINABLE = Data type 0 (basic data) has no data control rules.
error-DATA_CONTROL_NO_DATA_CENTER = This data type has no active data center yet. Set one in Group Center Maintenance first.
error-DATA_CONTROL_RULE_NOT_ALLOWED = Only controlled plants can have field conditions, not data center plants.
error-DATA_CONTROL_RULE_ALREADY_EXISTS = This field already has a condition.
error-DATA_CONTROL_RULE_NOT_FOUND = This field condition was not found.
error-DATA_CONTROL_FIELD_INVALID = This field cannot be set for this data type.
error-DATA_CONTROL_CONDITION_INVALID = The condition must be 1 to 11.
error-DATA_CONTROL_VALUE_INVALID = Values are required (up to 255 characters, separated by ;) and cannot contain ' " ) + > < = *. Greater and less conditions take one value.
error-DATA_CONTROL_FIELD_REJECTED = This value does not meet the data control conditions of this plant.

# 資料中心拋轉設定（aooi602 / gew_file、gez_file）
error-CARRY_SETTING_ALREADY_EXISTS = This data center and data type already have a carry setting.
error-CARRY_SETTING_NOT_FOUND = This carry setting was not found.
error-CARRY_SETTING_NO_DATA_TYPE = This data center does not own this data type. Set it in Group Center Maintenance first.
error-CARRY_SETTING_IN_USE = This carry setting has transfer records and cannot be deleted; change the carry mode instead.
error-CARRY_MODE_INVALID = The carry mode must be 1 (auto), 2 (after asking) or 3 (manual).
error-CARRY_TARGET_ALREADY_EXISTS = This plant is already a target.
error-CARRY_TARGET_NOT_FOUND = This target plant was not found.
error-CARRY_TARGET_IS_SOURCE = The source plant does not need to carry to itself.
error-CARRY_NOTIFY_INVALID = Notify must be e-mail addresses separated by ; (up to 1500 characters).
error-CARRY_FIELD_INVALID = This field cannot be used in carry conditions for this data type.
error-CARRY_CONDITION_ALREADY_EXISTS = This field already has a carry condition.
error-CARRY_CONDITION_NOT_FOUND = This carry condition was not found.
error-CARRY_DEFAULT_ALREADY_EXISTS = This field already has a default value.
error-CARRY_DEFAULT_NOT_FOUND = This default value was not found.
error-CARRY_DEFAULT_FIELD_INVALID = This field cannot have a default value.
error-CARRY_DEFAULT_VALUE_INVALID = The default value is required (up to 255 characters).
error-CARRY_BASIC_DATA_NOT_ALLOWED = Basic data cannot have carry conditions or default values, and "plant may delete" cannot be changed.

# 基本資料拋轉（aoop603）
error-BASIC_CARRY_NOT_SOURCE = This plant is not the basic data source plant of any active data center, so basic data cannot be carried here.
error-BASIC_CARRY_NO_TARGET = The basic data of this data center has no target plants. Set them in Carry Setting Maintenance first.
error-BASIC_CARRY_NOTHING_SELECTED = Select at least one kind of data and one target plant.
error-BASIC_CARRY_TARGETS_FIXED = The carry mode is not manual, so data is carried to all target plants and you cannot choose plants.
error-BASIC_CARRY_ITEM_INVALID = This kind of data cannot be carried.
error-BASIC_CARRY_REFERENCE_MISSING = A record refers to data that does not exist in the target plant.

# 料件拋轉（aimi100 carry）
error-ITEM_CARRY_NOT_CONFIRMED = Only active, confirmed items can be carried.
error-ITEM_CARRY_NOT_SOURCE = This plant is not the item source plant of any active data center, so items cannot be carried here.
error-ITEM_CARRIED_NOT_EDITABLE = This item was carried from the data center and cannot be changed in this plant.
error-ITEM_CARRIED_NOT_DELETABLE = This item was carried from the data center and cannot be deleted in this plant.
error-WAREHOUSE_CODE_EMPTY = Warehouse cannot be blank.
error-WAREHOUSE_CODE_TOO_LONG = Warehouse code is too long (max 10).
error-WAREHOUSE_ALREADY_EXISTS = This warehouse already exists.
error-WAREHOUSE_NOT_FOUND = The warehouse does not exist or is inactive.
error-WAREHOUSE_INACTIVE = The warehouse is inactive. Activate it first.
error-WAREHOUSE_IN_USE = The warehouse is used by items or item groups and cannot be deleted. Deactivate it instead.
error-WAREHOUSE_CLASS_INVALID = Storage class must be S (stock) or W (work in process).
error-WAREHOUSE_MRP_REQUIRES_USABLE = An unusable warehouse or location cannot be MRP usable.
error-WAREHOUSE_PRIORITY_INVALID = Priority cannot be less than 0.
error-WAREHOUSE_COST_CENTER_INVALID = The cost center must be an active cost or profit center department.
error-WAREHOUSE_LOCATION_CODE_EMPTY = Location cannot be blank.
error-WAREHOUSE_LOCATION_CODE_TOO_LONG = Location code is too long (max 10).
error-WAREHOUSE_LOCATION_ALREADY_EXISTS = This warehouse already has this location.
error-WAREHOUSE_LOCATION_NOT_FOUND = The location does not exist.
error-WAREHOUSE_LOCATION_IN_USE = The location is used by items or item groups and cannot be deleted.
error-WAREHOUSE_LOCATION_ACCOUNT_REQUIRED = With account method 2 the location account is required.
error-WAREHOUSE_ACCOUNT_METHOD_INVALID = Account method must be 0, 1 or 2.
