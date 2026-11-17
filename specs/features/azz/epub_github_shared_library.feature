# language: zh-TW
# TipTop 對應：無（TTRS 新增的工具作業；擴充 epub_github_library.feature 的「我的 GitHub 書庫」）
# 新系統名稱：沿用 github-epub-search（畫面「EPUB 搜尋」多一個「共用 GitHub 書庫」區）
# 新資料表：epub_github_shared_library（全公司共用的 GitHub repo 清單）
# 規則來源：使用者 2026-11-17 要求「不想每個人各自加入」，由管理員設定一份全公司共用的 repo 清單。
#           界線與「我的 GitHub 書庫」相同：納入哪些 repo 由人（管理員）指定、使用權利由指定的人判斷；
#           系統不自動掃描、發現其他 repo。
# 狀態：SL1–SL8 依建議確認（2026-11-17），進入 Red
#
#  SL1. 範圍：全公司一份清單（不分營運中心、不分使用者），最多 20 個 repo。
#       所有使用者搜尋時，除了自己的書庫，也一併比對共用書庫中每個 repo 的 EPUB 檔名。可以嗎？
#  SL2. 誰能管理：作業 github-epub-search 新增功能 share（管理共用書庫）；加入、移除共用書庫都檢查 share，
#       ADMIN 預設有（起始資料 migration 補上），其他權限類別由管理員在權限維護中自行授予。
#       看共用清單用 query（能搜尋的人都看得到清單，知道搜尋範圍包含哪些 repo）。可以嗎？
#  SL3. 畫面：EPUB 搜尋頁在「我的 GitHub 書庫」下方加「共用 GitHub 書庫」區：列出 repo、授權、加入者、加入時間；
#       有 share 功能的人才看得到輸入框與「加入」「移除」按鈕。輸入格式與檢查同 ML2：
#       不存在回 GITHUB_NOT_FOUND、重複回 EPUB_LIBRARY_DUPLICATE、超過 20 個回 EPUB_LIBRARY_FULL（沿用，不新增錯誤代碼）。可以嗎？
#  SL4. 搜尋：比對方式、排序、去重、下載與轉換都沿用 ML3（含 Git LFS、上限 100 MB）。
#       同一個 repo 同時在共用書庫與我的書庫時只讀一次。可以嗎？
#  SL5. 失敗與限制：沿用 ML4（讀取失敗略過該 repo 並提示「GitHub 暫時無法查詢」；不快取檔案清單）。
#       提醒：每個 repo 每次搜尋要呼叫 2 次 GitHub API；GitHub 沒有 token 時每小時只能呼叫 60 次，
#       共用書庫 repo 多、使用者多時很快用完，所以正式使用應在 .env 設定 TTRS_GITHUB_TOKEN（每小時 5000 次）。
#       目前先不做快取，之後真的不夠再另寫規格。可以嗎？
#  SL6. 資料表 epub_github_shared_library（共用 schema public，scope system）：
#       repo（擁有者/repo，主鍵）、created_by（加入者，zx01）、created_dept、created_at、updated_at、version；
#       依 ADR 0012 掛 ttrs_bump_version、ttrs_change_log 觸發器；移除時比對版本（已被改過回 RECORD_CHANGED）。
#       稽核紀錄記加入與移除的 repo。可以嗎？
#  SL7. ADR 0013 標準功能鈕：比照 ML7，只做新增（加入）、刪除（移除）、查詢（列出）；
#       例外（不做）：修改、列印、匯出 Excel、第一筆／上一筆／下一筆／最後一筆、失效、複製、異動記錄畫面
#       （異動仍由觸發器記錄）、確認／取消確認。可以嗎？
#  SL8. 測試：BDD 用記憶體假 GitHub（不連外網）；記憶體與 PostgreSQL 兩種後端都要通過；
#       share 的權限檢查加入 authorization.feature 的對照表。可以嗎？
#
# 不移植的欄位：無（TipTop 沒有對應資料表）。

