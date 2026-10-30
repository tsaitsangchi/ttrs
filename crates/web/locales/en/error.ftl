# 錯誤訊息：ID 為 error-<錯誤代碼>（DomainError::code()）；zh-TW 一律使用伺服器回傳的訊息
# 語系：en。ID 規則見 docs/adr/0005-i18n.md；三個語系的 ID 必須一致。

error-PROGRAM_NO_EMPTY = Program ID is required.
error-PROGRAM_NO_TOO_LONG = Program ID is too long (max 20).
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
