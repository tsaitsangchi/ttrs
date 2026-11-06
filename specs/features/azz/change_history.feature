# language: zh-TW
# TipTop 對應：azo_file 系統重要資料修改記錄檔（各程式自行 INSERT）、lib/4gl/cl_log.4gl（p_ 系列寫文字記錄檔）；
#             各表共同欄位 xxxacti／xxxuser／xxxgrup／xxxmodu／xxxdate
# 新系統名稱：Change History 資料異動記錄（所有資料維護作業共用；功能代號 history）
# 規則來源：docs/adr/0012-change-history.md（AU1–AU16 已確認，Key User 2026-11-06）；
#           tiptop/aoo/sch/azo_file.sch、tiptop/aoo/sch/gem_file.sch、tiptop/aim/4gl/aimi100.4gl:4399-4405、:7154-7160、
#           tiptop/lib/4gl/cl_log.4gl:40-142；無 topcust 覆寫。
# 狀態：CH1–CH9 依建議確認（Key User，2026-11-06），進入 Red。
#   實作分兩階段：第一階段工廠層的表（department、employee、classification_code）與共用機制；
#   第二階段共用層的表（CH1 其餘各表，寫入改為在交易內執行並設定操作情境），CH-7 標 @draft 待第二階段。
#
# 範例以部門維護（工廠層）與使用者維護（共用層）說明；規則適用於 CH1 所列的每張表。
#
# 確認事項（皆採建議）：
#  CH1. 第一階段套用的表（AU1；之後新作業建表時一併套用）：
#       | 層級 | 資料表 | 對應作業 |
#       | 共用 | program、program_name | 程式資料維護 |
#       | 共用 | permission_class | 權限類別維護 |
#       | 共用 | program_permission | 程式權限維護 |
#       | 共用 | user_account、user_plant | 使用者維護（含可使用營運中心） |
#       | 共用 | user_permission | 使用者權限維護 |
#       | 共用 | plant | 營運中心維護 |
#       | 共用 | account_policy | 帳號安全設定 |
#       | 共用 | table_dictionary、table_dictionary_text、column_dictionary_text | 資料字典 |
#       | 工廠 | department、employee、classification_code | 部門、員工、分類碼維護 |
#       不套用：module、menu_item、query_definition（起始資料，只由 migration 維護）、legacy_table、legacy_column（TipTop 目錄匯入）、
#       user_credential、password_reset_token（密碼與權杖，AU13 排除）、audit_log、change_log 本身、
#       unit、item、inventory_txn（早期試做表，料件 aimi100 重新移植時依新標準建表）。可以嗎？
#  CH2. 子表（program_name、user_plant、*_text）各列自成一筆記錄，以 request_id 與主檔串起來（AU15）；
#       異動記錄畫面在主檔的記錄下一併列出子表的異動。可以嗎？
#  CH3. 伺服器指令 ttrs-admin（緊急重設，ADR 0010）修改 user_account 時：actor_kind＝service、user_id＝「os:作業系統使用者」、
#       程式＝ttrs-admin，與 audit_log 現有寫法一致。可以嗎？
#  CH4. 修改、刪除、啟用／停用的請求一律必須帶 version；沒帶時回 400（請求格式錯誤），不視為版本衝突。可以嗎？
#  CH5. 異動記錄畫面的欄位名稱用資料字典（column_dictionary_text）的欄位說明，依語系顯示；字典沒有的欄位顯示資料庫欄位名稱。可以嗎？
#  CH6. 遮蔽欄位（AU13）第一階段只有 user_account.email、employee.email（只記「有修改」）；employee.extension 分機屬公司資料，不遮蔽。
#       可以嗎？
#  CH7. 月份分割：啟動 API 時自動建立本月與下個月的分割表，另有一個預設分割表接住其他月份，避免寫入失敗。可以嗎？
#  CH8. 程式資料維護刪除程式時現有的 audit_log 記錄（program_service，action＝delete）：改由 change_log 記錄，移除 audit_log 這筆。
#       audit_log 之後只記查詢、列印、匯出、登入、密碼重設等存取與安全事件。可以嗎？
#  CH9. 資料庫權限與觸發器的情境（CH-9、CH-10）只有 PostgreSQL 才有意義：新增標籤 @pg，記憶體後端執行 BDD 時略過，
#       `make bdd-pg` 才執行。可以嗎？
#
# 不移植的項目（AU16，已確認）：
#  | 項目 | 原因 |
#  | azo_file（azo01–azo06） | 由 change_log 取代，所有異動自動記錄且含改前改後的值 |
#  | cl_log 文字記錄檔 | 由 change_log 取代 |
#  | SELECT … FOR UPDATE 編輯鎖 | 改為 version 樂觀鎖 |
#  | xxxdate 只到日 | 改為 updated_at（timestamptz） |

