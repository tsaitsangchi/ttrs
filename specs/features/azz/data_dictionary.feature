# language: zh-TW
# TipTop 對應：p_zta 檔案架構修整記錄維護作業 / zta_file（表頭）、gat_file（表名稱多語言）、
#             gaq_file（欄位說明多語言）、ztb/ztc/ztd_file（欄位/索引/限制）、zs_file（修整記錄）
# 新系統名稱：Data Dictionary Maintenance 資料字典維護
# 規則來源：tiptop/azz/4gl/p_zta.4gl（版本 5.10.22-11.11.14），tiptop/azz/4fd/p_zta.4fd，
#           tiptop/azz/sch/zta_file.sch、gat_file.sch、gaq_file.sch，行號標於各場景
# 狀態：D1–D8 已確認，進入 Red（2026-10-29）
#
# 範圍（使用者已確認，2026-10-29）：只做「資料字典維護」
#   - 維護資料表與欄位的名稱、目的、多語言說明（繁中 zh-TW、英文 en、簡中 zh-CN）
#   - 欄位、索引、限制從 PostgreSQL 實際結構「唯讀」顯示；變更紀錄取自 migration 歷史
#   - 可匯出資料字典
#   - **畫面不執行任何 DDL**：TipTop 的 create/drop/alter table、synonym、匯入/產生 SQL 檔、
#     資料庫比對、任意 SQL 查詢（database_query）、kill_lock、lock mode 等皆不移植；
#     新系統的資料表結構只由 migrations/ 變更（見 CLAUDE.md）
#
# 確認事項：
#   D1. ✅ 已確認（Key User）：新增字典時，資料表必須已存在於資料庫（由 migration 建立），
#       否則 DICT_TABLE_NOT_IN_SCHEMA。TipTop 是先建字典再 ALTER/CREATE，新系統順序相反。
#   D2. ✅ 已確認（Key User）：欄位說明以（資料表, 欄位）為鍵。TipTop gaq_file 以「欄位代碼」為鍵
#       （全系統唯一，因欄位以表前綴命名，如 zz01），新系統欄位名稱會在不同表重複（如 program_no）。
#   D3. ✅ 已確認（Key User）：刪除字典時，一併刪除表與欄位的多語言說明（避免孤兒資料）。
#       TipTop 只刪表頭與結構定義、保留 gat 表說明（:14078 起「暫時不主動刪除相關資料」）。
#   D4. ✅ 已確認（Key User）：新增/修改/刪除不檢核權限。
#       TipTop 需 DBA 權限（g_dba_priv='1'，否則 azz-108）；新系統尚無登入與角色，導入角色後再另行規劃。
#   D5. ✅ 已確認（Key User）：變更紀錄列出「內容涉及該表的 migration」（版本、說明、套用時間），
#       不再人工登錄。TipTop zs_file 逐表記錄修整指令、工作單號、測試/正式區。
#   D6. ✅ 已確認（Key User）：匯出先做 CSV（Excel 可開啟），欄位比照 p_zta_exp2excel：
#       欄位代碼、型別、名稱、額外說明。TipTop 另有報表（p_zta_out）、資料庫手冊（p_zta_out4），之後再議。
#   D7. ✅ 已確認（Key User）：資料表類型（gat07）沿用 TipTop 代碼：P 參數、M 主檔、T 交易、S 統計、B 基本。
#   D8. ✅ 已確認（Key User）：資料表代碼上限 63（PostgreSQL 識別字上限；TipTop zta01 varchar2(15)
#       放不下新系統表名，如 program_permission 18 碼）；其餘依 .sch：表名稱 120、目的 255、備註 255、
#       欄位名稱 255、欄位說明 500、額外說明 2000、上游/下游 80/60。
#
# 不移植（範圍外）：zta07/zta17 Synonym、lockmode、zta10 ERP 資料庫同步旗標、zte_file 預設值設定、
#                  gen_program（TipTop 已隱藏未完成）、generate_database_guide 的 Genero 報表格式

