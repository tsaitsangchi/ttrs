# language: zh-TW
# TipTop 對應：p_zx 使用者資料維護 / zx_file
# 新系統名稱：User Maintenance 使用者維護（user-maintenance）的列印、匯出 Excel、確認／取消確認
# 規則來源：docs/adr/0013-standard-maintenance-actions.md（SA3 通用清單報表、SA9 確認功能鈕）；
#           tiptop/azz/4gl/p_zx.4gl:267（output 經 cl_chk_act_auth）。zx_file 沒有確認欄位，用 TTRS 標準確認欄位（SA9-1）。
# 狀態：依 ADR 0013 SA3、SA9 確認（Key User，2026-11-10），進入 Red
#
# 規則：
#   - 列印 output、匯出 Excel exporttoexcel：依目前的查詢條件與資料權限（同查詢 UQ4），依使用者代號排序。
#     欄位：使用者代號、使用者名稱、部門代號、權限類別、預設營運中心、資料有效。使用者是系統層資料，表頭不印營運中心。
#   - 確認 confirm／取消確認 notconfirm：同料件分群（item_group_confirm.feature）——新增、複製為未確認；
#     已確認可以修改、停用，不可刪除（RECORD_CONFIRMED_NOT_DELETABLE）；既有資料轉入時視為已確認。

功能: 使用者維護的標準功能鈕（User Maintenance Standard Actions）
  身為 系統管理員
  我想要 列印、匯出使用者清單並確認使用者設定
  以便 定期核對帳號

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 權限類別 "CLASS1" 已存在
    而且 營運中心 "TW01" 已啟用
    而且 選單程式 "menu-main" 已存在
    而且 已有使用者 "u001"
    而且 已有使用者 "u002"

  場景: USA-1 依條件列印使用者
    當 使用者以條件「使用者代號 = u*」列印使用者
    那麼 報表明細的第一欄應為 "u001,u002"
    而且 報表應可輸出為 PDF
    而且 稽核紀錄應有一筆程式 "user-maintenance"、動作 "output"、筆數 2

  場景: USA-2 匯出 Excel
    當 使用者不輸入條件匯出使用者為 Excel
    那麼 Excel 應有 2 筆明細，欄位與報表相同

  場景: USA-3 確認與取消確認
    當 使用者確認使用者 "u001"
    那麼 使用者 "u001" 的確認狀態應為:
      | 欄位   | 值     |
      | 已確認 | 是     |
      | 確認人 | tiptop |
    當 使用者取消確認使用者 "u001"
    那麼 使用者 "u001" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |

  場景: USA-4 已確認不可刪除
    假設 使用者 "u002" 已確認
    當 使用者刪除使用者 "u002"
    那麼 應出現錯誤 "RECORD_CONFIRMED_NOT_DELETABLE"
