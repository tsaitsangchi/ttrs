# language: zh-TW
# TipTop 對應：aooi101 單位資料維護 / gfe_file
# 新系統名稱：Unit Maintenance 單位維護（unit-maintenance）的查詢、複製、列印、匯出 Excel、確認／取消確認、上下筆
# 規則來源：docs/adr/0013-standard-maintenance-actions.md（SA1–SA3、SA5、SA9）；QBE 條件語法同 program_maintenance_query.feature
#           QBE1–QBE7。aooi101 是單身式作業（query、detail），沒有 reproduce、output、exporttoexcel、confirm；
#           依 Key User 要求（2026-11-10）補上。gfe_file 沒有確認欄位，用 TTRS 標準確認欄位（SA9-1）。
# 狀態：依 ADR 0013 確認（Key User，2026-11-10），進入 Red
#
# 規則：
#   - 查詢 query：可查詢單位、說明、小數位數與「狀態」欄位（資料有效、所有者、建立／修改、版本）及已確認否、確認人；
#     依單位代碼（位元組順序）排序、每頁 50 筆。單位資料量小，於 app 層過濾（兩種後端共用比對規則）。
#   - 複製 reproduce：輸入新單位代碼，說明與小數位數帶入；資料所有者為目前使用者、未確認。
#   - 列印 output、匯出 Excel exporttoexcel：依查詢條件；欄位：單位、說明、小數位數、資料有效。單位為系統層資料，表頭不印營運中心。
#   - 確認 confirm／取消確認 notconfirm：同料件分群（item_group_confirm.feature）；已確認不可刪除。
#   - 上下筆：移動表格的選取列，跨頁時自動換頁（畫面行為，不寫成情境）。

功能: 單位維護的標準功能鈕（Unit Maintenance Standard Actions）
  身為 物料管理人員
  我想要 查詢、複製、列印、匯出單位並確認設定
  以便 管理單位資料

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 已有單位 "PCS"
    而且 已有單位 "BOX"
    而且 已有單位 "KG"

  場景: USA-Q 查詢單位
    當 使用者以下列條件查詢單位:
      | 欄位 | 條件 |
      | 單位 | B*\|K* |
    那麼 查詢結果應為 "BOX,KG" 共 2 筆

  場景: USA-C 複製單位
    當 使用者將單位 "PCS" 複製為 "EA"
    那麼 操作應成功
    而且 單位 "EA" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |

  場景: USA-P 列印與匯出單位
    當 使用者以條件「單位代碼 = *」列印單位
    那麼 報表明細的第一欄應為 "BOX,KG,PCS"
    而且 報表應可輸出為 PDF
    當 使用者不輸入條件匯出單位為 Excel
    那麼 Excel 應有 3 筆明細，欄位與報表相同

  場景: USA-K 確認後不可刪除
    假設 單位 "BOX" 已確認
    當 使用者刪除單位 "BOX"
    那麼 應出現錯誤 "RECORD_CONFIRMED_NOT_DELETABLE"
