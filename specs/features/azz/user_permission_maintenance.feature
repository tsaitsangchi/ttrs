# language: zh-TW
# TipTop program: p_zxw 使用者權限類別設定作業 (user special permissions) / zxw_file (special permissions per user)
# New system name: User Special Permission Maintenance 使用者特殊權限維護 (p_zx action "維護使用者特殊權限")
# Rule source: tiptop/azz/4gl/p_zxw.4gl, tiptop/azz/4fd/p_zxw.4fd (p_zxw.per), tiptop/azz/sch/zxw_file.sch,
#           runtime merge tiptop/lib/4gl/cl_setup.4gl:524-855 (cl_set_priv), linkage tiptop/azz/4gl/p_zx.4gl:1003-1018,
#           p_zw.4gl:464-560, p_zy.4gl:561-593; action labels tiptop/config/4ad/0/azz/p_zxw.4ad; no topcust override.
# Login and permission framework: docs/adr/0006-authentication-authorization.md §4
# Status: X1–X11 confirmed (Key User, 2026-11-02; all as proposed; X9 one set for all plants until the DBA finds otherwise); entering Red
#
# What special permissions mean (cl_set_priv, cl_setup.4gl:551-598):
#   - A user without special permissions uses the permissions (zy_file) of the class set in User Maintenance (zx04).
#   - A user WITH special permissions uses only them; the class from User Maintenance is not used unless it is listed.
#     Each row is either
#       1 a permission class — the user gets that class's program permissions (zy_file), or
#       2 a program — the row itself holds allowed actions and data permissions for that program (same meaning as p_zy).
#   - When several rows give the same program, they are merged: actions are united and each data permission takes the
#     wider value (cl_setup.4gl:605-852). The merge belongs to the Authorizer and is specced with it.
#
# Scope:
#   - Header: user (must exist) and user name; rows: line no., kind, class or program, name, allowed actions,
#     other users' data, other departments' data, detail rows (p_zxw.per)
#   - Actions: add / modify / remove a row, delete all rows of a user, copy a user's rows to another user (FUN-7C0041)
#   - Linkage: User Maintenance class change moves the class row; user delete removes the rows;
#     a class used by special permissions counts as "in use" for p_zw and p_zy
#   - Not ported: report output via p_query and Excel export (shared features later); the unused header-change routine
#     p_zxw_u (no menu action reaches it)
#
# Decisions (all ✅ confirmed, 2026-11-02):
#   X1. Keep TipTop's meaning: special permissions replace the user's class (above). When a user gets the first row,
#       TipTop asks whether to add the user's own class first (azz-006, p_zxw.4gl:925-984). The new screen asks the same
#       with "加入原權限類別" ticked by default, so a user is not cut off from the class permissions by accident.
#   X2. A class row must name an existing, active class (azz-218, p_zxw.4gl:723-744) and holds no actions or data
#       permissions (TQC-5A0094). A program row must name an existing program that is not a menu (as p_zy Y6); its
#       actions follow the p_zy rules (only the program's own actions, Y1) and start with all of them; data permissions
#       start with 0 / 0 / 0 (FUN-870036, p_zxw.4gl:769-786).
#   X3. Line numbers are given automatically (max + 1, p_zxw.4gl:660-667) and only order the rows. TipTop only checks
#       that line numbers are unique; TTRS also refuses the same class or program twice for one user
#       (USER_PERMISSION_DUPLICATE), because a second row adds nothing.
#   X4. Removing one row and deleting all rows of a user need no other check (p_zxw.4gl:494-530, 797-818).
#       With no rows left, the user is back on the class from User Maintenance.
#   X5. Copy to another user (p_zxw.4gl:1167-1237): the target user must exist and have no special permissions yet
#       (-239) → USER_HAS_SPECIAL_PERMISSIONS.
#   X6. User Maintenance: changing a user's class moves that user's row of the old class to the new class
#       (p_zx.4gl:1003-1018; TipTop's UPDATE would also hit a program row whose code equals the class code — TTRS moves
#       class rows only). Deleting a user removes the rows (p_zx.4gl:1246-1249, as U9).
#   X7. A class named in any special-permission row is "in use": p_zw refuses to deactivate or delete it
#       (p_zw.4gl:474-486, 524-534, 552-560) and p_zy refuses to delete all its permissions (p_zy.4gl:583-591)
#       → PERMISSION_CLASS_IN_USE. These were left for this program in the p_zw and p_zy specs.
#   X8. The runtime merge (actions united; other users' data and detail rows united; other departments' data united
#       with TipTop's table for related departments, MOD-7B0137, rounding to the wider code when no exact code exists)
#       is specced and implemented with the Authorizer, not here.
#   X9. One set for all plants? The p_zxw header shows the current plant (g_dbs, p_zxw.4gl:478-492) and cl_set_priv reads
#       zxw_file without a schema prefix, so in TipTop each plant schema might hold its own zxw_file. Please ask the DBA
#       whether ZXW_FILE exists in every plant schema of TOPPROD or is a synonym of ds. I recommend one set for all
#       plants (no plant_id) unless the DBA finds different data per plant; plant_id can be added later by a migration.
#  X10. ta_zxw01 "確認否" is a customer-added column that no program reads or writes → not ported. Please confirm.
#  X11. Screen: as the 4fd, a header (user, user name) with user navigation and a row table (line, kind, class or
#       program, name, allowed actions, other users' data, other departments' data, detail rows); add and modify in a
#       dialog (actions as check boxes, like p_zy). Toolbar: 新增 / 修改 / 移除 / 刪除全部 / 複製. User Maintenance gets
#       the button "維護使用者特殊權限" that opens this screen on its user (p_zx action maintain_zxw).

