# language: zh-TW
# TipTop source: the .sch files of every TipTop table (tiptop/azz/sch/*.sch, 161 tables / 1,633 columns),
#             parsed into legacy/catalog/tables.json by scripts/tiptop/sch_catalog.py
# New system name: TipTop Catalog (TipTop 對照目錄); viewed from Data Dictionary Maintenance
# Rule source: .sch (Oracle DDL plus Chinese description per column); naming rules below
# Status: C1–C8 confirmed (Key User, 2026-10-30); entering Red
#
# Purpose:
#   Give each TipTop table and column a recognizable English name, together with the original code,
#   type, primary key and description, stored as a read-only cross-reference. When a TipTop program
#   is later ported, the new table and column names come straight from here. No physical tables are
#   created (D1 still applies: a data dictionary can only be built on a table that actually exists).
#
# Naming rules (confirmed; full naming in docs/legacy-map/tiptop-naming/azz.yaml, generated and checked by scripts/tiptop/gen_naming.py):
#   - Table: drop _file, use semantic English snake_case singular, at most 63 characters,
#     and avoid PostgreSQL reserved words (e.g. user → user_account).
#     Already-ported tables keep their current names (docs/legacy-map/azz.yaml), e.g. zz_file → program.
#   - Column: semantic English snake_case, unique within the table. Common TipTop columns are unified:
#       xxuser → created_by, xxgrup → created_dept, xxmodu → updated_by, xxdate → updated_on,
#       xxacti → active, language code (e.g. zx06/ze02/gae03) → lang, plant → plant_id
#   - Columns the .sch marks as "No Use": unused_<code> (e.g. unused_zx13), kept as a record
#   - When unsure, mark verify (yellow on screen) for the user to confirm
#
# Decisions (all ✅ confirmed, 2026-10-30):
#   C1. Where it shows: in Data Dictionary Maintenance, add a "TipTop 對照目錄" (TipTop catalog) mode
#       (switched from the toolbar) that browses the 161 TipTop tables read-only with the same layout
#       (main screen = table code / English name / Chinese name; Field tab = columns with English
#       names). In addition, a dictionary for an already-ported table gets a "TipTop 對照" lower tab
#       listing old column → new column.
#   C2. Source of truth: the English names live in a version-controlled naming file
#       (docs/legacy-map/tiptop-naming/*.yaml). Import is driven by the .sch catalog plus the naming
#       file and can be re-run (re-importing overwrites with the files). English names are not edited
#       on screen; changes go through the naming file.
#   C3. How import runs: a command (make legacy-catalog) reads the catalog and naming file and calls
#       the API to import. TTRS itself still never connects to the TipTop database.
#   C4. Prefill from the catalog: when creating a dictionary, if the table has a TipTop cross-reference
#       (e.g. program ← zz_file), automatically fill the zh-TW table name (.sch table description) and
#       column descriptions (.sch column descriptions). The user can still change them.
#   C5. "No Use" columns (98) are imported too (marked unused), not dropped.
#   C6. English name uniqueness: the import is rejected when two tables share an English name, or two
#       columns in one table share an English name (error code LEGACY_NAME_DUPLICATE).
#   C7. Scope: this round covers only the 161 tables currently imported (azz system admin). Other
#       modules are added later through the same process after legacy/ is imported.
#   C8. Naming style follows the 4 sample tables; the other 157 are drafted, and uncertain ones are
#       marked verify (highlighted on screen) for review.

