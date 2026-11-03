# language: zh-TW
# TipTop program: p_zw 權限類別建立作業 (permission class setup) / zw_file (permission classes)
# New system name: Permission Class Maintenance 權限類別維護
# Rule source: tiptop/azz/4gl/p_zw.4gl, tiptop/azz/4fd/p_zw.4fd, tiptop/azz/sch/zw_file.sch,
#           action labels tiptop/config/4ad/0/azz/p_zw.4ad; runtime idle handling tiptop/lib/4gl/cl_user.4gl:355-380,
#           cl_on_idle.4gl:40-110; linkage with users tiptop/azz/4gl/p_zx.4gl:582-605; no topcust override.
#           Line numbers are noted on each scenario.
# Login and permission framework: docs/adr/0006-authentication-authorization.md
# Status: W1–W11 confirmed (Key User, 2026-11-02; all as proposed); entering Red
#
# Scope:
#   - Maintain permission classes: class code, description, default menu, idle control / idle action / idle seconds, active code;
#     add / query / modify / delete / deactivate
#   - Batch action "更新所有使用者目錄編號" (update_all_user_menu): copy each class's default menu to its users (p_zw.4gl:761-776)
#   - Linkage with User Maintenance (p_zx, its U4 follow-up): an inactive class can't be assigned; an empty user menu
#     defaults to the class's default menu (p_zx.4gl:582-605)
#   - Buttons to "維護群組執行權限" (p_zy) and "維護群組列印權限" (p_zy2) are shown once those programs are ported
#   - Not ported: report output via p_query (report framework comes later), Excel export (shared feature comes later),
#     called-from-p_zy mode (ARG_VAL(1) = 'Y', p_zw.4gl:94, 789-791)
#   - Runtime idle handling (program zz28 → class zw04 → system aza33/aza35/aza36, cl_user.4gl:355-380)
#     is enforced by the web shell / login session (ADR 0006), not specced here; this program only maintains the settings
#   - Rows of other users' permission classes (zxw, zxw03 = '1') are also checked by TipTop on deactivate / delete / rename;
#     that part is added together with p_zxw user permissions
#   - TipTop message texts (azz-005, azz-217, azz-220, azz-221, azz-001) could not be read: ze_file is only in the database
#     and TOPTEST rejects the thin-mode client (DPY-3010). Their meanings below are inferred from the code around each call.
#
# Decisions (all ✅ confirmed, 2026-11-02):
#   W1. Class code can't be changed: TipTop allows renaming zw01 when system parameter g_chkey = Y and no user or zxw row
#       uses the class (azz-220, p_zw.4gl:464-487, 731-733). It then only updates zw_file, so the class's program permissions
#       in zy_file stay on the old code (azz-221 just tells the user). I recommend never allowing a rename
#       (PERMISSION_CLASS_CODE_IMMUTABLE); create a new class instead.
#   W2. Description required: TipTop zw02 is optional, but the description is what User Maintenance shows next to the class,
#       and the current TTRS table already has it NOT NULL. I recommend required (PERMISSION_CLASS_NAME_EMPTY), up to 80 characters.
#   W3. Description is not multilingual: TipTop stores one zw02. It's only seen by administrators, so I recommend one value
#       (no ADR 0005 _text table), the same as the user name (p_zx U5).
#   W4. Default menu (zw03) is optional; when entered it must be a program of module MENU (p_zw.4gl:489-497),
#       otherwise PERMISSION_CLASS_MENU_INVALID.
#   W5. Idle settings are kept with TipTop's rules: idle control 1 不控制 (default) / 2 依權限類別 / 3 依系統;
#       only 依權限類別 keeps idle action (1 僅警告 / 2 警告後關閉 / 3 直接關閉) and idle seconds, which are then required
#       (4fd required, p_zw.4gl:438, 503-509, 721-740); seconds 1–99999 (zw06 number(5)).
#       New codes IDLE_SETTING_REQUIRED and IDLE_SECONDS_INVALID. Should idle settings be kept at all in the web version
#       (session timeout is also an IT security policy)? I recommend keeping them so the data migrates 1:1.
#   W6. Deactivate: refused while any user has the class (azz-217, p_zw.4gl:511-535) → PERMISSION_CLASS_IN_USE.
#       Activating again is always allowed.
#   W7. Delete: refused while any user has the class (azz-217) or the class still has program permissions
#       (zy_file, azz-005, p_zw.4gl:537-582) → PERMISSION_CLASS_IN_USE / PERMISSION_CLASS_HAS_PERMISSIONS.
#       Nothing is deleted in cascade (same as TipTop).
#   W8. Basic programs are not auto-granted: on add, TipTop inserts 10 programs into zy_file (aoos901, cl_cmdask, p_contview,
#       p_cron, p_favorite, p_load_msg, p_query, p_view, udm_tree, webpasswd; p_zw.4gl:96-108, 333-343), and the action
#       "新增基本程式權限" does the same for a class without permissions (p_zw.4gl:970-999). These are GDC desktop tools or
#       password functions (webpasswd) that TTRS doesn't port. I recommend a new class starts with no program permissions;
#       functions every logged-in user may use (preferences, favorites) are allowed by the Authorizer itself, not copied into each class.
#   W9. "更新所有使用者目錄編號": TipTop asks for confirmation (azz-001), then for every class with a default menu sets the menu
#       of all its users (p_zw.4gl:761-776), without recording who changed them. I recommend keeping the same scope (all classes,
#       classes without a default menu are skipped), touching only users whose menu differs, and recording modifier and date on
#       those users (as User Maintenance does). The result reports how many users were updated.
#  W10. Audit fields: zwuser / zwgrup / zwmodu / zwdate exist in zw_file but p_zw never fills them (p_zw.4gl:321-323, 597-604).
#       I recommend recording owner, owner department, created date, modifier and modified date, like p_zx.
#  W11. Screen: TipTop is one editable grid (INPUT ARRAY) with the 7 columns of the 4fd and a row count.
#       I recommend the same table layout (columns and order as the 4fd) with a filter row for query; 新增 / 修改 open
#       a small row editor, and the toolbar has 新增 / 修改 / 刪除 / 停用・啟用 / 更新所有使用者目錄編號.

