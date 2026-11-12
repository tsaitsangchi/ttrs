# language: zh-TW
# TipTop 對應：無（TTRS 新增的工具作業；TTRS 為測試系統）
# 新系統名稱：GitHub EPUB Search GitHub EPUB 搜尋（github-epub-search，GE1）
# 規則來源：GitHub REST API——GET /search/repositories（關鍵字搜尋 repo，回傳授權 license.spdx_id）、
#           GET /repos/{owner}/{repo}/git/trees/{branch}?recursive=1（列出檔案；truncated 表示檔案太多未列完）、
#           檔案內容由 raw.githubusercontent.com 下載。程式碼搜尋不收錄 EPUB 這類二進位檔，所以不能直接搜「全部 EPUB」。
#           轉換沿用 epub_to_text.feature（ET5、ET6）。
# 狀態：GE1–GE9 依建議確認（2026-11-12），進入 Red
#
# 確認事項（皆採建議）：
#  GE1. 程式代號 github-epub-search，掛在「系統管理」選單。可以嗎？
#  GE2. 搜尋 repo：輸入關鍵字（如 epub、電子書），以 GitHub 搜尋 repo，列出名稱、說明、授權、星數、最後更新，一頁 30 筆、可翻頁。
#       可勾「只看開放授權」：只顯示授權為 CC0、CC-BY、CC-BY-SA、Unlicense、MIT、Apache-2.0、GPL 系列等開放授權的 repo。可以嗎？
#  GE3. 列出 EPUB：選一個 repo 後列出預設分支裡所有 .epub 檔的路徑、大小與 GitHub 連結；檔案太多、GitHub 未列完時顯示提示。可以嗎？
#  GE4. 轉換：一次選一本，伺服器從 GitHub 下載後直接轉成 TXT 並下載（同 EPUB 轉 TXT 的規則與 50 MB 上限）；不提供批次下載，
#       不保存檔案。畫面在轉換按鈕旁顯示該 repo 的授權，沒有授權資訊時顯示「未標示授權，請自行確認使用權利」。可以嗎？
#  GE5. GitHub token：放在 .env 的 TTRS_GITHUB_TOKEN（不寫進程式、資料庫或畫面）；沒有設定時以匿名方式呼叫（每小時 60 次）。
#       超過次數回 GITHUB_RATE_LIMITED 並顯示可再查詢的時間；連不上回 GITHUB_UNAVAILABLE。可以嗎？
#  GE6. 稽核紀錄：記錄使用者、搜尋關鍵字、轉換的 repo 與檔案路徑。可以嗎？
#  GE7. 權限：功能 query（搜尋、列出檔案）與 run（下載轉換）；ADMIN 預設都有。工具作業沒有資料，ADR 0013 的標準功能鈕全部列為例外。可以嗎？
#  GE8. 測試：BDD 以記憶體中的假 GitHub 來源執行，不連外網；另有一個需要網路、預設不執行的實測（同匯率自動匯入）。可以嗎？
#  GE9. 實作：GitHub 呼叫放 infra（HTTPS，沿用 reqwest），app 層透過 port 使用；搜尋結果不快取。可以嗎？

功能: GitHub EPUB 搜尋（GitHub EPUB Search）
  身為 使用者
  我想要 在 GitHub 上找有 EPUB 電子書的 repo 並轉成文字
  以便 取得可閱讀、可搜尋的純文字

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 GitHub 上有 repo:
      | repo           | 說明         | 授權         | 星數 |
      | alice/books    | 公有領域書籍 | CC0-1.0      | 120  |
      | bob/ebooks     | 電子書收藏   |              | 80   |
      | carol/epub-lib | EPUB 範例    | CC-BY-4.0    | 30   |
    而且 GitHub repo "alice/books" 有檔案:
      | 路徑                | 大小    |
      | README.md           | 1024    |
      | classics/war.epub   | 2048000 |
      | classics/peace.epub | 1024000 |

  # GE2
  場景: GE-1 以關鍵字搜尋 repo
    當 使用者在 GitHub 搜尋 "books"
    那麼 搜尋結果應為:
      | repo           | 授權      |
      | alice/books    | CC0-1.0   |
      | bob/ebooks     |           |
      | carol/epub-lib | CC-BY-4.0 |

  # GE2：只看開放授權
  場景: GE-2 只看開放授權的 repo
    當 使用者在 GitHub 搜尋 "books"，只看開放授權
    那麼 搜尋結果應為:
      | repo           | 授權      |
      | alice/books    | CC0-1.0   |
      | carol/epub-lib | CC-BY-4.0 |

  # GE3
  場景: GE-3 列出 repo 裡的 EPUB
    當 使用者列出 GitHub repo "alice/books" 的 EPUB
    那麼 EPUB 清單應為:
      | 路徑                | 大小    |
      | classics/peace.epub | 1024000 |
      | classics/war.epub   | 2048000 |

  # GE4：下載轉換
  場景: GE-4 轉換 repo 裡的一本 EPUB
    假設 GitHub 檔案 "alice/books" "classics/war.epub" 是一本 EPUB，書名 "戰爭"，內容 "<p>第一段</p>"
    當 使用者轉換 GitHub 檔案 "alice/books" "classics/war.epub"
    那麼 操作應成功
    而且 轉出的檔名應為 "war.txt"，書名 "戰爭"

  # GE4：沿用 50 MB 上限
  場景: GE-5 超過大小上限不下載
    假設 GitHub repo "alice/books" 的檔案 "classics/huge.epub" 大小為 60 MB
    當 使用者轉換 GitHub 檔案 "alice/books" "classics/huge.epub"
    那麼 應出現錯誤 "EPUB_TOO_LARGE"

  # GE5
  場景: GE-6 超過 GitHub 查詢次數
    假設 GitHub 查詢次數已用完
    當 使用者在 GitHub 搜尋 "books"
    那麼 應出現錯誤 "GITHUB_RATE_LIMITED"
