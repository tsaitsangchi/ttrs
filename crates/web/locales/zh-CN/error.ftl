# 錯誤訊息：ID 為 error-<錯誤代碼>（DomainError::code()）；zh-TW 一律使用伺服器回傳的訊息
# 語系：zh-CN。ID 規則見 docs/adr/0005-i18n.md；三個語系的 ID 必須一致。

error-PROGRAM_NO_EMPTY = 程序代号不可空白。
error-PROGRAM_NO_TOO_LONG = 程序代号长度不可超过 20 码。
error-PROGRAM_ALREADY_EXISTS = 程序已存在。
error-PROGRAM_NOT_FOUND = 程序不存在。
error-MODULE_EMPTY = 所属模块不可空白。
error-MODULE_NOT_FOUND = 模块不存在。
error-MODULE_NOT_ALLOWED = 共用程序模块不可作为作业程序的所属模块。
error-REPORT_TYPE_REQUIRED = 报表程序必须指定报表类型。
error-MENU_HAS_NO_PERMISSION = 菜单不是可执行程序，不可设定权限。
error-PERMISSION_CLASS_NOT_FOUND = 权限类别不存在。
error-QUERY_DEFINITION_NOT_FOUND = 经由 p_query 执行的程序必须先建立查询定义。
error-FIELD_TOO_LONG = 栏位长度超过上限。
error-LANGUAGE_NOT_SUPPORTED = 不支持的语系（zh-TW、en、zh-CN）。
error-QBE_INVALID_SYNTAX = 查询条件格式错误。
error-QBE_INVALID_NUMBER = 查询条件不是数字。
error-QBE_INVALID_DATE = 查询条件不是正确的日期（YYYY/MM/DD）。
error-QBE_FIELD_NOT_ALLOWED = 此栏位不可作为查询条件。
error-INFRA_ERROR = 系统错误，请稍后再试。
