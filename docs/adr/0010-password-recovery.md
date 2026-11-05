# ADR 0010：忘記密碼的自助重設與伺服器端緊急重設

- 狀態：已採用（Key User，2026-11-05，R1–R15 依草稿建議確認；R9 SMTP 未提供前不開放）
- 延伸：ADR 0009（Web 密碼登入）；登入檢查順序、帳號稽核原則、雜湊方式不變
- 相關：`specs/features/azz/password_reset.feature`（草稿）、`specs/features/azz/emergency_password_reset.feature`（草稿）

## 背景

ADR 0009 移植後，忘記密碼只能請管理員在使用者維護（p_zx）重設（p_zx.4gl:640-650）、被鎖定時再「解除鎖定」
（p_zx.4gl:367-400）。這樣有兩個缺口：

1. **使用者自助**：每次忘記密碼都要找管理員，管理員也會看到臨時密碼。
2. **管理員自己忘記密碼**：如果唯一的 ADMIN 忘記密碼或被鎖定，系統內就沒有人能幫他重設。

TipTop 的現況（萃取）：

- TipTop **沒有**「忘記密碼」功能；weblogin.4gl 登入失敗只顯示 lib-064，沒有重設入口。
- 使用者 Email 存在 `zx_file.zx09`（varchar2(80)）；TipTop 已經用它寄密碼相關信件：p_passnoty 在密碼快到期時
  以 `cl_jmail` 寄信到 zx09（p_passnoty.4gl:74-147），zx09 空白就不寄（:141-143）。TTRS 已移植為 `user_account.email`。
- p_updzx10 是**伺服器端**、不經畫面執行的密碼批次程式（開啟密碼編碼時把所有 zx10 轉碼，p_updzx10.4gl:1-104）；
  TipTop 沒有單一使用者的緊急重設指令，緊急時由 DBA 直接改資料庫。

所以這兩種機制都是 **TTRS 新增的功能**，不是 TipTop 原有行為；規則盡量沿用 webpasswd（密碼規則）、
p_zx（重設後的狀態）、p_passnoty（寄到 zx09）。

## 決策

### A. 自助重設密碼（forgot-password）

1. 登入畫面加「忘記密碼」：輸入**使用者代號**（zx01），按「寄送重設連結」。
2. 不論帳號存不存在、有沒有 Email，畫面都顯示**同一個訊息**（「若此帳號設有 Email，重設連結已寄出」），
   不透露帳號是否存在（同 ADR 0009 W2）。
3. 只有帳號**存在、有效、未過期、有 Email** 時才寄信；寄到 `user_account.email`（zx09），同 p_passnoty。
4. 重設連結帶一次性**隨機權杖**（256 bits）；資料庫只存權杖的 SHA-256 雜湊、有效期限、使用時間，不存權杖本身。
   有效期限 30 分鐘（R3）；用過或重新申請後，舊權杖立即失效。
5. 開啟連結後輸入新密碼與確認：密碼規則和「變更密碼」相同（webpasswd.4gl:78-118、:154-199：最小長度、複雜度、
   不可含帳號、可否與舊密碼相同、最長 64 字）。不用輸入舊密碼。
6. 重設成功：密碼更新日＝今天（zx16）、登入次數＝0（zx17）、不須改密碼（zx19＝N）、試誤次數歸零（zx20，R5），
   並**結束該使用者所有已登入的連線**（R6）；之後用新密碼正常登入（營運中心規則同 sign_in.feature）。
7. 防濫用：同一使用者 1 小時內最多申請 3 次（R7），超過時畫面仍顯示同一個訊息、但不再寄信。
8. 寄出重設信與重設成功，都寫入稽核紀錄；重設成功另寄一封「密碼已變更」通知到同一個 Email（R8）。
9. 這是登入前的流程，和 sign-in、change-password 一樣**不經 Authorizer**；新程式代號 `forgot-password`（R10），
   不能由 AI Actor 執行。
10. 寄信透過 app 層新 port `Mailer`；infra 以 SMTP 實作（設定放環境變數），記憶體後端把信件存在寄件匣供 BDD 驗證。
    沒有設定 SMTP 時，「忘記密碼」入口不顯示，API 回 `PASSWORD_RESET_UNAVAILABLE`（R9）。

### B. 伺服器端緊急重設（ttrs-admin）

1. 新增指令程式 `ttrs-admin`（R11），只在伺服器上、以 `DATABASE_URL` 連線執行；**沒有對應的 HTTP API、不能從 Web 呼叫**。
2. `ttrs-admin reset-password <使用者代號>`：
   - 產生一組隨機臨時密碼，**只在終端機顯示一次**（不寫入紀錄檔）；
   - 須改密碼（zx19＝Y，同 p_zx.4gl:648）、試誤次數歸零（同 user_unlock）、結束該使用者所有已登入的連線；
   - 稽核紀錄寫入執行者為「伺服器指令」及作業系統使用者名稱（R12）。
3. 停用或過期的帳號：預設拒絕；加 `--reactivate` 才同時恢復有效（R13）。
4. 另加 `ttrs-admin unlock <使用者代號>`：只把試誤次數歸零（同 user_unlock）。