功能: 資料異動記錄（Change History）
  身為 負責查核的人員
  我想要 查到每筆資料由誰、在何時、透過哪支程式、把哪些欄位從什麼改成什麼
  以便 追查錯誤與符合內部控制的查核要求

  背景:
    假設 目前使用者為 "tiptop"，所屬部門 "MIS"，權限類別 "CLASS1"
    而且 營運中心 "TW01" 已啟用

  # ADR 0012 §2、§3：新增也寫入記錄（含建立者，AU4）
  場景: CH-1 新增資料寫入異動記錄
    當 使用者在營運中心 "TW01" 新增部門 "MIS"，簡稱 "資訊部"
    那麼 部門 "MIS" 的異動記錄應為:
      | 版本 | 動作   | 操作者 | 程式                   | 變動欄位       |
      | 1    | insert | tiptop | department-maintenance | 部門編號,簡稱… |
    而且 部門 "MIS" 的版本應為 1

  # §2 改前／改後、變動欄位；§1 updated_by、updated_at、version
  場景: CH-2 修改資料記錄改前改後的值
    假設 營運中心 "TW01" 已有部門 "MIS"，簡稱 "資訊部"
    當 使用者 "mary" 將部門 "MIS" 的簡稱改為 "資訊處"
    那麼 部門 "MIS" 最新一筆異動記錄應為:
      | 版本 | 動作   | 操作者 | 欄位 | 改前   | 改後   |
      | 2    | update | mary   | 簡稱 | 資訊部 | 資訊處 |
    而且 部門 "MIS" 的最後修改者應為 "mary"，最後修改時間應到秒

  # §2 刪除時保留整筆內容
  場景: CH-3 刪除後仍查得到原本的內容
    假設 營運中心 "TW01" 已有部門 "TMP"，簡稱 "暫時部門"
    當 使用者刪除部門 "TMP"
    那麼 部門 "TMP" 最新一筆異動記錄的動作應為 "delete"，改前的簡稱為 "暫時部門"

  # §3 業務動作取代 update
  場景: CH-4 停用與啟用記錄為業務動作
    假設 營運中心 "TW01" 已有部門 "MIS"，簡稱 "資訊部"
    當 使用者停用部門 "MIS"
    而且 使用者啟用部門 "MIS"
    那麼 部門 "MIS" 異動記錄的動作依序應為 "insert,deactivate,activate"

  # §4 樂觀鎖（AU3）
  場景: CH-5 兩人同時修改時後存檔的人收到版本衝突
    假設 營運中心 "TW01" 已有部門 "MIS"，簡稱 "資訊部"
    而且 使用者 "mary" 與 "john" 都讀取了部門 "MIS" 的第 1 版
    當 使用者 "mary" 以第 1 版將部門 "MIS" 的簡稱改為 "資訊處"
    而且 使用者 "john" 以第 1 版將部門 "MIS" 的簡稱改為 "電腦室"
    那麼 應出現錯誤 "RECORD_CHANGED"
    而且 部門 "MIS" 的簡稱應為 "資訊處"

  # §2 actor_kind、on_behalf_of（ADR 0006）
  場景: CH-6 AI 代表使用者操作時記錄被代表的人
    假設 營運中心 "TW01" 已有部門 "MIS"，簡稱 "資訊部"
    當 AI 代表使用者 "mary" 將部門 "MIS" 的簡稱改為 "資訊處"
    那麼 部門 "MIS" 最新一筆異動記錄的操作者類型應為 "ai"，代表 "mary"

  # §2 request_id（AU15、CH2）；第二階段（共用層的表）
  @draft
  場景: CH-7 主檔與子表一次存檔以同一個請求代號串起來
    假設 使用者 "mary" 可使用營運中心 "TW01"
    當 使用者將 "mary" 的可使用營運中心改為 "TW01、TW02"，預設 "TW02"
    那麼 使用者 "mary" 的異動記錄應包含 user_account 與 user_plant 的異動，且請求代號相同

  # §7 敏感欄位遮蔽（AU13、CH6）
  場景: CH-8 個資欄位只記有修改
    假設 營運中心 "TW01" 已有員工 "E001"，電子郵件 "a@example.com"
    當 使用者將員工 "E001" 的電子郵件改為 "b@example.com"
    那麼 員工 "E001" 最新一筆異動記錄的電子郵件改前改後都應為 "***"
    而且 變動欄位應包含 "電子郵件"

  # §5 只能新增（AU5）
  @pg
  場景: CH-9 異動記錄不可修改或刪除
    假設 營運中心 "TW01" 已有部門 "MIS"，簡稱 "資訊部"
    當 以應用程式的資料庫角色修改或刪除部門 "MIS" 的異動記錄
    那麼 資料庫應拒絕

  # §3 直接 SQL 也記錄（AU8）
  @pg
  場景: CH-10 不經 TTRS 直接修改資料庫也會記錄
    假設 營運中心 "TW01" 已有部門 "MIS"，簡稱 "資訊部"
    當 以 SQL 直接將部門 "MIS" 的簡稱改為 "資訊處"
    那麼 部門 "MIS" 最新一筆異動記錄的操作者類型應為 "db"
    而且 部門 "MIS" 的版本應為 2

  # §8 異動記錄功能與權限（AU11）；各作業的 history 功能另列在 authorization.feature 對照表。
  # 查詢資料目前不套用資料權限（ADR 0006 只有修改、刪除才套用），異動記錄比照查詢；刪除後的資料仍可查（CH-3）。
  場景: CH-11 沒有異動記錄權限時不可查詢
    假設 營運中心 "TW01" 已有部門 "MIS"，簡稱 "資訊部"
    而且 權限類別 "CLASS1" 已存在
    而且 模組 "AZZ" 已存在
    而且 模組 "AOO" 已存在
    而且 模組 "AIM" 已存在
    而且 已登錄 TTRS 已移植的作業
    當 權限類別 "CLASS1" 在作業 "department-maintenance" 沒有功能 "history" 時，目前使用者執行 "查詢部門 MIS 的異動記錄"
    那麼 應出現錯誤 "PERMISSION_DENIED"
