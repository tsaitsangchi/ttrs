# language: zh-TW
# TipTop 對應：無（TipTop 沒有忘記密碼功能；TTRS 新增，ADR 0010 §A）
#   資料表：zx_file（zx09 Email、zx16 密碼更新日、zx17 已登入次數、zx19 須改密碼、zx20 已試誤次數）、
#           gbt_file（帳號稽核原則）；新增 password_reset_token（TTRS 新表）
# 新系統名稱：Forgot Password 忘記密碼（forgot-password）
# 規則來源：tiptop/azz/4gl/p_passnoty.4gl:139-143（寄到 zx09、zx09 空白不寄）、
#           tiptop/azz/4gl/webpasswd.4gl:78-118、:154-199（新密碼規則 ver_passwd）、
#           tiptop/azz/4gl/weblogin.4gl:432-444（帳號無效）、:516-532（同一訊息 lib-064）、
#           tiptop/azz/4gl/p_zx.4gl:367-400（user_unlock）、tiptop/azz/sch/zx_file.sch。相關 ADR 0009、ADR 0010。
# 狀態：ADR 0010 R1–R10、R15 依建議確認（Key User，2026-11-05），進入 Red
#
# 確認事項（皆採建議，詳見 docs/adr/0010-password-recovery.md）：
#   R1 輸入使用者代號（建議）還是 Email
#   R2 帳號不存在／無 Email／停用顯示同一訊息
#   R3 連結有效 30 分鐘、只能用一次
#   R4 連結網址由 TTRS_PUBLIC_URL 設定
#   R5 自助重設成功同時解除鎖定
#   R6 重設成功結束所有已登入連線
#   R7 同一使用者 1 小時最多申請 3 次
#   R8 重設成功寄「密碼已變更」通知
#   R9 SMTP 設定；未設定前不開放
#   R10 程式代號 forgot-password
#   R15 信件依使用者語系寄送
# 不移植欄位：本規格不新增 TipTop 欄位對應（見 ADR 0010「不移植的欄位」）。

