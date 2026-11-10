# language: zh-TW
# TipTop 對應：aimi110 查詢（Q.查詢）/ imz_file
# 規則來源：
#   - tiptop/aim/4gl/aimi110.4gl:194-230 aimi110_curs（CONSTRUCT 畫面欄位）
#   - tiptop/aim/4gl/aimi110.4gl:476-489 資料權限過濾 imzuser／imzgrup，依 imz01 排序
#   - QBE 條件語法、空白查全部、分頁：沿用 program_maintenance_query.feature 的 QBE1–QBE4、QBE7
# 狀態：GQ1–GQ3 由 Claude 規畫確認（使用者授權代為確認，2026-11-10），進入 Red；其餘比照料件查詢 IQ1–IQ10
#
# 設計：API：POST /api/plants/{plant}/item-groups/query（QueryRequest → Page<ItemGroupDto>）；畫面比照料件查詢。
#
# 確認事項：
#  GQ1. 可查詢欄位：CONSTRUCT 中有 TTRS 對應的欄位（第一階段 41 欄＋IM24 補的產品分類、經濟／平均訂購量）＋「狀態」頁籤欄位；
#       分群碼區分大小寫；依分群碼排序；資料權限依 imzuser／imzgrup 過濾（同料件 IQ2、IQ4、IQ5）。
#  GQ2. 進入作業時不自動查詢，每頁 50 筆（同料件 IQ6）。
#  GQ3. 不可查詢的欄位：
#        | 欄位 | 原因 |
#        | imzicd01–imzicd79（ICD） | IC 設計行業別，不移植（IG12） |
#        | imz918–imz925 批號序號 | 第二階段（IG3） |
#        | imz906 單位使用方式 | 固定單一單位（IG6） |
#        | imzud01–imzud15 自訂欄位 | 自訂欄位另案（IG12） |
#        條件輸入這些欄位時回 QBE_FIELD_NOT_ALLOWED。

功能: 料件分群查詢（Item Group Maintenance，QBE）
  身為 物料管理人員
  我想要 在料件分群欄位上輸入 TipTop 慣用的查詢條件
  以便 快速找到要維護的分群碼

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 營運中心 "TW01" 已啟用
    而且 已有單位 "PCS"
    而且 營運中心 "TW01" 已有下列料件分群:
      | 分群碼 | 說明 | 來源碼 | 庫存單位 | 資料有效 |
      | FG     | 成品 | M      | PCS      | 是       |
      | RM     | 原料 | P      | PCS      | 是       |
      | RM2    | 舊料 | P      |          | 否       |

  場景: 不輸入條件查詢全部，依分群碼排序
    當 使用者在 "TW01" 以下列條件查詢料件分群:
      | 欄位 | 條件 |
    那麼 查詢結果應為 "FG,RM,RM2" 共 3 筆

  場景大綱: 各欄位的 QBE 條件
    當 使用者在 "TW01" 以下列條件查詢料件分群:
      | 欄位   | 條件   |
      | <欄位> | <條件> |
    那麼 查詢結果應為 "<結果>"

    例子:
      | 欄位     | 條件 | 結果   |
      | 分群碼   | RM*  | RM,RM2 |
      | 說明     | *料  | RM,RM2 |
      | 來源碼   | M    | FG     |
      | 庫存單位 | =    | RM2    |
      | 資料有效 | N    | RM2    |

  場景: 不可查詢的欄位
    當 使用者在 "TW01" 以下列條件查詢料件分群:
      | 欄位         | 條件 |
      | 單位使用方式 | 1    |
    那麼 應出現錯誤 "QBE_FIELD_NOT_ALLOWED"