功能: 資料字典維護（Data Dictionary Maintenance）
  身為 系統管理員
  我想要 維護每張資料表與欄位的名稱、目的與多語言說明，並查看實際結構
  以便 開發與使用者都能看懂資料庫裡每張表、每個欄位的意義

  背景:
    假設 模組 "AZZ" 已存在
    而且 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 資料庫中已有資料表 "program"

  # :1265-1340 p_zta_a 新增：產生日期＝今天、產生者＝目前使用者（zta05、zta06）
  場景: 新增資料表字典並帶入預設值
    當 使用者為資料表 "program" 建立字典，模組 "AZZ"，類型 "主檔"
    那麼 操作應成功
    而且 資料表 "program" 的字典應為:
      | 欄位     | 值     |
      | 模組     | AZZ    |
      | 類型     | 主檔   |
      | 建立者   | tiptop |
      | 建立日期 | 今天   |

  # D1：資料表必須已由 migration 建立
  場景: 資料表不存在於資料庫時不可建立字典
    當 使用者為資料表 "no_such_table" 建立字典，模組 "AZZ"，類型 "主檔"
    那麼 應出現錯誤 "DICT_TABLE_NOT_IN_SCHEMA"

  # :13459-13524 p_zta_u 修改表頭：模組、類型、上游/下游檔案（資料表代碼不可改）
  場景: 修改資料表字典
    假設 資料表 "program" 已有字典，模組 "AZZ"
    而且 模組 "AIM" 已存在
    當 使用者將資料表 "program" 的字典改為模組 "AIM"，類型 "參數"，上游 "module"，下游 "program_name"
    那麼 操作應成功
    而且 資料表 "program" 的字典應為:
      | 欄位 | 值           |
      | 模組 | AIM          |
      | 類型 | 參數         |
      | 上游 | module       |
      | 下游 | program_name |

  # :1520-1535 鍵值重複（-239）
  場景: 同一資料表不可重複建立字典
    假設 資料表 "program" 已有字典，模組 "AZZ"
    當 使用者為資料表 "program" 建立字典，模組 "AZZ"，類型 "主檔"
    那麼 應出現錯誤 "DICT_ALREADY_EXISTS"

  # :1607-1626 AFTER INPUT 必填：zta01、zta06、gat06（模組）
  場景大綱: 必填欄位檢核
    當 使用者為資料表 "program" 建立字典，模組 "<模組>"，類型 "主檔"
    那麼 應出現錯誤 "<錯誤代碼>"

    例子:
      | 模組 | 錯誤代碼         |
      |      | MODULE_EMPTY     |
      | ZZZ  | MODULE_NOT_FOUND |

  # :1545-1600 gat03 名稱、gat04 目的依「目前語言」維護；gat06 模組、gat07 類型為各語言共用
  場景: 表名稱與目的依語系分別維護
    假設 資料表 "program" 已有字典，模組 "AZZ"
    當 使用者以語系 "zh-TW" 將資料表 "program" 命名為 "程式資料檔"，目的 "記錄系統所有作業程式"
    而且 使用者以語系 "en" 將資料表 "program" 命名為 "Program Registry"，目的 "All programs of the system"
    那麼 資料表 "program" 在語系 "zh-TW" 的名稱應為 "程式資料檔"
    而且 資料表 "program" 在語系 "en" 的名稱應為 "Program Registry"
    而且 資料表 "program" 在語系 "zh-CN" 的名稱應為 ""

  場景: 不支援的語系
    假設 資料表 "program" 已有字典，模組 "AZZ"
    當 使用者以語系 "ja" 將資料表 "program" 命名為 "プログラム"，目的 ""
    那麼 應出現錯誤 "LANGUAGE_NOT_SUPPORTED"

  # 4fd Page3 Field：欄位清單由資料庫實際結構唯讀顯示（新系統不經畫面改結構）
  場景: 顯示資料表的實際欄位結構
    假設 資料表 "program" 已有字典，模組 "AZZ"
    當 使用者查看資料表 "program" 的欄位
    那麼 欄位 "program_no" 應為:
      | 項目     | 值          |
      | 型別     | varchar(20) |
      | 不可空白 | 是          |
      | 主鍵     | 是          |
    而且 欄位 "remark" 應為:
      | 項目     | 值           |
      | 型別     | varchar(255) |
      | 不可空白 | 否           |
      | 主鍵     | 否           |

  # D2；gaq03 名稱、gaq04 說明、gaq05 額外說明（p_feldname）
  場景: 維護欄位的多語言說明
    假設 資料表 "program" 已有字典，模組 "AZZ"
    當 使用者以語系 "zh-TW" 說明資料表 "program" 的欄位 "program_no"：名稱 "程式代號"，說明 "鍵值"，額外說明 "a 開頭 7 碼取第 4 碼為類別"
    那麼 操作應成功
    而且 資料表 "program" 欄位 "program_no" 在語系 "zh-TW" 的名稱應為 "程式代號"

  場景: 說明不存在的欄位
    假設 資料表 "program" 已有字典，模組 "AZZ"
    當 使用者以語系 "zh-TW" 說明資料表 "program" 的欄位 "no_such_column"：名稱 "x"，說明 ""，額外說明 ""
    那麼 應出現錯誤 "DICT_COLUMN_NOT_FOUND"

  # 4fd Page4 Index、Page5 Constraint：唯讀顯示
  場景: 顯示資料表的索引與限制
    假設 資料表 "program" 已有字典，模組 "AZZ"
    當 使用者查看資料表 "program_permission" 的限制
    那麼 應有主鍵限制，欄位為 "class_code,program_no"
    而且 應有外鍵限制，欄位 "program_no" 參照 "program.program_no"

  # D8：長度依 gat_file.sch / gaq_file.sch
  場景大綱: 說明長度不可超過上限
    假設 資料表 "program" 已有字典，模組 "AZZ"
    當 使用者以語系 "zh-TW" 將資料表 "program" 的 "<項目>" 設為 <長度> 個字元
    那麼 應出現錯誤 "FIELD_TOO_LONG"

    例子:
      | 項目     | 長度 |
      | 表名稱   | 121  |
      | 表目的   | 256  |
      | 欄位名稱 | 256  |
      | 欄位說明 | 501  |
      | 額外說明 | 2001 |

  # D3：:14078-14238 刪除；表與欄位的多語言說明一併刪除，實際資料表不受影響
  場景: 刪除資料字典
    假設 資料表 "program" 已有字典，模組 "AZZ"
    而且 資料表 "program" 在語系 "zh-TW" 已命名為 "程式資料檔"
    而且 資料表 "program" 欄位 "program_no" 在語系 "zh-TW" 已說明為 "程式代號"
    當 使用者刪除資料表 "program" 的字典
    那麼 操作應成功
    而且 資料表 "program" 應無字典
    而且 資料表 "program" 應無任何語系的表說明
    而且 資料表 "program" 應無任何欄位說明
    而且 資料庫中仍應有資料表 "program"

  # D5：變更紀錄改由 migration 歷史推得
  場景: 查看資料表的變更紀錄
    假設 資料表 "program" 已有字典，模組 "AZZ"
    當 使用者查看資料表 "program" 的變更紀錄
    那麼 變更紀錄應包含 migration "0003"
    而且 變更紀錄應包含 migration "0007"
    而且 變更紀錄不應包含 migration "0001"

  # D6：比照 p_zta_exp2excel 的欄位
  場景: 匯出資料字典
    假設 資料表 "program" 已有字典，模組 "AZZ"
    而且 資料表 "program" 欄位 "program_no" 在語系 "zh-TW" 已說明為 "程式代號"
    當 使用者以語系 "zh-TW" 匯出資料表 "program" 的資料字典
    那麼 匯出內容應包含一列 "program_no,varchar(20),程式代號,"