功能: 忘記密碼自助重設（Forgot Password）
  身為 忘記密碼的使用者
  我想要 以寄到自己 Email 的連結重設密碼
  以便 不必請管理員重設，也不讓管理員知道我的密碼

  背景:
    假設 權限類別 "CLASS1" 已存在
    而且 選單程式 "menu-main" 已存在
    而且 營運中心 "TW01" 已啟用
    而且 帳號稽核原則為:
      | 項目               | 值   |
      | 密碼更新依據       | 天數 |
      | 密碼更新週期（天） | 90   |
      | 密碼最小長度       | 8    |
      | 密碼複雜度         | M    |
      | 可與原密碼相同     | 否   |
      | 允許最大試誤次數   | 5    |
    而且 已設定寄信伺服器
    而且 已有使用者 "u001"，預設營運中心 "TW01"，Web 密碼 "Secret123"，Email "u001@example.com"

  場景: 申請重設後寄出連結
    當 以使用者代號 "u001" 申請重設密碼
    那麼 應顯示「若此帳號設有 Email，重設連結已寄出」
    而且 "u001@example.com" 應收到 1 封重設密碼信

  # R2：不透露帳號是否存在（同 ADR 0009 W2、weblogin lib-064）
  場景大綱: 不寄信時仍顯示同一個訊息
    假設 <狀況>
    當 以使用者代號 "<代號>" 申請重設密碼
    那麼 應顯示「若此帳號設有 Email，重設連結已寄出」
    而且 不應寄出任何信件

    例子:
      | 狀況                         | 代號  |
      | 使用者 "nouse" 不存在        | nouse |
      | 使用者 "u001" 沒有 Email     | u001  |
      | 使用者 "u001" 已停用         | u001  |
      | 使用者 "u001" 已過有效日期   | u001  |

  # 新密碼規則同變更密碼；重設後狀態：zx16＝今天、zx17＝0、zx19＝N、zx20＝0
  場景: 以連結設定新密碼後可登入
    假設 使用者 "u001" 已申請重設密碼
    當 以收到的重設連結將密碼設為 "NewPass99"，確認 "NewPass99"
    那麼 操作應成功
    而且 使用者 "u001" 的密碼更新日應為今天
    而且 使用者 "u001" 不須改密碼
    而且 以使用者代號 "u001"、密碼 "NewPass99" 登入應成功
    而且 以使用者代號 "u001"、密碼 "Secret123" 登入應出現錯誤 "SIGN_IN_FAILED"

  # webpasswd.4gl:78-118、:154-199 ver_passwd（不需輸入舊密碼）
  場景大綱: 新密碼須符合帳號稽核原則
    假設 使用者 "u001" 已申請重設密碼
    當 以收到的重設連結將密碼設為 "<新密碼>"，確認 "<確認>"
    那麼 應出現錯誤 "<錯誤>"
    而且 重設連結仍可使用

    例子:
      | 新密碼       | 確認         | 錯誤                      |
      | NewPass99    | NewPass98    | PASSWORD_CONFIRM_MISMATCH |
      | Ab1          | Ab1          | PASSWORD_TOO_SHORT        |
      | u001Pass9    | u001Pass9    | PASSWORD_CONTAINS_USER_ID |
      | NoDigitsHere | NoDigitsHere | PASSWORD_NEEDS_DIGIT      |
      | Secret123    | Secret123    | PASSWORD_SAME_AS_OLD      |

  # R3：一次性、30 分鐘、重新申請後舊連結失效
  場景大綱: 無效的重設連結
    假設 使用者 "u001" 已申請重設密碼
    而且 <狀況>
    當 以收到的重設連結將密碼設為 "NewPass99"，確認 "NewPass99"
    那麼 應出現錯誤 "PASSWORD_RESET_TOKEN_INVALID"

    例子:
      | 狀況                                         |
      | 已過 31 分鐘                                 |
      | 已用此連結將密碼設為 "OtherPass1"            |
      | 使用者 "u001" 又申請了一次重設密碼           |
      | 使用者 "u001" 已停用                         |

  場景: 竄改過的連結無效
    當 以重設連結 "not-a-real-token" 將密碼設為 "NewPass99"，確認 "NewPass99"
    那麼 應出現錯誤 "PASSWORD_RESET_TOKEN_INVALID"

  # R5：自助重設成功同時解除鎖定（同 p_zx.4gl:367-400 user_unlock）
  場景: 被鎖定的帳號重設後可登入
    假設 以使用者代號 "u001"、密碼 "wrong" 登入 5 次
    而且 使用者 "u001" 已申請重設密碼
    當 以收到的重設連結將密碼設為 "NewPass99"，確認 "NewPass99"
    那麼 使用者 "u001" 的已試誤次數應為 0
    而且 以使用者代號 "u001"、密碼 "NewPass99" 登入應成功

  # R6
  場景: 重設成功後結束已登入的連線
    假設 使用者 "u001" 已登入
    而且 使用者 "u001" 已申請重設密碼
    當 以收到的重設連結將密碼設為 "NewPass99"，確認 "NewPass99"
    那麼 使用者 "u001" 原本的連線應已失效

  # R7
  場景: 申請次數超過上限不再寄信
    假設 使用者 "u001" 在 1 小時內已申請重設密碼 3 次
    當 以使用者代號 "u001" 申請重設密碼
    那麼 應顯示「若此帳號設有 Email，重設連結已寄出」
    而且 "u001@example.com" 共應收到 3 封重設密碼信

  # R8
  場景: 重設成功寄出通知信
    假設 使用者 "u001" 已申請重設密碼
    當 以收到的重設連結將密碼設為 "NewPass99"，確認 "NewPass99"
    那麼 "u001@example.com" 應收到 1 封密碼已變更通知信

  # R15
  場景: 信件依使用者語系寄送
    假設 使用者 "u001" 的語系為 "en"
    當 以使用者代號 "u001" 申請重設密碼
    那麼 "u001@example.com" 收到的重設密碼信應為 "en" 語系

  # R9
  場景: 未設定寄信伺服器時不開放
    假設 未設定寄信伺服器
    當 以使用者代號 "u001" 申請重設密碼
    那麼 應出現錯誤 "PASSWORD_RESET_UNAVAILABLE"

  # ADR 0010 決策 A4：只存權杖雜湊
  場景: 重設權杖不以明文保存
    當 以使用者代號 "u001" 申請重設密碼
    那麼 資料庫中不應存有收到的重設權杖原文
