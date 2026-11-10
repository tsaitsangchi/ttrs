# language: zh-TW
# TipTop 對應：axmi110 產品分類維護作業 / oba_file
# 新系統名稱：Product Classification Maintenance 產品分類維護的查詢、複製、列印、匯出 Excel、確認／取消確認、上下筆
# 規則來源：docs/adr/0013-standard-maintenance-actions.md（SA1–SA3、SA5、SA9）；tiptop/axm/4gl/axmi110.4gl:110-153
#           （功能 query、detail、output、exporttoexcel）、:477-541（查詢欄位 oba01、oba02、oba10、oba11、oba111）。
#           axmi110 沒有 reproduce、confirm；依 Key User 要求（2026-11-10）補上。oba_file 沒有確認欄位，用 TTRS 標準確認欄位（SA9-1）。
# 狀態：依 ADR 0013 確認（Key User，2026-11-10），進入 Red
#
# 規則：
#   - 查詢 query：TipTop 只開放 oba01、oba02、oba10、oba11、oba111；依 ADR 0013 SA1 擴及產品分類的全部欄位與「狀態」欄位、
#     已確認否、確認人。依分類碼排序、每頁 50 筆；資料量小，於 app 層過濾。
#   - 複製 reproduce：輸入新分類碼，其餘欄位帶入；資料所有者為目前使用者、未確認。
#   - 列印 output、匯出 Excel exporttoexcel：依查詢條件；欄位：產品分類、分類名稱、成本中心、銷貨收入科目、資料有效。
#   - 確認 confirm／取消確認 notconfirm：同料件分群（item_group_confirm.feature）；已確認不可刪除。
#   - 上下筆：移動表格的選取列，跨頁時自動換頁（畫面行為，不寫成情境）。

功能: 產品分類維護的標準功能鈕（Product Classification Standard Actions）
  身為 業務或成本人員
  我想要 查詢、複製、列印、匯出產品分類並確認設定
  以便 管理產品分類

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 營運中心 "TW01" 已啟用
    而且 營運中心 "TW01" 已有產品分類 "P01"
    而且 營運中心 "TW01" 已有產品分類 "P02"
    而且 營運中心 "TW01" 已有產品分類 "Q01"

  場景: PCS-Q 查詢產品分類
    當 使用者以下列條件查詢產品分類:
      | 欄位     | 條件 |
      | 產品分類 | P*   |
    那麼 查詢結果應為 "P01,P02" 共 2 筆

  場景: PCS-C 複製產品分類
    當 使用者將產品分類 "P01" 複製為 "P09"
    那麼 操作應成功
    而且 產品分類 "P09" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |

  場景: PCS-P 列印與匯出產品分類
    當 使用者以條件「產品分類 = P*」列印產品分類
    那麼 報表明細的第一欄應為 "P01,P02"
    而且 報表應可輸出為 PDF
    當 使用者不輸入條件匯出產品分類為 Excel
    那麼 Excel 應有 3 筆明細，欄位與報表相同

  場景: PCS-K 確認後不可刪除
    假設 產品分類 "Q01" 已確認
    當 使用者刪除營運中心 "TW01" 產品分類 "Q01"
    那麼 應出現錯誤 "RECORD_CONFIRMED_NOT_DELETABLE"
