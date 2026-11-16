# language: zh-TW
# TipTop 對應：無（TTRS 新增的工具作業；擴充 epub_web_search.feature 的 GitHub 來源）
# 新系統名稱：沿用 github-epub-search（畫面「EPUB 搜尋」，不新增畫面區塊）
# 規則來源：使用者 2026-11-16 以關鍵字「庆余年」搜不到 jugetaozi/ibooks 裡的「庆余年(精校版）.epub」，要求寫規格。
#           原因：現有 GitHub 來源用 search/repositories，只比對 repo 名稱、說明與 README，不比對 repo 裡的檔名。
# 狀態：GF1–GF7 依建議確認（2026-11-16）；前提實測後使用者決定「照樣實作」，進入 Red。
# 實測（2026-11-16，有 token）：「庆余年 extension:epub」0 筆；改用「filename:庆余年 extension:epub」有 1 筆（shkey/books，另一版本）；
#       filename:紅樓夢、filename:红楼梦、filename:1Q84 各 1 筆；filename:三国演义、filename:围城 0 筆；
#       repo:jugetaozi/ibooks 只收錄 README.md，裡面的 EPUB 都沒收錄 → GitHub 只收錄部分檔案，找不到不代表沒有。
# 實作時的調整（依實測與現有規則）：
#   (1) GF1 查詢改為「filename:<關鍵字> extension:epub」（不加 filename: 一律 0 筆）；畫面註明「GitHub 只收錄部分檔案」。
#   (2) GF5 同一檔名在多個 repo：合併成一筆即可，不列「其他來源」（WS6 的「其他來源」只記不同來源）。
#   (3) GF2 的提示以代碼 GITHUB_NO_TOKEN 回傳，畫面依語系顯示。
#   (4) 實測發現：搜得到的多半是 Git LFS 存的檔案（raw 網址只是文字指標，GitHub 才收錄得到）；
#       下載時遇到 LFS 指標改從 media.githubusercontent.com 取得實際內容，指標上的大小超過 100 MB 不下載。
#       實測下載 shkey/books 的「庆余年」5.7 MB 成功（crates/infra/tests/epub_sources_live.rs，需 token，預設不執行）。
#
# 先說明一個前提風險（必須先實測）：
#   依檔名找檔案要用 GitHub 的程式碼搜尋 API（search/code），它一定要登入 token（匿名呼叫實測回 401）。
#   GitHub 文件說這個 API 只收錄預設分支、小於 384 KB 的檔案，而且可能不收錄二進位檔；EPUB 是 zip（二進位），
#   多半大於 384 KB（例如「庆余年(精校版）.epub」是 5.7 MB），所以「依檔名」很可能也搜不到。
#   Red 階段第一步：使用者放好 token 後，用真實 API 查「庆余年 extension:epub」。
#   查不到就停在這裡回報，不實作；那種情況下「貼上網址」仍是取得 GitHub 上單一檔案的方法。
#
#  GF1. 範圍：多來源搜尋的 GitHub 來源，在有 token 時，除了原本的 repo 搜尋，再加一次「依檔名」搜尋：
#       查詢條件為「filename:<關鍵字> extension:epub」，簡體與繁體寫法各查一次（同 WS 的關鍵字寫法）。
#       查到的每個 EPUB 檔列成一本書（不是 repo）：書名＝檔名去掉 .epub，作者不填，授權＝該 repo 的授權（沒有就「未標示」），
#       可直接「下載 EPUB」或「轉換並下載」。原本的 repo 結果照舊列出。可以嗎？
#  GF2. 沒有 token 時：不做依檔名搜尋，GitHub 只搜 repo；畫面在 GitHub 來源旁提示「未設定 GitHub token，GitHub 只比對 repo 名稱與說明」。可以嗎？
#  GF3. token：由使用者自己到 GitHub 建立「fine-grained personal access token」，只要公開 repo 的唯讀權限（不勾任何額外權限），
#       放在 .env 的 TTRS_GITHUB_TOKEN（現有設定，GitHub 搜尋已在使用）；不要貼在對話或畫面上。系統只用它查詢與下載，不寫入任何東西。可以嗎？
#  GF4. 次數限制：GitHub 程式碼搜尋每分鐘只能查 10 次（有 token 時），一次搜尋最多用 2 次（簡、繁各一）。
#       被限流時，該次只少了依檔名的結果，畫面沿用 WS7 的提示「GitHub 暫時無法查詢」，其他來源照常。可以嗎？
#  GF5. 排序與去重：沿用 WS3、WS4（依檔名判斷繁簡，沒有下載次數的排在同層最後）；
#       同一個檔名出現在多個 repo 時，合併成一筆，第一個 repo 為主，其他列在「其他來源」（沿用 WS6 的去重）。可以嗎？
#  GF6. 下載與轉換：從 raw.githubusercontent.com 下載該檔案（沿用 GE 的 GitHub 下載），上限 100 MB、不保存檔案；
#       稽核紀錄記 repo、檔案路徑與位元組數（沿用 WS9）。可以嗎？
#  GF7. 測試：BDD 用記憶體假的 GitHub（不連外網）；另有需要 token 與網路、預設不執行的實測（查「庆余年」）。可以嗎？
#
# 不移植的欄位：無（TipTop 沒有對應資料表）。不新增資料表、錯誤代碼或權限動作（沿用 github-epub-search 的 query、run）。
# ADR 0013 標準功能鈕：沿用 GE7 的例外（只有 query、run）。