功能: TipTop 對照目錄（TipTop Catalog）
  身為 系統管理員
  我想要 查到每張 TipTop 資料表與欄位對應的英文名稱與說明
  以便 移植作業時沿用一致的命名，並看懂舊系統的資料結構

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 TipTop 資料目錄有資料表 "zx_file"，說明 "部門使用者檔"，欄位:
      | 欄位   | 型別         | 主鍵 | 說明       | 備註                              |
      | zx01   | varchar2(10) | 是   | 使用者代號 | 使用者代號,(Login user name)      |
      | zx02   | varchar2(80) |      | 使用者名稱 |                                   |
      | zx06   | varchar2(1)  |      | 使用語言別 | 使用語言別(0.中文/1.英文/2.簡體中文) |
      | zx13   | varchar2(1)  |      | No Use     |                                   |
      | zxuser | varchar2(10) |      | OWNER      |                                   |
    而且 命名檔將 "zx_file" 命名為 "user_account"，欄位:
      | 欄位   | 英文名稱    |
      | zx01   | user_id     |
      | zx02   | user_name   |
      | zx06   | lang        |
      | zx13   | unused_zx13 |
      | zxuser | created_by  |

  場景: 匯入後可查到 TipTop 資料表的英文名稱與欄位對照
    當 系統管理員匯入 TipTop 對照目錄
    那麼 操作應成功
    而且 TipTop 資料表 "zx_file" 的英文名稱應為 "user_account"，說明 "部門使用者檔"
    而且 TipTop 資料表 "zx_file" 的欄位對照應為:
      | 欄位   | 英文名稱    | 型別         | 主鍵 | 說明       | 未使用 |
      | zx01   | user_id     | varchar2(10) | 是   | 使用者代號 | 否     |
      | zx02   | user_name   | varchar2(80) | 否   | 使用者名稱 | 否     |
      | zx06   | lang        | varchar2(1)  | 否   | 使用語言別 | 否     |
      | zx13   | unused_zx13 | varchar2(1)  | 否   | No Use     | 是     |
      | zxuser | created_by  | varchar2(10) | 否   | OWNER      | 否     |

  # C2：可重複匯入，以命名檔為準
  場景: 重新匯入以命名檔為準更新英文名稱
    假設 已匯入 TipTop 對照目錄
    而且 命名檔將 "zx_file" 的欄位 "zx02" 改名為 "display_name"
    當 系統管理員匯入 TipTop 對照目錄
    那麼 TipTop 資料表 "zx_file" 欄位 "zx02" 的英文名稱應為 "display_name"

  # C6
  場景: 同一張表的英文欄位名稱重複時拒絕匯入
    假設 命名檔將 "zx_file" 的欄位 "zx02" 改名為 "user_id"
    當 系統管理員匯入 TipTop 對照目錄
    那麼 應出現錯誤 "LEGACY_NAME_DUPLICATE"
    而且 TipTop 對照目錄應為空

  場景: 英文名稱不符命名規則時拒絕匯入
    假設 命名檔將 "zx_file" 命名為 "User-Account"
    當 系統管理員匯入 TipTop 對照目錄
    那麼 應出現錯誤 "LEGACY_NAME_INVALID"

  場景: 依舊表代碼、英文名稱或說明查詢對照
    假設 已匯入 TipTop 對照目錄
    當 使用者以 "使用者" 查詢 TipTop 對照目錄
    那麼 查詢結果應包含 TipTop 資料表 "zx_file"
    當 使用者以 "user_acc*" 查詢 TipTop 對照目錄
    那麼 查詢結果應包含 TipTop 資料表 "zx_file"

  # C1：已移植的表在資料字典顯示 TipTop 對照
  場景: 已移植的資料表可查到對應的 TipTop 舊表
    假設 模組 "AZZ" 已存在
    而且 資料庫中已有資料表 "program"
    而且 TipTop 資料目錄有資料表 "zz_file"，說明 "程式資料檔"，欄位:
      | 欄位 | 型別         | 主鍵 | 說明     | 備註 |
      | zz01 | varchar2(20) | 是   | 程式代號 |      |
    而且 命名檔將 "zz_file" 命名為 "program"，欄位:
      | 欄位 | 英文名稱   |
      | zz01 | program_no |
    而且 已匯入 TipTop 對照目錄
    當 使用者查看資料表 "program" 的 TipTop 對照
    那麼 對應的 TipTop 舊表應為 "zz_file"
    而且 欄位 "program_no" 對應的 TipTop 欄位應為 "zz01"

  # C4：建立字典時帶入 .sch 說明
  場景: 建立字典時自動帶入 TipTop 的中文名稱與欄位說明
    假設 模組 "AZZ" 已存在
    而且 資料庫中已有資料表 "program"
    而且 TipTop 資料目錄有資料表 "zz_file"，說明 "程式資料檔"，欄位:
      | 欄位 | 型別         | 主鍵 | 說明     | 備註 |
      | zz01 | varchar2(20) | 是   | 程式代號 |      |
    而且 命名檔將 "zz_file" 命名為 "program"，欄位:
      | 欄位 | 英文名稱   |
      | zz01 | program_no |
    而且 已匯入 TipTop 對照目錄
    當 使用者為資料表 "program" 建立字典，模組 "AZZ"，類型 "主檔"
    那麼 資料表 "program" 在語系 "zh-TW" 的名稱應為 "程式資料檔"
    而且 資料表 "program" 欄位 "program_no" 在語系 "zh-TW" 的名稱應為 "程式代號"
