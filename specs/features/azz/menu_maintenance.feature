# language: zh-TW
# TipTop 對應：p_zm 系統目錄資料建立作業 / zm_file（目錄檔）；cl_create_4sm 依目錄產生個人起始選單（udm tree）
# 新系統名稱：Menu Maintenance 選單維護（menu-maintenance，MN1）＋ 側欄樹狀選單
# 規則來源：tiptop/azz/4gl/p_zm.4gl:121-180（查詢單頭目錄代號）、:182-257（功能：新增、查詢、修改、單身、複製、列印、
#           匯出 Excel、重建選單 redomenu、執行 execute）、:336-426（單頭 zm01：須有程式名稱 azz-705；改代號時不可形成迴圈
#           azz-700／azz-701）、:316-327（改目錄代號連動 zm04）、:604-636（刪除目錄：刪除該目錄的單身）、:637-930（單身：
#           序號 zm03 重複 -239、子項 zm04 須有程式名稱 azz-705、不可加入自己或上層目錄 azz-702）、:931-960（重新編號
#           p_zm_b_y）、:1290-1345（複製）、:1346-1395（上層目錄檢查）、:1396-1545（zx05 使用到的目錄須重建 4sm）；
#           tiptop/lib/4gl/cl_create_4sm.4gl:4-9（以 zx05 為根節點；zx05 或 zm_file 異動要重建）、:190-246（依 zm03 排序，
#           有單身的子項為目錄節點、否則為程式節點；**不依權限過濾**，執行程式時才由 p_go 檢查）、:257-285（節點文字取
#           gaz03，客製名稱 gaz05＝Y 優先）；tiptop/azz/sch/zm_file.sch；無 topcust 覆寫。
#           TTRS 現況：migrations/0003 menu_item（無序號）、user_account.menu_program（zx05）、permission_class.default_menu
#           （zw03）；側欄寫死在 crates/web/src/main.rs。
# 狀態：MN1–MN16 依建議確認（Key User，2026-11-09）；MN17 追加，進入 Red
#
# TipTop 的做法（萃取）：
#   - zm_file 主鍵 (zm01 目錄代號, zm03 序號)，zm04 為子項程式代號；子項本身若在 zm_file 有單身，就是下一層目錄。
#   - 目錄也是程式（TTRS 為模組 MENU 的程式），名稱維護在程式資料（gaz_file）。
#   - 每位使用者的 zx05 是自己選單的根；cl_create_4sm 依此產生 .4sm 檔，目錄異動後要「重建選單」。
#   - 選單不依權限隱藏；使用者點選沒有權限的程式時，由執行時的權限檢查擋下。
#
# 確認事項（皆採建議）：
#  MN1. 程式代號 menu-maintenance（ADR 0008），版面比照 p_zm.4fd：單頭目錄代號＋名稱，單身序號、程式代號、程式名稱。可以嗎？
#  MN2. 資料：menu_item 加序號 seq（zm03）；主鍵改為 (menu_no, seq)，另設同一目錄內程式不可重複（TipTop 允許同一程式出現兩次，
#       建議不允許）。既有資料依程式代號排序給號 10、20、30…（新 migration）。可以嗎？
#  MN3. 目錄代號須為已登錄、模組 MENU 的程式；子項須為已登錄的程式（取代 azz-705「未維護名稱」的檢查）。可以嗎？
#  MN4. 不可把自己或上層目錄加入單身（azz-702，MENU_CYCLE）；層數上限沿用程式權限依選單產生的 20 層。可以嗎？
#  MN5. 「修改」目錄代號（p_zm.4gl:316-327）不移植：目錄就是程式，改代號在程式維護做（已連動選單）。可以嗎？
#  MN6. 序號：新增單身時預設「最大序號＋10」；保留「重新編號」（p_zm_b_y，改為 10、20、30…）；另加「上移／下移」（TTRS 新增）。
#       序號在同一目錄內不可重複（-239 → MENU_SEQ_DUPLICATE）。可以嗎？
#  MN7. 複製目錄（reproduce，p_zm.4gl:1290-1345）：輸入新目錄代號（須為已登錄的 MENU 程式且還沒有單身），複製全部單身。可以嗎？
#  MN8. 刪除目錄（p_zm.4gl:604-636 只刪該目錄的單身）：仍是使用者的選單（zx05）、權限類別的預設選單（zw03）或其他目錄的子項時
#       拒絕（MENU_IN_USE，TipTop 不檢查）。可以嗎？
#  MN9. 側欄根節點：使用者的選單（zx05）；空白時用權限類別的預設選單（zw03）；再空白用 menu-main。可以嗎？
#  MN10. 依權限隱藏（TipTop 不隱藏）：沒有執行權限的程式不顯示；沒有可顯示子項的目錄也不顯示。伺服器仍照樣檢查權限。
#        採建議：隱藏。
#  MN11. 尚未移植到 TTRS 的程式（轉入的 TipTop 目錄會有很多）：不顯示。
#  MN12. 畫面網址統一為 /<模組小寫>/<程式代號>；目前 5 支不一致：/aim/items、/aoo/classification-codes、/azz/user-permissions、
#        /azz/permission-classes、/azz/program-permissions（例如改為 /aoo/classification-code-maintenance）。舊網址轉到新網址。可以嗎？
#  MN13. 圖示：TipTop 沒有。程式節點用前端各畫面登錄的圖示，目錄節點一律用資料夾圖示；不在資料庫加欄位。可以嗎？
#  MN14. 起始資料：新 migration 建立 menu-main（首頁之下：庫存管理 aim、整體管理 aoo、系統管理 azz）與目前側欄相同的目錄，
#        既有使用者 zx05 空白者不改（依 MN9 退回 menu-main）。TOPTEST 憑證可用後，另案轉入正式區的 zm_file。可以嗎？
#  MN15. 首頁固定顯示在側欄最上方（TTRS 新增，不在目錄內）。目錄異動後，使用者重新整理頁面即看到新選單（取代 redomenu 重建 .4sm）。
#  MN17. （Key User 2026-11-09 追加）子項是目錄且該目錄還有單身時，不可從上層目錄移除（MENU_NOT_EMPTY），
#        避免整組作業從側欄消失；TipTop p_zm 不檢查。須先移除子目錄的單身。
#  MN16. 不移植的項目：
#        | 項目 | 原因 |
#        | 重建選單 redomenu、zx05 重建 .4sm（p_zm_redo_menu、p_zm_zx05_prepare） | 選單由 API 即時組成，不產生檔案 |
#        | 執行 execute（udm 預覽目錄） | 側欄即是預覽；需要時可在維護畫面加「預覽」 |
#        | 列印系統結構表、程式總覽（p_zm_o1、p_zm_o2）、匯出 Excel | 先不做；需要時依 ADR 0011 另案 |
#        | 修改目錄代號（p_zm_u） | 見 MN5，改在程式維護 |
#        | gaz05 客製名稱優先 | TTRS 程式名稱只有一組（ADR 0005 多語言），沒有客製碼 |

