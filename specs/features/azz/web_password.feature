# language: zh-TW
# TipTop 對應：weblogin Web 使用者登入（密碼驗證）、webpasswd 變更密碼、p_zx 使用者維護（Web 密碼、解除鎖定）
#   資料表：zx_file（zx10 Web 密碼、zx16 密碼更新日、zx17 已登入次數、zx19 須改密碼、zx20 已試誤次數）、
#           gbt_file（系統帳號稽核原則）
# 新系統名稱：Sign In 登入（sign-in）、Change Password 變更密碼（change-password）
# 規則來源：tiptop/azz/4gl/weblogin.4gl:426-620（user_validate、pwd_validate）、:329-388（須改密碼）、
#           tiptop/azz/4gl/webpasswd.4gl:67-199（變更密碼、ver_passwd）、
#           tiptop/azz/4gl/p_zx.4gl:367-400（user_unlock）、:447、:561、:640-650（預設密碼與須改密碼）、
#           tiptop/azz/sch/zx_file.sch、tiptop/azz/sch/gbt_file.sch；無 topcust 覆寫。相關 ADR 0009。
# 狀態：ADR 0009 W1–W12 依草稿確認（Key User，2026-11-04），進入 Red
#
# 本規格的帳號稽核原則（gbt_file）以背景設定；起始值見 ADR 0009 W5。
# 營運中心的規則（預設營運中心、可使用、停用、切換）見 sign_in.feature，不在此重複。

功能: Web 密碼登入（Sign In with Web Password）
  身為 使用者
  我想要 以使用者代號與 Web 密碼登入 TTRS
  以便 只有知道密碼的人能以我的身分使用系統

  背景:
    假設 權限類別 "CLASS1" 已存在
    而且 選單程式 "menu-main" 已存在
    而且 營運中心 "TW01" 已啟用
    而且 帳號稽核原則為:
      | 項目               | 值   |
      | 密碼更新依據       | 天數 |
      | 密碼更新週期（天） | 90   |
      | 提前通知天數       | 7    |
      | 密碼最小長度       | 8    |
      | 密碼複雜度         | M    |
      | 可與原密碼相同     | 否   |
      | 允許最大試誤次數   | 5    |
    而且 已有使用者 "u001"，預設營運中心 "TW01"，Web 密碼 "Secret123"

  # weblogin.4gl:478-620 密碼正確即登入；試誤次數歸零（:611-615）
  場景: 以使用者代號與密碼登入
    當 以使用者代號 "u001"、密碼 "Secret123" 登入
    那麼 操作應成功
    而且 登入的使用者應為 "u001"
    而且 目前營運中心應為 "TW01"
    而且 使用者 "u001" 的已試誤次數應為 0

  # weblogin.4gl:432-444、:516-532 帳號不存在與密碼錯誤同一訊息（lib-064，W2）
  場景大綱: 帳號或密碼錯誤
    當 以使用者代號 "<代號>"、密碼 "<密碼>" 登入
    那麼 應出現錯誤 "SIGN_IN_FAILED"

    例子:
      | 代號  | 密碼      |
      | u001  | wrong     |
      | nouse | Secret123 |

  # weblogin.4gl:525-528 密碼錯誤時試誤次數＋1；:450-462 達上限即鎖定（azz1090，W3）
  場景: 試誤次數達上限即鎖定
    假設 以使用者代號 "u001"、密碼 "wrong" 登入 5 次
    當 以使用者代號 "u001"、密碼 "Secret123" 登入
    那麼 應出現錯誤 "USER_LOCKED"

  # p_zx.4gl:367-400 解除鎖定
  場景: 管理員解除鎖定後可登入
    假設 以使用者代號 "u001"、密碼 "wrong" 登入 5 次
    當 使用者解除使用者 "u001" 的鎖定
    而且 以使用者代號 "u001"、密碼 "Secret123" 登入
    那麼 操作應成功

  # weblogin.4gl:432-444、:465-473 帳號停用、過期（同 sign_in.feature SQ3）
  場景: 已停用的使用者不可登入
    假設 使用者 "u001" 已停用
    當 以使用者代號 "u001"、密碼 "Secret123" 登入
    那麼 應出現錯誤 "USER_INACTIVE"

  # p_zx.4gl:447、:561 新增使用者時密碼預設為使用者代號，下次登入須改密碼
  場景: 新使用者以代號為密碼登入後須先改密碼
    假設 已有新使用者 "u002"，預設營運中心 "TW01"
    當 以使用者代號 "u002"、密碼 "u002" 登入
    那麼 應出現錯誤 "PASSWORD_CHANGE_REQUIRED"
    而且 使用者 "u002" 只能變更密碼

  # p_zx.4gl:640-650 管理員設定密碼後須改密碼（W7）
  場景: 管理員重設密碼後須先改密碼
    當 使用者將使用者 "u001" 的 Web 密碼設為 "Reset0001"
    而且 以使用者代號 "u001"、密碼 "Reset0001" 登入
    那麼 應出現錯誤 "PASSWORD_CHANGE_REQUIRED"

  # weblogin.4gl:551-565 密碼過期（W4：要求先改密碼）；:566-578 到期前提醒
  場景大綱: 密碼依天數到期
    假設 使用者 "u001" 的密碼更新日為 <天前> 天前
    當 以使用者代號 "u001"、密碼 "Secret123" 登入
    那麼 <結果>

    例子:
      | 天前 | 結果                                   |
      | 91   | 應出現錯誤 "PASSWORD_CHANGE_REQUIRED"  |
      | 85   | 操作應成功，並提示密碼將於 5 天後到期  |
      | 10   | 操作應成功                             |

  # webpasswd.4gl:67-150 變更密碼：成功後更新日為今天、登入次數歸零、不須改密碼
  場景: 變更密碼
    當 使用者 "u001" 將密碼由 "Secret123" 改為 "NewPass99"
    那麼 操作應成功
    而且 以使用者代號 "u001"、密碼 "NewPass99" 登入應成功
    而且 以使用者代號 "u001"、密碼 "Secret123" 登入應出現錯誤 "SIGN_IN_FAILED"

  # webpasswd.4gl:78-118、:154-199 ver_passwd
  場景大綱: 新密碼須符合帳號稽核原則
    當 使用者 "u001" 將密碼由 "<舊密碼>" 改為 "<新密碼>"，確認 "<確認>"
    那麼 應出現錯誤 "<錯誤>"

    例子:
      | 舊密碼    | 新密碼       | 確認         | 錯誤                       |
      | wrong     | NewPass99    | NewPass99    | OLD_PASSWORD_INCORRECT     |
      | Secret123 | NewPass99    | NewPass98    | PASSWORD_CONFIRM_MISMATCH  |
      | Secret123 | Ab1          | Ab1          | PASSWORD_TOO_SHORT         |
      | Secret123 | u001Pass9    | u001Pass9    | PASSWORD_CONTAINS_USER_ID  |
      | Secret123 | NoDigitsHere | NoDigitsHere | PASSWORD_NEEDS_DIGIT       |
      | Secret123 | Secret123    | Secret123    | PASSWORD_SAME_AS_OLD       |

  # ADR 0009 決策 2：只存雜湊
  場景: 密碼不以明文保存
    那麼 使用者 "u001" 存放的密碼不等於 "Secret123"
