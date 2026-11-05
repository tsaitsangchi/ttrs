# language: zh-TW
# TipTop 對應：無（TipTop 緊急時由 DBA 直接改 zx_file；TTRS 新增，ADR 0010 §B、§C）
#   參考：p_updzx10 伺服器端、不經畫面的密碼批次（tiptop/azz/4gl/p_updzx10.4gl:1-104）
#   資料表：zx_file（zx10 Web 密碼、zx18 可用截止日期、zx19 須改密碼、zx20 已試誤次數）、zxw_file／zy_file（使用者維護權限）
# 新系統名稱：ttrs-admin 伺服器指令（reset-password、unlock）
# 規則來源：tiptop/azz/4gl/p_zx.4gl:640-650（設定密碼後須改密碼）、:367-400（user_unlock）、
#           tiptop/azz/4gl/weblogin.4gl:432-444、:465-473（帳號有效、未過期）、tiptop/azz/sch/zx_file.sch。
#           相關 ADR 0009、ADR 0010。
# 狀態：ADR 0010 R11–R14 依建議確認（Key User，2026-11-05），進入 Red
#
# 確認事項（皆採建議，詳見 docs/adr/0010-password-recovery.md）：
#   R11 獨立指令程式 ttrs-admin（crates/api 的 bin），沒有 HTTP API
#   R12 臨時密碼隨機產生、只顯示一次（建議）還是由執行者輸入
#   R13 停用／過期帳號預設拒絕，加 --reactivate 才恢復
#   R14 至少保留一位有效的使用者維護管理員（LAST_USER_ADMIN）
# 不移植欄位：本規格不新增 TipTop 欄位對應（見 ADR 0010「不移植的欄位」）。

功能: 伺服器端緊急重設密碼（ttrs-admin）
  身為 系統管理員（能登入伺服器）
  我想要 在沒有人能登入 TTRS 時重設使用者的密碼
  以便 唯一的管理員忘記密碼或被鎖定時，仍能恢復使用

  背景:
    假設 權限類別 "CLASS1" 已存在
    而且 選單程式 "menu-main" 已存在
    而且 營運中心 "TW01" 已啟用
    而且 已有使用者 "admin"，預設營運中心 "TW01"，Web 密碼 "Secret123"

  # p_zx.4gl:640-650 設定密碼後須改密碼；R12 隨機臨時密碼只顯示一次
  場景: 重設後以臨時密碼登入須先改密碼
    當 在伺服器執行 "ttrs-admin reset-password admin"
    那麼 指令應成功並顯示一組臨時密碼
    而且 以使用者代號 "admin"、顯示的臨時密碼登入應出現錯誤 "PASSWORD_CHANGE_REQUIRED"
    而且 以使用者代號 "admin"、密碼 "Secret123" 登入應出現錯誤 "SIGN_IN_FAILED"

  # 同 user_unlock（p_zx.4gl:367-400）
  場景: 重設同時解除鎖定
    假設 以使用者代號 "admin"、密碼 "wrong" 登入 5 次
    當 在伺服器執行 "ttrs-admin reset-password admin"
    那麼 使用者 "admin" 的已試誤次數應為 0

  場景: 只解除鎖定、不改密碼
    假設 以使用者代號 "admin"、密碼 "wrong" 登入 5 次
    當 在伺服器執行 "ttrs-admin unlock admin"
    那麼 指令應成功
    而且 以使用者代號 "admin"、密碼 "Secret123" 登入應成功

  場景: 重設後結束已登入的連線
    假設 使用者 "admin" 已登入
    當 在伺服器執行 "ttrs-admin reset-password admin"
    那麼 使用者 "admin" 原本的連線應已失效

  場景: 使用者不存在
    當 在伺服器執行 "ttrs-admin reset-password nouse"
    那麼 指令應失敗並顯示錯誤 "USER_NOT_FOUND"

  # R13：weblogin.4gl:432-444、:465-473 停用／過期不可登入
  場景大綱: 停用或過期的帳號
    假設 <狀況>
    當 在伺服器執行 "<指令>"
    那麼 <結果>

    例子:
      | 狀況                         | 指令                                           | 結果                                   |
      | 使用者 "admin" 已停用        | ttrs-admin reset-password admin                | 指令應失敗並顯示錯誤 "USER_INACTIVE"   |
      | 使用者 "admin" 已過有效日期  | ttrs-admin reset-password admin                | 指令應失敗並顯示錯誤 "USER_EXPIRED"    |
      | 使用者 "admin" 已停用        | ttrs-admin reset-password admin --reactivate   | 指令應成功並顯示一組臨時密碼           |

  # R12：臨時密碼不寫入紀錄；稽核記錄執行者
  場景: 留下稽核紀錄但不記錄密碼
    當 作業系統使用者 "root" 在伺服器執行 "ttrs-admin reset-password admin"
    那麼 稽核紀錄應有一筆「伺服器指令 root 重設使用者 admin 的密碼」
    而且 稽核紀錄與程式紀錄都不應含有臨時密碼

  # R11
  場景: 緊急重設不能從 Web 呼叫
    那麼 API 不應提供緊急重設密碼的端點

  # R14 §C：至少保留一位有效、擁有使用者維護 modify 權限的使用者
  場景大綱: 不可移除最後一位使用者維護管理員
    假設 只有使用者 "admin" 擁有使用者維護（user-maintenance）的 modify 權限
    當 <操作>
    那麼 應出現錯誤 "LAST_USER_ADMIN"

    例子:
      | 操作                                        |
      | 使用者將使用者 "admin" 停用                 |
      | 使用者刪除使用者 "admin"                    |
      | 使用者將使用者 "admin" 的權限類別改為無權限 |

  場景: 還有其他管理員時可以停用
    假設 使用者 "admin2" 也擁有使用者維護（user-maintenance）的 modify 權限
    當 使用者將使用者 "admin" 停用
    那麼 操作應成功