功能: GitHub 依檔名搜尋 EPUB（GitHub EPUB File Search）
  身為 使用者
  我想要 用書名找到 GitHub repo 裡的 EPUB 檔
  以便 不必先知道是哪一個 repo

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 GitHub 上有下列 EPUB 檔:
      | repo             | 路徑                      | 授權 |
      | jugetaozi/ibooks | 庆余年(精校版）.epub       |      |
      | alice/books      | novels/慶餘年.epub         | MIT  |
      | bob/books        | 慶餘年.epub                |      |
      | alice/books      | novels/三國演義.epub       | MIT  |

  # GF1：依檔名找到檔案，列成一本書；簡繁寫法都查
  場景: GF-1 有 token 時以書名找到 repo 裡的 EPUB 檔
    假設 已設定 GitHub token
    當 使用者搜尋 "慶餘年"
    那麼 結果應有 "庆余年(精校版）"，來源為 "GitHub"
    而且 結果應有 "慶餘年"，來源為 "GitHub"
    而且 結果不應有 "三國演義"

  # GF2：沒有 token 時不做依檔名搜尋並提示
  場景: GF-2 沒有 token 時只搜 repo 並提示
    假設 未設定 GitHub token
    當 使用者搜尋 "慶餘年"
    那麼 結果不應有 "庆余年(精校版）"
    而且 應提示 "GITHUB_NO_TOKEN"

  # GF5：同檔名合併
  場景: GF-3 同一個檔名在多個 repo 合併成一筆
    假設 已設定 GitHub token
    當 使用者搜尋 "慶餘年"
    那麼 繁體中文結果中 "慶餘年" 應只有一筆，來源為 "GitHub"

  # GF4：限流只影響依檔名的結果
  場景: GF-4 程式碼搜尋被限流時其他來源照常
    假設 已設定 GitHub token
    而且 GitHub 程式碼搜尋被限流
    當 使用者搜尋 "慶餘年"
    那麼 結果不應有 "庆余年(精校版）"
    而且 應提示來源 "GitHub" 暫時無法查詢
    而且 結果應有 "alice/books"，來源為 "GitHub"

  # GF6：下載與轉換
  場景: GF-5 下載依檔名找到的 EPUB 並轉換
    假設 已設定 GitHub token
    當 使用者搜尋 "庆余年"
    而且 使用者轉換 "GitHub" 的 "庆余年(精校版）"，輸出 "txt"
    那麼 操作應成功
    而且 轉出的檔名應為 "庆余年(精校版）.txt"
    而且 這次下載的稽核紀錄內容應含 "jugetaozi/ibooks" 與 "庆余年(精校版）.epub"
