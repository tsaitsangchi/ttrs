# language: zh-TW
# TipTop program: aooi901 工廠資料建立 (plant setup) / azp_file (plants)
# New system name: Plant Maintenance 營運中心維護
# Rule source: tiptop/aoo/4gl/aooi901.4gl, tiptop/aoo/4fd/aooi901.4fd (aooi901.per), tiptop/aoo/sch/azp_file.sch,
#           action labels tiptop/config/4ad/0/aoo/aooi901.4ad; no topcust override. Line numbers are noted on each scenario.
# Data model: docs/adr/0001-architecture.md §4 (one schema + plant_id), docs/adr/0006-authentication-authorization.md §5
# Status: P1–P12 confirmed (Key User, 2026-11-02; all as proposed); entering Red
#
# Scope:
#   - Maintain plants: plant code, name, time zone, address, phone, fax, active code; add / query / modify / delete / copy / deactivate
#   - In TipTop a plant is a database schema: the DBA creates the schema first, aooi901 then registers it.
#     In TTRS a plant is one row of the plant table; adding a plant creates no table or schema (ADR 0001 §4).
#   - Not in this program: current-plant switching and RLS (ADR 0006 §5, zxx_file), plant parameters such as
#     base currency (aza_file aza17, program aoos010) and company names per language (zo_file); they are ported with their programs.
#     Plants used per module (azq_file) — aooi901 only has commented-out code for it.
#
# Decisions (all ✅ confirmed, 2026-11-02):
#   P1. Database-connection fields are not ported, because a plant is no longer a database:
#       azp03 schema code (unique, required in TipTop), azp04 Oracle DB link (creates a PUBLIC DATABASE LINK, :1072-1154),
#       azp20 MSSQL host IP, azp08 (no use), the actions "修改連線字串" / "報表資料庫維護" (connection strings, :248-280)
#       and the unused copy-azp-to-every-schema routine i901_t (:1049-1070).
#       azp03 is still needed once, by the data migration (each source schema's rows get plant_id = azp01);
#       that mapping lives in docs/legacy-map, not in the plant table.
#   P2. azp053 "ERP 資料庫否" is not ported: it marks whether the schema is an ERP database (default Y, :303-304),
#       and only triggers a warning and menu regeneration. Every TTRS plant is an ERP plant; rows with N are
#       not migrated. Please confirm with the TOPPROD data that no real plant uses N.
#   P3. Plant code: required, up to 10 characters (azp01 varchar2(10)), stored in upper case.
#       TipTop doesn't upper-case it (4fd case NONE), but TTRS already upper-cases plant codes everywhere
#       (items, users), so migrated codes are upper-cased too. New codes PLANT_ID_TOO_LONG.
#   P4. Plant code can't be changed: TipTop allows a rename when g_chkey = Y, asks for confirmation if users use it
#       (aoo-023), then moves zx08 / zxy03 to the new code (:420-452, 807-831). In TTRS plant_id is part of the key of
#       every business table, so a rename would rewrite all business data. I recommend never allowing it
#       (PLANT_ID_IMMUTABLE); create a new plant instead.
#   P5. Name required: TipTop azp02 is optional, but the name is what the plant switcher and user screens show, and the
#       current TTRS table already has it NOT NULL (PLANT_NAME_EMPTY, up to 80 characters). One name, not multilingual
#       (official names per language belong to the company data zo_file, ported later).
#   P6. Time zone as an IANA name (e.g. Asia/Taipei, Asia/Shanghai) instead of TipTop's fixed offsets GMT+12…GMT-11
#       (aooi901.per ITEMS; shown only when system parameter azz05 = Y, :103-109). IANA names handle daylight saving,
#       and TTRS will use the plant's time zone for "today" (TipTop g_today) once plant switching exists.
#       Blank means Asia/Taipei; an unknown name is PLANT_TIME_ZONE_INVALID. Migration maps the offsets
#       (GMT+8 → Asia/Taipei unless the plant says otherwise; the list is reviewed with you).
#   P7. Address (azp051, 255), phone (azp06, 40) and fax (azp07, 40) are kept as optional text, 1:1.
#   P8. Concurrent-user limit is not ported: azp09 / azp10 and the department limit gbo_file (:520-564, 346-383)
#       count Genero processes per database (gbq_file). With company SSO and a web front end, limiting sessions
#       is an IdP / IT policy. Keep it if your Genero license terms still matter for the new system?
#   P9. Active code: azp_file has no active flag, but TTRS already has plant.active, and an inactive plant can't be used
#       for new users or new business data (PLANT_NOT_FOUND, as today). I recommend deactivate / activate actions;
#       deactivating is always allowed and existing data stays as it is.
#  P10. Delete: TipTop refuses while users may use the plant (zxy, aoo-088, :944-953). TTRS refuses while any user has
#       it as default or allowed plant, or any business data belongs to it (PLANT_IN_USE); deactivate instead.
#       Nothing is deleted in cascade.
#  P11. Copy (reproduce, :994-1046) is kept: the new plant gets the name, time zone, address, phone and fax of the source.
#  P12. Screen: as the 4fd, one record form (group01: code, name, time zone, address, phone, fax) with record navigation,
#       plus a plant list; toolbar 新增 / 修改 / 刪除 / 停用・啟用 / 複製. Audit fields (owner, created date, modifier,
#       modified date) are recorded like p_zx; azp_file has none.

