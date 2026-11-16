# language: zh-TW
# TipTop 對應：無（TTRS 新增的工具作業；擴充 epub_web_search.feature 的 EPUB 搜尋）
# 新系統名稱：沿用 github-epub-search（畫面「EPUB 搜尋」多一個「貼上網址」區）
# 規則來源：使用者 2026-11-13 要求「系統能對任意網址下載 EPUB」。
# 狀態：UD1–UD8 依建議確認（2026-11-16 使用者「往下做」），進入 Red
#
# 範圍說明：由使用者提供網址，系統下載或讀取「那一頁」；系統不自己到網路上搜尋、爬站，也不維護網站清單。
# 網址是使用者自己選的，使用權利由使用者自己判斷（同 WS5）。系統不繞過登入、付費牆或 DRM。
#
#  UD1. 畫面：EPUB 搜尋頁加「貼上網址」欄位與「讀取」按鈕。可以嗎？
#  UD2. 兩種網址：(a) 直接指向 EPUB 檔（內容是 EPUB 就視為一本書）；(b) 一般網頁——伺服器只讀這一頁（上限 2 MB），
#       列出頁面上副檔名為 .epub 的連結（最多 100 個，標示檔名與網址），使用者選一個再下載；不會再點進其他頁面。可以嗎？
#  UD3. 下載與轉換：沿用現有流程——下載原始 EPUB，或轉成 TXT／PDF、簡轉繁、英文翻譯；上限 50 MB、不保存檔案；
#       內容不是 EPUB（不是 zip、沒有 container.xml）回 EPUB_INVALID。可以嗎？
#  UD4. 安全（伺服器代為連線，必須防止被拿去連內部網路，SSRF）：
#       只允許 http／https；連線前把網域解析成 IP，位址是本機（127.*、::1）、私有網段（10.*、172.16–31.*、192.168.*、fc00::/7）、
#       連結本地（169.254.*，含雲端中繼資料位址）、保留位址者一律拒絕（回 URL_NOT_ALLOWED）；轉址最多 5 次，每次轉址都重新檢查；
#       連線逾時 30 秒；不帶 cookie 與任何憑證；連線時只送通用 User-Agent，不帶使用者資料。可以嗎？
#  UD5. robots.txt：讀取網頁（UD2 的 b）前先看該網站的 robots.txt，禁止時不讀（回 URL_ROBOTS_DISALLOWED）；
#       直接下載使用者給的 EPUB 檔連結不檢查。可以嗎？
#  UD6. 稽核紀錄：記使用者、網址（不含查詢字串）與位元組數。可以嗎？
#  UD7. 權限：沿用 github-epub-search 的 run；新增錯誤代碼 URL_NOT_ALLOWED、URL_ROBOTS_DISALLOWED、URL_UNREACHABLE（三語翻譯）。可以嗎？
#  UD8. 測試：BDD 用記憶體假的網路（不連外網，位址解析也用假的）驗證 SSRF 防護與頁面解析；另有需要網路、預設不執行的實測。可以嗎？
#
# 不做（不是本功能）：自動搜尋或爬取全網路、維護網站清單、繞過登入／付費／DRM。
# 不移植的欄位：無。不新增資料表。ADR 0013 標準功能鈕：沿用 GE7 的例外。

功能: 以網址下載 EPUB（EPUB URL Download）
  身為 使用者
  我想要 貼上一個網址就下載或列出其中的 EPUB
  以便 取得我自己找到的書

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"

  場景: UD-1 直接給 EPUB 檔的網址
    假設 網址 "https://books.example.com/a/dream.epub" 回傳一本 EPUB，書名 "紅樓夢"
    當 使用者讀取網址 "https://books.example.com/a/dream.epub"
    那麼 應視為一本書，書名 "紅樓夢"

  場景: UD-2 給網頁時列出頁面上的 EPUB 連結
    假設 網址 "https://books.example.com/list" 是網頁，含連結:
      | 連結                                  |
      | /files/a.epub                         |
      | https://cdn.example.com/b.EPUB        |
      | /about.html                           |
    當 使用者讀取網址 "https://books.example.com/list"
    那麼 應列出 EPUB 連結:
      | 檔名   | 網址                                    |
      | a.epub | https://books.example.com/files/a.epub  |
      | b.EPUB | https://cdn.example.com/b.EPUB          |

  場景: UD-3 下載列出的其中一本並轉換
    假設 網址 "https://books.example.com/files/a.epub" 回傳一本 EPUB，書名 "測試"
    當 使用者轉換網址 "https://books.example.com/files/a.epub"，輸出 "txt"
    那麼 操作應成功
    而且 轉出的檔名應為 "a.txt"

  場景大綱: UD-4 不允許連到內部網路
    當 使用者讀取網址 "<網址>"
    那麼 應出現錯誤 "URL_NOT_ALLOWED"

    例子:
      | 網址                                       |
      | http://localhost:3000/api/users            |
      | http://127.0.0.1/a.epub                    |
      | http://10.10.130.40/a.epub                 |
      | http://192.168.1.5/a.epub                  |
      | http://169.254.169.254/latest/meta-data/   |
      | http://[::1]/a.epub                        |
      | ftp://books.example.com/a.epub             |
      | file:///etc/passwd                         |

  場景: UD-5 網域解析到內部位址也不允許
    假設 網域 "evil.example.com" 解析到 "10.0.0.8"
    當 使用者讀取網址 "http://evil.example.com/a.epub"
    那麼 應出現錯誤 "URL_NOT_ALLOWED"

  場景: UD-6 轉址到內部位址不允許
    假設 網址 "https://books.example.com/go" 轉址到 "http://127.0.0.1:8080/x.epub"
    當 使用者讀取網址 "https://books.example.com/go"
    那麼 應出現錯誤 "URL_NOT_ALLOWED"

  場景: UD-7 轉址超過 5 次
    假設 網址 "https://books.example.com/loop" 一直轉址到自己
    當 使用者讀取網址 "https://books.example.com/loop"
    那麼 應出現錯誤 "URL_UNREACHABLE"

  場景: UD-8 內容不是 EPUB
    假設 網址 "https://books.example.com/x.epub" 回傳內容 "hello"
    當 使用者轉換網址 "https://books.example.com/x.epub"，輸出 "txt"
    那麼 應出現錯誤 "EPUB_INVALID"

  場景: UD-9 超過 50 MB 不下載
    假設 網址 "https://books.example.com/big.epub" 的內容有 60 MB
    當 使用者轉換網址 "https://books.example.com/big.epub"，輸出 "txt"
    那麼 應出現錯誤 "EPUB_TOO_LARGE"

  場景: UD-10 robots.txt 禁止時不讀網頁
    假設 網站 "https://books.example.com" 的 robots.txt 禁止所有路徑
    而且 網址 "https://books.example.com/list" 是網頁，含連結:
      | 連結          |
      | /files/a.epub |
    當 使用者讀取網址 "https://books.example.com/list"
    那麼 應出現錯誤 "URL_ROBOTS_DISALLOWED"

  場景: UD-11 稽核紀錄只記網址不含查詢字串
    假設 網址 "https://books.example.com/a.epub?token=secret" 回傳一本 EPUB，書名 "測試"
    當 使用者讀取網址 "https://books.example.com/a.epub?token=secret"
    那麼 稽核紀錄應有網址 "books.example.com/a.epub" 的 "github-epub-search" "query" 一筆，且內容不含 "secret"
