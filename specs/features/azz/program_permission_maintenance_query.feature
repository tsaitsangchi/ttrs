# language: zh-TW
# TipTop 對應：p_zy 查詢（Q.查詢）/ zy_file
# 規則來源：
#   - tiptop/azz/4gl/p_zy.4gl:143-189 p_zy_curs（CONSTRUCT 單頭 zy01、單身 zy02、zy03、zy04、zy05、zy07）
#   - tiptop/azz/4gl/p_zy.4gl:194-203 SELECT UNIQUE zy01 … ORDER BY 1；筆數 COUNT(DISTINCT zy01)
#   - tiptop/azz/4gl/p_zy.4gl:552-556、938-955 p_zy_show → p_zy_b_fill(g_wc)：單身只顯示符合條件的程式
#   - tiptop/azz/4gl/p_zy.4gl:150-151 由 p_zw 帶入權限類別時不進查詢（g_argv1）
#   - QBE 條件語法、空白查全部、分頁：沿用 program_maintenance_query.feature 已確認的 QBE1–QBE4、QBE7
# 狀態：YQ1–YQ5 依草稿確認（Key User，2026-11-02），進入 Red
#
# 設計：共用 ttrs_domain::qbe。查詢結果是「權限類別」（單頭），每個權限類別再列出符合條件的程式（單身）。
#   API：POST /api/program-permissions/query（QueryRequest → 權限類別分頁），
#        POST /api/permission-classes/{權限類別}/permission-query（同一組條件 → 該權限類別符合條件的程式）。
#   p_zy 無資料權限條件。
#
# 畫面需求（不寫成 BDD 場景，由畫面實作）：
#   - 「查詢」：單頭權限類別與單身第一列改為輸入條件；「確定」執行、「放棄」回到原畫面。
#   - 查詢後單頭顯示筆數與目前第幾個權限類別，提供 第一筆 / 上一筆 / 下一筆 / 最後一筆 / 跳到第 N 筆。
#   - 單身只列出符合條件的程式（YQ3），並標示「已依查詢條件篩選」；清除查詢條件後恢復全部。
#   - 由權限類別維護帶入權限類別（/azz/program-permissions/:class）時不進查詢，直接顯示該權限類別。
#
# 確認事項（皆採草稿）：
#   YQ1. 可查詢欄位：比照 CONSTRUCT 的 6 個欄位——權限類別、程式代號、允許執行功能、他人資料權限、
#        他部門資料權限、單身權限。資料權限三欄以代碼查詢（他人 0–4、他部門 0–8、單身 0–3），
#        查詢模式提示顯示各代碼的意義。程式名稱不在 CONSTRUCT 內，不提供。
#   YQ2. 只有單頭（權限類別）條件或沒有條件時，查的是全部權限類別，包含還沒有任何程式權限的（如新建的類別）；
#        有單身條件時，只列出至少有一個程式符合條件的權限類別。
#        （TipTop 一律從 zy_file 查，沒有程式權限的權限類別查不到，要從 p_zw「維護群組執行權限」進入。）
#   YQ3. 有單身條件時，單身只顯示符合條件的程式（同 TipTop p_zy_b_fill(g_wc)）。
#   YQ4. 允許執行功能：TTRS 以勾選方式呈現（不是一串文字），條件改為「逐一比對每個功能」——
#        confirm 找出允許 confirm 的程式；conf* 也可；<>confirm 找出不允許 confirm 的程式；= 找出沒有任何功能的程式。
#        （TipTop 是比對整串 zy03 文字，習慣輸入 *confirm*；這種寫法在 TTRS 結果相同。）
#   YQ5. 權限類別代碼、程式代號區分大小寫（同 TipTop，沒有 UPSHIFT）。

