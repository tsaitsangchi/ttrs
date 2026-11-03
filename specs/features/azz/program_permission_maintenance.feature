# language: zh-TW
# TipTop program: p_zy 執行權限設定作業 (run permission setup) / zy_file (permissions per class and program)
# New system name: Program Permission Maintenance 程式執行權限維護
# Rule source: tiptop/azz/4gl/p_zy.4gl, tiptop/azz/4fd/p_zy.4fd (p_zy.per), tiptop/azz/sch/zy_file.sch,
#           action editor tiptop/sub/4gl/s_act_define.4gl, permission check tiptop/lib/4gl/cl_chk_act_auth.4gl,
#           auto-grant from p_zz tiptop/azz/4gl/p_zz.4gl:1843-1891, action labels tiptop/config/4ad/0/azz/p_zy.4ad;
#           no topcust override. Line numbers are noted on each scenario.
# Login and permission framework: docs/adr/0006-authentication-authorization.md §4
# Status: Y1–Y12 confirmed (Key User, 2026-11-02; all as proposed); entering Red
#
# What a permission row means (zy_file, one row per permission class + program):
#   - Having the row lets users of the class run the program (cl_setup → cl_set_priv: no row = lib-214 "no permission").
#   - zy03 allowed actions: comma-separated TipTop action ids (insert, query, modify, delete, reproduce, detail, output, …),
#     chosen only from the program's default actions zz04 (s_act_define.4gl:98-165). The .sch text "AURQCOB"
#     describes the old character-mode letters; the Genero version stores action ids.
#   - zy04 other users' data / zy05 other departments' data (query, modify, delete):
#       0 all three · 1 query + modify · 2 query + delete · 3 query only · 4 none
#       zy05 also 5–8 "related departments" (department groups zyw_file): 5 query + modify · 6 query + delete ·
#       7 query only · 8 all three (p_zy.per ITEMS; cl_chk_act_auth.4gl:97-122)
#   - zy07 detail rows: 0 add + delete · 1 add only · 2 delete only · 3 neither (p_zy.per ITEMS)
#   - Enforcing these values (Authorizer, data-permission checks, query filters, merging with user permissions zxw)
#     is ADR 0006 §4 and is specced with the Authorizer; this program only maintains them.
#
# Scope:
#   - Header: permission class (must exist and be active), its description and active code (p_zy.per group01)
#   - Rows: program, program name, allowed actions, other users' data, other departments' data, detail rows
#   - Actions: add / modify / remove a program row, delete all rows of a class, copy a class's rows to another class,
#     generate rows by program (p_zy_g) or by menu (p_zy_k), delete rows by menu (p_zy_d)
#   - Not ported: Express report permission (p_biy), "權限加嚴控管設定" (p_auth_as / gbl_file, comes with the Authorizer),
#     reports p_zy_r01 / p_zy_r02 and Excel export (shared features later), print method zy06 (removed by TipTop FUN-5C0057)
#
# Decisions (all ✅ confirmed, 2026-11-02):
#   Y1. Allowed actions are stored as a list of action ids taken from the program's default actions
#       (TTRS program field "可執行功能", TipTop zz04), which is therefore read as a comma-separated list,
#       trimmed and lower-cased (cl_set_priv DOWNSHIFT). Order follows the program's list.
#       An action the program doesn't offer is PERMISSION_ACTION_NOT_ALLOWED. An empty list is allowed
#       (the program can be opened but no controlled action runs).
#   Y2. Action names: TipTop shows names from gbd_file (per program, else 'standard'). TTRS translates the common
#       TipTop action ids itself (insert 新增, query 查詢, modify 修改, delete 刪除, …) and shows any other id as is.
#       gap_file / gbd_file are not ported now; gap06 "always allowed" flags are not ported — every listed action
#       can be selected (navigation actions such as exit, first, next are always allowed by the Authorizer).
#   Y3. A new row starts with all of the program's default actions and 0 / 0 / 0 (p_zy.4gl:700-717, 916-933;
#       p_zz.4gl:1852-1856). TipTop's batch generation uses zz18 / zz19 / zz32 as defaults, but those program fields
#       were not ported (legacy-map azz: disabled in the Genero version), so batch generation also uses 0 / 0 / 0.
#   Y4. Values are kept as TipTop codes ('0'–'4', '0'–'8', '0'–'3'); anything else is PERMISSION_LEVEL_INVALID.
#       zy05 5–8 (related departments) only take effect once department groups (p_zyw) are ported.
#   Y5. The class must exist and be active to add, modify, copy into or generate (azz-218, p_zy.4gl:389-401, 638-643);
#       PERMISSION_CLASS_NOT_FOUND / PERMISSION_CLASS_INACTIVE. The program must exist (PROGRAM_NOT_FOUND).
#   Y6. Menu programs get no permission row (MENU_HAS_NO_PERMISSION, as p_zz already does). TipTop's
#       generate-by-menu also inserts the menu itself with empty values (p_zy.4gl:1684-1692); in TTRS a menu is shown
#       when it contains a program the user may run (p_zm, later).
#   Y7. Removing one program row has no extra check (p_zy.4gl:743-763). Deleting all rows of a class is refused
#       while users have that class (azz-217, p_zy.4gl:561-593) → PERMISSION_CLASS_IN_USE.
#   Y8. Copy (p_zy.4gl:1164-1255): the target class must exist, be active and have no rows yet
#       (PERMISSION_CLASS_HAS_PERMISSIONS); rows are copied with their actions and levels; owner = current user.
#   Y9. Generate by program (p_zy_g): the user filters programs (code, module, type) and ticks the ones to add;
#       programs the class already has are skipped (p_zy.4gl:1397-1401). The result reports how many were added.
#  Y10. Generate by menu (p_zy_k): every program reachable from the menu, through sub-menus up to 20 levels
#       (mfg2733, p_zy.4gl:1590-1694), gets a row; the menu must be a menu program (MENU_PROGRAM_INVALID).
#       TipTop then lists the programs the class already has, all ticked, and resets the ticked ones to the defaults
#       (p_zy_k2, :2070-2210). I recommend the opposite default: existing rows stay as they are unless the user
#       ticks "reset existing rows to defaults", so tuned permissions are not lost by accident.
#  Y11. Delete by menu (p_zy_d): removes the rows of every program reachable from the menu, after confirmation.
#  Y12. Screen: as the 4fd, a header (class, description, active) with class navigation and a row table
#       (program, name, allowed actions, other users' data, other departments' data, detail rows); actions are edited
#       in a dialog with check boxes and select all / none (s_act_define). Toolbar: 新增 / 刪除 / 刪除全部 / 複製 /
#       依程式整批產生 / 依選單整批產生 / 依選單整批刪除. The p_zw button "維護群組執行權限" opens this screen on its class.
#       Created / modified by and on are recorded per row (zyuser, zygrup, zymodu, zydate).
#
# Also found while reading p_zz_gen (p_zz.4gl:1858-1869): when p_zz grants a program the class already has, TipTop
# resets zy03 to zz04. TTRS's program maintenance currently leaves an existing row unchanged; I recommend keeping that.

