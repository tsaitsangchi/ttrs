# language: zh-TW
# TipTop 對應：無（TTRS 新增的工具作業；擴充 epub_web_search.feature 的 GitHub 來源）
# 新系統名稱：沿用 github-epub-search（畫面「EPUB 搜尋」多一個「我的 GitHub 書庫」區）
# 新資料表：epub_github_library（每位使用者自己指定的 GitHub repo 清單）
# 規則來源：使用者 2026-11-16 要求「不貼網址、只用搜尋」就能取得 jugetaozi/ibooks 裡的「庆余年(精校版）.epub」。
#           GitHub 的程式碼搜尋不收錄該 repo 的 EPUB（實測 repo:jugetaozi/ibooks 只收錄 README.md），
#           系統也不自動掃描、索引其他 repo；改由使用者自己指定要納入搜尋的 repo（與「貼上網址」同一條界線：來源由使用者選、權利由使用者判斷）。
# 狀態：ML1–ML8 依建議確認（2026-11-17），進入 Red
# 實作時的調整：ML3 每個 repo 搜尋時呼叫兩次 GitHub API（repo 資訊取授權與預設分支、檔案清單）；
#   ML5 移除時比對版本（ADR 0012），清單已被改過回 RECORD_CHANGED。
#
#  ML1. 範圍：每位使用者各自一份清單（只影響自己的搜尋結果，別人看不到）；每人最多 20 個 repo。可以嗎？
#  ML2. 畫面：EPUB 搜尋頁加「我的 GitHub 書庫」區：輸入 repo（「擁有者/repo」或 github.com 網址皆可）→「加入」；
#       下方列出已加入的 repo（repo、授權、加入時間），每列有「移除」。加入時向 GitHub 確認 repo 存在，
#       不存在回 GITHUB_NOT_FOUND；重複加入回 EPUB_LIBRARY_DUPLICATE；超過 20 個回 EPUB_LIBRARY_FULL（新增錯誤代碼，三語翻譯）。可以嗎？
#  ML3. 搜尋：查關鍵字時，另外讀取清單中每個 repo 預設分支的檔案清單（每個 repo 一次 GitHub API 呼叫），
#       檔名（不含資料夾、去掉 .epub）與關鍵字比對，比對方式沿用 RL1（轉繁體、小寫、去空白標點，繁簡寫法都算），
#       含有關鍵字的 EPUB 檔列成一本書：來源「GitHub」，授權＝該 repo 的授權，可直接下載或轉換（含 Git LFS，上限 100 MB）。
#       排序沿用 RL2，去重沿用 WS6。可以嗎？
#  ML4. 失敗與限制：清單中的 repo 讀取失敗（已刪除、限流、逾時）時略過該 repo，畫面提示「GitHub 暫時無法查詢」（沿用 WS7），
#       其他照常；GitHub 只回部分檔案清單（超大 repo）時只比對回來的部分。每次搜尋都重新讀取，不快取檔案清單。可以嗎？
#  ML5. 權限：沿用作業 github-epub-search；加入用 insert、移除用 delete、看清單用 query（ADMIN 預設有，起始資料 migration 補上）；
#       每人只能看、改自己的清單。稽核紀錄記加入與移除的 repo。可以嗎？
#  ML6. 資料表 epub_github_library（共用 schema public，scope system；不分營運中心）：
#       user_id、repo（擁有者/repo）、created_at、updated_at、version，主鍵 (user_id, repo)；
#       依 ADR 0012 掛 ttrs_bump_version、ttrs_change_log 觸發器。可以嗎？
#  ML7. ADR 0013 標準功能鈕：這是個人清單，不是維護作業。只做新增（加入）、刪除（移除）、查詢（列出）；
#       例外（不做）：修改（repo 名稱就是鍵，改名＝刪除再加入）、列印、匯出 Excel、第一筆／上一筆／下一筆／最後一筆、
#       失效、複製、異動記錄畫面（異動仍由觸發器記錄）、確認／取消確認。可以嗎？
#  ML8. 測試：BDD 用記憶體假 GitHub（不連外網）；記憶體與 PostgreSQL 兩種後端都要通過。可以嗎？
#
# 不移植的欄位：無（TipTop 沒有對應資料表）。

功能: 我的 GitHub 書庫（EPUB GitHub Library）
  身為 使用者
  我想要 指定幾個 GitHub repo 一起納入 EPUB 搜尋
  以便 不必貼網址，用關鍵字就找到這些 repo 裡的書

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

  # ML2：加入與列出
  場景: ML-1 加入 repo 到我的書庫
    當 使用者把 "https://github.com/jugetaozi/ibooks" 加入我的 GitHub 書庫
    那麼 操作應成功
    而且 我的 GitHub 書庫應為 "jugetaozi/ibooks"

  場景: ML-2 不存在的 repo 不能加入
    當 使用者把 "nobody/nothing" 加入我的 GitHub 書庫
    那麼 應出現錯誤 "GITHUB_NOT_FOUND"

  場景: ML-3 重複加入
    假設 使用者已把 "jugetaozi/ibooks" 加入我的 GitHub 書庫
    當 使用者把 "jugetaozi/ibooks" 加入我的 GitHub 書庫
    那麼 應出現錯誤 "EPUB_LIBRARY_DUPLICATE"

  場景: ML-4 每人最多 20 個 repo
    假設 使用者的 GitHub 書庫已有 20 個 repo
    當 使用者把 "alice/books" 加入我的 GitHub 書庫
    那麼 應出現錯誤 "EPUB_LIBRARY_FULL"

  # ML3：只用搜尋就找到書庫裡的書
  場景: ML-5 搜尋時比對書庫中 repo 的 EPUB 檔名
    假設 使用者已把 "jugetaozi/ibooks" 加入我的 GitHub 書庫
    當 使用者搜尋 "慶餘年"
    那麼 結果應有 "庆余年(精校版）"，來源為 "GitHub"
    而且 結果不應有 "E019_围城"
    而且 結果不應有 "README.md"

  場景: ML-6 從書庫找到的書可以轉換
    假設 使用者已把 "jugetaozi/ibooks" 加入我的 GitHub 書庫
    而且 GitHub 檔案 "jugetaozi/ibooks" "庆余年(精校版）.epub" 是一本 EPUB，書名 "庆余年"，內容 "第一章"
    當 使用者搜尋 "慶餘年"
    而且 使用者轉換 "GitHub" 的 "庆余年(精校版）"，輸出 "txt"
    那麼 操作應成功
    而且 轉出的檔名應為 "庆余年(精校版）.txt"

  # ML1：只影響自己
  場景: ML-7 別人的書庫不影響我的搜尋
    假設 使用者 "u002" 已把 "jugetaozi/ibooks" 加入他的 GitHub 書庫
    當 使用者搜尋 "慶餘年"
    那麼 結果不應有 "庆余年(精校版）"

  # ML2：移除
  場景: ML-8 移除後不再搜尋該 repo
    假設 使用者已把 "jugetaozi/ibooks" 加入我的 GitHub 書庫
    當 使用者把 "jugetaozi/ibooks" 從我的 GitHub 書庫移除
    而且 使用者搜尋 "慶餘年"
    那麼 結果不應有 "庆余年(精校版）"

  # ML4：讀取失敗只略過
  場景: ML-9 書庫 repo 讀取失敗時其他照常並提示
    假設 使用者已把 "jugetaozi/ibooks" 加入我的 GitHub 書庫
    而且 GitHub 查詢次數已用完
    當 使用者搜尋 "慶餘年"
    那麼 應提示來源 "GitHub" 暫時無法查詢