功能: 權限類別維護（Permission Class Maintenance）
  身為 系統管理員
  我想要 維護權限類別、預設選單與閒置設定
  以便 使用者依權限類別取得 TipTop 相同的執行權限與選單

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 權限類別 "CLASS1" 已存在
    而且 選單程式 "menu-main" 已存在
    而且 選單程式 "menu-sales" 已存在
    而且 營運中心 "TW01" 已啟用

  # p_zw.4gl:434-449 新增預設：閒置控制 1 不控制、資料有效 Y；W10 記錄資料所有者
  場景: 新增權限類別帶入預設值
    當 使用者新增權限類別 "SALES"，說明 "業務人員"，預設選單 "menu-sales"
    那麼 操作應成功
    而且 權限類別 "SALES" 應為:
      | 欄位         | 值         |
      | 說明         | 業務人員   |
      | 預設選單     | menu-sales |
      | 閒置控制     | 不控制     |
      | 閒置處理     |            |
      | 閒置秒數     |            |
      | 資料有效     | 是         |
      | 資料所有者   | tiptop     |
      | 資料所有部門 | MIS        |
      | 建立日期     | 今天       |

  # W8：不自動加入 TipTop 基本程式權限（p_zw.4gl:333-343 不移植）
  場景: 新增的權限類別沒有任何程式權限
    當 使用者新增權限類別 "SALES"，說明 "業務人員"，預設選單 ""
    那麼 操作應成功
    而且 權限類別 "SALES" 應沒有程式權限

  # p_zw.4gl:451-462 鍵值不可重複（-239）
  場景: 權限類別代號不可重複
    當 使用者新增權限類別 "CLASS1"，說明 "重複"，預設選單 ""
    那麼 應出現錯誤 "PERMISSION_CLASS_ALREADY_EXISTS"

  # p_zw.4fd zw01 required；zw_file.sch zw01 varchar2(10)；W2 說明必填、最多 80 字
  場景大綱: 代號與說明的必填與長度
    當 使用者新增權限類別 "<代號>"，說明 "<說明>"，預設選單 ""
    那麼 應出現錯誤 "<錯誤>"

    例子:
      | 代號        | 說明     | 錯誤                          |
      |             | 業務人員 | PERMISSION_CLASS_CODE_EMPTY    |
      | CLASS012345 | 業務人員 | PERMISSION_CLASS_CODE_TOO_LONG |
      | SALES       |          | PERMISSION_CLASS_NAME_EMPTY    |

  # W1：代號建立後不可修改（TipTop p_zw.4gl:464-487 有條件允許，不移植）
  場景: 權限類別代號不可修改
    當 使用者將權限類別 "CLASS1" 的代號改為 "CLASS9"
    那麼 應出現錯誤 "PERMISSION_CLASS_CODE_IMMUTABLE"

  # p_zw.4gl:489-497 預設選單必須是模組 MENU 的程式
  場景: 預設選單必須是選單程式
    假設 模組 "AIM" 已存在
    而且 已有程式 "item-maintenance" 模組 "AIM"
    當 使用者新增權限類別 "SALES"，說明 "業務人員"，預設選單 "item-maintenance"
    那麼 應出現錯誤 "PERMISSION_CLASS_MENU_INVALID"

  # p_zw.4gl:503-509、721-740 只有「依權限類別」才保留閒置處理與秒數，其餘清空
  場景大綱: 閒置控制方式決定閒置設定是否保留
    當 使用者將權限類別 "CLASS1" 設為閒置控制 "<閒置控制>"，閒置處理 "警告後關閉"，閒置秒數 600
    那麼 操作應成功
    而且 權限類別 "CLASS1" 的閒置處理應為 "<處理>"
    而且 權限類別 "CLASS1" 的閒置秒數應為 "<秒數>"

    例子:
      | 閒置控制   | 處理       | 秒數 |
      | 依權限類別 | 警告後關閉 | 600  |
      | 依系統     |            |      |
      | 不控制     |            |      |

  # p_zw.4fd zw05、zw06 required（可輸入時）；zw06 number(5)
  場景大綱: 依權限類別時閒置處理與秒數必填且秒數在範圍內
    當 使用者將權限類別 "CLASS1" 設為閒置控制 "依權限類別"，閒置處理 "<處理>"，閒置秒數 <秒數>
    那麼 應出現錯誤 "<錯誤>"

    例子:
      | 處理     | 秒數   | 錯誤                  |
      |          | 600    | IDLE_SETTING_REQUIRED |
      | 僅警告   | 0      | IDLE_SECONDS_INVALID  |
      | 僅警告   | 100000 | IDLE_SECONDS_INVALID  |

  # p_zw.4gl:597-614 修改；W10 記錄最後修改者與日期
  場景: 修改權限類別記錄修改者
    當 使用者將權限類別 "CLASS1" 的說明改為 "一般使用者"
    那麼 操作應成功
    而且 權限類別 "CLASS1" 的說明應為 "一般使用者"
    而且 權限類別 "CLASS1" 的最後修改者應為 "tiptop"
    而且 權限類別 "CLASS1" 的最後修改日期應為 "今天"

  # p_zw.4gl:511-535 仍有使用者屬於此類別時不可改為無效（azz-217）
  場景: 有使用者的權限類別不可停用
    假設 已有使用者 "u001"
    當 使用者停用權限類別 "CLASS1"
    那麼 應出現錯誤 "PERMISSION_CLASS_IN_USE"
    而且 權限類別 "CLASS1" 的資料有效應為 "是"

  場景: 停用與啟用權限類別
    假設 權限類別 "TEMP" 已存在
    當 使用者停用權限類別 "TEMP"
    那麼 操作應成功
    而且 權限類別 "TEMP" 的資料有效應為 "否"
    當 使用者啟用權限類別 "TEMP"
    那麼 權限類別 "TEMP" 的資料有效應為 "是"

  # p_zw.4gl:537-562 有使用者時不可刪除（azz-217）
  場景: 有使用者的權限類別不可刪除
    假設 已有使用者 "u001"
    當 使用者刪除權限類別 "CLASS1"
    那麼 應出現錯誤 "PERMISSION_CLASS_IN_USE"

  # p_zw.4gl:563-568 仍有程式權限（zy_file）時不可刪除（azz-005）
  場景: 有程式權限的權限類別不可刪除
    假設 權限類別 "TEMP" 已存在
    而且 模組 "AIM" 已存在
    而且 已有程式 "item-maintenance" 模組 "AIM"
    而且 權限類別 "TEMP" 可執行程式 "item-maintenance"
    當 使用者刪除權限類別 "TEMP"
    那麼 應出現錯誤 "PERMISSION_CLASS_HAS_PERMISSIONS"

  # p_zw.4gl:573-581 無使用者、無程式權限時直接刪除
  場景: 刪除未使用的權限類別
    假設 權限類別 "TEMP" 已存在
    當 使用者刪除權限類別 "TEMP"
    那麼 操作應成功
    而且 權限類別 "TEMP" 應不存在

  # p_zw.4gl:761-776 更新所有使用者目錄編號：有預設選單的類別，其使用者選單改為預設選單；W9 只動不同者並記錄修改者
  場景: 更新所有使用者的選單為權限類別的預設選單
    假設 權限類別 "SALES" 已存在，預設選單 "menu-sales"
    而且 權限類別 "TEMP" 已存在
    而且 已有使用者 "u001"，權限類別 "SALES"，選單 "menu-main"
    而且 已有使用者 "u002"，權限類別 "SALES"，選單 "menu-sales"
    而且 已有使用者 "u003"，權限類別 "TEMP"，選單 "menu-main"
    當 使用者執行更新所有使用者目錄編號
    那麼 更新的使用者數應為 1
    而且 使用者 "u001" 的選單應為 "menu-sales"
    而且 使用者 "u001" 的最後修改者應為 "tiptop"
    而且 使用者 "u003" 的選單應為 "menu-main"

  # ----- 與使用者維護（p_zx）的連動：p_zx U4 待 p_zw 補上的規則 -----

  # p_zx.4gl:589-602 指定的權限類別必須有效（azz-218）
  場景: 無效的權限類別不可指定給使用者
    假設 權限類別 "TEMP" 已存在
    而且 使用者停用權限類別 "TEMP"
    當 使用者新增使用者 "u009"，名稱 "測試"，部門 "SAL"，權限類別 "TEMP"，選單 "menu-main"，預設營運中心 "TW01"
    那麼 應出現錯誤 "PERMISSION_CLASS_INACTIVE"

  # p_zx.4gl:584-588 使用者選單空白時帶入權限類別的預設選單
  場景: 使用者選單空白時帶入權限類別的預設選單
    假設 權限類別 "SALES" 已存在，預設選單 "menu-sales"
    當 使用者新增使用者 "u009"，名稱 "測試"，部門 "SAL"，權限類別 "SALES"，選單 ""，預設營運中心 "TW01"
    那麼 操作應成功
    而且 使用者 "u009" 的選單應為 "menu-sales"