功能: 共用 GitHub 書庫（EPUB GitHub Shared Library）
  身為 管理員
  我想要 設定一份全公司共用的 GitHub repo 清單
  以便 每位使用者不必各自加入，用關鍵字就找到這些 repo 裡的書

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 GitHub 上有 repo:
      | repo             | 說明                 | 授權 | 星數 |
      | jugetaozi/ibooks | 2012-2018读过的电子书 |      | 747  |
      | alice/books      | 書                   | MIT  | 10   |
    而且 GitHub repo "jugetaozi/ibooks" 有檔案:
      | 路徑                  | 大小 |
      | 庆余年(精校版）.epub   | 1000 |
      | E019_围城.epub        | 1000 |
      | README.md             | 10   |

  # SL3：加入與列出
  場景: SL-1 加入 repo 到共用書庫
    當 使用者把 "https://github.com/jugetaozi/ibooks" 加入共用 GitHub 書庫
    那麼 操作應成功
    而且 共用 GitHub 書庫應為 "jugetaozi/ibooks"

  場景: SL-2 不存在的 repo 不能加入
    當 使用者把 "nobody/nothing" 加入共用 GitHub 書庫
    那麼 應出現錯誤 "GITHUB_NOT_FOUND"

  場景: SL-3 重複加入
    假設 共用 GitHub 書庫已有 "jugetaozi/ibooks"
    當 使用者把 "jugetaozi/ibooks" 加入共用 GitHub 書庫
    那麼 應出現錯誤 "EPUB_LIBRARY_DUPLICATE"

  場景: SL-4 共用書庫最多 20 個 repo
    假設 共用 GitHub 書庫已有 20 個 repo
    當 使用者把 "alice/books" 加入共用 GitHub 書庫
    那麼 應出現錯誤 "EPUB_LIBRARY_FULL"

  # SL1：所有使用者的搜尋都包含共用書庫
  場景: SL-5 其他使用者不必自己加入就找得到
    假設 共用 GitHub 書庫已有 "jugetaozi/ibooks"
    而且 目前使用者為 "u002"，所屬部門 "MIS"，權限類別 "CLASS1"
    當 使用者搜尋 "慶餘年"
    那麼 結果應有 "庆余年(精校版）"，來源為 "GitHub"
    而且 結果不應有 "E019_围城"

  場景: SL-6 從共用書庫找到的書可以轉換
    假設 共用 GitHub 書庫已有 "jugetaozi/ibooks"
    而且 GitHub 檔案 "jugetaozi/ibooks" "庆余年(精校版）.epub" 是一本 EPUB，書名 "庆余年"，內容 "第一章"
    當 使用者搜尋 "慶餘年"
    而且 使用者轉換 "GitHub" 的 "庆余年(精校版）"，輸出 "txt"
    那麼 操作應成功
    而且 轉出的檔名應為 "庆余年(精校版）.txt"

  # SL4：同一個 repo 在兩個書庫只算一次
  場景: SL-7 同一個 repo 同時在共用書庫與我的書庫
    假設 共用 GitHub 書庫已有 "jugetaozi/ibooks"
    而且 使用者已把 "jugetaozi/ibooks" 加入我的 GitHub 書庫
    當 使用者搜尋 "慶餘年"
    那麼 結果中 "庆余年(精校版）" 應只有一筆

  # SL3：移除
  場景: SL-8 移除後不再搜尋該 repo
    假設 共用 GitHub 書庫已有 "jugetaozi/ibooks"
    當 使用者把 "jugetaozi/ibooks" 從共用 GitHub 書庫移除
    而且 使用者搜尋 "慶餘年"
    那麼 結果不應有 "庆余年(精校版）"

  # SL5：讀取失敗只略過
  場景: SL-9 共用書庫 repo 讀取失敗時其他照常並提示
    假設 共用 GitHub 書庫已有 "jugetaozi/ibooks"
    而且 GitHub 查詢次數已用完
    當 使用者搜尋 "慶餘年"
    那麼 應提示來源 "GitHub" 暫時無法查詢