功能: 程式執行權限維護（Program Permission Maintenance）
  身為 系統管理員
  我想要 設定每個權限類別可執行的程式、動作與資料權限
  以便 伺服器依 TipTop 相同的權限資料檢查每個操作

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 權限類別 "CLASS1" 已存在
    而且 權限類別 "SALES" 已存在
    而且 模組 "AIM" 已存在
    而且 已有程式 "item-maintenance" 模組 "AIM"，可執行功能 "insert, query, modify, delete, reproduce"
    而且 已有程式 "item-master" 模組 "AIM"，可執行功能 "query, output"

  # p_zy.4gl:700-741、916-933 新增：允許執行功能帶入程式的基本執行功能，資料權限預設 0（Y3）
  場景: 新增程式權限帶入預設值
    當 使用者為權限類別 "SALES" 新增程式 "item-maintenance" 的權限
    那麼 操作應成功
    而且 權限類別 "SALES" 的程式 "item-maintenance" 權限應為:
      | 欄位           | 值                                       |
      | 允許執行功能   | insert, query, modify, delete, reproduce |
      | 他人資料權限   | 0                                        |
      | 他部門資料權限 | 0                                        |
      | 單身權限       | 0                                        |
      | 資料所有者     | tiptop                                   |

  # Y1 動作代號去空白、轉小寫，依程式的功能順序存放
  場景: 允許執行功能依程式的功能順序存放
    當 使用者為權限類別 "SALES" 新增程式 "item-maintenance" 的權限，允許執行功能 " QUERY , insert "
    那麼 操作應成功
    而且 權限類別 "SALES" 的程式 "item-maintenance" 允許執行功能應為 "insert, query"

  # s_act_define.4gl:98-165 只能勾選程式提供的功能（Y1）
  場景: 不可設定程式沒有提供的功能
    當 使用者為權限類別 "SALES" 新增程式 "item-master" 的權限，允許執行功能 "query, delete"
    那麼 應出現錯誤 "PERMISSION_ACTION_NOT_ALLOWED"

  場景: 允許執行功能可以全部不勾
    當 使用者為權限類別 "SALES" 新增程式 "item-master" 的權限，允許執行功能 ""
    那麼 操作應成功
    而且 權限類別 "SALES" 的程式 "item-master" 允許執行功能應為 ""

  # p_zy.4gl:765-783 同一權限類別的程式不可重複（-239）
  場景: 同一權限類別不可重複設定同一程式
    假設 權限類別 "SALES" 可執行程式 "item-maintenance"
    當 使用者為權限類別 "SALES" 新增程式 "item-maintenance" 的權限
    那麼 應出現錯誤 "PERMISSION_ALREADY_EXISTS"

  # p_zy.4gl:916-924 程式必須存在；Y6 選單程式不設定權限
  場景大綱: 程式必須存在且不是選單
    假設 選單程式 "menu-inv" 已存在
    當 使用者為權限類別 "SALES" 新增程式 "<程式>" 的權限
    那麼 應出現錯誤 "<錯誤>"

    例子:
      | 程式     | 錯誤                   |
      | item-unknown  | PROGRAM_NOT_FOUND      |
      | menu-inv | MENU_HAS_NO_PERMISSION |

  # p_zy.4gl:389-401、638-643 權限類別必須存在且有效（azz-218，Y5）
  場景大綱: 權限類別必須存在且有效
    假設 權限類別 "TEMP" 已存在
    而且 使用者停用權限類別 "TEMP"
    當 使用者為權限類別 "<權限類別>" 新增程式 "item-maintenance" 的權限
    那麼 應出現錯誤 "<錯誤>"

    例子:
      | 權限類別 | 錯誤                       |
      | NOCLASS  | PERMISSION_CLASS_NOT_FOUND |
      | TEMP     | PERMISSION_CLASS_INACTIVE  |

  # p_zy.per zy04 0–4、zy05 0–8、zy07 0–3（Y4）
  場景大綱: 資料權限代碼必須在範圍內
    假設 權限類別 "SALES" 可執行程式 "item-maintenance"
    當 使用者將權限類別 "SALES" 的程式 "item-maintenance" 的<欄位>設為 "<代碼>"
    那麼 應出現錯誤 "PERMISSION_LEVEL_INVALID"

    例子:
      | 欄位           | 代碼 |
      | 他人資料權限   | 5    |
      | 他部門資料權限 | 9    |
      | 單身權限       | 4    |

  # p_zy.4gl:785-817 修改；Y12 記錄修改者
  場景: 修改程式權限記錄修改者
    假設 權限類別 "SALES" 可執行程式 "item-maintenance"
    當 使用者將權限類別 "SALES" 的程式 "item-maintenance" 的他部門資料權限設為 "7"
    那麼 操作應成功
    而且 權限類別 "SALES" 的程式 "item-maintenance" 權限應為:
      | 欄位           | 值     |
      | 他部門資料權限 | 7      |
      | 最後修改者     | tiptop |

  # p_zy.4gl:743-763 移除單一程式權限
  場景: 移除單一程式權限
    假設 權限類別 "SALES" 可執行程式 "item-maintenance"
    當 使用者移除權限類別 "SALES" 的程式 "item-maintenance" 的權限
    那麼 操作應成功
    而且 權限類別 "SALES" 應不擁有程式 "item-maintenance" 的權限

  # p_zy.4gl:561-593 有使用者屬於此權限類別時不可刪除全部權限（azz-217，Y7）
  場景: 有使用者的權限類別不可刪除全部權限
    假設 營運中心 "TW01" 已啟用
    而且 選單程式 "menu-main" 已存在
    而且 權限類別 "SALES" 可執行程式 "item-maintenance"
    而且 已有使用者 "u001"，權限類別 "SALES"，選單 "menu-main"
    當 使用者刪除權限類別 "SALES" 的全部權限
    那麼 應出現錯誤 "PERMISSION_CLASS_IN_USE"

  場景: 刪除權限類別的全部權限
    假設 權限類別 "SALES" 可執行程式 "item-maintenance"
    而且 權限類別 "SALES" 可執行程式 "item-master"
    當 使用者刪除權限類別 "SALES" 的全部權限
    那麼 操作應成功
    而且 權限類別 "SALES" 應沒有程式權限

  # p_zy.4gl:1164-1255 複製到另一個權限類別：目標需存在、有效且尚無權限（Y8）
  場景: 複製權限到另一個權限類別
    假設 權限類別 "MFG" 已存在
    而且 權限類別 "SALES" 可執行程式 "item-maintenance"
    而且 使用者將權限類別 "SALES" 的程式 "item-maintenance" 的他人資料權限設為 "3"
    當 使用者將權限類別 "SALES" 的權限複製到 "MFG"
    那麼 操作應成功
    而且 權限類別 "MFG" 的程式 "item-maintenance" 權限應為:
      | 欄位         | 值     |
      | 他人資料權限 | 3      |
      | 資料所有者   | tiptop |

  場景: 複製的目標權限類別已有權限時不可複製
    假設 權限類別 "MFG" 已存在
    而且 權限類別 "SALES" 可執行程式 "item-maintenance"
    而且 權限類別 "MFG" 可執行程式 "item-master"
    當 使用者將權限類別 "SALES" 的權限複製到 "MFG"
    那麼 應出現錯誤 "PERMISSION_CLASS_HAS_PERMISSIONS"

  # p_zy.4gl:1257-1488 依程式整批產生：已有的程式略過（Y9）
  場景: 依程式整批產生權限
    假設 權限類別 "SALES" 可執行程式 "item-maintenance"
    而且 使用者將權限類別 "SALES" 的程式 "item-maintenance" 的他人資料權限設為 "3"
    當 使用者為權限類別 "SALES" 依程式整批產生 "item-maintenance、item-master" 的權限
    那麼 新增的權限筆數應為 1
    而且 權限類別 "SALES" 的程式 "item-master" 允許執行功能應為 "query, output"
    而且 權限類別 "SALES" 的程式 "item-maintenance" 權限應為:
      | 欄位         | 值 |
      | 他人資料權限 | 3  |

  # p_zy.4gl:1490-1694 依選單整批產生：含子選單內的程式，選單本身不設定（Y6、Y10）
  場景: 依選單整批產生權限
    假設 選單程式 "menu-inv" 已存在
    而且 選單程式 "menu-sub" 已存在
    而且 選單 "menu-inv" 包含程式 "item-maintenance"
    而且 選單 "menu-inv" 包含程式 "menu-sub"
    而且 選單 "menu-sub" 包含程式 "item-master"
    當 使用者為權限類別 "SALES" 依選單 "menu-inv" 整批產生權限
    那麼 新增的權限筆數應為 2
    而且 權限類別 "SALES" 應擁有程式 "item-maintenance" 的權限
    而且 權限類別 "SALES" 應擁有程式 "item-master" 的權限
    而且 權限類別 "SALES" 應不擁有程式 "menu-sub" 的權限

  # Y10 已有的程式權限預設不變；勾選「重設為預設值」時才重設（TipTop p_zy_k2 預設全部重設）
  場景大綱: 依選單整批產生時已有的權限
    假設 選單程式 "menu-inv" 已存在
    而且 選單 "menu-inv" 包含程式 "item-maintenance"
    而且 權限類別 "SALES" 可執行程式 "item-maintenance"
    而且 使用者將權限類別 "SALES" 的程式 "item-maintenance" 的他人資料權限設為 "3"
    當 使用者為權限類別 "SALES" 依選單 "menu-inv" 整批產生權限，重設已有權限 "<重設>"
    那麼 權限類別 "SALES" 的程式 "item-maintenance" 權限應為:
      | 欄位         | 值       |
      | 他人資料權限 | <結果>   |

    例子:
      | 重設 | 結果 |
      | 否   | 3    |
      | 是   | 0    |

  場景: 整批產生的選單必須是選單程式
    當 使用者為權限類別 "SALES" 依選單 "item-maintenance" 整批產生權限
    那麼 應出現錯誤 "MENU_PROGRAM_INVALID"

  # p_zy.4gl:1696-1857 依選單整批刪除（含子選單內的程式，Y11）
  場景: 依選單整批刪除權限
    假設 選單程式 "menu-inv" 已存在
    而且 選單程式 "menu-sub" 已存在
    而且 選單 "menu-inv" 包含程式 "menu-sub"
    而且 選單 "menu-sub" 包含程式 "item-master"
    而且 權限類別 "SALES" 可執行程式 "item-maintenance"
    而且 權限類別 "SALES" 可執行程式 "item-master"
    當 使用者為權限類別 "SALES" 依選單 "menu-inv" 整批刪除權限
    那麼 操作應成功
    而且 權限類別 "SALES" 應不擁有程式 "item-master" 的權限
    而且 權限類別 "SALES" 應擁有程式 "item-maintenance" 的權限
