# Permission Class Maintenance 權限類別維護（TipTop p_zw）
# 語系：zh-TW。ID 規則見 docs/adr/0005-i18n.md；三個語系的 ID 必須一致。

class-breadcrumb = 系統管理 / Permission Class Maintenance
class-title = 權限類別維護
class-list-title = 權限類別
class-field-code = 權限類別
class-field-name = 權限說明
class-field-menu = 使用目錄編號
class-field-idle-control = 閒置設定
class-field-idle-action = 閒置處理
class-field-idle-seconds = 閒置時間（秒）
class-opt-idle-off = 不控制
class-opt-idle-class = 依權限類別設定
class-opt-idle-system = 依整體系統設定
class-opt-idle-warn = 僅提出警告
class-opt-idle-warn-close = 警告後關閉程式
class-opt-idle-close = 直接關閉程式
class-opt-none = （未設定）
class-action-activate = 啟用
class-action-deactivate = 停用
class-action-permissions = 維護群組執行權限
class-action-update-menus = 更新所有使用者目錄編號
class-dialog-new = 新增權限類別
class-dialog-edit = 修改權限類別 { $code }
class-dialog-update-menus = 更新所有使用者目錄編號
class-confirm-update-menus = 將所有使用者的選單改為其權限類別的使用目錄編號（未設定使用目錄編號的權限類別不變）。是否執行？
class-hint-idle = 只有「依權限類別設定」時才使用本類別的閒置處理與閒置時間。
class-msg-saved = 權限類別 { $code } 已存檔
class-msg-deleted = 權限類別 { $code } 已刪除
class-msg-activated = 權限類別 { $code } 已啟用
class-msg-deactivated = 權限類別 { $code } 已停用
class-msg-confirmed = 已確認權限類別 { $code }
class-msg-unconfirmed = 已取消確認權限類別 { $code }
class-msg-copied = 已將權限類別 { $from } 複製為 { $to }
class-field-copy-as = 複製為
class-msg-menus-updated = 已更新 { $count } 位使用者的選單
class-note-permissions = 群組執行權限（p_zy）與列印權限（p_zy2）的維護將於後續移植；新增的權限類別不會自動帶入任何程式權限。
# 查詢模式提示（WQ2：可輸入代碼或繁中顯示文字）
class-hint-qbe-idle-control = 1 不控制 | 2 依權限類別 | 3 依系統
class-hint-qbe-idle-action = 1 僅警告 | 2 警告後關閉 | 3 直接關閉
