# Plant Maintenance 營運中心維護（TipTop aooi901）
# 語系：zh-TW。ID 規則見 docs/adr/0005-i18n.md；三個語系的 ID 必須一致。

plant-breadcrumb = 整體管理 / Plant Maintenance
plant-title = 營運中心維護
plant-list-title = 營運中心
plant-field-id = 營運中心代碼
plant-field-name = 營運中心名稱
plant-field-schema-name = 資料庫代碼
plant-hint-schema-name = 空白時自動給下一號（ds01…）
plant-field-time-zone = 時區
plant-field-address = 地址
plant-field-phone = 營運中心電話
plant-field-fax = 營運中心傳真
plant-field-active = 資料有效
plant-field-created-by = 資料所有者
plant-field-created-on = 建立日期
plant-field-updated-by = 最後修改者
plant-field-updated-on = 最後修改日期
plant-field-copy-as = 複製為新代號
plant-status-active = 有效
plant-status-inactive = 停用
plant-action-activate = 啟用
plant-action-deactivate = 停用
plant-hint-time-zone = 時區用於計算營運中心的「今天」（營運中心切換上線後生效）。
plant-msg-saved = 營運中心 { $id } 已存檔
plant-msg-deleted = 營運中心 { $id } 已刪除
plant-msg-activated = 營運中心 { $id } 已啟用
plant-msg-deactivated = 營運中心 { $id } 已停用
plant-msg-copied = 已將營運中心 { $from } 複製為 { $to }
plant-note-schema = 新增營運中心只需建立一筆資料，不必另建資料庫或 schema；TipTop 的資料庫代碼、DB link、上線人數等設定不再使用。有使用者或業務資料的營運中心不可刪除，請改為停用。