功能: 程式執行權限維護查詢（Program Permission Maintenance，QBE）
  身為 系統管理員
  我想要 以權限類別或程式權限的內容查詢
  以便 找出哪些權限類別可以執行某支程式、或擁有某個功能

  # 新增程式時自動授權給目前使用者的權限類別 ADMIN（p_zz）；下表中已有的權限改為表中的值
  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "ADMIN"
    而且 權限類別 "ADMIN" 已存在
    而且 權限類別 "MFG" 已存在
    而且 權限類別 "SALES" 已存在
    而且 權限類別 "TEMP" 已存在
    而且 模組 "AIM" 已存在
    而且 已有程式 "aimi100" 模組 "AIM"，可執行功能 "insert, query, modify, delete, confirm"
    而且 已有程式 "aimi101" 模組 "AIM"，可執行功能 "query, output"
    而且 已有程式 "aimq102" 模組 "AIM"，可執行功能 "query, output"
    而且 已有下列程式執行權限:
      | 權限類別 | 程式代號 | 允許執行功能                         | 他人資料權限 | 他部門資料權限 | 單身權限 |
      | ADMIN    | aimi100  | insert, query, modify, delete, confirm | 0            | 0              | 0        |
      | ADMIN    | aimi101  | query, output                        | 0            | 0              | 0        |
      | ADMIN    | aimq102  | query, output                        | 0            | 0              | 0        |
      | MFG      | aimi100  | query, modify                        | 2            | 4              | 3        |
      | SALES    | aimi101  | query                                | 0            | 3              | 0        |
      | SALES    | aimq102  |                                      | 0            | 0              | 0        |

  # QBE4、YQ2：沒有條件時包含沒有程式權限的 TEMP；p_zy.4gl:197 依權限類別排序
  場景: 不輸入條件查詢全部權限類別
    當 使用者以下列條件查詢程式執行權限:
      | 欄位 | 條件 |
    那麼 查詢結果應為 "ADMIN,MFG,SALES,TEMP" 共 4 筆

  # YQ2：只有單頭條件
  場景: 以權限類別查詢
    當 使用者以下列條件查詢程式執行權限:
      | 欄位     | 條件     |
      | 權限類別 | S*\|T*   |
    那麼 查詢結果應為 "SALES,TEMP"
    而且 權限類別 "SALES" 符合條件的程式應為 "aimi101,aimq102"

  # YQ2、YQ3：有單身條件時只列出有符合程式的權限類別，單身只顯示符合的程式
  場景: 以程式代號查詢
    當 使用者以下列條件查詢程式執行權限:
      | 欄位     | 條件    |
      | 程式代號 | aimi100 |
    那麼 查詢結果應為 "ADMIN,MFG" 共 2 筆
    而且 權限類別 "ADMIN" 符合條件的程式應為 "aimi100"

  # YQ4：逐一比對每個功能
  場景大綱: 以允許執行功能查詢
    當 使用者以下列條件查詢程式執行權限:
      | 欄位         | 條件   |
      | 允許執行功能 | <條件> |
    那麼 查詢結果應為 "<權限類別>"
    而且 權限類別 "<檢查類別>" 符合條件的程式應為 "<程式>"

    例子:
      | 條件       | 權限類別        | 檢查類別 | 程式            |
      | confirm    | ADMIN           | ADMIN    | aimi100         |
      | *confirm*  | ADMIN           | ADMIN    | aimi100         |
      | modify     | ADMIN,MFG       | MFG      | aimi100         |
      | output     | ADMIN           | ADMIN    | aimi101,aimq102 |
      | <>modify   | ADMIN,SALES     | SALES    | aimi101,aimq102 |
      | =          | SALES           | SALES    | aimq102         |

  # YQ1：資料權限以代碼查詢
  場景大綱: 以資料權限代碼查詢
    當 使用者以下列條件查詢程式執行權限:
      | 欄位   | 條件   |
      | <欄位> | <條件> |
    那麼 查詢結果應為 "<結果>"

    例子:
      | 欄位           | 條件  | 結果      |
      | 他人資料權限   | 2\|3  | MFG       |
      | 他部門資料權限 | 3:4   | MFG,SALES |
      | 單身權限       | >0    | MFG       |

  場景: 單頭與單身條件同時成立（AND）
    當 使用者以下列條件查詢程式執行權限:
      | 欄位     | 條件          |
      | 權限類別 | ADMIN\|SALES  |
      | 程式代號 | aimi101       |
    那麼 查詢結果應為 "ADMIN,SALES"
    而且 權限類別 "SALES" 符合條件的程式應為 "aimi101"

  # 同一列須同時符合所有單身條件（同 TipTop 單一 WHERE）
  場景: 多個單身條件須在同一支程式上成立
    當 使用者以下列條件查詢程式執行權限:
      | 欄位           | 條件    |
      | 程式代號       | aimi101 |
      | 他部門資料權限 | 3       |
    那麼 查詢結果應為 "SALES"

  # YQ5
  場景: 程式代號區分大小寫
    當 使用者以下列條件查詢程式執行權限:
      | 欄位     | 條件    |
      | 程式代號 | AIMI100 |
    那麼 查詢結果應為 ""

  場景大綱: 不合法的條件
    當 使用者以下列條件查詢程式執行權限:
      | 欄位   | 條件   |
      | <欄位> | <條件> |
    那麼 應出現錯誤 "<錯誤代碼>"

    例子:
      | 欄位     | 條件 | 錯誤代碼              |
      | 程式代號 | a:   | QBE_INVALID_SYNTAX    |
      | 程式名稱 | 料件* | QBE_FIELD_NOT_ALLOWED |

  # QBE7 分頁（以權限類別為單位）
  場景: 查詢結果分頁
    當 使用者以下列條件查詢程式執行權限，每頁 3 筆，取第 2 頁:
      | 欄位 | 條件 |
    那麼 查詢結果應為 "TEMP" 共 4 筆
