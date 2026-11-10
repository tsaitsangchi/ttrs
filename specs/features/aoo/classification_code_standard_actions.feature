# language: zh-TW
# TipTop 對應：aooi300 分類碼資料維護 / azf_file
# 新系統名稱：Classification Code Maintenance 分類碼維護的查詢、複製、列印、匯出 Excel、確認／取消確認、上下筆
# 規則來源：docs/adr/0013-standard-maintenance-actions.md（SA1–SA3、SA5、SA9）；aooi300 是單身式作業（query、detail），
#           沒有 reproduce、output、exporttoexcel、confirm；依 Key User 要求（2026-11-10）補上。
#           azf_file 沒有確認欄位，用 TTRS 標準確認欄位（SA9-1）。
# 狀態：依 ADR 0013 確認（Key User，2026-11-10），進入 Red
#
# 規則：
#   - 都在目前選的類別內進行（CC1：一支作業，以類別切換）。
#   - 查詢 query：可查詢代號、說明、存貨差異科目一／二、成本性質與「狀態」欄位、已確認否、確認人；依代號排序、每頁 50 筆；
#     資料量小，於 app 層過濾。
#   - 複製 reproduce：同一類別的新代號，其餘欄位帶入；資料所有者為目前使用者、未確認。
#   - 列印 output、匯出 Excel exporttoexcel：依查詢條件；欄位：代號、說明、資料有效。
#   - 確認 confirm／取消確認 notconfirm：同料件分群（item_group_confirm.feature）；已確認不可刪除。
#   - 上下筆：移動表格的選取列，跨頁時自動換頁（畫面行為，不寫成情境）。

功能: 分類碼維護的標準功能鈕（Classification Code Standard Actions）
  身為 物料管理人員
  我想要 查詢、複製、列印、匯出分類碼並確認設定
  以便 管理料件的分類

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 營運中心 "TW01" 已啟用
    而且 類別 "D" 已有分類碼 "D01"
    而且 類別 "D" 已有分類碼 "D02"
    而且 類別 "D" 已有分類碼 "X01"

  場景: CCS-Q 查詢分類碼
    當 使用者以下列條件查詢類別 "D" 的分類碼:
      | 欄位 | 條件 |
      | 代號 | D*   |
    那麼 查詢結果應為 "D01,D02" 共 2 筆

  場景: CCS-C 複製分類碼
    當 使用者將類別 "D" 的分類碼 "D01" 複製為 "D09"
    那麼 操作應成功
    而且 類別 "D" 的分類碼 "D09" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |

  場景: CCS-P 列印與匯出分類碼
    當 使用者以條件「代號 = D*」列印類別 "D" 的分類碼
    那麼 報表明細的第一欄應為 "D01,D02"
    而且 報表應可輸出為 PDF
    當 使用者不輸入條件匯出類別 "D" 的分類碼為 Excel
    那麼 Excel 應有 3 筆明細，欄位與報表相同

  場景: CCS-K 確認後不可刪除
    假設 類別 "D" 的分類碼 "X01" 已確認
    當 使用者刪除類別 "D" 的分類碼 "X01"
    那麼 應出現錯誤 "RECORD_CONFIRMED_NOT_DELETABLE"
    當 使用者取消確認類別 "D" 的分類碼 "X01"
    那麼 類別 "D" 的分類碼 "X01" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |
