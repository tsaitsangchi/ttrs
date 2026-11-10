# language: zh-TW
# TipTop 對應：p_zw 權限類別維護 / zw_file
# 新系統名稱：Permission Class Maintenance 權限類別維護（permission-class-maintenance）的複製、列印、匯出 Excel、確認／取消確認
# 規則來源：docs/adr/0013-standard-maintenance-actions.md（SA3 通用清單報表、SA5 複製、SA9 確認功能鈕）。
#           tiptop/azz/4gl/p_zw.4gl 只有單身維護（detail）、查詢與更新使用者選單，沒有 reproduce、output、exporttoexcel、confirm；
#           依 Key User 要求（2026-11-10）補上。zw_file 沒有確認欄位，用 TTRS 標準確認欄位（SA9-1）。
# 狀態：依 ADR 0013 SA3、SA5、SA9 確認（Key User，2026-11-10），進入 Red
#
# 規則：
#   - 複製 reproduce：輸入新代號，說明、預設選單、閒置設定帶入；資料所有者為目前使用者、未確認。
#     程式執行權限（zy_file）不複製，由程式執行權限維護（p_zy）的複製處理。
#   - 列印 output、匯出 Excel exporttoexcel：依查詢條件，依權限類別代號排序；欄位：權限類別、說明、預設選單、資料有效。
#   - 確認 confirm／取消確認 notconfirm：同料件分群（item_group_confirm.feature）；已確認不可刪除。

功能: 權限類別維護的標準功能鈕（Permission Class Maintenance Standard Actions）
  身為 系統管理員
  我想要 複製、列印、匯出權限類別並確認設定
  以便 快速建立相似的權限類別並定期核對

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 權限類別 "CLASS1" 已存在
    而且 選單程式 "menu-main" 已存在
    而且 營運中心 "TW01" 已啟用
    而且 權限類別 "SALES" 已存在，預設選單 "menu-main"

  場景: WSA-1 複製權限類別
    當 使用者將權限類別 "SALES" 複製為 "SALES2"
    那麼 操作應成功
    而且 權限類別 "SALES2" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |

  場景: WSA-2 依條件列印權限類別
    當 使用者以條件「權限類別代號 = S*」列印權限類別
    那麼 報表明細的第一欄應為 "SALES"
    而且 報表應可輸出為 PDF

  場景: WSA-3 匯出 Excel
    當 使用者不輸入條件匯出權限類別為 Excel
    那麼 Excel 應有 2 筆明細，欄位與報表相同

  場景: WSA-4 確認後不可刪除
    假設 權限類別 "SALES" 已確認
    當 使用者刪除權限類別 "SALES"
    那麼 應出現錯誤 "RECORD_CONFIRMED_NOT_DELETABLE"
    當 使用者取消確認權限類別 "SALES"
    那麼 權限類別 "SALES" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |
