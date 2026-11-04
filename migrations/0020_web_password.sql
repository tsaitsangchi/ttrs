-- Web 密碼登入（docs/adr/0009-password-sign-in.md；specs/features/azz/web_password.feature）
-- TipTop: zx_file 的 zx10、zx16、zx17、zx19、zx20（weblogin、webpasswd、p_zx）；gbt_file 系統帳號稽核原則（p_passpriv）

-- 使用者的密碼狀態；沒有資料列的使用者尚未設定密碼，初始密碼＝使用者代號且須改密碼（W1）
CREATE TABLE user_credential (
    user_id                  varchar(10) PRIMARY KEY REFERENCES user_account (user_id) ON DELETE CASCADE,
    password_hash            text,
    password_changed_on      date,
    password_use_count       integer NOT NULL DEFAULT 0,
    password_change_required boolean NOT NULL DEFAULT true,
    failed_sign_in_count     integer NOT NULL DEFAULT 0
);
COMMENT ON TABLE user_credential IS 'TipTop: zx_file 的 Web 密碼欄位（zx10、zx16、zx17、zx19、zx20）；ADR 0009';
COMMENT ON COLUMN user_credential.password_hash IS 'zx10 Web 密碼：Argon2id 雜湊（PHC 字串），不存明文；TipTop 編碼值不轉入（W1）';
COMMENT ON COLUMN user_credential.password_changed_on IS 'zx16 密碼更新日期';
COMMENT ON COLUMN user_credential.password_use_count IS 'zx17 此密碼已登入次數（依次數到期時才計）';
COMMENT ON COLUMN user_credential.password_change_required IS 'zx19 下次登入須改密碼';
COMMENT ON COLUMN user_credential.failed_sign_in_count IS 'zx20 已試誤次數';

-- 系統帳號稽核原則（只有一筆，同 gbt00＝0）；起始值依 W5
CREATE TABLE account_policy (
    policy_key        varchar(1) PRIMARY KEY DEFAULT '0' CHECK (policy_key = '0'),
    expiry_basis      varchar(1) NOT NULL DEFAULT '1' CHECK (expiry_basis IN ('0', '1', '2')),
    cycle_days        integer NOT NULL DEFAULT 90 CHECK (cycle_days >= 0),
    use_limit         integer NOT NULL DEFAULT 0 CHECK (use_limit >= 0),
    notify            integer NOT NULL DEFAULT 7 CHECK (notify >= 0),
    min_length        integer NOT NULL DEFAULT 8 CHECK (min_length >= 0),
    complexity        varchar(1) NOT NULL DEFAULT 'M' CHECK (complexity IN ('N', 'Y', 'M', 'S')),
    allow_same_as_old boolean NOT NULL DEFAULT false,
    max_failures      integer NOT NULL DEFAULT 5 CHECK (max_failures >= 0),
    updated_by        text,
    updated_on        date
);
COMMENT ON TABLE account_policy IS 'TipTop: gbt_file 系統帳號稽核原則（p_passpriv）；ADR 0009';
COMMENT ON COLUMN account_policy.policy_key IS 'gbt00 鍵值（固定 0）';
COMMENT ON COLUMN account_policy.expiry_basis IS 'gbt01 密碼要求變更依據：0 不要求、1 依天數、2 依次數';
COMMENT ON COLUMN account_policy.cycle_days IS 'gbt02 密碼更新週期（天）';
COMMENT ON COLUMN account_policy.use_limit IS 'gbt03 密碼可用次數';
COMMENT ON COLUMN account_policy.notify IS 'gbt04 提前通知天數（依次數時為剩餘次數）';
COMMENT ON COLUMN account_policy.min_length IS 'gbt05 密碼最小長度';
COMMENT ON COLUMN account_policy.complexity IS 'gbt06 密碼複雜度：N 不限、Y 不可含帳號、M 另須含數字、S 另須含符號';
COMMENT ON COLUMN account_policy.allow_same_as_old IS 'gbt08 更換密碼可否與原密碼相同';
COMMENT ON COLUMN account_policy.max_failures IS 'gbt10 允許最大試誤次數（0 不鎖定）';
COMMENT ON COLUMN account_policy.updated_by IS 'gbtmodu 資料修改人';
COMMENT ON COLUMN account_policy.updated_on IS 'gbtdate 資料修改日';
INSERT INTO account_policy (policy_key) VALUES ('0');

-- 使用者維護的解除鎖定（p_zx user_unlock）：同 PORTED_PROGRAMS，ADMIN 有全部功能
UPDATE program
   SET default_actions = default_actions || ', user_unlock'
 WHERE program_no = 'user-maintenance'
   AND default_actions NOT LIKE '%user_unlock%';
UPDATE program_permission
   SET actions = array_append(actions, 'user_unlock')
 WHERE class_code = 'ADMIN'
   AND program_no = 'user-maintenance'
   AND NOT ('user_unlock' = ANY (actions));
