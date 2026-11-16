# language: zh-TW
# TipTop 對應：無（TTRS 新增的工具作業）
# 新系統名稱：EPUB to Text EPUB 轉 TXT（epub-to-text，ET1）
# 規則來源：EPUB 3／EPUB 2 規格——META-INF/container.xml 指向 OPF；OPF 的 spine 決定閱讀順序；
#           META-INF/encryption.xml 表示有加密（DRM）。另參考使用者先前的 Python 版工具 /home/hugo/tools/epub2txt。
# 狀態：ET1–ET8 依建議確認（2026-11-12），進入 Red
#
# 確認事項（皆採建議）：
#  ET1. 程式代號 epub-to-text，掛在「系統管理」選單；TipTop 沒有對應作業，不需要 legacy-map。可以嗎？
#  ET2. 使用方式（使用者 2026-11-12 說明）：在畫面上傳 EPUB → 伺服器轉成 TXT → 直接下載同名的 .txt。可一次選多本，逐一轉換下載；
#       轉完在畫面列出每本的書名、作者、章節數（方便確認轉的是哪本）。可以嗎？
#  ET3. 不保存檔案：EPUB 與轉出的文字只在記憶體處理，不存資料庫、不留檔；稽核紀錄只記使用者、檔名與大小。可以嗎？
#  ET4. 單一檔案上限 100 MB，超過回 EPUB_TOO_LARGE。可以嗎？
#  ET5. 轉換規則：依 OPF spine 的順序串接各章；標題、段落、<br> 轉成換行，章節之間空一行；去掉 script、style 與其他標籤；
#       HTML entity 解碼；圖片略過；連續空行最多保留一行。有加密（META-INF/encryption.xml，DRM）的書不轉換，回 EPUB_ENCRYPTED；
#       不是 EPUB（不是 zip、沒有 container.xml 或 OPF）回 EPUB_INVALID。可以嗎？
#  ET6. 輸出：UTF-8、不加 BOM、換行用 CRLF（Windows 記事本可直接開）。可以嗎？
#  ET7. 權限與功能鈕：只有一個功能「執行」（run）；ADMIN 預設有。工具作業沒有資料，ADR 0013 的標準功能鈕（新增、修改、刪除、查詢、
#       列印、匯出、上下筆、失效、複製、異動記錄、確認）全部列為例外（同每日匯率複製只有 run）。可以嗎？
#  ET8. 實作：轉換邏輯放 app 層（輸入 EPUB 位元組、輸出文字，不碰資料庫）；新增 Rust 套件 zip（讀 EPUB）與 quick-xml（解析 XML／XHTML，
#       工作區已間接使用）。可以嗎？

功能: EPUB 轉 TXT（EPUB to Text）
  身為 使用者
  我想要 把 EPUB 電子書轉成純文字檔
  以便 用任何文字編輯器閱讀或搜尋內容

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"

  # ET5：依 spine 順序、標籤轉換行、章節之間空一行；ET2 書名、作者、章節數
  場景: ET-1 依閱讀順序轉成文字
    假設 有一本 EPUB，書名 "測試之書"，作者 "王小明"，閱讀順序為:
      | 檔案       | 內容                                                        |
      | ch2.xhtml  | <h1>第二章</h1><p>乙段落。</p>                              |
      | ch1.xhtml  | <h1>第一章</h1><p>甲段落&amp;一。</p><p>第二行<br/>換行</p> |
    而且 閱讀順序為 "ch1.xhtml, ch2.xhtml"
    當 使用者將這本 EPUB 轉成文字
    那麼 操作應成功
    而且 書名應為 "測試之書"，作者 "王小明"，共 2 章
    而且 轉出的文字應為:
      """
      第一章
      甲段落&一。
      第二行
      換行

      第二章
      乙段落。
      """

  # ET5：script、style、圖片略過
  場景: ET-2 略過 script、style 與圖片
    假設 有一本 EPUB，書名 "書"，作者 "某"，閱讀順序為:
      | 檔案      | 內容                                                                          |
      | c1.xhtml  | <style>p{}</style><p>正文</p><script>x()</script><img src="a.png"/><p>結尾</p> |
    當 使用者將這本 EPUB 轉成文字
    那麼 轉出的文字應為:
      """
      正文
      結尾
      """

  # ET5：加密（DRM）
  場景: ET-3 有加密的 EPUB 不轉換
    假設 有一本有加密的 EPUB
    當 使用者將這本 EPUB 轉成文字
    那麼 應出現錯誤 "EPUB_ENCRYPTED"

  # ET5：不是 EPUB
  場景: ET-4 不是 EPUB 的檔案
    當 使用者將內容為 "hello" 的檔案轉成文字
    那麼 應出現錯誤 "EPUB_INVALID"

  # ET4：大小上限
  場景: ET-5 超過大小上限
    當 使用者將 101 MB 的檔案轉成文字
    那麼 應出現錯誤 "EPUB_TOO_LARGE"

  # ET6：輸出 CRLF、UTF-8 無 BOM
  場景: ET-6 下載的文字檔格式
    假設 有一本 EPUB，書名 "書"，作者 "某"，閱讀順序為:
      | 檔案      | 內容                    |
      | c1.xhtml  | <p>一</p><p>二</p>      |
    當 使用者將這本 EPUB 轉成文字
    那麼 下載的文字檔應為 UTF-8 不含 BOM，換行為 CRLF
