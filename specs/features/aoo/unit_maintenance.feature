# language: zh-TW
# TipTop 對應：aooi101 單位資料維護 / gfe_file（單位檔）
# 新系統名稱：Unit Maintenance 單位維護（unit-maintenance，UN1）
# 規則來源：tiptop/aoo/4gl/aooi101.4gl:84-125（功能 query、detail、output、related_document、exporttoexcel）、:130-420（單身）、
#           :195-198（新增預設：小數位數 0、資料有效 Y）、:249-260（單位重複 -239）、:261-267（小數位數不可小於 0）、
#           :268-275（資料有效只能 Y／N）、:426-455（查詢欄位 gfe01、gfe02、gfe03、gfeacti）、:694-711（單位建立後不可修改）；
#           tiptop/aoo/4fd/aooi101.4fd；tiptop/aoo/sch/gfe_file.sch。無 topcust 覆寫。
#           TTRS 現況：migrations/0001 的 unit（unit_code、name，早期試做）。
# 狀態：UN1–UN7 依建議確認（Key User，2026-11-09），進入 Red（料件移植 IM16 的第三步）
#
# TipTop 的做法（萃取）：
#   - 多筆單身表格：單位 gfe01（4 字）、說明 gfe02（80 字）、小數位數 gfe03、資料有效 gfeacti（另有所有者、修改者等欄位）。
#   - 刪除時不檢查是否被料件等資料引用；刪除會連帶刪除相關文件（cl_del_doc）。
#
# 確認事項（皆採建議）：
#  UN1. 程式代號 unit-maintenance；版面比照 aooi101.4fd 單一表格，新增／修改以小視窗編輯。可以嗎？
#  UN2. 範圍：單位是全公司共用的代碼，TTRS 放共用 schema（scope: system，沿用既有的 unit 表並以新 migration 擴充）。
#       TipTop gfe_file 在各營運中心資料庫各一份；轉入時以總公司（或指定營運中心）的資料為準，差異請 Key User 確認。可以嗎？
#  UN3. 單位區分大小寫、最多 4 字、建立後不可修改（TipTop 沒有轉大寫）。可以嗎？
#  UN4. 小數位數 0–6（TipTop 只檢查不可小於 0；TTRS 的數量欄位為 NUMERIC，建議上限 6）；預設 0。可以嗎？
#  UN5. 刪除：已被料件引用（ima25、ima31、ima44、ima55、ima63 等單位欄位）時不可刪除、請改為停用（UNIT_IN_USE）；
#       料件移植前沒有引用。TipTop 不檢查。可以嗎？
#  UN6. 停用的單位不可再被新料件選用（料件移植時檢查）；既有資料不受影響。可以嗎？
#  UN7. 不移植的項目：
#        | 項目 | 原因 |
#        | 相關文件 related_document（cl_doc、cl_del_doc） | 文件管理尚未移植 |
#        | 列印 output、匯出 Excel | 依 ADR 0011 另案 |
#        | 單位換算（aooi102、aooi103） | 另一支作業，料件第二階段（IM1）一起做 |

功能: 單位維護（Unit Maintenance）
  身為 物料管理人員
  我想要 維護計量單位
  以便 料件與交易使用一致的單位與小數位數

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"

  # aooi101.4gl:195-198 新增預設
  場景: UN-1 新增單位
    當 使用者新增單位 "PCS"，說明 "個"
    那麼 單位 "PCS" 應為:
      | 欄位     | 值 |
      | 說明     | 個 |
      | 小數位數 | 0  |
      | 資料有效 | 是 |

  # :249-260 重複
  場景: UN-2 單位不可重複
    假設 已有單位 "PCS"
    當 使用者新增單位 "PCS"，說明 "件"
    那麼 應出現錯誤 "UNIT_ALREADY_EXISTS"

  # :261-267、UN4 小數位數
  場景大綱: UN-3 小數位數須在 0 到 6 之間
    當 使用者新增單位 "KG"，說明 "公斤"，小數位數 <位數>
    那麼 <結果>

    例子:
      | 位數 | 結果                                  |
      | 3    | 單位 "KG" 的小數位數應為 3            |
      | -1   | 應出現錯誤 "UNIT_DECIMALS_INVALID"    |
      | 7    | 應出現錯誤 "UNIT_DECIMALS_INVALID"    |

  # UN3 長度
  場景: UN-4 單位最多 4 字
    當 使用者新增單位 "PIECE"，說明 "個"
    那麼 應出現錯誤 "UNIT_CODE_TOO_LONG"

  # :268-275 停用
  場景: UN-5 停用單位
    假設 已有單位 "PCS"
    當 使用者停用單位 "PCS"
    那麼 單位 "PCS" 的資料有效應為 "否"