功能: 營運中心維護（Plant Maintenance）
  身為 系統管理員
  我想要 維護營運中心的名稱、時區與聯絡資料
  以便 使用者、權限與業務資料可以依營運中心區分

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 權限類別 "CLASS1" 已存在
    而且 選單程式 "menu_main" 已存在
    而且 營運中心 "TW01" 已啟用

  # aooi901.4gl:296-388 新增；P6 時區空白時為 Asia/Taipei；P9、P12 資料有效與資料所有者
  場景: 新增營運中心帶入預設值
    當 使用者新增營運中心 "CN01"，名稱 "昆山廠"，時區 ""
    那麼 操作應成功
    而且 營運中心 "CN01" 應為:
      | 欄位       | 值          |
      | 名稱       | 昆山廠      |
      | 時區       | Asia/Taipei |
      | 資料有效   | 是          |
      | 資料所有者 | tiptop      |
      | 建立日期   | 今天        |

  # P3 代號轉大寫
  場景: 營運中心代號轉為大寫
    當 使用者新增營運中心 "cn02"，名稱 "蘇州廠"，時區 "Asia/Shanghai"
    那麼 操作應成功
    而且 營運中心 "CN02" 的時區應為 "Asia/Shanghai"

  # aooi901.4gl:423-434 鍵值不可重複（-239）
  場景: 營運中心代號不可重複
    當 使用者新增營運中心 "TW01"，名稱 "重複"，時區 ""
    那麼 應出現錯誤 "PLANT_ALREADY_EXISTS"

  # aooi901.per azp01 NOT NULL；azp_file.sch azp01 varchar2(10)；P5 名稱必填
  場景大綱: 代號與名稱的必填與長度
    當 使用者新增營運中心 "<代號>"，名稱 "<名稱>"，時區 ""
    那麼 應出現錯誤 "<錯誤>"

    例子:
      | 代號        | 名稱   | 錯誤              |
      |             | 測試廠 | PLANT_ID_EMPTY    |
      | PLANT012345 | 測試廠 | PLANT_ID_TOO_LONG |
      | CN03        |        | PLANT_NAME_EMPTY  |

  # P6 時區必須是有效的 IANA 時區名稱
  場景: 時區必須是有效的時區名稱
    當 使用者新增營運中心 "CN03"，名稱 "測試廠"，時區 "Taipei"
    那麼 應出現錯誤 "PLANT_TIME_ZONE_INVALID"

  # P4 代號建立後不可修改（TipTop aooi901.4gl:807-831 改代號連動 zx08、zxy03，不移植）
  場景: 營運中心代號不可修改
    當 使用者將營運中心 "TW01" 的代號改為 "TW09"
    那麼 應出現錯誤 "PLANT_ID_IMMUTABLE"

  # aooi901.4gl:753-911 修改；P7 地址、電話、傳真原樣保存；P12 記錄修改者
  場景: 修改營運中心記錄修改者
    當 使用者將營運中心 "TW01" 的地址改為 "新竹市科學園區"，電話改為 "03-5551234"
    那麼 操作應成功
    而且 營運中心 "TW01" 的地址應為 "新竹市科學園區"
    而且 營運中心 "TW01" 的電話應為 "03-5551234"
    而且 營運中心 "TW01" 的最後修改者應為 "tiptop"

  # P9 停用後不可再指定給使用者（PLANT_NOT_FOUND：營運中心不存在或未啟用）
  場景: 停用的營運中心不可設為使用者的預設營運中心
    當 使用者停用營運中心 "TW01"
    那麼 操作應成功
    而且 營運中心 "TW01" 的資料有效應為 "否"
    當 使用者新增使用者 "u001"，名稱 "王小明"，部門 "SAL"，權限類別 "CLASS1"，選單 "menu_main"，預設營運中心 "TW01"
    那麼 應出現錯誤 "PLANT_NOT_FOUND"

  場景: 啟用停用的營運中心
    假設 使用者停用營運中心 "TW01"
    當 使用者啟用營運中心 "TW01"
    那麼 營運中心 "TW01" 的資料有效應為 "是"

  # aooi901.4gl:944-953 使用者可使用此營運中心時不可刪除（aoo-088）
  場景: 有使用者的營運中心不可刪除
    假設 已有使用者 "u001"
    當 使用者刪除營運中心 "TW01"
    那麼 應出現錯誤 "PLANT_IN_USE"

  # P10 已有業務資料的營運中心不可刪除
  場景: 有業務資料的營運中心不可刪除
    假設 單位 "PCS" 已存在
    而且 "TW01" 已有料件 "A001"
    當 使用者刪除營運中心 "TW01"
    那麼 應出現錯誤 "PLANT_IN_USE"

  場景: 刪除未使用的營運中心
    假設 營運中心 "CN09" 已啟用
    當 使用者刪除營運中心 "CN09"
    那麼 操作應成功
    而且 營運中心 "CN09" 應不存在

  # aooi901.4gl:994-1046 複製：新代號，其餘欄位沿用來源；P11、P12 資料所有者為目前使用者
  場景: 複製營運中心
    假設 使用者將營運中心 "TW01" 的地址改為 "新竹市科學園區"，電話改為 "03-5551234"
    當 使用者將營運中心 "TW01" 複製為 "TW02"
    那麼 操作應成功
    而且 營運中心 "TW02" 的地址應為 "新竹市科學園區"
    而且 營運中心 "TW02" 的資料有效應為 "是"

  場景: 複製的新代號不可重複
    假設 營運中心 "CN01" 已啟用
    當 使用者將營運中心 "TW01" 複製為 "CN01"
    那麼 應出現錯誤 "PLANT_ALREADY_EXISTS"
