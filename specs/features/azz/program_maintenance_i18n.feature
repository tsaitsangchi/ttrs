# language: zh-TW
# TipTop 對應：p_zz 程式名稱多語言（gaz_file：gaz01 程式代號、gaz02 語言別、gaz03 程式名稱、gaz06 報表抬頭）
# 新系統：Program Maintenance 程式維護 — 程式名稱與報表抬頭依語系維護，並提供介面多語言
# 規則來源：tiptop/azz/4gl/p_zz.4gl :1045-1060（modify_program_name / modify_report_title）、
#           :1075-1100 p_zz_show_rpttitle / p_zz_update_zz03（依 g_lang 取 gaz），tiptop/azz/sch/gaz_file.sch
# 狀態：L1–L7 依草稿假設確認（2026-10-29），進入 Red
#
# 範圍（使用者已確認，2026-10-29）：程式名稱多語言＋畫面介面多語言，語系為繁中 zh-TW、英文 en、簡中 zh-CN
#
# 確認事項（皆採草稿假設）：
#   L1. ✅ 必填：TipTop 不強制任何語系的程式名稱。新系統也不強制。
#   L2. ✅ 顯示：使用者語系沒有名稱時，退回顯示繁中名稱；繁中也沒有則空白。
#   L3. ✅ 介面語系：側邊欄提供切換，預設繁中，記在瀏覽器（localStorage）；日後有登入時改存使用者設定。
#   L4. ✅ 錯誤訊息：前端依錯誤代碼（如 PROGRAM_ALREADY_EXISTS）顯示三語訊息，
#       尚未翻譯的代碼退回伺服器回傳的繁中訊息。
#   L5. ✅ 語言別對照（舊資料轉入用）：TipTop gaz02 為 1 碼：0=繁中、1=英文、2=簡中。
#   L6. ✅ 介面翻譯範圍：先做 Program Maintenance 畫面與共用外框（側邊欄、頁首），
#       aimi100、資料字典等其他畫面之後逐步套用。
#   L7. ✅ 報表抬頭（gaz06）同樣依語系維護。（比照 gaz_file 結構）
#
# 介面多語言（L3、L4、L6）屬畫面行為，不寫成 BDD 場景，以截圖驗證三種語系的畫面。

功能: 程式維護多語言（Program Maintenance）
  身為 系統管理員
  我想要 為每支程式維護繁中、英文、簡中的程式名稱與報表抬頭
  以便 不同語系的使用者在選單與畫面上看到自己的語言

  背景:
    假設 模組 "AIM" 已存在
    而且 權限類別 "CLASS1" 已存在
    而且 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 已有程式 "aimi100" 模組 "AIM"

  # gaz_file 主鍵 (gaz01, gaz02)：每個語系一筆
  場景: 依語系分別維護程式名稱與報表抬頭
    當 使用者以語系 "zh-TW" 將程式 "aimi100" 命名為 "料件基本資料維護作業"，報表抬頭 "料件清單"
    而且 使用者以語系 "en" 將程式 "aimi100" 命名為 "Item Master Maintenance"，報表抬頭 "Item List"
    那麼 程式 "aimi100" 的語系名稱應為:
      | 語系  | 名稱                    | 報表抬頭  |
      | zh-TW | 料件基本資料維護作業    | 料件清單  |
      | en    | Item Master Maintenance | Item List |
      | zh-CN |                         |           |

  場景: 修改某一語系不影響其他語系
    假設 程式 "aimi100" 在語系 "zh-TW" 名稱為 "料件基本資料維護作業"
    而且 程式 "aimi100" 在語系 "en" 名稱為 "Item Master Maintenance"
    當 使用者以語系 "en" 將程式 "aimi100" 命名為 "Item Maintenance"，報表抬頭 ""
    那麼 程式 "aimi100" 在語系 "zh-TW" 的名稱應為 "料件基本資料維護作業"
    而且 程式 "aimi100" 在語系 "en" 的名稱應為 "Item Maintenance"

  場景: 不支援的語系
    當 使用者以語系 "ja" 將程式 "aimi100" 命名為 "品目マスタ"，報表抬頭 ""
    那麼 應出現錯誤 "LANGUAGE_NOT_SUPPORTED"

  # L2：p_zz_update_zz03 依 g_lang 取名稱；缺少時退回繁中
  場景大綱: 依使用者語系顯示程式名稱
    假設 程式 "aimi100" 在語系 "zh-TW" 名稱為 "料件基本資料維護作業"
    而且 程式 "aimi100" 在語系 "en" 名稱為 "Item Master Maintenance"
    當 使用者以語系 "<語系>" 查看程式 "aimi100"
    那麼 顯示的程式名稱應為 "<名稱>"

    例子:
      | 語系  | 名稱                    |
      | zh-TW | 料件基本資料維護作業    |
      | en    | Item Master Maintenance |
      | zh-CN | 料件基本資料維護作業    |

  # :1973-2101 複製時名稱一併複製（所有語系）
  場景: 複製程式時所有語系名稱一併複製
    假設 程式 "aimi100" 在語系 "zh-TW" 名稱為 "料件基本資料維護作業"
    而且 程式 "aimi100" 在語系 "en" 名稱為 "Item Master Maintenance"
    當 使用者將程式 "aimi100" 複製為 "aimi100c"
    那麼 程式 "aimi100c" 在語系 "en" 的名稱應為 "Item Master Maintenance"
    而且 程式 "aimi100c" 在語系 "zh-TW" 的名稱應為 "料件基本資料維護作業"

  # :343-346 以名稱查詢時，任一語系符合即符合
  場景: 以英文名稱查詢程式
    假設 程式 "aimi100" 在語系 "en" 名稱為 "Item Master Maintenance"
    當 使用者以下列條件查詢程式:
      | 欄位     | 條件   |
      | 程式名稱 | Item*  |
    那麼 查詢結果應為 "aimi100"

  # gaz_file.sch：gaz03 varchar2(500)、gaz06 varchar2(255)；每個語系各自檢核
  場景大綱: 各語系名稱長度不可超過上限
    當 使用者以語系 "<語系>" 將程式 "aimi100" 的 "<項目>" 設為 <長度> 個字元
    那麼 應出現錯誤 "FIELD_TOO_LONG"

    例子:
      | 語系  | 項目     | 長度 |
      | en    | 程式名稱 | 501  |
      | zh-CN | 報表抬頭 | 256  |
