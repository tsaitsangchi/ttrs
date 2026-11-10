# language: zh-TW
# TipTop 對應：aimi110 料件分群碼維護的「列印」（output）與「匯出 Excel」（exporttoexcel）/ imz_file
# 新系統名稱：Item Group Maintenance 料件分群維護（item-group-maintenance）的列印、匯出 Excel
# 規則來源：tiptop/aim/4gl/aimi110.4gl:546-550（output 經 cl_chk_act_auth）、:2727-2740（aimi110_out：沒有查詢條件但有目前這筆時
#           印這一筆，都沒有時 9057；報表交給 p_query）；docs/adr/0011-reports-and-printing.md、
#           docs/adr/0013-standard-maintenance-actions.md（SA3）。
# 狀態：依 ADR 0013 SA3 確認（Key User，2026-11-10），進入 Red
#
# 規則：同料件列印（item_maintenance_report.feature），差異：
#   - 欄位：分群碼、說明、來源碼、庫存單位、產品分類、補貨策略碼、資料有效（有效／停用）；依分群碼排序。
#   - 尚未查詢但畫面有一筆時以「分群碼 = 該筆」列印（標準版沒有排除無效）。

功能: 料件分群列印與匯出 Excel（Item Group Report）
  身為 物料管理人員
  我想要 把料件分群印成 PDF 或匯出 Excel
  以便 核對各分群的預設值

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 營運中心 "TW01" 已啟用
    而且 已有單位 "PCS"
    而且 營運中心 "TW01" 已有下列料件分群:
      | 分群碼 | 說明 | 來源碼 | 庫存單位 | 資料有效 |
      | FG     | 成品 | M      | PCS      | 是       |
      | RM     | 原料 | P      | PCS      | 是       |
      | RM2    | 舊料 | P      |          | 否       |

  場景: IGR-1 依條件列印料件分群
    當 使用者以條件「分群碼 = RM*」列印營運中心 "TW01" 的料件分群
    那麼 報表明細應為:
      | 分群碼 | 說明 | 來源碼 | 庫存單位 | 資料有效 |
      | RM     | 原料 | P      | PCS      | 有效     |
      | RM2    | 舊料 | P      |          | 停用     |
    而且 報表應可輸出為 PDF
    而且 報表表頭應有營運中心 "TW01"、報表名稱「料件分群」、列印人 "tiptop" 與列印時間

  場景: IGR-2 匯出 Excel
    當 使用者不輸入條件匯出營運中心 "TW01" 的料件分群為 Excel
    那麼 Excel 應有 3 筆明細，欄位與報表相同

  場景: IGR-3 列印寫入稽核紀錄
    當 使用者以條件「分群碼 = RM*」列印營運中心 "TW01" 的料件分群
    那麼 稽核紀錄應有一筆程式 "item-group-maintenance"、動作 "output"、筆數 2
