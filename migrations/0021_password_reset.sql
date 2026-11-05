-- 忘記密碼與緊急重設（docs/adr/0010-password-recovery.md；specs/features/azz/password_reset.feature、
-- emergency_password_reset.feature）。TTRS 新增，TipTop 無對應表；寄信對象為 user_account.email（zx09）

-- 連線版本：重設密碼後＋1，舊連線即失效（R6）
ALTER TABLE user_credential ADD COLUMN session_version integer NOT NULL DEFAULT 0;
COMMENT ON COLUMN user_credential.session_version IS 'TTRS 新增：連線版本，忘記密碼重設或 ttrs-admin 重設後＋1，舊連線失效（ADR 0010 R6）';

-- 重設連結的權杖：只存 SHA-256 雜湊（決策 A4），30 分鐘內有效、只能用一次（R3）
CREATE TABLE password_reset_token (
    token_hash  varchar(64) PRIMARY KEY,
    user_id     varchar(10) NOT NULL REFERENCES user_account (user_id) ON DELETE CASCADE,
    created_at  timestamptz NOT NULL,
    expires_at  timestamptz NOT NULL,
    used_at     timestamptz
);
CREATE INDEX password_reset_token_user_idx ON password_reset_token (user_id, created_at);
COMMENT ON TABLE password_reset_token IS 'TTRS 新增（TipTop 無對應）：忘記密碼的重設權杖；ADR 0010';
COMMENT ON COLUMN password_reset_token.token_hash IS '權杖的 SHA-256（十六進位）；權杖原文只出現在寄給使用者的連結';
COMMENT ON COLUMN password_reset_token.user_id IS 'zx01 使用者代號';
COMMENT ON COLUMN password_reset_token.created_at IS '申請時間（同一使用者 1 小時最多 3 次，R7）';
COMMENT ON COLUMN password_reset_token.expires_at IS '到期時間（申請後 30 分鐘，R3）';
COMMENT ON COLUMN password_reset_token.used_at IS '已使用或因重新申請、重設成功而失效的時間';

-- 程式代號（R10）：列入程式清單，登入前使用、不需權限設定（不給任何權限類別）
INSERT INTO program (program_no, module_code, program_type, created_by, created_dept)
VALUES ('forgot-password', 'AZZ', 'I', 'tiptop', '')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
VALUES ('forgot-password', 'zh-TW', '忘記密碼'),
       ('forgot-password', 'en', 'Forgot Password'),
       ('forgot-password', 'zh-CN', '忘记密码')
ON CONFLICT (program_no, lang) DO NOTHING;