功能: 選單維護與側欄樹狀選單（Menu Maintenance）
  身為 系統管理員
  我想要 用目錄資料決定每位使用者的側欄選單
  以便 不必改程式就能調整作業的分類與順序

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 權限類別 "CLASS1" 已存在
    而且 模組 "MENU" 已存在
    而且 模組 "AOO" 已存在
    而且 模組 "AZZ" 已存在
    而且 模組 "AIM" 已存在
    而且 已登錄 TTRS 已移植的作業
    而且 權限類別 "CLASS1" 擁有全部已移植作業的權限
    而且 營運中心 "TW01" 已啟用
    而且 已有選單程式 "menu-main"、"menu-aoo"、"menu-sales"

  # p_zm.4gl:637-930 單身維護；MN6 序號
  場景: MN-1 新增目錄的單身
    當 使用者在目錄 "menu-aoo" 加入程式:
      | 序號 | 程式代號               |
      | 10   | plant-maintenance      |
      | 20   | department-maintenance |
    那麼 目錄 "menu-aoo" 的單身應為 "plant-maintenance,department-maintenance"

  # MN6 新增時預設序號
  場景: MN-2 不指定序號時接在最後
    假設 目錄 "menu-aoo" 已有 "plant-maintenance"（序號 10）
    當 使用者在目錄 "menu-aoo" 加入程式 "employee-maintenance"
    那麼 目錄 "menu-aoo" 中 "employee-maintenance" 的序號應為 20

  # p_zm.4gl:760-773 序號重複（MN6）
  場景: MN-3 序號重複
    假設 目錄 "menu-aoo" 已有 "plant-maintenance"（序號 10）
    當 使用者在目錄 "menu-aoo" 以序號 10 加入程式 "employee-maintenance"
    那麼 應出現錯誤 "MENU_SEQ_DUPLICATE"

  # MN2 同一目錄內程式不可重複
  場景: MN-4 同一目錄不可重複加入同一程式
    假設 目錄 "menu-aoo" 已有 "plant-maintenance"（序號 10）
    當 使用者在目錄 "menu-aoo" 加入程式 "plant-maintenance"
    那麼 應出現錯誤 "MENU_ITEM_DUPLICATE"

  # p_zm.4gl:775-788 子項須為已登錄的程式（MN3）
  場景: MN-5 子項須為已登錄的程式
    當 使用者在目錄 "menu-aoo" 加入程式 "no-such-program"
    那麼 應出現錯誤 "PROGRAM_NOT_FOUND"

  # p_zm.4gl:784-792、:1346-1395 不可加入自己或上層目錄（MN4）
  場景大綱: MN-6 不可形成迴圈
    假設 目錄 "menu-main" 已有 "menu-aoo"（序號 10）
    當 使用者在目錄 "<目錄>" 加入程式 "<子項>"
    那麼 應出現錯誤 "MENU_CYCLE"

    例子:
      | 目錄     | 子項      |
      | menu-aoo | menu-aoo  |
      | menu-aoo | menu-main |

  # p_zm_b_y（p_zm.4gl:931-960）重新編號；MN6 上移
  場景: MN-7 上移與重新編號
    假設 目錄 "menu-aoo" 已有 "plant-maintenance"（序號 10）
    而且 目錄 "menu-aoo" 已有 "department-maintenance"（序號 15）
    而且 目錄 "menu-aoo" 已有 "employee-maintenance"（序號 40）
    當 使用者將目錄 "menu-aoo" 的 "employee-maintenance" 上移
    而且 使用者將目錄 "menu-aoo" 重新編號
    那麼 目錄 "menu-aoo" 的單身應為 "plant-maintenance,employee-maintenance,department-maintenance"
    而且 目錄 "menu-aoo" 的序號應為 "10,20,30"

  # MN17 子目錄還有單身時不可移除
  場景: MN-14 已掛程式的子目錄不可從上層目錄移除
    假設 目錄 "menu-main" 已有 "menu-aoo"（序號 10）
    而且 目錄 "menu-aoo" 已有 "plant-maintenance"（序號 10）
    當 使用者從目錄 "menu-main" 移除 "menu-aoo"
    那麼 應出現錯誤 "MENU_NOT_EMPTY"
    而且 目錄 "menu-main" 的單身應為 "menu-aoo"

  場景: MN-15 沒有單身的子目錄與一般作業可以移除
    假設 目錄 "menu-main" 已有 "menu-aoo"（序號 10）
    而且 目錄 "menu-main" 已有 "plant-maintenance"（序號 20）
    當 使用者從目錄 "menu-main" 移除 "menu-aoo"
    而且 使用者從目錄 "menu-main" 移除 "plant-maintenance"
    那麼 目錄 "menu-main" 的單身應為 ""

  # p_zm.4gl:1290-1345 複製（MN7）
  場景: MN-8 複製目錄
    假設 目錄 "menu-aoo" 已有 "plant-maintenance"（序號 10）
    當 使用者將目錄 "menu-aoo" 複製為 "menu-sales"
    那麼 目錄 "menu-sales" 的單身應為 "plant-maintenance"

  # p_zm.4gl:604-636 刪除；MN8 仍被使用時拒絕
  場景: MN-9 仍被使用的目錄不可刪除
    假設 目錄 "menu-main" 已有 "menu-aoo"（序號 10）
    當 使用者刪除目錄 "menu-aoo"
    那麼 應出現錯誤 "MENU_IN_USE"

  # cl_create_4sm.4gl:190-246 依序號組成樹狀；有單身的子項為目錄節點（MN9、MN15）
  場景: MN-10 側欄依使用者的選單組成
    假設 目錄 "menu-main" 已有 "menu-aoo"（序號 10）
    而且 目錄 "menu-aoo" 已有 "department-maintenance"（序號 10）
    而且 目錄 "menu-aoo" 已有 "plant-maintenance"（序號 20）
    而且 已有使用者 "u001"，權限類別 "CLASS1"，選單 "menu-main"
    當 使用者 "u001" 取得側欄選單
    那麼 側欄選單應為:
      | 層級 | 節點                   | 類型 |
      | 1    | menu-aoo               | 目錄 |
      | 2    | department-maintenance | 程式 |
      | 2    | plant-maintenance      | 程式 |

  # MN9 根節點退回規則
  場景: MN-11 使用者沒有選單時用權限類別的預設選單
    假設 權限類別 "CLASS1" 的預設選單為 "menu-sales"
    而且 目錄 "menu-sales" 已有 "plant-maintenance"（序號 10）
    而且 已有使用者 "u002"，權限類別 "CLASS1"，選單 ""
    當 使用者 "u002" 取得側欄選單
    那麼 側欄選單的第一層應為 "plant-maintenance"

  # MN10、MN11 依權限與移植狀態隱藏（建議案）
  場景: MN-12 沒有權限或尚未移植的程式不顯示，空目錄也不顯示
    假設 目錄 "menu-main" 已有 "menu-aoo"（序號 10）
    而且 目錄 "menu-aoo" 已有 "plant-maintenance"（序號 10）
    而且 已有程式 "aimi999" 模組 "AIM"
    而且 目錄 "menu-main" 已有 "aimi999"（序號 20）
    而且 已有使用者 "u001"，權限類別 "CLASS1"，選單 "menu-main"
    而且 權限類別 "CLASS1" 沒有作業 "plant-maintenance" 的權限
    當 使用者 "u001" 取得側欄選單
    那麼 側欄選單應為空

  # cl_create_4sm.4gl:257-285 節點文字取程式名稱，依語系（ADR 0005）
  場景: MN-13 節點名稱依介面語系
    假設 目錄 "menu-main" 已有 "plant-maintenance"（序號 10）
    而且 程式 "plant-maintenance" 的英文名稱為 "Plant Maintenance"
    而且 已有使用者 "u001"，權限類別 "CLASS1"，選單 "menu-main"
    當 使用者 "u001" 以語系 "en" 取得側欄選單
    那麼 側欄選單的第一層名稱應為 "Plant Maintenance"
