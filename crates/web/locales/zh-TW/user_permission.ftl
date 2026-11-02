# User Special Permission Maintenance 使用者特殊權限維護（TipTop p_zxw）
# 語系：zh-TW。ID 規則見 docs/adr/0005-i18n.md；三個語系的 ID 必須一致。

uperm-breadcrumb = 系統管理 / User Special Permission Maintenance
uperm-title = 使用者特殊權限維護
uperm-list-title = 特殊權限
uperm-note = 使用者有特殊權限時，只使用這裡列出的權限類別與程式，不再使用使用者維護設定的權限類別；全部刪除後回到原權限類別。多列給同一程式時取較寬的權限（隨伺服器端權限檢查上線）。
uperm-kind-class = 權限類別
uperm-kind-program = 程式
uperm-field-user = 使用者代號
uperm-field-user-name = 使用者名稱
uperm-field-user-class = 使用者維護的權限類別
uperm-field-state = 目前使用
uperm-state-class = 使用者維護的權限類別
uperm-state-special = 特殊權限
uperm-field-line = 項次
uperm-field-kind = 分類
uperm-field-target = 權限類別／程式代號
uperm-field-target-name = 說明／程式名稱
uperm-field-include-own = 先加入原權限類別 { $class }
uperm-field-target-user = 複製到使用者
uperm-msg-none = 沒有特殊權限，使用使用者維護設定的權限類別
uperm-msg-removed = 已移除第 { $line } 項特殊權限
uperm-msg-added = 已新增 { $target } 的特殊權限
uperm-msg-copied = 已複製 { $count } 筆特殊權限到 { $target }
uperm-dialog-add = 新增特殊權限（{ $user }）
uperm-dialog-edit = 修改特殊權限（{ $user } / { $program }）
uperm-dialog-copy = 複製 { $user } 的特殊權限
uperm-dialog-delete-all = 刪除 { $user } 的全部特殊權限
uperm-hint-first = 這是此使用者的第一筆特殊權限；有特殊權限後就不再使用使用者維護的權限類別，建議先加入原權限類別。
uperm-hint-add = 權限類別須有效；程式不可為選單，允許執行功能預設為程式的全部功能、資料權限 0，新增後可再修改。
uperm-hint-copy = 目標使用者必須還沒有任何特殊權限。
uperm-confirm-delete-all = 將刪除此使用者的全部特殊權限，之後改用使用者維護設定的權限類別。是否執行？
