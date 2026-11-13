# EPUB 轉換（epub_to_text.feature、epub_to_pdf.feature）
epub-breadcrumb = 系统管理 / EPUB Converter
epub-title = EPUB 转换
epub-hint = 选择一个或多个 EPUB 电子书，系统会按下方选项转成 TXT（UTF-8、Windows 换行）或 PDF（A5 竖向、含书签与页码），并下载同名文件。文件不会保存在系统中；有加密（DRM）的书无法转换；单一文件上限 50 MB。
epub-action-choose = 选择 EPUB 文件
epub-msg-running = 转换中…
epub-msg-done = { $file }：已下载（{ $title }／{ $author }，{ $chapters } 章）
epub-msg-failed = { $file }：转换失败，{ $error }
epub-field-format = 输出格式
epub-format-txt = TXT 文本文件
epub-format-pdf = PDF 电子书
epub-field-s2t = 简体转繁体（书名、作者与正文）
epub-field-lang = 语言
epub-lang-none = 不翻译（原文）
epub-lang-translate = 英文翻成繁体中文
epub-field-consent = 我了解书的内容会发送到 Claude 翻译，且我有权使用这本书
epub-hint-translate = 翻译会使用服务器 Claude 账号的订阅用量：一本书可能要几分钟到几十分钟，单本上限 30 万字符，同一时间只能翻一本；用量不足时会停下来，已翻的部分保留 24 小时，重新上传同一本可续翻。只收英文书。
epub-msg-translating = { $file }：翻译中，已翻 { $done }／{ $total } 段
