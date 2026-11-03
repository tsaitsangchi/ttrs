# language: zh-TW
# TipTop 對應：aooi040 員工姓名資料維護 / gen_file（員工姓名資料）
# 新系統名稱：Employee Maintenance 員工維護
# 規則來源：tiptop/aoo/4gl/aooi040.4gl、tiptop/aoo/4fd/aooi040.4fd（aooi040.per）、tiptop/aoo/sch/gen_file.sch、
#           tiptop/azz/sch/gbc_file.sch（名稱多語言）；無 topcust 覆寫。行號標於各場景。
# 狀態：EQ1–EQ9 依草稿確認（Key User，2026-11-03），進入 Red
#
# TipTop 的做法：單檔多欄（表格）作業，在單身新增、修改、刪除員工（aooi040.4gl:180-470）；
#   部門須存在且有效（:608-622）。總參數 aza44＝Y 時員工姓名另存多語言（gbc_file，:343-350、:390）。
#
# 確認事項（皆採草稿）：
#   EQ1. 層級（ADR 0007 §2）：員工是工廠層，每廠的 schema 各一份（同部門，department_maintenance.feature DQ1）。
#   EQ2. 欄位：
#        | 欄位 | TipTop | 規則 |
#        | 員工編號 | gen01 varchar2(10) | 必填、轉大寫、不可重複（-239，:328-339）；建立後不可修改 |
#        | 員工姓名 | gen02 varchar2(40) | 必填（TipTop 未強制） |
#        | 部門 | gen03 varchar2(10) | 選填；有值時須為本營運中心存在（aoo-005）且有效（9028）的部門（:608-622） |
#        | 職稱 | gen04 varchar2(80) | 選填 |
#        | 分機 | gen05 varchar2(5) | 選填 |
#        | E-mail | gen06 varchar2(80) | 選填；有值時檢查格式（同使用者維護的電子郵件，TipTop 不檢查） |
#        | 資料有效 | genacti | 新增預設 Y（:254） |
#        資料所有者、所有部門、修改者、修改日照 TTRS 慣例記錄。
#   EQ3. 員工姓名不做多語言：TipTop 只在 aza44＝Y 時存 gbc_file；請確認 TOPPROD 的 aza44 與 gbc_file 是否有員工姓名資料，
#        有的話改依 ADR 0005 另建 `employee_text`。
#   EQ4. 員工與使用者（zx_file）沒有關聯欄位，維持各自維護。
#   EQ5. 權限：功能 query、detail（新增、修改、刪除，單身新增列／刪除列另看單身權限）；TipTop 沒有設定資料所有者，
#        不檢查資料權限。
#   EQ6. 刪除：TipTop 不檢查引用（:380-405），目前也沒有引用員工的資料表，照 TipTop 直接刪除；
#        日後有單據引用員工時再加檢查。停用一律允許。
#   EQ7. 不移植：銀行資料（gen_bank_inf，開 gpyi032 薪資作業）、MDM／SPC 整合、相關文件、報表列印與匯出 Excel（共用功能）。
#   EQ9. 不移植的欄位：沒有，gen_file 的 11 個欄位都有對應。
#   EQ8. 查詢與畫面：可查 CONSTRUCT 的 7 個欄位（:628-630），員工編號與部門條件轉大寫、依員工編號排序（:692）；
#        表格欄位依序為員工編號、姓名、部門（含部門名稱）、職稱、分機、E-mail、資料有效。

功能: 員工維護（Employee Maintenance）
  身為 系統管理員
  我想要 維護各營運中心的員工
  以便 單據與報表可以引用員工與其部門

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 營運中心 "TW01" 已啟用
    而且 "TW01" 已有部門 "SAL"

  # aooi040.4gl:246-258 新增預設資料有效 Y
  場景: 新增員工帶入預設值
    當 使用者在 "TW01" 新增員工 "e001"，姓名 "王小明"，部門 "sal"
    那麼 操作應成功
    而且 "TW01" 員工 "E001" 應為:
      | 欄位       | 值     |
      | 姓名       | 王小明 |
      | 部門       | SAL    |
      | 部門名稱   | SAL    |
      | 資料有效   | 是     |
      | 資料所有者 | tiptop |

  # aooi040.4gl:328-339 員工編號不可重複（-239）
  場景: 員工編號不可重複
    假設 "TW01" 已有員工 "E001"，部門 "SAL"
    當 使用者在 "TW01" 新增員工 "E001"，姓名 "重複"，部門 ""
    那麼 應出現錯誤 "EMPLOYEE_ALREADY_EXISTS"

  # EQ2 編號、姓名必填
  場景大綱: 員工編號與姓名的檢查
    當 使用者在 "TW01" 新增員工 "<員工編號>"，姓名 "<姓名>"，部門 ""
    那麼 應出現錯誤 "<錯誤>"

    例子:
      | 員工編號    | 姓名   | 錯誤                   |
      |             | 王小明 | EMPLOYEE_ID_EMPTY      |
      | EMP01234567 | 王小明 | EMPLOYEE_ID_TOO_LONG   |
      | E001        |        | EMPLOYEE_NAME_EMPTY    |

  # aooi040.4gl:608-622 部門須存在（aoo-005）且有效（9028）
  場景大綱: 員工的部門須為有效的部門
    假設 "TW01" 已有部門 "MFG"
    而且 使用者停用 "TW01" 部門 "MFG"
    當 使用者在 "TW01" 新增員工 "E001"，姓名 "王小明"，部門 "<部門>"
    那麼 應出現錯誤 "<錯誤>"

    例子:
      | 部門 | 錯誤                   |
      | NONE | DEPARTMENT_NOT_FOUND   |
      | MFG  | DEPARTMENT_INACTIVE    |

  # EQ1 員工屬於營運中心：別的營運中心的部門不能用
  場景: 員工只能指定本營運中心的部門
    假設 營運中心 "CN01" 已啟用
    當 使用者在 "CN01" 新增員工 "E001"，姓名 "王小明"，部門 "SAL"
    那麼 應出現錯誤 "DEPARTMENT_NOT_FOUND"

  # EQ2 E-mail 格式
  場景: E-mail 須為有效格式
    當 使用者在 "TW01" 新增員工 "E001"，姓名 "王小明"，部門 "SAL"，E-mail "not-an-email"
    那麼 應出現錯誤 "EMPLOYEE_EMAIL_INVALID"

  # EQ2 員工編號建立後不可修改
  場景: 員工編號建立後不可修改
    假設 "TW01" 已有員工 "E001"，部門 "SAL"
    當 使用者將 "TW01" 員工 "E001" 的編號改為 "E002"
    那麼 應出現錯誤 "EMPLOYEE_ID_IMMUTABLE"

  # aooi040.4gl:380-405 刪除不檢查引用（EQ6）
  場景: 刪除員工
    假設 "TW01" 已有員工 "E001"，部門 "SAL"
    當 使用者刪除 "TW01" 員工 "E001"
    那麼 操作應成功
    而且 "TW01" 員工 "E001" 應不存在

  # EQ8 查詢：部門條件轉大寫
  場景: 以部門查詢員工
    假設 "TW01" 已有部門 "MFG"
    而且 "TW01" 已有員工 "E001"，部門 "SAL"
    而且 "TW01" 已有員工 "E002"，部門 "MFG"
    當 使用者在 "TW01" 以下列條件查詢員工:
      | 欄位 | 條件 |
      | 部門 | sal  |
    那麼 查詢結果應為 "E001"
