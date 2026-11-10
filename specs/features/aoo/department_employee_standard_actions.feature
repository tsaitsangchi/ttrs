# language: zh-TW
# TipTop 對應：aooi030 部門名稱資料維護 / gem_file、aooi040 員工資料維護 / gen_file
# 新系統名稱：Department Maintenance 部門維護、Employee Maintenance 員工維護的複製、列印、匯出 Excel、確認／取消確認、上下筆
# 規則來源：docs/adr/0013-standard-maintenance-actions.md（SA2 表格上下筆、SA3 通用清單報表、SA5 複製、SA9 確認功能鈕）。
#           aooi030、aooi040 是單身式作業（只有 detail），沒有 reproduce、confirm；aooi040 也沒有 output、exporttoexcel
#           （部門的列印見 department_report.feature）。依 Key User 要求（2026-11-10）補上。
#           gem_file、gen_file 沒有確認欄位，用 TTRS 標準確認欄位（SA9-1）。
# 狀態：依 ADR 0013 確認（Key User，2026-11-10），進入 Red
#
# 規則：
#   - 部門複製：輸入新部門編號，簡稱、全稱與會計資料帶入；對應成本中心是來源部門本身時改為新部門。
#   - 員工複製：輸入新員工編號，姓名、部門、職稱、分機、電子郵件帶入。
#   - 複製的資料：資料所有者為目前使用者、未確認。
#   - 員工列印、匯出 Excel：依查詢條件、員工編號排序；欄位：員工編號、姓名、部門、職稱、分機、電子郵件、資料有效。
#   - 確認 confirm／取消確認 notconfirm：同料件分群（item_group_confirm.feature）；已確認不可刪除。
#   - 上下筆：移動表格的選取列，跨頁時自動換頁（畫面行為，不寫成情境）。

功能: 部門與員工維護的標準功能鈕（Department / Employee Standard Actions）
  身為 人事或管理部門人員
  我想要 複製、列印、匯出部門與員工資料並確認設定
  以便 快速建立相似資料並定期核對

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 營運中心 "TW01" 已啟用
    而且 "TW01" 已有部門 "SAL"
    而且 "TW01" 已有員工 "E001"，部門 "SAL"
    而且 "TW01" 已有員工 "E002"，部門 "SAL"

  場景: DSA-1 複製部門
    當 使用者將部門 "SAL" 複製為 "SAL2"
    那麼 操作應成功
    而且 部門 "SAL2" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |

  場景: DSA-2 確認部門後不可刪除
    假設 部門 "SAL" 已確認
    當 使用者刪除 "TW01" 部門 "SAL"
    那麼 應出現錯誤 "RECORD_CONFIRMED_NOT_DELETABLE"
    當 使用者取消確認部門 "SAL"
    那麼 部門 "SAL" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |

  場景: ESA-1 複製員工
    當 使用者將員工 "E001" 複製為 "E009"
    那麼 操作應成功
    而且 員工 "E009" 的確認狀態應為:
      | 欄位   | 值 |
      | 已確認 | 否 |

  場景: ESA-2 列印與匯出員工
    當 使用者以條件「員工編號 = E*」列印員工
    那麼 報表明細的第一欄應為 "E001,E002"
    而且 報表應可輸出為 PDF
    而且 稽核紀錄應有一筆程式 "employee-maintenance"、動作 "output"、筆數 2
    當 使用者不輸入條件匯出員工為 Excel
    那麼 Excel 應有 2 筆明細，欄位與報表相同

  場景: ESA-3 確認員工後不可刪除
    假設 員工 "E002" 已確認
    當 使用者刪除 "TW01" 員工 "E002"
    那麼 應出現錯誤 "RECORD_CONFIRMED_NOT_DELETABLE"
    當 使用者確認員工 "E002"
    那麼 應出現錯誤 "RECORD_ALREADY_CONFIRMED"
