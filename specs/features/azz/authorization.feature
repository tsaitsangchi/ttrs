# language: zh-TW
# TipTop 對應：權限檢查共用函式（無單一作業）
#   - cl_set_priv 取得有效權限：tiptop/lib/4gl/cl_setup.4gl:524-855
#   - cl_chk_act_auth 功能與資料權限檢查：tiptop/lib/4gl/cl_chk_act_auth.4gl:38-237
#   - cl_detail_input_auth 單身新增／刪除列：tiptop/lib/4gl/cl_detail_input_auth.4gl:45-72
#   - cl_chk_tgrup_list 相關部門清單：tiptop/lib/4gl/cl_chk_act_auth.4gl:315-393
#   - 查詢的資料權限條件：tiptop/azz/4gl/p_zx.4gl:194-204（aimi100.4gl:703-714 同樣寫法）
#   資料表：zy_file（程式執行權限）、zxw_file（使用者特殊權限）、gbl_file（自訂動作對應，本次不做）、
#           zyw_file（部門權限群組，p_zyw 移植後才有）
# 新系統名稱：Authorizer 權限檢查（app 層；ADR 0006 §4、§6）
# 狀態：AQ1–AQ13 依草稿確認（Key User，2026-11-03），進入 Red
#
# TipTop 怎麼做：
#   1. 開作業時 cl_set_priv 算出有效權限：允許執行功能（g_priv1）、他人資料（g_priv2）、
#      他部門資料（g_priv3）、單身（g_priv5）。沒有權限列就不能開（lib-214）。
#      - 使用者沒有特殊權限：用自己權限類別在 zy_file 的那一列（cl_setup.4gl:551-567）。
#      - 使用者有特殊權限：只用特殊權限，不再看自己的權限類別（:569-589）。
#        類別列（zxw03=1）取該類別在 zy_file 的程式列，程式列（zxw03=2）直接用 zxw05–zxw08（:591-602），
#        多列合併（:605-853）。
#   2. 每個按鈕先 cl_chk_act_auth：功能代號要在允許清單裡（lib-219）；
#      修改（modify、detail）、刪除（delete）時，若資料所有者不是自己就看他人資料權限，
#      資料所有部門不是自己的部門就看他部門資料權限（:97-122；拒絕訊息 9007）。
#   3. 單身新增列、刪除列另看單身權限（cl_detail_input_auth）。
#   4. 查詢時：他人資料 = 4 只查自己的資料；他部門資料 = 4 只查自己部門的資料；5–8 只查相關部門（p_zx.4gl:194-204）。
#
# 權限等級（p_zy 已確認的代碼，Y4）：
#   他人資料 zy04／他部門資料 zy05（查詢、修改、刪除）：0 全部 · 1 查詢＋修改 · 2 查詢＋刪除 · 3 只能查詢 · 4 都不行
#   zy05 另有相關部門 5 查詢＋修改 · 6 查詢＋刪除 · 7 只能查詢 · 8 全部（只限相關部門；其他部門都不行）
#   單身 zy07：0 新增＋刪除 · 1 只能新增 · 2 只能刪除 · 3 都不行
#
# 各操作需要的功能（AQ3；「資料」欄：O = 檢查資料所有者與所有部門，F = 查詢依資料權限過濾，
#   R+ / R- = 單身新增列／刪除列另看單身權限）：
#   作業     操作                                       功能                         資料    TipTop 出處
#   p_zz     查詢、查看程式（含名稱、異動紀錄、所屬選單）   query                               p_zz.4gl:170-655
#   p_zz     新增程式                                   insert
#   p_zz     修改程式、設定程式名稱                       modify                       O       p_zz.4gl:1262-1263
#   p_zz     刪除程式                                   delete                       O
#   p_zz     複製程式                                   reproduce
#   p_zz     授權程式給權限類別                           authorization                        p_zz.4gl:604-610
#   p_zta    查詢、查看資料字典（含欄位、索引、限制、修整記錄、
#            實體資料表清單、TipTop 對照）               query                               p_zta.4gl:727-962
#   p_zta    新增／修改／刪除資料字典                     insert / modify / delete
#   p_zta    設定資料表說明／欄位說明                     maintain_table_description / maintain_field_description
#   p_zta    匯出資料字典                               exporttoexcel
#   p_zta    查詢 TipTop 對照目錄                        query                               （TTRS 新增）
#   p_zta    匯入 TipTop 對照目錄                        zta_import                          （TTRS 新增）
#   p_zx     查詢、查看使用者                            query                        F O     p_zx.4gl:194-204, 232-319
#   p_zx     新增使用者                                 insert
#   p_zx     修改使用者                                 modify                       O       p_zx.4gl:867-868
#   p_zx     刪除使用者                                 delete                       O
#   p_zx     複製使用者                                 reproduce
#   p_zx     停用／啟用使用者                            invalid
#   p_zw     查詢、查看權限類別                          query                               p_zw.4gl:126-180
#   p_zw     新增權限類別                               detail                       R+      p_zw.4gl:270-271
#   p_zw     修改、停用、啟用權限類別                     detail
#   p_zw     刪除權限類別                               detail                       R-
#   p_zw     更新所有使用者選單                          detail                              p_zw.4gl:682-683（單身內的動作）
#   aooi901  查詢、查看營運中心                          query                               aooi901.4gl:373-655
#   aooi901  新增／修改／刪除／複製營運中心               insert / modify / delete / reproduce
#   aooi901  停用／啟用營運中心                          modify                              （azp_file 無有效碼，P9）
#   p_zy     查詢、查看程式執行權限                       query                               p_zy.4gl:213-299
#   p_zy     新增程式列                                 detail                       R+      p_zy.4gl:653-654
#   p_zy     修改程式列                                 detail
#   p_zy     移除程式列                                 detail                       R-
#   p_zy     刪除權限類別的全部權限                       delete
#   p_zy     複製到另一權限類別                          reproduce
#   p_zy     依程式產生／依選單產生／依選單刪除            batch_gen_by_prog / batch_gen_by_menu / batch_del_by_menu
#   p_zxw    查看使用者特殊權限                          query                               p_zxw.4gl:214-254
#   p_zxw    新增特殊權限列                              detail                       R+      p_zxw.4gl:562-563
#   p_zxw    修改特殊權限程式列                          detail
#   p_zxw    移除特殊權限列                              detail                       R-
#   p_zxw    刪除使用者的全部特殊權限                     delete
#   p_zxw    複製到另一使用者                            reproduce
#   aimi100  查詢、查看料件                              query                               aimi100.4gl:791-1203
#   aimi100  新增／刪除料件                              insert / delete
#   aimi100  確認／取消確認料件                          confirm / notconfirm                aimi100.4gl:1178, 1203
#   aimi100  修改、作廢、複製、訂單無效、多語言品名、異動記錄  modify / invalid / reproduce / order_invalid / update_item / history
#                                                       （item_maintenance.feature；menu :7725-7958）
#   aooi030  查詢部門                                   query                               aooi030.4gl:163-166
#   aooi030  新增／修改／停用／刪除部門（基本欄位）       detail                       R+ R-   aooi030.4gl:231-232
#   aooi030  會計資料維護                               account                             aooi030.4gl:167-170
#   aooi040  查詢員工                                   query
#   aooi040  新增／修改／停用／刪除員工                   detail                       R+ R-   aooi040.4gl
#   p_zm     查詢／單身維護／複製／刪除選單目錄             query／detail／reproduce／delete    p_zm.4gl
#            （側欄樹狀選單只要求登入，依權限隱藏節點，menu_maintenance.feature MN10）
#   參考清單（下拉選單、開窗、欄位檢查）：程式、權限類別、使用者、營運中心的全部清單，只要求登入（AQ4）
#
# 確認事項（皆採草稿）：
#   AQ1. 有效權限照 cl_set_priv：
#        - 沒有特殊權限用權限類別；有特殊權限只用特殊權限（同 p_zxw 已確認的規則）。
#        - 多列合併（依特殊權限項次順序）：功能取聯集（依出現順序、轉小寫）；他人資料、單身取聯集；他部門資料取聯集，
#          5–8 照 TipTop 對照表（MOD-7B0137），聯集沒有剛好的代碼時取較寬的代碼（例如 1 與 6 → 0）。
#        - 印表方式 zy06 不移植（TipTop FUN-5C0057 已不用）。
#        - 目前使用者的部門、權限類別在登入時決定（OIDC 之前由 dev 模式的環境變數提供），
#          Authorizer 直接使用，不在每次檢查時重查使用者資料。
#   AQ2. 檢查順序：先看有沒有本作業權限 → 功能 → 讀到資料後看資料權限 → 單身權限。
#        權限檢查在輸入檢查之前，沒權限時一律回 PERMISSION_DENIED（HTTP 403），附作業、功能與原因
#        （沒有本作業權限／沒有此功能／他人資料／他部門資料／單身），
#        畫面依原因顯示 TipTop 的 lib-214、lib-219、9007 訊息。
#   AQ3. 上面「各操作需要的功能」對照表。TTRS 與 TipTop 畫面操作方式不同的地方：
#        - p_zw、p_zy、p_zxw 在 TipTop 是在單身裡新增、修改、刪除列，TTRS 的逐筆操作都要 detail；
#          TipTop 的 insert（輸入單頭後直接進單身）在 TTRS 沒有對應操作。
#        - aooi901 的停用／啟用是 TTRS 新增的（P9），用 modify。
#        - TipTop 對照目錄是 TTRS 新增的，歸在 p_zta：查詢用 query，匯入用 zta_import（TipTop「匯入」）。
#        - 資料字典匯出（CSV）用 exporttoexcel。
#   AQ4. 參考清單（下拉選單、開窗、欄位檢查用的全部清單：GET /api/programs、/api/permission-classes、
#        /api/users、/api/plants）只要求登入，不檢查程式權限，同 TipTop 開窗 q_* 與欄位檢查直接讀表。
#        各作業自己的查詢（POST …/query、以代號查看單筆）才檢查該作業的 query。
#   AQ5. 資料權限只套用在 TipTop 有套用的作業：
#        - p_zx：查詢依資料權限過濾，修改、刪除檢查資料所有者與所有部門。
#        - p_zz：修改、刪除檢查，查詢不過濾（p_zz.4gl 沒有查詢條件）。
#        - p_zw、aooi901、p_zy、p_zxw、p_zta：TipTop 沒有設定資料所有者，不檢查。
#        - aimi100：這次只檢查功能；料件的資料所有者、所有部門（imauser、imagrup）要新增欄位，
#          等 aimi100 正式移植時再做。
#   AQ6. TTRS 補強（TipTop 靠「查不到就點不到」，API 可以直接帶代號）：
#        - 被資料權限過濾掉的資料，用代號查看、修改、刪除也拒絕。
#        - 他部門資料 5–8 修改、刪除時也要求資料所有部門在相關部門內（TipTop 只在查詢時限制）。
#        - p_zyw 移植前，相關部門只有自己的部門（同 TipTop 部門不在任何群組時 cl_chk_tgrup_list 的結果）。
#        - 部門比對用完全相同（p_zx 的寫法）；aimi100 用前綴比對（MATCHES 'SAL*'），等 aimi100 移植時再討論。
#   AQ7. 自訂動作對應（gbl_file，維護作業 p_auth_as）這次不做。沒有對應時 TipTop 也不對自訂動作檢查資料權限，
#        所以 p_zx 停用／啟用（invalid）、p_zz 授權（authorization）不檢查資料所有者。
#        請 DBA 協助確認 TOPPROD 的 gbl_file 是否有 p_zx、p_zz 的資料；有的話再提前做。
#   AQ8. 單身權限只套用在 p_zw、p_zy、p_zxw 的新增列、移除列。p_zz、p_zta 在 TipTop 也有單身，
#        但 TTRS 的程式名稱、資料表與欄位說明是逐筆設定，沒有新增列／刪除列的操作，不套用。
#   AQ9. AI 代理（ADR 0006 §6）：目前使用者加上身分種類（人員／AI／服務帳號）。
#        AI 只能執行 query、output、insert、modify、detail、reproduce，其餘功能
#        （delete、invalid、confirm、notconfirm、batch_*、authorization 等）一律拒絕，即使代表的使用者有權限。
#  AQ10. 拒絕紀錄：ADR 0006 §7 要寫入稽核，但稽核事件表要等稽核 ADR；現有 audit_log 是 p_zz 的異動紀錄，
#        畫面會顯示。這次先寫應用程式日誌，稽核 ADR 定案後改寫稽核事件。
#  AQ11. 起始資料：新增 migration 登錄已移植的 8 支作業與其功能清單（上表各作業的功能），
#        權限類別 ADMIN 擁有全部功能、資料權限 0/0/0、單身 0；記憶體 seeded() 同步。
#        日後匯入 TipTop 的 zz_file、zy_file 時以匯入資料為準。
#  AQ12. 既有規格不改：BDD 預設不檢查權限；本規格的「目前使用者執行 …」步驟才啟用真正的 Authorizer
#        （記憶體與 PostgreSQL 同一套）。各操作的功能宣告由下面的對照表場景逐一驗證；
#        之後新增 use case 時，要同時加進本規格的對照表。
#  AQ13. 畫面：新增 GET /api/me/permissions/{作業} 回傳有效權限。各作業畫面隱藏沒有權限的按鈕；
#        沒有本作業權限時顯示訊息、不載入資料；資料權限（他人、他部門）照 TipTop 按下後才提示。
#        左側選單依權限過濾等 p_zm 選單一起做。
#
# 不在本次範圍：OIDC 登入（ADR 0006 第 4 步）、營運中心切換與 RLS（第 5 步）、p_zm 選單、
#   部門權限群組 p_zyw、自訂動作對應 p_auth_as（gbl_file）、料件資料權限。

