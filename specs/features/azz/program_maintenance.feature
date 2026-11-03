# language: zh-TW
# TipTop 對應：p_zz 程式資料建立 / zz_file（輔助：gaz_file 程式名稱、zy_file 群組權限、zm_file 選單）
# 規則來源：tiptop/azz/4gl/p_zz.4gl（版本 5.10.22-11.11.09），行號標於各場景
# 狀態：Q1–Q7 已確認（2026-10-29）。2026-11-03 依 ADR 0008（程式代號改用英文）改寫，P1–P5 依草稿確認（Key User，2026-11-03）。
#
# 依 ADR 0008 改寫（待確認）：
#   P1. 程式代號是英文代號：小寫英文、數字與連字號，英文字母開頭，最多 40 字（E1）；輸入時轉小寫，
#       其他字元 → PROGRAM_NO_INVALID。本規格的範例改用英文代號（如 item-maintenance），TipTop 代號只在出處註明。
#   P2. 新增「TipTop 程式代號」欄位（zz01）：選填，不可與其他程式重複（PROGRAM_TIPTOP_NO_DUPLICATE），可查詢；
#       只用於轉檔與對照（ADR 0008 §1）。
#   P3. 程式類別必填（PROGRAM_TYPE_REQUIRED），不再由代號第 4 碼推導（原 :917-918）；模組為 MENU 時程式類別為 M，不用輸入。
#       以下情境未寫程式類別者為 I（輸入）。
#   P4. 執行指令照輸入保存，不再依標準命名自動產生（原 :917-926）；複製時執行指令沿用來源，不再重產（原 :2045-2049）。
#   P5. p_query 查詢定義（Q5）：執行指令為「p_query <查詢單代號>」時，該查詢單代號必須已有查詢定義
#       （原規則以程式代號比對，英文代號後改取執行指令的參數）。
#
# 確認事項：
#   Q1. ✅ 已確認（Key User）：Genero 執行環境專屬欄位全部保留
#       zz08 執行指令、zz27 視窗型態、zz06 報表副檔名、zz10 程式群組、zz22 固定列印方式、
#       zz21 固定列印條件、zz25 SQL 執行計畫（Set explain on）、zz26 錯誤寫 log。
#       （欄位意義依 tiptop/azz/sch/zz_file.sch 更正，2026-10-29）
#       新系統原樣儲存，不套用規則（見場景「Genero 執行環境欄位原樣保存」）。
#   Q2. ✅ 已確認（Key User）：一律允許修改程式代號，不再受系統參數 g_chkey（:1124）控制；
#       改代號是在「修改」中直接變更程式代號欄位（p_zz_u :1313），連動選單與權限
#       （見場景「修改程式代號時連動選單與權限」）。
#   Q3. ✅ 已確認（Key User）：刪除程式時一律連帶刪除名稱、權限、選單等資料，
#       不再像 TipTop 詢問使用者（:1447）（見場景「刪除程式時連帶刪除相關資料並留下紀錄」）。
#   Q4. ✅ 已確認（Key User）：zz07 延後列印次數、zz16 目前報表列印次數、zz17 報表寬度。
#       新增時 zz07、zz16 預設 0（:781-782）。
#   Q5. ✅ 已確認（Key User）：納入。執行指令含 p_query 的程式（p_query 本身除外）
#       必須已有同代號的查詢定義（zai_file，:973-982），否則錯誤 QUERY_DEFINITION_NOT_FOUND。→ 依 P5 改寫
#   Q6. ✅ 已確認（Key User）：不保留程式代號含「_行業別」時的提示（smb_file，:913,:2999）。
#   Q7. ✅ 已確認（Key User）：p_link/p_base_act/p_base_per/p_help/p_zr 等開發工具連結（:453-565）
#       之後會搬，但不在本規格範圍；各工具另立 feature，屆時再補程式維護的連結入口。
#
# 審核後新增（Key User 已同意）：
#   - 複製程式時執行指令重產為「$FGLRUN $<模組>i/<新代號>」（:2045-2049）。→ 依 P4 取消，改為沿用來源

