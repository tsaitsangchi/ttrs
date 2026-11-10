# language: zh-TW
# TipTop 對應：aooi160 交運方式維護 / ged_file（交運方式維護檔）
# 新系統名稱：Shipping Method Maintenance 交運方式維護（shipping-method-maintenance，SM1）
# 規則來源：tiptop/aoo/4gl/aooi160.4gl:229-242（交運方式重複 -239）、:243（刪除）、:355（查詢欄位 ged01、ged02）；
#           tiptop/aoo/4fd/aooi160.4fd；tiptop/aoo/sch/ged_file.sch。無 topcust 覆寫。
# 狀態：SM1–SM6 依建議確認（Key User，2026-11-10），進入 Red
#
# TipTop 的做法（萃取）：
#   - 多筆單身表格，只有兩欄：交運方式 ged01（10 字）、說明 ged02（80 字）。
#   - 表上沒有資料有效、所有者、修改者等欄位；刪除時不檢查是否被訂單、出貨單引用。
#   - 功能只有 query、detail、output、exporttoexcel；其餘標準功能鈕依 ADR 0013 補上。
#
# 確認事項（皆採建議）：
#  SM1. 程式代號 shipping-method-maintenance；版面比照 aooi160.4fd 單一表格。可以嗎？
#  SM2. 範圍：全公司共用，放共用 schema（scope: system）。可以嗎？
#  SM3. TipTop 沒有資料有效欄位；TTRS 依 ADR 0013 加上「資料有效」（停用／啟用）與狀態頁籤欄位，轉入資料一律為有效。可以嗎？
#  SM4. 刪除：已被訂單、出貨單引用時不可刪除，請改為停用（SHIPPING_METHOD_IN_USE）；引用的作業移植後才檢查。可以嗎？
#  SM5. 代號建立後不可修改、區分大小寫、最多 10 字；說明不做多語言。可以嗎？
#  SM6. 不移植的欄位：無（ged_file 只有 ged01、ged02，皆移植）。
#
# 標準功能鈕（ADR 0013）：新增、修改、刪除、查詢、列印、匯出 Excel、上下筆、停用／啟用、複製、異動記錄、
#   確認／取消確認（TTRS 標準確認欄位 SA9-1，已確認不可刪除）。列印與匯出欄位：交運方式、說明、資料有效。

功能: 交運方式維護（Shipping Method Maintenance）
  身為 業務人員
  我想要 維護交運方式
  以便 訂單與出貨單選用

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"

  場景: SM-1 新增交運方式
    當 使用者新增交運方式 "AIR"，說明 "空運"
    那麼 操作應成功
    而且 交運方式 "AIR" 應為有效

  # aooi160.4gl:229-242 重複
  場景: SM-2 交運方式不可重複
    假設 已有交運方式 "AIR"
    當 使用者新增交運方式 "AIR"，說明 "航空"
    那麼 應出現錯誤 "SHIPPING_METHOD_ALREADY_EXISTS"

  場景: SM-3 交運方式最多 10 字
    當 使用者新增交運方式 "AIRFREIGHT1"，說明 "空運"
    那麼 應出現錯誤 "SHIPPING_METHOD_CODE_TOO_LONG"

  場景: SM-4 修改說明
    假設 已有交運方式 "AIR"
    當 使用者將交運方式 "AIR" 的說明改為 "空運（含快遞）"
    那麼 交運方式 "AIR" 的說明應為 "空運（含快遞）"

  # SM3 停用
  場景: SM-5 停用與啟用交運方式
    假設 已有交運方式 "AIR"
    當 使用者停用交運方式 "AIR"
    那麼 交運方式 "AIR" 的資料有效應為 "否"
    當 使用者啟用交運方式 "AIR"
    那麼 交運方式 "AIR" 的資料有效應為 "是"

  # :243 刪除
  場景: SM-6 刪除交運方式
    假設 已有交運方式 "AIR"
    當 使用者刪除交運方式 "AIR"
    那麼 操作應成功
    而且 交運方式 "AIR" 應不存在

  場景: SM-7 已確認的交運方式不可刪除
    假設 已有交運方式 "AIR"
    而且 交運方式 "AIR" 已確認
    當 使用者刪除交運方式 "AIR"
    那麼 應出現錯誤 "RECORD_CONFIRMED_NOT_DELETABLE"

  # :355 查詢
  場景: SM-8 查詢交運方式
    假設 已有交運方式 "AIR"
    而且 已有交運方式 "SEA"
    而且 已有交運方式 "EXP"
    當 使用者以下列條件查詢交運方式:
      | 欄位     | 條件     |
      | 交運方式 | A*\|S*   |
    那麼 查詢結果應為 "AIR,SEA" 共 2 筆

  場景: SM-9 複製交運方式
    假設 已有交運方式 "AIR"
    當 使用者將交運方式 "AIR" 複製為 "AIR2"
    那麼 操作應成功
    而且 交運方式 "AIR2" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |
