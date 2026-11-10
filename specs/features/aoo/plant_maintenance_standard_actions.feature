# language: zh-TW
# TipTop 對應：aooi901 營運中心資料維護 / azp_file
# 新系統名稱：Plant Maintenance 營運中心維護（plant-maintenance）的列印、匯出 Excel、確認／取消確認
# 規則來源：docs/adr/0013-standard-maintenance-actions.md（SA3 通用清單報表、SA9 確認功能鈕）。
#           tiptop/aoo/4gl/aooi901.4gl 沒有 output、exporttoexcel、confirm 功能（ring menu 只有新增、查詢、修改、刪除、複製）；
#           依 Key User 要求（2026-11-10，所有維護作業都有標準功能鈕）補上。azp_file 沒有確認欄位，用 TTRS 標準確認欄位（SA9-1）。
# 狀態：依 ADR 0013 SA3、SA9 確認（Key User，2026-11-10），進入 Red
#
# 規則：
#   - 列印 output、匯出 Excel exporttoexcel：依目前的查詢條件（沒查詢時為全部），依營運中心代碼排序。
#     欄位：營運中心代碼、名稱、資料庫代碼、時區、電話、資料有效。營運中心是系統層資料，報表表頭不印營運中心。
#   - 確認 confirm／取消確認 notconfirm：同料件分群（item_group_confirm.feature）——新增、複製為未確認；
#     已確認可以修改、停用，不可刪除（RECORD_CONFIRMED_NOT_DELETABLE）；既有資料轉入時視為已確認。

功能: 營運中心維護的標準功能鈕（Plant Maintenance Standard Actions）
  身為 系統管理員
  我想要 列印、匯出營運中心清單並確認營運中心設定
  以便 核對各營運中心的設定

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 營運中心 "TW01" 已啟用
    而且 已新增營運中心 "TW02"，資料庫代碼 "ds02"
    而且 使用者停用營運中心 "TW02"

  場景: PSA-1 依條件列印營運中心
    當 使用者以條件「營運中心代碼 = TW*」列印營運中心
    那麼 報表明細應為:
      | 營運中心代碼 | 資料庫代碼 | 資料有效 |
      | TW01         | ds01       | 有效     |
      | TW02         | ds02       | 停用     |
    而且 報表應可輸出為 PDF
    而且 稽核紀錄應有一筆程式 "plant-maintenance"、動作 "output"、筆數 2

  場景: PSA-2 匯出 Excel
    當 使用者不輸入條件匯出營運中心為 Excel
    那麼 Excel 應有 2 筆明細，欄位與報表相同

  場景: PSA-3 確認與取消確認
    當 使用者確認營運中心 "TW01"
    那麼 營運中心 "TW01" 的確認狀態應為:
      | 欄位   | 值     |
      | 已確認 | 是     |
      | 確認人 | tiptop |
    當 使用者取消確認營運中心 "TW01"
    那麼 營運中心 "TW01" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |
      | 確認人 |    |

  場景: PSA-4 已確認不可刪除
    假設 營運中心 "TW02" 已確認
    當 使用者刪除營運中心 "TW02"
    那麼 應出現錯誤 "RECORD_CONFIRMED_NOT_DELETABLE"

  場景: PSA-5 重複確認、未確認就取消確認
    假設 營運中心 "TW02" 已確認
    當 使用者確認營運中心 "TW02"
    那麼 應出現錯誤 "RECORD_ALREADY_CONFIRMED"
    當 使用者取消確認營運中心 "TW01"
    那麼 應出現錯誤 "RECORD_NOT_CONFIRMED"