功能: 權限檢查（Authorization）
  身為 系統管理員
  我想要 每個操作都依 TipTop 的權限設定檢查
  以便 使用者只能執行被授權的作業、功能與資料

  背景:
    假設 目前使用者為 "u001"，所屬部門 "SAL"，權限類別 "SALES"
    而且 權限類別 "SALES" 已存在
    而且 權限類別 "MGR" 已存在
    而且 權限類別 "AUDIT" 已存在
    而且 模組 "AZZ" 已存在
    而且 模組 "AOO" 已存在
    而且 模組 "AIM" 已存在
    而且 已登錄 TTRS 已移植的作業
    而且 已有程式 "item-master" 模組 "AIM"，可執行功能 "query, modify"
    而且 營運中心 "TW01" 已啟用
    而且 單位 "PCS" 已存在
    而且 已有使用者 "u001"，資料所有者 "u001"，所有部門 "SAL"

  # ---------- 有效權限（cl_set_priv）----------

  # cl_setup.4gl:551-567 沒有特殊權限時用權限類別的程式列（AQ1）
  場景: 沒有特殊權限時用權限類別的權限
    假設 權限類別 "SALES" 對作業 "user-maintenance" 的權限為 "query, modify"，他人資料 "3"，他部門資料 "4"，單身 "0"
    當 查看目前使用者在作業 "user-maintenance" 的有效權限
    那麼 有效權限應為:
      | 欄位         | 值            |
      | 允許執行功能 | query, modify |
      | 他人資料     | 3             |
      | 他部門資料   | 4             |
      | 單身         | 0             |

  # cl_setup.4gl:560-563 權限類別沒有這支作業（lib-214）
  場景: 權限類別沒有本作業的權限
    當 目前使用者執行 "查詢使用者"
    那麼 應被拒絕，作業 "user-maintenance"、功能 "query"、原因 "沒有本作業權限"

  # item_auto_number.feature AN6：取料號屬於料件維護的新增（insert）或複製（reproduce），有其中一個就能取號
  場景: 取料號需要料件維護的新增或複製功能
    假設 權限類別 "SALES" 對作業 "item-maintenance" 的權限為 "query"，他人資料 "0"，他部門資料 "0"，單身 "0"
    當 目前使用者執行 "取料號 FG"
    那麼 應被拒絕，作業 "item-maintenance"、功能 "reproduce"、原因 "沒有此功能"

  場景: 只有料件複製功能也能取料號
    假設 權限類別 "SALES" 對作業 "item-maintenance" 的權限為 "query, reproduce"，他人資料 "0"，他部門資料 "0"，單身 "0"
    當 目前使用者執行 "取料號 FG"
    那麼 不應被權限拒絕

  # cl_setup.4gl:569-589 有特殊權限時只用特殊權限，不看自己的權限類別（AQ1）
  場景: 有特殊權限的使用者只用特殊權限
    假設 權限類別 "SALES" 對作業 "user-maintenance" 的權限為 "query"，他人資料 "0"，他部門資料 "0"，單身 "0"
    而且 權限類別 "MGR" 對作業 "program-maintenance" 的權限為 "query"，他人資料 "0"，他部門資料 "0"，單身 "0"
    而且 使用者 "u001" 有特殊權限類別 "MGR"
    當 目前使用者執行 "查詢使用者"
    那麼 應被拒絕，作業 "user-maintenance"、功能 "query"、原因 "沒有本作業權限"

  # cl_setup.4gl:591-602, 605-655 類別列與程式列一起合併；功能取聯集，依出現順序（AQ1）
  場景: 特殊權限的類別列與程式列合併功能
    假設 權限類別 "MGR" 對作業 "user-maintenance" 的權限為 "query, modify"，他人資料 "3"，他部門資料 "3"，單身 "3"
    而且 使用者 "u001" 有特殊權限類別 "MGR"
    而且 使用者 "u001" 有特殊權限程式 "user-maintenance"，允許執行功能 "query, delete"，他人資料 "2"，他部門資料 "4"，單身 "1"
    當 查看目前使用者在作業 "user-maintenance" 的有效權限
    那麼 有效權限應為:
      | 欄位         | 值                    |
      | 允許執行功能 | query, modify, delete |
      | 他人資料     | 2                     |
      | 他部門資料   | 3                     |
      | 單身         | 1                     |

  # cl_setup.4gl:656-696 他人資料取聯集（查詢、修改、刪除各自有一列允許就允許）
  場景大綱: 他人資料權限的合併
    假設 權限類別 "MGR" 對作業 "user-maintenance" 的權限為 "query"，他人資料 "<甲>"，他部門資料 "0"，單身 "0"
    而且 權限類別 "AUDIT" 對作業 "user-maintenance" 的權限為 "query"，他人資料 "<乙>"，他部門資料 "0"，單身 "0"
    而且 使用者 "u001" 有特殊權限類別 "MGR"
    而且 使用者 "u001" 有特殊權限類別 "AUDIT"
    當 查看目前使用者在作業 "user-maintenance" 的有效權限
    那麼 有效權限的他人資料應為 "<結果>"

    例子:
      | 甲 | 乙 | 結果 |
      | 1  | 2  | 0    |
      | 1  | 3  | 1    |
      | 2  | 4  | 2    |
      | 3  | 4  | 3    |
      | 4  | 4  | 4    |
      | 0  | 4  | 0    |

  # cl_setup.4gl:698-790 他部門資料取聯集；5–8 照 MOD-7B0137 對照表，沒有剛好的代碼時取較寬的（AQ1）
  場景大綱: 他部門資料權限的合併
    假設 權限類別 "MGR" 對作業 "user-maintenance" 的權限為 "query"，他人資料 "0"，他部門資料 "<甲>"，單身 "0"
    而且 權限類別 "AUDIT" 對作業 "user-maintenance" 的權限為 "query"，他人資料 "0"，他部門資料 "<乙>"，單身 "0"
    而且 使用者 "u001" 有特殊權限類別 "MGR"
    而且 使用者 "u001" 有特殊權限類別 "AUDIT"
    當 查看目前使用者在作業 "user-maintenance" 的有效權限
    那麼 有效權限的他部門資料應為 "<結果>"

    例子:
      | 甲 | 乙 | 結果 |
      | 1  | 2  | 0    |
      | 1  | 3  | 1    |
      | 2  | 3  | 2    |
      | 3  | 4  | 3    |
      | 1  | 5  | 1    |
      | 1  | 6  | 0    |
      | 3  | 5  | 1    |
      | 3  | 6  | 2    |
      | 3  | 8  | 0    |
      | 4  | 7  | 7    |
      | 5  | 6  | 8    |
      | 5  | 7  | 5    |
      | 7  | 8  | 8    |

  # cl_setup.4gl:828-852 單身權限取聯集
  場景大綱: 單身權限的合併
    假設 權限類別 "MGR" 對作業 "program-permission-maintenance" 的權限為 "query, detail"，他人資料 "0"，他部門資料 "0"，單身 "<甲>"
    而且 權限類別 "AUDIT" 對作業 "program-permission-maintenance" 的權限為 "query, detail"，他人資料 "0"，他部門資料 "0"，單身 "<乙>"
    而且 使用者 "u001" 有特殊權限類別 "MGR"
    而且 使用者 "u001" 有特殊權限類別 "AUDIT"
    當 查看目前使用者在作業 "program-permission-maintenance" 的有效權限
    那麼 有效權限的單身應為 "<結果>"

    例子:
      | 甲 | 乙 | 結果 |
      | 1  | 2  | 0    |
      | 1  | 3  | 1    |
      | 2  | 3  | 2    |
      | 3  | 3  | 3    |

  # ---------- 功能（cl_chk_act_auth.4gl:77-93）----------

  # 各操作需要的功能（AQ3）：缺了該功能就拒絕；有全部功能時不會因權限被拒絕（可能因資料不存在等其他原因失敗）
  場景大綱: 每個操作檢查所屬作業的功能
    當 權限類別 "SALES" 在作業 "<作業>" 沒有功能 "<功能>" 時，目前使用者執行 "<操作>"
    那麼 應被拒絕，作業 "<作業>"、功能 "<功能>"、原因 "沒有此功能"
    當 權限類別 "SALES" 在作業 "<作業>" 擁有全部功能時，目前使用者執行 "<操作>"
    那麼 不應被權限拒絕

    例子:
      | 作業    | 功能                       | 操作                       |
      | program-maintenance    | query                      | 查詢程式                   |
      | program-maintenance    | query                      | 查看程式 item-master           |
      | program-maintenance    | insert                     | 新增程式 item-batch           |
      | program-maintenance    | modify                     | 修改程式 item-master           |
      | program-maintenance    | modify                     | 設定程式 item-master 的名稱    |
      | program-maintenance    | delete                     | 刪除程式 item-master           |
      | program-maintenance    | reproduce                  | 複製程式 item-master           |
      | program-maintenance    | authorization              | 授權程式 item-master 給 MGR    |
      | program-maintenance    | output                     | 列印程式                   |
      | program-maintenance    | exporttoexcel              | 匯出程式為 Excel           |
      | program-maintenance | history | 查詢作業 program-maintenance u001 的異動記錄 |
      | data-dictionary   | query                      | 查詢資料字典               |
      | data-dictionary   | query                      | 查看資料表 item 的欄位     |
      | data-dictionary   | insert                     | 新增資料字典 item          |
      | data-dictionary   | modify                     | 修改資料字典 item          |
      | data-dictionary   | delete                     | 刪除資料字典 item          |
      | data-dictionary   | maintain_table_description | 設定資料表 item 的說明     |
      | data-dictionary   | maintain_field_description | 設定欄位 item.name 的說明  |
      | data-dictionary   | exporttoexcel              | 匯出資料字典 item          |
      | data-dictionary   | exporttoexcel              | 匯出資料字典 item 為 Excel |
      | data-dictionary   | output                     | 列印資料字典 item          |
      | data-dictionary   | query                      | 查詢 TipTop 對照目錄       |
      | data-dictionary   | zta_import                 | 匯入 TipTop 對照目錄       |
      | data-dictionary | history | 查詢作業 data-dictionary item 的異動記錄 |
      | user-maintenance    | query                      | 查詢使用者                 |
      | user-maintenance    | query                      | 查看使用者 u001            |
      | user-maintenance    | insert                     | 新增使用者 u100            |
      | user-maintenance    | modify                     | 修改使用者 u001            |
      | user-maintenance    | delete                     | 刪除使用者 u001            |
      | user-maintenance    | reproduce                  | 複製使用者 u001            |
      | user-maintenance    | invalid                    | 停用使用者 u001            |
      | user-maintenance    | invalid                    | 啟用使用者 u001            |
      | user-maintenance    | user_unlock                | 解除使用者 u001 的鎖定     |
      | user-maintenance    | confirm                    | 確認使用者 u001            |
      | user-maintenance    | notconfirm                 | 取消確認使用者 u001        |
      | user-maintenance    | output                     | 列印使用者                 |
      | user-maintenance    | exporttoexcel              | 匯出使用者為 Excel         |
      | user-maintenance | history | 查詢作業 user-maintenance u001 的異動記錄 |
      | permission-class-maintenance    | query                      | 查詢權限類別               |
      | permission-class-maintenance    | query                      | 查看權限類別 MGR           |
      | permission-class-maintenance    | detail                     | 新增權限類別 NEW           |
      | permission-class-maintenance    | detail                     | 修改權限類別 MGR           |
      | permission-class-maintenance    | detail                     | 停用權限類別 MGR           |
      | permission-class-maintenance    | detail                     | 刪除權限類別 MGR           |
      | permission-class-maintenance    | detail                     | 更新所有使用者選單         |
      | permission-class-maintenance    | reproduce                  | 複製權限類別 MGR           |
      | permission-class-maintenance    | confirm                    | 確認權限類別 MGR           |
      | permission-class-maintenance    | notconfirm                 | 取消確認權限類別 MGR       |
      | permission-class-maintenance    | output                     | 列印權限類別               |
      | permission-class-maintenance    | exporttoexcel              | 匯出權限類別為 Excel       |
      | permission-class-maintenance | history | 查詢作業 permission-class-maintenance MGR 的異動記錄 |
      | plant-maintenance | query                      | 查詢營運中心               |
      | plant-maintenance | query                      | 查看營運中心 TW01          |
      | plant-maintenance | insert                     | 新增營運中心 TW09          |
      | plant-maintenance | modify                     | 修改營運中心 TW01          |
      | plant-maintenance | modify                     | 停用營運中心 TW01          |
      | plant-maintenance | delete                     | 刪除營運中心 TW01          |
      | plant-maintenance | reproduce                  | 複製營運中心 TW01          |
      | plant-maintenance | confirm                    | 確認營運中心 TW01          |
      | plant-maintenance | notconfirm                 | 取消確認營運中心 TW01      |
      | plant-maintenance | output                     | 列印營運中心               |
      | plant-maintenance | exporttoexcel              | 匯出營運中心為 Excel       |
      | plant-maintenance | history | 查詢作業 plant-maintenance TW01 的異動記錄 |
      | program-permission-maintenance    | query                      | 查詢程式執行權限           |
      | program-permission-maintenance    | query                      | 查看權限類別 MGR 的權限    |
      | program-permission-maintenance    | detail                     | 新增 MGR 的程式列 item-maintenance  |
      | program-permission-maintenance    | detail                     | 修改 MGR 的程式列 item-maintenance  |
      | program-permission-maintenance    | detail                     | 移除 MGR 的程式列 item-maintenance  |
      | program-permission-maintenance    | delete                     | 刪除 MGR 的全部權限        |
      | program-permission-maintenance    | reproduce                  | 複製 MGR 的權限到 AUDIT    |
      | program-permission-maintenance    | batch_gen_by_prog          | 依程式產生 MGR 的權限      |
      | program-permission-maintenance    | batch_gen_by_menu          | 依選單產生 MGR 的權限      |
      | program-permission-maintenance    | batch_del_by_menu          | 依選單刪除 MGR 的權限      |
      | program-permission-maintenance    | output                     | 列印程式執行權限           |
      | program-permission-maintenance    | exporttoexcel              | 匯出程式執行權限為 Excel   |
      | program-permission-maintenance | history | 查詢作業 program-permission-maintenance MGR 的異動記錄 |
      | user-permission-maintenance   | query                      | 查看使用者 u001 的特殊權限 |
      | user-permission-maintenance   | query                      | 查詢使用者特殊權限         |
      | user-permission-maintenance   | output                     | 列印使用者特殊權限         |
      | user-permission-maintenance   | exporttoexcel              | 匯出使用者特殊權限為 Excel |
      | user-permission-maintenance   | detail                     | 新增 u001 的特殊權限列     |
      | user-permission-maintenance   | detail                     | 修改 u001 的特殊權限程式列 |
      | user-permission-maintenance   | detail                     | 移除 u001 的特殊權限列     |
      | user-permission-maintenance   | delete                     | 刪除 u001 的全部特殊權限   |
      | user-permission-maintenance   | reproduce                  | 複製 u001 的特殊權限       |
      | user-permission-maintenance | history | 查詢作業 user-permission-maintenance u001 的異動記錄 |
      | item-maintenance | query                      | 查詢料件                   |
      | item-maintenance | insert                     | 新增料件 A001              |
      | item-maintenance | delete                     | 刪除料件 A001              |
      | item-maintenance | confirm                    | 確認料件 A001              |
      | item-maintenance | notconfirm                 | 取消確認料件 A001          |
      | item-maintenance | modify                     | 修改料件 A001              |
      | item-maintenance | invalid                    | 作廢料件 A001              |
      | item-maintenance | reproduce                  | 複製料件 A001 為 A002      |
      | item-maintenance | order_invalid              | 切換料件 A001 的訂單無效   |
      | item-maintenance | update_item                | 設定料件 A001 的英文品名   |
      | item-maintenance | history                    | 查詢料件 A001 的異動記錄   |
      | item-maintenance | output                     | 列印料件                   |
      | item-maintenance | exporttoexcel              | 匯出料件為 Excel           |
      | department-maintenance | query                      | 查詢部門                   |
      | department-maintenance | detail                     | 新增部門 D01               |
      | department-maintenance | detail                     | 修改部門 D01               |
      | department-maintenance | account                    | 修改部門 D01 的會計資料    |
      | department-maintenance | detail                     | 刪除部門 D01               |
      | department-maintenance | output                     | 列印部門                   |
      | department-maintenance | exporttoexcel              | 匯出部門為 Excel           |
      | department-maintenance | reproduce                  | 複製部門 D01               |
      | department-maintenance | confirm                    | 確認部門 D01               |
      | department-maintenance | notconfirm                 | 取消確認部門 D01           |
      | department-maintenance | history                    | 查詢部門 D01 的異動記錄    |
      | employee-maintenance | query                      | 查詢員工                   |
      | employee-maintenance | detail                     | 新增員工 E01               |
      | employee-maintenance | detail                     | 刪除員工 E01               |
      | employee-maintenance | reproduce                  | 複製員工 E01               |
      | employee-maintenance | confirm                    | 確認員工 E01               |
      | employee-maintenance | notconfirm                 | 取消確認員工 E01           |
      | employee-maintenance | output                     | 列印員工                   |
      | employee-maintenance | exporttoexcel              | 匯出員工為 Excel           |
      | employee-maintenance | history                    | 查詢員工 E01 的異動記錄    |
      | classification-code-maintenance | query           | 查詢分類碼 D               |
      | classification-code-maintenance | detail          | 新增分類碼 D X01           |
      | classification-code-maintenance | detail          | 修改分類碼 D X01           |
      | classification-code-maintenance | detail          | 停用分類碼 D X01           |
      | classification-code-maintenance | detail          | 刪除分類碼 D X01           |
      | classification-code-maintenance | reproduce       | 複製分類碼 D X01           |
      | classification-code-maintenance | confirm         | 確認分類碼 D X01           |
      | classification-code-maintenance | notconfirm      | 取消確認分類碼 D X01       |
      | classification-code-maintenance | output          | 列印分類碼 D               |
      | classification-code-maintenance | exporttoexcel   | 匯出分類碼 D 為 Excel      |
      | classification-code-maintenance | history         | 查詢分類碼 D X01 的異動記錄 |
      | menu-maintenance | query                      | 查詢選單目錄               |
      | menu-maintenance | detail                     | 在目錄 menu-a 加入程式 plant-maintenance |
      | menu-maintenance | reproduce                  | 複製目錄 menu-a 為 menu-b  |
      | menu-maintenance | delete                     | 刪除目錄 menu-a            |
      | menu-maintenance | history                    | 查詢作業 menu-maintenance menu-a 的異動記錄 |
      | unit-maintenance | query                      | 查詢單位                   |
      | unit-maintenance | detail                     | 新增單位 KG                |
      | unit-maintenance | detail                     | 修改單位 KG                |
      | unit-maintenance | detail                     | 停用單位 KG                |
      | unit-maintenance | detail                     | 刪除單位 KG                |
      | unit-maintenance | reproduce                  | 複製單位 KG                |
      | unit-maintenance | confirm                    | 確認單位 KG                |
      | unit-maintenance | notconfirm                 | 取消確認單位 KG            |
      | unit-maintenance | output                     | 列印單位                   |
      | unit-maintenance | exporttoexcel              | 匯出單位為 Excel           |
      | unit-maintenance | history                    | 查詢作業 unit-maintenance KG 的異動記錄 |
      | encoding-nature-maintenance | query         | 查詢編碼性質               |
      | encoding-nature-maintenance | detail        | 新增編碼性質 A1            |
      | encoding-nature-maintenance | detail        | 修改編碼性質 A1            |
      | encoding-nature-maintenance | detail        | 停用編碼性質 A1            |
      | encoding-nature-maintenance | detail        | 刪除編碼性質 A1            |
      | encoding-nature-maintenance | reproduce     | 複製編碼性質 A1            |
      | encoding-nature-maintenance | confirm       | 確認編碼性質 A1            |
      | encoding-nature-maintenance | notconfirm    | 取消確認編碼性質 A1        |
      | encoding-nature-maintenance | output        | 列印編碼性質               |
      | encoding-nature-maintenance | exporttoexcel | 匯出編碼性質為 Excel       |
      | encoding-nature-maintenance | history       | 查詢作業 encoding-nature-maintenance A1 的異動記錄 |
      | encoding-segment-maintenance | query        | 查詢編碼獨立段             |
      | encoding-segment-maintenance | insert       | 新增編碼獨立段 COLOR       |
      | encoding-segment-maintenance | modify       | 修改編碼獨立段 COLOR       |
      | encoding-segment-maintenance | invalid      | 失效編碼獨立段 COLOR       |
      | encoding-segment-maintenance | delete       | 刪除編碼獨立段 COLOR       |
      | encoding-segment-maintenance | reproduce    | 複製編碼獨立段 COLOR       |
      | encoding-segment-maintenance | detail       | 維護編碼獨立段 COLOR 的單身 |
      | encoding-segment-maintenance | confirm      | 確認編碼獨立段 COLOR       |
      | encoding-segment-maintenance | notconfirm   | 取消確認編碼獨立段 COLOR   |
      | encoding-segment-maintenance | output       | 列印編碼獨立段             |
      | encoding-segment-maintenance | exporttoexcel | 匯出編碼獨立段為 Excel    |
      | group-center-maintenance     | query        | 查詢集團中心               |
      | group-center-maintenance     | insert       | 新增集團中心 DC            |
      | group-center-maintenance     | modify       | 修改集團中心 DC            |
      | group-center-maintenance     | invalid      | 失效集團中心 DC            |
      | group-center-maintenance     | delete       | 刪除集團中心 DC            |
      | group-center-maintenance     | reproduce    | 複製集團中心 DC            |
      | group-center-maintenance     | data_type_setting | 設定集團中心 DC 的資料類別 |
      | group-center-maintenance     | confirm      | 確認集團中心 DC            |
      | group-center-maintenance     | notconfirm   | 取消確認集團中心 DC        |
      | group-center-maintenance     | output       | 列印集團中心               |
      | group-center-maintenance     | exporttoexcel | 匯出集團中心為 Excel      |
      | group-center-maintenance     | history      | 查詢作業 group-center-maintenance DC 的異動記錄 |
      | data-control-maintenance     | query        | 查詢資料控管原則           |
      | data-control-maintenance     | detail       | 新增資料類別 1 的受控營運中心 TW02 |
      | data-control-maintenance     | detail       | 刪除資料類別 1 的受控營運中心 TW02 |
      | data-control-maintenance     | condition_detail | 新增資料類別 1 營運中心 TW02 的欄位條件 |
      | data-control-maintenance     | condition_detail | 修改資料類別 1 營運中心 TW02 的欄位條件 |
      | data-control-maintenance     | condition_detail | 刪除資料類別 1 營運中心 TW02 的欄位條件 |
      | data-control-maintenance     | reproduce    | 複製資料類別 1 營運中心 TW02 的欄位條件 |
      | data-control-maintenance     | output       | 列印資料控管原則           |
      | data-control-maintenance     | exporttoexcel | 匯出資料控管原則為 Excel  |
      | data-control-maintenance     | history      | 查詢作業 data-control-maintenance 1 的異動記錄 |
      | carry-setting-maintenance    | query        | 查詢拋轉設定               |
      | carry-setting-maintenance    | insert       | 新增拋轉設定 DC 1          |
      | carry-setting-maintenance    | modify       | 修改拋轉設定 DC 1          |
      | carry-setting-maintenance    | delete       | 刪除拋轉設定 DC 1          |
      | carry-setting-maintenance    | reproduce    | 複製拋轉設定 DC 1          |
      | carry-setting-maintenance    | detail       | 新增拋轉設定 DC 1 的拋轉營運中心 TW02 |
      | carry-setting-maintenance    | detail       | 刪除拋轉設定 DC 1 的拋轉營運中心 TW02 |
      | carry-setting-maintenance    | detail       | 新增拋轉設定 DC 1 營運中心 TW02 的拋轉條件 |
      | carry-setting-maintenance    | set_default_value | 新增拋轉設定 DC 1 營運中心 TW02 的預設值 |
      | carry-setting-maintenance    | output       | 列印拋轉設定               |
      | carry-setting-maintenance    | exporttoexcel | 匯出拋轉設定為 Excel      |
      | carry-setting-maintenance    | history      | 查詢作業 carry-setting-maintenance DC 的異動記錄 |
      | basic-data-carry             | query        | 開啟基本資料拋轉           |
      | basic-data-carry             | carry        | 執行基本資料拋轉           |
      | carry-history-query          | query        | 查詢拋轉歷史               |
      | carry-history-query          | output       | 列印拋轉歷史               |
      | carry-history-query          | exporttoexcel | 匯出拋轉歷史為 Excel      |
      | encoding-segment-maintenance | history      | 查詢作業 encoding-segment-maintenance COLOR 的異動記錄 |
      | encoding-class-maintenance | query        | 查詢編碼類別               |
      | encoding-class-maintenance | query        | 試算編碼類別 FG            |
      | encoding-class-maintenance | insert       | 新增編碼類別 FG            |
      | encoding-class-maintenance | modify       | 修改編碼類別 FG            |
      | encoding-class-maintenance | invalid      | 失效編碼類別 FG            |
      | encoding-class-maintenance | delete       | 刪除編碼類別 FG            |
      | encoding-class-maintenance | reproduce    | 複製編碼類別 FG            |
      | encoding-class-maintenance | detail       | 維護編碼類別 FG 的各段     |
      | encoding-class-maintenance | confirm      | 確認編碼類別 FG            |
      | encoding-class-maintenance | notconfirm   | 取消確認編碼類別 FG        |
      | encoding-class-maintenance | output       | 列印編碼類別               |
      | encoding-class-maintenance | exporttoexcel | 匯出編碼類別為 Excel      |
      | encoding-class-maintenance | history      | 查詢作業 encoding-class-maintenance FG 的異動記錄 |
      | encoding-year-maintenance | query         | 查詢編碼年對照             |
      | encoding-year-maintenance | detail        | 新增編碼年對照 2026        |
      | encoding-year-maintenance | detail        | 修改編碼年對照 2026        |
      | encoding-year-maintenance | detail        | 停用編碼年對照 2026        |
      | encoding-year-maintenance | detail        | 刪除編碼年對照 2026        |
      | encoding-year-maintenance | reproduce     | 複製編碼年對照 2026        |
      | encoding-year-maintenance | confirm       | 確認編碼年對照 2026        |
      | encoding-year-maintenance | notconfirm    | 取消確認編碼年對照 2026    |
      | encoding-year-maintenance | output        | 列印編碼年對照             |
      | encoding-year-maintenance | exporttoexcel | 匯出編碼年對照為 Excel     |
      | encoding-year-maintenance | history       | 查詢作業 encoding-year-maintenance 2026 的異動記錄 |
      | project-maintenance | query | 查詢專案 |
      | project-maintenance | insert | 新增專案 P1 |
      | project-maintenance | modify | 修改專案 P1 |
      | project-maintenance | invalid | 停用專案 P1 |
      | project-maintenance | delete | 刪除專案 P1 |
      | project-maintenance | reproduce | 複製專案 P1 |
      | project-maintenance | confirm | 確認專案 P1 |
      | project-maintenance | notconfirm | 取消確認專案 P1 |
      | project-maintenance | output | 列印專案 |
      | project-maintenance | exporttoexcel | 匯出專案為 Excel |
      | project-maintenance | history | 查詢專案 P1 的異動記錄 |
      | tax-type-maintenance | query | 查詢稅別 |
      | tax-type-maintenance | detail | 新增稅別 T5 |
      | tax-type-maintenance | detail | 修改稅別 T5 |
      | tax-type-maintenance | detail | 停用稅別 T5 |
      | tax-type-maintenance | detail | 刪除稅別 T5 |
      | tax-type-maintenance | reproduce | 複製稅別 T5 |
      | tax-type-maintenance | confirm | 確認稅別 T5 |
      | tax-type-maintenance | notconfirm | 取消確認稅別 T5 |
      | tax-type-maintenance | output | 列印稅別 |
      | tax-type-maintenance | exporttoexcel | 匯出稅別為 Excel |
      | tax-type-maintenance | history | 查詢作業 tax-type-maintenance T5 的異動記錄 |
      | monthly-exchange-rate-maintenance | query | 查詢每月匯率 |
      | monthly-exchange-rate-maintenance | insert | 新增每月匯率 USD 202611 |
      | monthly-exchange-rate-maintenance | modify | 修改每月匯率 USD 202611 |
      | monthly-exchange-rate-maintenance | invalid | 停用每月匯率 USD 202611 |
      | monthly-exchange-rate-maintenance | delete | 刪除每月匯率 USD 202611 |
      | monthly-exchange-rate-maintenance | reproduce | 複製每月匯率 USD 202611 |
      | monthly-exchange-rate-maintenance | confirm | 確認每月匯率 USD 202611 |
      | monthly-exchange-rate-maintenance | notconfirm | 取消確認每月匯率 USD 202611 |
      | monthly-exchange-rate-maintenance | output | 列印每月匯率 |
      | monthly-exchange-rate-maintenance | exporttoexcel | 匯出每月匯率為 Excel |
      | monthly-exchange-rate-maintenance | history | 查詢作業 monthly-exchange-rate-maintenance USD 的異動記錄 |
      | monthly-exchange-rate-maintenance | re_rate | 執行月底重評價 2026-11-30 |
      | daily-exchange-rate-maintenance | query | 查詢每日匯率 |
      | daily-exchange-rate-maintenance | insert | 新增每日匯率 USD 2026-11-02 |
      | daily-exchange-rate-maintenance | modify | 修改每日匯率 USD 2026-11-02 |
      | daily-exchange-rate-maintenance | invalid | 停用每日匯率 USD 2026-11-02 |
      | daily-exchange-rate-maintenance | delete | 刪除每日匯率 USD 2026-11-02 |
      | daily-exchange-rate-maintenance | reproduce | 複製每日匯率 USD 2026-11-02 |
      | daily-exchange-rate-maintenance | confirm | 確認每日匯率 USD 2026-11-02 |
      | daily-exchange-rate-maintenance | notconfirm | 取消確認每日匯率 USD 2026-11-02 |
      | daily-exchange-rate-maintenance | output | 列印每日匯率 |
      | daily-exchange-rate-maintenance | exporttoexcel | 匯出每日匯率為 Excel |
      | daily-exchange-rate-maintenance | history | 查詢作業 daily-exchange-rate-maintenance USD 的異動記錄 |
      | daily-exchange-rate-maintenance | unify_rate | 執行統一匯率 2026-11-02 |
      | daily-exchange-rate-maintenance | auto_import | 匯入台銀匯率 |
      | daily-exchange-rate-maintenance | auto_import | 匯入海關匯率 |
      | daily-exchange-rate-copy | run | 執行每日匯率複製 2026-11-09 |
      | daily-exchange-rate-copy | output | 列印每日匯率複製結果 |
      | daily-exchange-rate-copy | exporttoexcel | 匯出每日匯率複製結果為 Excel |
      | monthly-exchange-rate-maintenance | query | 查看每月匯率設定 |
      | daily-exchange-rate-maintenance | query | 查看每日匯率設定 |
      | general-management-parameters | query | 查看整體管理系統參數 |
      | epub-to-text | run | 將 EPUB 轉成文字 |
      | epub-to-text | translate | 提交 EPUB 翻譯 |
      | github-epub-search | query | 在 GitHub 搜尋 EPUB |
      | github-epub-search | query | 列出 GitHub repo 的 EPUB |
      | github-epub-search | run | 轉換 GitHub 上的 EPUB |
      | github-epub-search | query | 多來源搜尋 EPUB |
      | github-epub-search | run | 下載 搜尋結果的 EPUB |
      | github-epub-search | query | 讀取 網址 的 EPUB |
      | github-epub-search | run | 下載 網址 的 EPUB |
      | github-epub-search | query | 列出 我的 GitHub 書庫 |
      | github-epub-search | insert | 加入 我的 GitHub 書庫 a/b |
      | github-epub-search | delete | 移除 我的 GitHub 書庫 a/b |
      | github-epub-search | query | 列出 共用 GitHub 書庫 |
      | github-epub-search | share | 加入 共用 GitHub 書庫 a/b |
      | github-epub-search | share | 移除 共用 GitHub 書庫 a/b |
      | unit-conversion-maintenance | query | 查詢單位換算 |
      | unit-conversion-maintenance | detail | 新增單位換算 KG G |
      | unit-conversion-maintenance | detail | 修改單位換算 KG G |
      | unit-conversion-maintenance | detail | 停用單位換算 KG G |
      | unit-conversion-maintenance | detail | 刪除單位換算 KG G |
      | unit-conversion-maintenance | reproduce | 複製單位換算 KG G |
      | unit-conversion-maintenance | confirm | 確認單位換算 KG G |
      | unit-conversion-maintenance | notconfirm | 取消確認單位換算 KG G |
      | unit-conversion-maintenance | output | 列印單位換算 |
      | unit-conversion-maintenance | exporttoexcel | 匯出單位換算為 Excel |
      | unit-conversion-maintenance | history | 查詢作業 unit-conversion-maintenance KG 的異動記錄 |
      | item-unit-conversion-maintenance | query | 查詢料件 A001 的單位換算 |
      | item-unit-conversion-maintenance | query | 取用料件 A001 的換算率 |
      | item-unit-conversion-maintenance | detail | 新增料件 A001 的單位換算 |
      | item-unit-conversion-maintenance | detail | 刪除料件 A001 的單位換算 |
      | item-unit-conversion-maintenance | output | 列印料件單位換算 |
      | item-unit-conversion-maintenance | exporttoexcel | 匯出料件單位換算為 Excel |
      | item-unit-conversion-maintenance | history | 查詢料件 A001 單位換算的異動記錄 |
      | accounting-period-maintenance | query | 查詢會計年度 |
      | accounting-period-maintenance | query | 查詢日期 2026-03-15 的會計期間 |
      | accounting-period-maintenance | insert | 新增會計年度 2026 |
      | accounting-period-maintenance | modify | 修改會計年度 2026 |
      | accounting-period-maintenance | delete | 刪除會計年度 2026 |
      | accounting-period-maintenance | reproduce | 複製會計年度 2026 |
      | accounting-period-maintenance | confirm | 確認會計年度 2026 |
      | accounting-period-maintenance | notconfirm | 取消確認會計年度 2026 |
      | accounting-period-maintenance | output | 列印會計期間 |
      | accounting-period-maintenance | exporttoexcel | 匯出會計期間為 Excel |
      | accounting-period-maintenance | history | 查詢作業 accounting-period-maintenance 2026 的異動記錄 |
      | general-management-parameters | modify | 修改整體管理系統參數 |
      | general-management-parameters | output | 列印整體管理系統參數 |
      | general-management-parameters | exporttoexcel | 匯出整體管理系統參數為 Excel |
      | general-management-parameters | history | 查詢作業 general-management-parameters 0 的異動記錄 |
      | currency-maintenance | query | 查詢幣別 |
      | currency-maintenance | detail | 新增幣別 USD |
      | currency-maintenance | detail | 修改幣別 USD |
      | currency-maintenance | detail | 停用幣別 USD |
      | currency-maintenance | detail | 刪除幣別 USD |
      | currency-maintenance | reproduce | 複製幣別 USD |
      | currency-maintenance | confirm | 確認幣別 USD |
      | currency-maintenance | notconfirm | 取消確認幣別 USD |
      | currency-maintenance | output | 列印幣別 |
      | currency-maintenance | exporttoexcel | 匯出幣別為 Excel |
      | currency-maintenance | history | 查詢作業 currency-maintenance USD 的異動記錄 |
      | region-maintenance | query | 查詢區域 |
      | region-maintenance | detail | 新增區域 AS |
      | region-maintenance | detail | 修改區域 AS |
      | region-maintenance | detail | 停用區域 AS |
      | region-maintenance | detail | 刪除區域 AS |
      | region-maintenance | reproduce | 複製區域 AS |
      | region-maintenance | confirm | 確認區域 AS |
      | region-maintenance | notconfirm | 取消確認區域 AS |
      | region-maintenance | output | 列印區域 |
      | region-maintenance | exporttoexcel | 匯出區域為 Excel |
      | region-maintenance | history | 查詢作業 region-maintenance AS 的異動記錄 |
      | country-maintenance | query | 查詢國別 |
      | country-maintenance | detail | 新增國別 TW |
      | country-maintenance | detail | 修改國別 TW |
      | country-maintenance | detail | 停用國別 TW |
      | country-maintenance | detail | 刪除國別 TW |
      | country-maintenance | reproduce | 複製國別 TW |
      | country-maintenance | confirm | 確認國別 TW |
      | country-maintenance | notconfirm | 取消確認國別 TW |
      | country-maintenance | output | 列印國別 |
      | country-maintenance | exporttoexcel | 匯出國別為 Excel |
      | country-maintenance | history | 查詢作業 country-maintenance TW 的異動記錄 |
      | area-maintenance | query | 查詢地區 |
      | area-maintenance | detail | 新增地區 TPE |
      | area-maintenance | detail | 修改地區 TPE |
      | area-maintenance | detail | 停用地區 TPE |
      | area-maintenance | detail | 刪除地區 TPE |
      | area-maintenance | reproduce | 複製地區 TPE |
      | area-maintenance | confirm | 確認地區 TPE |
      | area-maintenance | notconfirm | 取消確認地區 TPE |
      | area-maintenance | output | 列印地區 |
      | area-maintenance | exporttoexcel | 匯出地區為 Excel |
      | area-maintenance | history | 查詢作業 area-maintenance TPE 的異動記錄 |
      | shipping-method-maintenance | query | 查詢交運方式 |
      | shipping-method-maintenance | detail | 新增交運方式 AIR |
      | shipping-method-maintenance | detail | 修改交運方式 AIR |
      | shipping-method-maintenance | detail | 停用交運方式 AIR |
      | shipping-method-maintenance | detail | 刪除交運方式 AIR |
      | shipping-method-maintenance | reproduce | 複製交運方式 AIR |
      | shipping-method-maintenance | confirm | 確認交運方式 AIR |
      | shipping-method-maintenance | notconfirm | 取消確認交運方式 AIR |
      | shipping-method-maintenance | output | 列印交運方式 |
      | shipping-method-maintenance | exporttoexcel | 匯出交運方式為 Excel |
      | shipping-method-maintenance | history | 查詢作業 shipping-method-maintenance AIR 的異動記錄 |
      | report-title-maintenance | query | 查詢報表抬頭 |
      | report-title-maintenance | detail | 新增報表抬頭 BS |
      | report-title-maintenance | detail | 修改報表抬頭 BS |
      | report-title-maintenance | detail | 停用報表抬頭 BS |
      | report-title-maintenance | detail | 刪除報表抬頭 BS |
      | report-title-maintenance | reproduce | 複製報表抬頭 BS |
      | report-title-maintenance | confirm | 確認報表抬頭 BS |
      | report-title-maintenance | notconfirm | 取消確認報表抬頭 BS |
      | report-title-maintenance | output | 列印報表抬頭 |
      | report-title-maintenance | exporttoexcel | 匯出報表抬頭為 Excel |
      | report-title-maintenance | history | 查詢作業 report-title-maintenance BS 的異動記錄 |
      | product-classification-maintenance | query    | 查詢產品分類               |
      | product-classification-maintenance | detail   | 新增產品分類 P01           |
      | product-classification-maintenance | detail   | 修改產品分類 P01           |
      | product-classification-maintenance | detail   | 停用產品分類 P01           |
      | product-classification-maintenance | detail   | 刪除產品分類 P01           |
      | product-classification-maintenance | reproduce | 複製產品分類 P01          |
      | product-classification-maintenance | confirm  | 確認產品分類 P01           |
      | product-classification-maintenance | notconfirm | 取消確認產品分類 P01     |
      | product-classification-maintenance | output   | 列印產品分類               |
      | product-classification-maintenance | exporttoexcel | 匯出產品分類為 Excel  |
      | product-classification-maintenance | history  | 查詢產品分類 P01 的異動記錄 |
      | item-group-maintenance | query                | 查詢料件分群               |
      | item-group-maintenance | insert               | 新增料件分群 G01           |
      | item-group-maintenance | modify               | 修改料件分群 G01           |
      | item-group-maintenance | invalid              | 停用料件分群 G01           |
      | item-group-maintenance | reproduce            | 複製料件分群 G01 為 G02    |
      | item-group-maintenance | delete               | 刪除料件分群 G01           |
      | item-group-maintenance | confirm              | 確認料件分群 G01           |
      | item-group-maintenance | notconfirm           | 取消確認料件分群 G01       |
      | item-group-maintenance | output               | 列印料件分群               |
      | item-group-maintenance | exporttoexcel        | 匯出料件分群為 Excel       |
      | item-group-maintenance | history              | 查詢料件分群 G01 的異動記錄 |

  # AQ2 權限檢查在輸入檢查之前
  場景: 沒有權限時先回權限錯誤
    假設 權限類別 "SALES" 對作業 "user-maintenance" 的權限為 "query"，他人資料 "0"，他部門資料 "0"，單身 "0"
    當 目前使用者執行 "新增使用者，代號空白"
    那麼 應被拒絕，作業 "user-maintenance"、功能 "insert"、原因 "沒有此功能"

  # cl_set_priv DOWNSHIFT、cl_chk_act_auth equalsIgnoreCase：功能代號不分大小寫
  場景: 功能代號不分大小寫
    假設 權限類別 "SALES" 對作業 "user-maintenance" 的權限為 "QUERY"，他人資料 "0"，他部門資料 "0"，單身 "0"
    當 目前使用者執行 "查詢使用者"
    那麼 不應被權限拒絕

  # ---------- 資料權限：修改、刪除（cl_chk_act_auth.4gl:97-122）----------

  # u002 他人建立、同部門；u003 他人建立、他部門；u004 自己建立、同部門；
  # u005 自己建立、他部門：所有者相同也還要看他部門資料權限（cl_chk_act_auth.4gl:99-122 所有者、部門各自檢查）
  場景大綱: 修改、刪除他人或他部門的使用者
    假設 已有使用者 "u002"，資料所有者 "u009"，所有部門 "SAL"
    而且 已有使用者 "u003"，資料所有者 "u009"，所有部門 "MFG"
    而且 已有使用者 "u004"，資料所有者 "u001"，所有部門 "SAL"
    而且 已有使用者 "u005"，資料所有者 "u001"，所有部門 "MFG"
    而且 權限類別 "SALES" 對作業 "user-maintenance" 的權限為 "query, modify, delete"，他人資料 "<他人>"，他部門資料 "<他部門>"，單身 "0"
    當 目前使用者執行 "<操作>"
    那麼 <結果>

    例子:
      | 他人 | 他部門 | 操作            | 結果                                                  |
      | 0    | 0      | 修改使用者 u002 | 不應被權限拒絕                                        |
      | 1    | 0      | 修改使用者 u002 | 不應被權限拒絕                                        |
      | 1    | 0      | 刪除使用者 u002 | 應被拒絕，作業 "user-maintenance"、功能 "delete"、原因 "他人資料"   |
      | 2    | 0      | 修改使用者 u002 | 應被拒絕，作業 "user-maintenance"、功能 "modify"、原因 "他人資料"   |
      | 2    | 0      | 刪除使用者 u002 | 不應被權限拒絕                                        |
      | 3    | 0      | 刪除使用者 u002 | 應被拒絕，作業 "user-maintenance"、功能 "delete"、原因 "他人資料"   |
      | 0    | 1      | 修改使用者 u003 | 不應被權限拒絕                                        |
      | 0    | 1      | 刪除使用者 u003 | 應被拒絕，作業 "user-maintenance"、功能 "delete"、原因 "他部門資料" |
      | 0    | 2      | 修改使用者 u003 | 應被拒絕，作業 "user-maintenance"、功能 "modify"、原因 "他部門資料" |
      | 0    | 2      | 刪除使用者 u003 | 不應被權限拒絕                                        |
      | 0    | 3      | 修改使用者 u003 | 應被拒絕，作業 "user-maintenance"、功能 "modify"、原因 "他部門資料" |
      | 0    | 3      | 修改使用者 u002 | 不應被權限拒絕                                        |
      | 3    | 3      | 修改使用者 u004 | 不應被權限拒絕                                        |
      | 3    | 3      | 刪除使用者 u004 | 不應被權限拒絕                                        |
      | 0    | 3      | 修改使用者 u005 | 應被拒絕，作業 "user-maintenance"、功能 "modify"、原因 "他部門資料" |

  # AQ6 他部門資料 5–8：p_zyw 移植前相關部門只有自己的部門；
  # TipTop 的 cl_chk_act_auth 對 5、8 允許修改、6、8 允許刪除任何部門（只靠查詢過濾），TTRS 另外檢查部門
  場景大綱: 相關部門權限在部門群組移植前只含自己的部門
    假設 已有使用者 "u002"，資料所有者 "u009"，所有部門 "SAL"
    而且 已有使用者 "u003"，資料所有者 "u009"，所有部門 "MFG"
    而且 權限類別 "SALES" 對作業 "user-maintenance" 的權限為 "query, modify, delete"，他人資料 "0"，他部門資料 "<他部門>"，單身 "0"
    當 目前使用者執行 "<操作>"
    那麼 <結果>

    例子:
      | 他部門 | 操作            | 結果                                                  |
      | 8      | 修改使用者 u002 | 不應被權限拒絕                                        |
      | 8      | 刪除使用者 u002 | 不應被權限拒絕                                        |
      | 8      | 修改使用者 u003 | 應被拒絕，作業 "user-maintenance"、功能 "modify"、原因 "他部門資料" |
      | 6      | 刪除使用者 u003 | 應被拒絕，作業 "user-maintenance"、功能 "delete"、原因 "他部門資料" |

  # p_zz.4gl:1262-1263 程式維護也檢查資料所有者（AQ5）
  場景: 程式維護檢查資料所有者
    假設 已有程式 "item-unit" 模組 "AIM"，資料所有者 "u009"，所有部門 "SAL"
    而且 權限類別 "SALES" 對作業 "program-maintenance" 的權限為 "query, modify, delete"，他人資料 "2"，他部門資料 "0"，單身 "0"
    當 目前使用者執行 "修改程式 item-unit"
    那麼 應被拒絕，作業 "program-maintenance"、功能 "modify"、原因 "他人資料"

  # AQ5 TipTop 沒設資料所有者的作業不檢查資料權限
  場景大綱: 沒有資料權限的作業不檢查資料所有者
    假設 營運中心 "TW02" 已啟用，資料所有者 "u009"，所有部門 "MFG"
    而且 權限類別 "OTHER" 已存在，資料所有者 "u009"，所有部門 "MFG"
    而且 權限類別 "SALES" 對作業 "<作業>" 擁有全部功能，他人資料 "3"，他部門資料 "3"，單身 "0"
    當 目前使用者執行 "<操作>"
    那麼 不應被權限拒絕

    例子:
      | 作業    | 操作               |
      | plant-maintenance | 修改營運中心 TW02  |
      | permission-class-maintenance    | 修改權限類別 OTHER |

  # ---------- 資料權限：查詢（p_zx.4gl:194-204）----------

  # 他人資料 4 只查自己建立的；他部門資料 4 只查自己部門的；5–8 只查相關部門（目前只有自己的部門）
  場景大綱: 查詢使用者依資料權限過濾
    假設 已有使用者 "u002"，資料所有者 "u009"，所有部門 "SAL"
    而且 已有使用者 "u003"，資料所有者 "u009"，所有部門 "MFG"
    而且 已有使用者 "u004"，資料所有者 "u001"，所有部門 "MFG"
    而且 權限類別 "SALES" 對作業 "user-maintenance" 的權限為 "query"，他人資料 "<他人>"，他部門資料 "<他部門>"，單身 "0"
    當 目前使用者以條件 "使用者代號 u00*" 查詢使用者
    那麼 查詢結果應為 "<結果>"

    例子:
      | 他人 | 他部門 | 結果                |
      | 3    | 3      | u001,u002,u003,u004 |
      | 4    | 0      | u001,u004           |
      | 0    | 4      | u001,u002           |
      | 4    | 4      | u001                |
      | 0    | 7      | u001,u002           |

  # AQ6 查不到的資料以代號直接查看、修改也拒絕
  場景大綱: 被資料權限過濾的使用者不能直接查看或修改
    假設 已有使用者 "u003"，資料所有者 "u009"，所有部門 "MFG"
    而且 權限類別 "SALES" 對作業 "user-maintenance" 的權限為 "query, modify"，他人資料 "0"，他部門資料 "4"，單身 "0"
    當 目前使用者執行 "<操作>"
    那麼 應被拒絕，作業 "user-maintenance"、功能 "<功能>"、原因 "他部門資料"

    例子:
      | 操作            | 功能   |
      | 查看使用者 u003 | query  |
      | 修改使用者 u003 | modify |

  # ---------- 單身權限（cl_detail_input_auth.4gl:45-72）----------

  場景大綱: 單身新增列、移除列另看單身權限
    假設 已有程式 "item-batch" 模組 "AIM"，可執行功能 "query"
    而且 權限類別 "MGR" 可執行程式 "item-maintenance"
    而且 權限類別 "SALES" 對作業 "program-permission-maintenance" 的權限為 "query, detail"，他人資料 "0"，他部門資料 "0"，單身 "<單身>"
    當 目前使用者執行 "<操作>"
    那麼 <結果>

    例子:
      | 單身 | 操作                      | 結果                                              |
      | 1    | 新增 MGR 的程式列 item-batch | 不應被權限拒絕                                    |
      | 2    | 新增 MGR 的程式列 item-batch | 應被拒絕，作業 "program-permission-maintenance"、功能 "detail"、原因 "單身" |
      | 1    | 移除 MGR 的程式列 item-maintenance | 應被拒絕，作業 "program-permission-maintenance"、功能 "detail"、原因 "單身" |
      | 2    | 移除 MGR 的程式列 item-maintenance | 不應被權限拒絕                                    |
      | 3    | 修改 MGR 的程式列 item-maintenance | 不應被權限拒絕                                    |

  # ---------- 參考清單（AQ4）----------

  場景大綱: 參考清單只要求登入
    當 目前使用者取得 "<清單>" 的全部清單
    那麼 不應被權限拒絕

    例子:
      | 清單     |
      | 程式     |
      | 權限類別 |
      | 使用者   |
      | 營運中心 |

  # ---------- AI 代理（ADR 0006 §6，AQ9）----------

  場景大綱: AI 代理只能執行查詢與草稿類功能
    假設 權限類別 "SALES" 對作業 "user-maintenance" 擁有全部功能，他人資料 "0"，他部門資料 "0"，單身 "0"
    而且 目前使用者以 AI 代理身分操作
    當 目前使用者執行 "<操作>"
    那麼 <結果>

    例子:
      | 操作            | 結果                                                    |
      | 查詢使用者      | 不應被權限拒絕                                          |
      | 修改使用者 u001 | 不應被權限拒絕                                          |
      | 刪除使用者 u001 | 應被拒絕，作業 "user-maintenance"、功能 "delete"、原因 "AI 不可執行"  |
      | 停用使用者 u001 | 應被拒絕，作業 "user-maintenance"、功能 "invalid"、原因 "AI 不可執行" |