功能: 程式維護（Program Maintenance）
  身為 系統管理員
  我想要 維護系統中每支作業程式的登錄資料
  以便 選單、權限與執行控制都能依此設定運作

  背景:
    假設 模組 "AIM" 已存在
    而且 模組 "AGL" 已存在
    而且 權限類別 "CLASS1" 已存在
    而且 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"

  # :775-797 新增時預設值；:803-806 建立者資訊
  場景: 新增程式成功並帶入預設值
    當 使用者新增程式 "item-maintenance" 模組 "AIM"
    那麼 操作應成功
    而且 程式 "item-maintenance" 的資料應為:
      | 欄位       | 值     |
      | 報表類型   | 無     |
      | 閒置控制   | 依系統 |
      | 建立者     | tiptop |
      | 建立部門   | MIS    |
      | 最後修改者 |        |
      | 延後列印次數 | 0    |
      | 目前列印次數 | 0    |
      | 報表副檔名   | 固定 |
      | 列印查詢條件 | 否   |
      | 維護作業可修改鍵值 | 否 |
      | 設計日期     | 今天 |
      | 最後修改日期 | 今天 |

  # Q1 確認保留：Genero 執行環境欄位原樣儲存、原樣讀回（:775-797 新增、:1313-1407 修改）
  場景: Genero 執行環境欄位原樣保存
    當 使用者新增程式 "item-maintenance" 模組 "AIM"，執行環境設定為:
      | 欄位         | 值                   |
      | 執行指令     | $FGLRUN $AIMi/aimi100 |
      | 視窗型態     | sm1                  |
      | 報表副檔名   | 固定                 |
      | SQL 執行計畫 | 是                   |
      | 錯誤寫入記錄 | 否                   |
    那麼 操作應成功
    而且 程式 "item-maintenance" 的執行環境設定應為:
      | 欄位         | 值                   |
      | 執行指令     | $FGLRUN $AIMi/aimi100 |
      | 視窗型態     | sm1                  |
      | 報表副檔名   | 固定                 |
      | SQL 執行計畫 | 是                   |
      | 錯誤寫入記錄 | 否                   |

  # 4fd 其餘欄位原樣儲存、原樣讀回（zz04 可執行功能、zz05、zz13、zz31、gaz06 報表抬頭）
  場景: 其他設定欄位原樣保存
    當 使用者新增程式 "item-maintenance" 模組 "AIM"，其他設定為:
      | 欄位               | 值                  |
      | 可執行功能         | insert,modify,delete |
      | 列印查詢條件       | 是                  |
      | 維護作業可修改鍵值 | 是                  |
      | 其他備註           | 由 MIS 維護         |
      | 報表抬頭           | 料件清單            |
    那麼 操作應成功
    而且 程式 "item-maintenance" 的其他設定應為:
      | 欄位               | 值                  |
      | 可執行功能         | insert,modify,delete |
      | 列印查詢條件       | 是                  |
      | 維護作業可修改鍵值 | 是                  |
      | 其他備註           | 由 MIS 維護         |
      | 報表抬頭           | 料件清單            |

  # :1045-1060 修改程式名稱與報表抬頭（TipTop 以 modify_program_name / modify_report_title 動作更新 gaz_file）
  場景: 修改程式名稱與報表抬頭
    假設 已有程式 "item-maintenance" 模組 "AIM"，名稱 "料件基本資料維護作業"
    當 使用者將程式 "item-maintenance" 的名稱改為 "料件主檔維護"，報表抬頭改為 "料件清單"
    那麼 操作應成功
    而且 程式 "item-maintenance" 的名稱應為 "料件主檔維護"
    而且 程式 "item-maintenance" 的其他設定應為:
      | 欄位     | 值       |
      | 報表抬頭 | 料件清單 |

  # P3 程式類別必填，不再由代號推導；選單為 M
  場景: 程式類別必填
    當 使用者新增程式 "item-inquiry" 模組 "AIM"，程式類別 ""
    那麼 應出現錯誤 "PROGRAM_TYPE_REQUIRED"

  場景: 選單程式的程式類別為 M
    當 使用者新增程式 "menu-aim" 模組 "MENU"，程式類別 ""
    那麼 操作應成功
    而且 程式 "menu-aim" 的程式類別應為 "M"

  # P1 程式代號格式：轉小寫；只能有小寫英文、數字與連字號，英文字母開頭
  場景: 程式代號轉為小寫
    當 使用者新增程式 "Item-Maintenance" 模組 "AIM"
    那麼 操作應成功
    而且 程式 "item-maintenance" 的程式類別應為 "I"

  場景大綱: 程式代號格式
    當 使用者新增程式 "<程式代號>" 模組 "AIM"
    那麼 應出現錯誤 "PROGRAM_NO_INVALID"

    例子:
      | 程式代號         |
      | item_maintenance |
      | 1item            |
      | item maintenance |

  # P2 TipTop 程式代號：選填、不可重複
  場景: 記錄 TipTop 程式代號
    當 使用者新增程式 "item-maintenance" 模組 "AIM"，TipTop 程式代號 "aimi100"
    那麼 操作應成功
    而且 程式 "item-maintenance" 的 TipTop 程式代號應為 "aimi100"

  場景: TipTop 程式代號不可重複
    假設 已有程式 "item-maintenance" 模組 "AIM"，TipTop 程式代號 "aimi100"
    當 使用者新增程式 "item-master" 模組 "AIM"，TipTop 程式代號 "aimi100"
    那麼 應出現錯誤 "PROGRAM_TIPTOP_NO_DUPLICATE"

  # Q5、:973-982，P5 執行指令為 p_query <查詢單代號> 時，該查詢單必須已有查詢定義（zai_file）
  場景大綱: p_query 衍生程式必須有查詢定義
    假設 查詢定義 "<已有查詢定義>" 已存在
    當 使用者新增程式 "<程式代號>" 模組 "AIM"，執行指令 "<執行指令>"
    那麼 <結果>

    例子:
      | 程式代號 | 執行指令                   | 已有查詢定義 | 結果                                       |
      | item-query-a     | $FGLRUN p_query aimq900    | aimq900      | 操作應成功                                 |
      | item-query-b     | $FGLRUN p_query aimq901    | aimq900      | 應出現錯誤 "QUERY_DEFINITION_NOT_FOUND"     |
      | custom-query     | $FGLRUN $AZZi/p_query      | aimq900      | 操作應成功                                 |
      | item-maintenance | $FGLRUN $AIMi/aimi100      | aimq900      | 操作應成功                                 |

  # 欄位長度依 TipTop 定義（zz_file.sch、gaz_file.sch 的 varchar2 長度，以字元計）
  場景大綱: 欄位長度不可超過 TipTop 定義
    當 使用者新增程式 "<程式代號>" 模組 "AIM"，"<欄位>" 為 <長度> 個字元
    那麼 <結果>

    例子: 超過上限
      | 程式代號 | 欄位         | 長度 | 結果                          |
      | item-maintenance | 程式名稱     | 501  | 應出現錯誤 "FIELD_TOO_LONG"   |
      | item-maintenance | 報表抬頭     | 256  | 應出現錯誤 "FIELD_TOO_LONG"   |
      | item-maintenance | 執行指令     | 61   | 應出現錯誤 "FIELD_TOO_LONG"   |
      | item-maintenance | 程式群組     | 11   | 應出現錯誤 "FIELD_TOO_LONG"   |
      | item-maintenance | 固定列印條件 | 81   | 應出現錯誤 "FIELD_TOO_LONG"   |
      | item-maintenance | 固定列印方式 | 2    | 應出現錯誤 "FIELD_TOO_LONG"   |
      | item-maintenance | 視窗型態     | 21   | 應出現錯誤 "FIELD_TOO_LONG"   |
      | item-maintenance | 可執行功能   | 3001 | 應出現錯誤 "FIELD_TOO_LONG"   |
      | item-maintenance | 其他備註     | 256  | 應出現錯誤 "FIELD_TOO_LONG"   |

    例子: 剛好等於上限
      | 程式代號 | 欄位         | 長度 | 結果       |
      | item-maintenance | 程式名稱     | 500  | 操作應成功 |
      | item-maintenance | 執行指令     | 60   | 操作應成功 |
      | item-maintenance | 固定列印方式 | 1    | 操作應成功 |

  # P1 英文代號最多 40 字（TipTop zz01 為 20）
  場景: 程式代號不可超過 40 個字元
    當 使用者新增程式 "item-maintenance-item-maintenance-item-ma" 模組 "AIM"
    那麼 應出現錯誤 "PROGRAM_NO_TOO_LONG"

  # :901-911 鍵值不可重複
  場景: 程式代號不可重複
    假設 已有程式 "item-maintenance" 模組 "AIM"
    當 使用者新增程式 "item-maintenance" 模組 "AIM"
    那麼 應出現錯誤 "PROGRAM_ALREADY_EXISTS"

  # 4fd：zz01、zz011 為 notNull/required
  場景大綱: 必填欄位檢核
    當 使用者新增程式 "<程式代號>" 模組 "<模組>"
    那麼 應出現錯誤 "<錯誤代碼>"

    例子:
      | 程式代號 | 模組 | 錯誤代碼          |
      |          | AIM  | PROGRAM_NO_EMPTY  |
      | item-maintenance |      | MODULE_EMPTY      |
      | item-maintenance | ZZZ  | MODULE_NOT_FOUND  |

  # :254-268 模組下拉排除共用程式模組，另加 MENU
  場景大綱: 共用程式模組不可作為程式所屬模組
    當 使用者新增程式 "shared-test" 模組 "<模組>"
    那麼 應出現錯誤 "MODULE_NOT_ALLOWED"

    例子:
      | 模組 |
      | SUB  |
      | CSUB |
      | LIB  |
      | CLIB |
      | QRY  |
      | CQRY |

  # :957-965 報表程式（類別 R）必須指定報表類型，不可為「無」（TQC-810008）
  場景: 報表程式必須指定報表類型
    當 使用者新增程式 "item-list-report" 模組 "AIM"，程式類別 "R"
    那麼 應出現錯誤 "REPORT_TYPE_REQUIRED"

  # :944-951、:1114-1133 只有「由程式設定」才可輸入閒置時間與處理方式，否則清空
  場景大綱: 閒置控制方式決定閒置設定是否保留
    當 使用者新增程式 "item-maintenance" 模組 "AIM"，閒置控制 "<閒置控制>"，閒置秒數 600，逾時處理 "警告後關閉"
    那麼 程式 "item-maintenance" 的閒置秒數應為 "<秒數>"
    而且 程式 "item-maintenance" 的逾時處理應為 "<處理>"

    例子:
      | 閒置控制 | 秒數 | 處理       |
      | 由程式   | 600  | 警告後關閉 |
      | 依系統   |      |            |
      | 不控制   |      |            |

  # :932-938 帳別傳遞旗標僅總帳類模組（AGL/CGL/GGL/CGGL）適用
  場景大綱: 帳別傳遞僅適用總帳模組
    當 使用者新增程式 "<程式代號>" 模組 "<模組>"，傳遞帳別 "是"
    那麼 程式 "<程式代號>" 的傳遞帳別應為 "<結果>"

    例子:
      | 程式代號 | 模組 | 結果 |
      | account-maintenance | AGL  | 是   |
      | item-maintenance | AIM  | 否   |

  # :851、:1843-1891 新增後自動為建立者的權限類別建立該程式權限
  場景: 新增程式後自動授權給建立者的權限類別
    當 使用者新增程式 "item-maintenance" 模組 "AIM"
    那麼 權限類別 "CLASS1" 應擁有程式 "item-maintenance" 的權限

  # :1124、:1388-1392 於「修改」中變更程式代號（TipTop 沒有獨立的改代號功能），連動選單與群組權限
  場景: 修改程式代號時連動選單與權限
    假設 已有程式 "item-maintenance" 模組 "AIM"
    而且 選單 "menu-aim" 包含程式 "item-maintenance"
    當 使用者將程式 "item-maintenance" 的代號改為 "item-master"
    那麼 操作應成功
    而且 選單 "menu-aim" 應包含程式 "item-master"
    而且 權限類別 "CLASS1" 應擁有程式 "item-master" 的權限
    而且 程式 "item-maintenance" 應不存在

  # :1338、:1378 修改時記錄修改者與日期（MOD-B50035）
  場景: 修改程式時記錄修改者
    假設 已有程式 "item-maintenance" 模組 "AIM"
    而且 目前使用者為 "alice"，所屬部門 "MIS"，權限類別 "CLASS1"
    當 使用者將程式 "item-maintenance" 的模組改為 "AGL"
    那麼 程式 "item-maintenance" 的最後修改者應為 "alice"
    而且 程式 "item-maintenance" 的最後修改日期應為今天

  # :1434-1459 刪除時連帶刪除權限、選單、名稱，並寫入異動紀錄（azo_file）
  場景: 刪除程式時連帶刪除相關資料並留下紀錄
    假設 已有程式 "item-maintenance" 模組 "AIM"
    而且 選單 "menu-aim" 包含程式 "item-maintenance"
    當 使用者刪除程式 "item-maintenance"
    那麼 操作應成功
    而且 程式 "item-maintenance" 應不存在
    而且 選單 "menu-aim" 應不包含程式 "item-maintenance"
    而且 權限類別 "CLASS1" 應不擁有程式 "item-maintenance" 的權限
    而且 程式 "item-maintenance" 應無任何語系名稱
    而且 應有一筆 "tiptop" 刪除程式 "item-maintenance" 的異動紀錄

  # :1973-2101 複製：新代號不可重複；名稱一併複製；建立者改為目前使用者；P4 執行指令沿用來源
  場景: 複製程式
    假設 已有程式 "item-maintenance" 模組 "AIM"，名稱 "料件基本資料維護作業"
    而且 目前使用者為 "alice"，所屬部門 "MIS"，權限類別 "CLASS1"
    當 使用者將程式 "item-maintenance" 複製為 "item-maintenance-copy"
    那麼 操作應成功
    而且 程式 "item-maintenance-copy" 的名稱應為 "料件基本資料維護作業"
    而且 程式 "item-maintenance-copy" 的建立者應為 "alice"
    而且 程式 "item-maintenance-copy" 的設計日期應為今天

  場景: 複製時新代號不可重複
    假設 已有程式 "item-maintenance" 模組 "AIM"
    而且 已有程式 "item-master" 模組 "AIM"
    當 使用者將程式 "item-maintenance" 複製為 "item-master"
    那麼 應出現錯誤 "PROGRAM_ALREADY_EXISTS"

  # :455-611 選單（MENU）不是可執行程式：不可設定群組權限；非選單不可維護目錄
  場景: 選單類程式不可設定群組權限
    假設 已有程式 "menu-aim" 模組 "MENU"
    當 使用者為程式 "menu-aim" 設定權限類別 "CLASS1" 的權限
    那麼 應出現錯誤 "MENU_HAS_NO_PERMISSION"

  # :1589-1603 群組權限：同一程式下權限類別不可重複，且必須存在
  場景: 設定群組權限時權限類別必須存在
    假設 已有程式 "item-maintenance" 模組 "AIM"
    當 使用者為程式 "item-maintenance" 設定權限類別 "NOPE" 的權限
    那麼 應出現錯誤 "PERMISSION_CLASS_NOT_FOUND"