### C. 避免沒有管理員可用（R14）

- 使用者維護中，停用、刪除或移除權限後，若系統內就沒有「有效、且擁有使用者維護 modify 權限」的使用者，就拒絕這次操作，
  錯誤 `LAST_USER_ADMIN`。ttrs-admin 是最後手段，這條規則是第一道防線。

## 確認事項（皆採建議）

| # | 問題 | 建議 |
|---|---|---|
| R1 | 忘記密碼時輸入**使用者代號**還是 **Email**？（同一個 Email 可能對應多個代號，zx09 沒有唯一限制） | 使用者代號 |
| R2 | 帳號不存在、沒有 Email、停用都顯示同一個訊息、不透露原因，可以嗎？ | 可以（同 W2） |
| R3 | 重設連結有效期限？ | 30 分鐘、只能用一次 |
| R4 | 重設連結的網址：要用哪個對外網址（例如 `https://ttrs.<公司網域>`）？由環境變數 `TTRS_PUBLIC_URL` 設定，可以嗎？ | 可以 |
| R5 | 帳號因試誤被鎖定時，自助重設成功要**同時解除鎖定**嗎？（能收到 Email 已證明身分；不解除就仍要找管理員） | 解除 |
| R6 | 重設成功後結束該使用者所有已登入的連線，可以嗎？ | 可以 |
| R7 | 申請次數上限：同一使用者 1 小時 3 次，可以嗎？ | 可以 |
| R8 | 重設成功後寄「密碼已變更」通知信，可以嗎？（帳號被他人重設時本人才會察覺） | 可以 |
| R9 | 寄信用哪台 SMTP（主機、連接埠、帳號、寄件人）？沒有 SMTP 前「忘記密碼」先不開放，可以嗎？ | 請提供；未提供前不開放 |
| R10 | 新程式代號 `forgot-password`（重設密碼），要列入程式維護清單嗎？ | 列入，但不需權限設定（登入前） |
| R11 | 緊急重設用獨立指令程式 `ttrs-admin`（放在 `crates/api` 的 bin），可以嗎？ | 可以 |
| R12 | 緊急重設產生隨機臨時密碼且只顯示一次，還是讓執行者自己輸入？ | 隨機產生、只顯示一次 |
| R13 | 緊急重設遇到停用／過期帳號：預設拒絕，加 `--reactivate` 才恢復，可以嗎？ | 可以 |
| R14 | 加「至少保留一位有效的使用者維護管理員」規則（`LAST_USER_ADMIN`），可以嗎？ | 可以 |
| R15 | 信件內容（主旨、內文）要三語嗎？依使用者的語系寄送？ | 依使用者語系，翻譯放 ftl |

### 不移植的欄位

本 ADR 不新增 TipTop 欄位對應；新資料表 `password_reset_token` 是 TTRS 新增的，沒有對應的 TipTop 表。

| 項目 | 原因 |
|---|---|
| p_passnoty（密碼到期通知批次） | 不在本 ADR；之後與背景作業一起移植（ADR 0009 W12），會共用本 ADR 的 `Mailer` |
| p_updzx10（密碼編碼批次） | 不移植；TTRS 一律雜湊（ADR 0009 W12），ttrs-admin 不提供批次轉碼 |

## 影響

- 新資料表 `password_reset_token`（public、scope: system）：使用者代號、權杖雜湊、建立時間、到期時間、使用時間。
- 新 port：`Mailer`、`PasswordResetTokens`（memory.rs 與 infra/pg.rs 同時實作）。
- 新錯誤代碼：`PASSWORD_RESET_TOKEN_INVALID`、`PASSWORD_RESET_UNAVAILABLE`、`LAST_USER_ADMIN`（三語翻譯）。
- 新畫面：登入畫面加「忘記密碼」連結、「申請重設」、「設定新密碼」（pages/azz/）。
- legacy-map：`docs/legacy-map/programs.yaml` 加 forgot-password（TTRS 新增，無 TipTop 程式）。

## 部署設定

- 忘記密碼（R4、R9）：設定 `TTRS_SMTP_HOST`、`TTRS_SMTP_FROM`（必要），`TTRS_SMTP_PORT`、`TTRS_SMTP_TLS`
  （`starttls` 預設／`tls`／`none`）、`TTRS_SMTP_USER`、`TTRS_SMTP_PASSWORD`（選用），以及對外網址 `TTRS_PUBLIC_URL`。
  沒有 `TTRS_SMTP_HOST` 時登入畫面不顯示「忘記密碼」；有 SMTP 但缺 `TTRS_PUBLIC_URL` 時 API 啟動失敗。
- 緊急重設（R11）：在伺服器上執行 `DATABASE_URL=… ttrs-admin reset-password <使用者代號> [--reactivate]`、
  `ttrs-admin unlock <使用者代號>`（`crates/api/src/bin/ttrs-admin.rs`）；稽核紀錄的執行者為 `os:<SUDO_USER 或 USER>`。
