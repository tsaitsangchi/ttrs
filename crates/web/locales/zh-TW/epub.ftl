# EPUB 轉換（epub_to_text.feature、epub_to_pdf.feature）
epub-breadcrumb = 系統管理 / EPUB Converter
epub-title = EPUB 轉換
epub-hint = 選擇一個或多個 EPUB 電子書，系統會依下方選項轉成 TXT（UTF-8、Windows 換行）或 PDF（A5 直式、含書籤與頁碼），並下載同名檔案。檔案不會保存在系統中；有加密（DRM）的書無法轉換；單一檔案上限 100 MB。
epub-action-choose = 選擇 EPUB 檔案
epub-msg-running = 轉換中…
epub-msg-done = { $file }：已下載（{ $title }／{ $author }，{ $chapters } 章）
epub-msg-failed = { $file }：轉換失敗，{ $error }
epub-field-format = 輸出格式
epub-format-txt = TXT 文字檔
epub-format-pdf = PDF 電子書
epub-field-s2t = 簡體轉繁體（書名、作者與內文）
epub-field-lang = 語言
epub-lang-none = 不翻譯（原文）
epub-lang-translate = 英文翻成繁體中文
epub-field-consent = 我了解書的內容會送到 Claude 翻譯，且我有權使用這本書
epub-hint-translate = 翻譯會使用伺服器 Claude 帳號的訂閱用量：一本書可能要數分鐘到數十分鐘，單本上限 30 萬字元，同一時間只能翻一本；用量不足時會停下來，已翻的部分保留 24 小時，重新上傳同一本可續翻。只收英文書。
epub-msg-translating = { $file }：翻譯中，已翻 { $done }／{ $total } 段