功能: 使用者特殊權限維護（User Special Permission Maintenance）
  身為 系統管理員
  我想要 為個別使用者指定權限類別或單支程式的權限
  以便 少數使用者可以有別於其權限類別的執行權限

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 權限類別 "CLASS1" 已存在
    而且 權限類別 "SALES" 已存在
    而且 營運中心 "TW01" 已啟用
    而且 選單程式 "menu_main" 已存在
    而且 模組 "AIM" 已存在
    而且 已有程式 "aimi100" 模組 "AIM"，可執行功能 "insert, query, modify, delete"
    而且 已有程式 "aimi101" 模組 "AIM"，可執行功能 "query, output"
    而且 已有使用者 "u001"
    而且 已有使用者 "u002"

  # p_zxw.4gl:925-984 第一次設定時詢問是否加入使用者原本的權限類別（azz-006，X1）
  場景大綱: 第一次設定特殊權限時可加入原權限類別
    當 使用者為使用者 "u001" 新增特殊權限類別 "SALES"，加入原權限類別 "<加入>"
    那麼 操作應成功
    而且 使用者 "u001" 的特殊權限應為 "<結果>"

    例子:
      | 加入 | 結果                       |
      | 是   | 1 類別 CLASS1、2 類別 SALES |
      | 否   | 1 類別 SALES               |

  # p_zxw.4gl:769-786 程式列帶入程式的可執行功能，資料權限預設 0（X2）
  場景: 新增程式特殊權限帶入預設值
    當 使用者為使用者 "u001" 新增特殊權限程式 "aimi101"
    那麼 操作應成功
    而且 使用者 "u001" 的特殊權限程式 "aimi101" 應為:
      | 欄位           | 值            |
      | 允許執行功能   | query, output |
      | 他人資料權限   | 0             |
      | 他部門資料權限 | 0             |
      | 單身權限       | 0             |

  # X2 程式列的功能只能選程式提供的（同 p_zy Y1）
  場景: 程式特殊權限只能設定程式提供的功能
    當 使用者為使用者 "u001" 新增特殊權限程式 "aimi101"，允許執行功能 "query, delete"
    那麼 應出現錯誤 "PERMISSION_ACTION_NOT_ALLOWED"

  # p_zxw.4gl:723-744 權限類別必須存在且有效（azz-218，X2）
  場景大綱: 特殊權限的權限類別必須存在且有效
    假設 權限類別 "TEMP" 已存在
    而且 使用者停用權限類別 "TEMP"
    當 使用者為使用者 "u001" 新增特殊權限類別 "<權限類別>"，加入原權限類別 "否"
    那麼 應出現錯誤 "<錯誤>"

    例子:
      | 權限類別 | 錯誤                       |
      | NOCLASS  | PERMISSION_CLASS_NOT_FOUND |
      | TEMP     | PERMISSION_CLASS_INACTIVE  |

  # X2 程式必須存在且不是選單
  場景大綱: 特殊權限的程式必須存在且不是選單
    當 使用者為使用者 "u001" 新增特殊權限程式 "<程式>"
    那麼 應出現錯誤 "<錯誤>"

    例子:
      | 程式      | 錯誤                   |
      | aimi999   | PROGRAM_NOT_FOUND      |
      | menu_main | MENU_HAS_NO_PERMISSION |

  # X3 同一使用者不可重複設定同一權限類別或程式
  場景: 同一使用者不可重複設定同一程式
    假設 使用者 "u001" 有特殊權限程式 "aimi101"
    當 使用者為使用者 "u001" 新增特殊權限程式 "aimi101"
    那麼 應出現錯誤 "USER_PERMISSION_DUPLICATE"

  # p_zxw.4gl:345-362 使用者必須存在
  場景: 使用者必須存在
    當 使用者為使用者 "u999" 新增特殊權限程式 "aimi101"
    那麼 應出現錯誤 "USER_NOT_FOUND"

  # p_zxw.4gl:820-850 修改程式列
  場景: 修改程式特殊權限記錄修改者
    假設 使用者 "u001" 有特殊權限程式 "aimi100"
    當 使用者將使用者 "u001" 的特殊權限程式 "aimi100" 的他人資料權限設為 "3"
    那麼 操作應成功
    而且 使用者 "u001" 的特殊權限程式 "aimi100" 應為:
      | 欄位         | 值     |
      | 他人資料權限 | 3      |
      | 最後修改者   | tiptop |

  # p_zxw.4gl:797-818 移除單一列；:494-530 刪除全部（X4）
  場景: 移除單一特殊權限
    假設 使用者 "u001" 有特殊權限類別 "SALES"
    而且 使用者 "u001" 有特殊權限程式 "aimi101"
    當 使用者移除使用者 "u001" 的第 1 項特殊權限
    那麼 操作應成功
    而且 使用者 "u001" 的特殊權限應為 "2 程式 aimi101"

  場景: 刪除使用者的全部特殊權限
    假設 使用者 "u001" 有特殊權限類別 "SALES"
    而且 使用者 "u001" 有特殊權限程式 "aimi101"
    當 使用者刪除使用者 "u001" 的全部特殊權限
    那麼 操作應成功
    而且 使用者 "u001" 應沒有特殊權限

  # p_zxw.4gl:1167-1237 複製到另一個使用者：目標須存在且尚無特殊權限（X5）
  場景: 複製特殊權限到另一個使用者
    假設 使用者 "u001" 有特殊權限類別 "SALES"
    而且 使用者 "u001" 有特殊權限程式 "aimi101"
    當 使用者將使用者 "u001" 的特殊權限複製到 "u002"
    那麼 操作應成功
    而且 使用者 "u002" 的特殊權限應為 "1 類別 SALES、2 程式 aimi101"

  場景: 複製的目標使用者已有特殊權限時不可複製
    假設 使用者 "u001" 有特殊權限類別 "SALES"
    而且 使用者 "u002" 有特殊權限程式 "aimi101"
    當 使用者將使用者 "u001" 的特殊權限複製到 "u002"
    那麼 應出現錯誤 "USER_HAS_SPECIAL_PERMISSIONS"

  # ----- 與使用者維護、權限類別維護、程式執行權限維護的連動 -----

  # p_zx.4gl:1003-1018 使用者改權限類別時，特殊權限中的原權限類別跟著改（X6）
  場景: 使用者改權限類別時特殊權限中的原權限類別跟著改
    當 使用者為使用者 "u001" 新增特殊權限程式 "aimi101"
    而且 使用者為使用者 "u001" 新增特殊權限類別 "SALES"，加入原權限類別 "否"
    而且 使用者將使用者 "u001" 的權限類別改為 "SALES"
    那麼 使用者 "u001" 的特殊權限應為 "1 程式 aimi101、2 類別 SALES"

  場景: 使用者原權限類別的特殊權限列跟著改
    假設 使用者 "u001" 有特殊權限類別 "CLASS1"
    當 使用者將使用者 "u001" 的權限類別改為 "SALES"
    那麼 使用者 "u001" 的特殊權限應為 "1 類別 SALES"

  # p_zx.4gl:1246-1249 刪除使用者時連同特殊權限（X6）
  場景: 刪除使用者時連同特殊權限
    假設 使用者 "u001" 有特殊權限程式 "aimi101"
    當 使用者刪除使用者 "u001"
    那麼 使用者 "u001" 應沒有特殊權限

  # p_zw.4gl:524-534、p_zy.4gl:583-591 特殊權限使用中的權限類別不可停用、不可刪除全部權限（X7）
  場景: 特殊權限使用中的權限類別不可停用
    假設 權限類別 "TEMP" 已存在
    而且 使用者 "u001" 有特殊權限類別 "TEMP"
    當 使用者停用權限類別 "TEMP"
    那麼 應出現錯誤 "PERMISSION_CLASS_IN_USE"

  場景: 特殊權限使用中的權限類別不可刪除全部權限
    假設 權限類別 "TEMP" 已存在
    而且 權限類別 "TEMP" 可執行程式 "aimi100"
    而且 使用者 "u001" 有特殊權限類別 "TEMP"
    當 使用者刪除權限類別 "TEMP" 的全部權限
    那麼 應出現錯誤 "PERMISSION_CLASS_IN_USE"
