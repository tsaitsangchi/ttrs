-- 匯率（TipTop aooi060 / azj_file、aooi070 / azk_file、aoop010；
--   specs/features/aoo/exchange_rate_monthly.feature、exchange_rate_daily.feature、exchange_rate_daily_copy.feature）
-- 共用 schema（ER1）；標準欄位與觸發器（ADR 0012）、確認欄位（ADR 0013 SA9）。另建系統參數表放本國幣別（ER2）

-- 系統參數（aza_file 整體管理系統參數；本批只有本國幣別 aza17，其他參數與維護畫面隨批次 3 aoos010）
CREATE TABLE system_parameter (
    parameter_key   char(1) PRIMARY KEY DEFAULT '0' CHECK (parameter_key = '0'),
    base_currency   varchar(4),
    version         integer NOT NULL DEFAULT 1,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz
);
COMMENT ON TABLE system_parameter IS 'TipTop: aza_file 整體管理系統參數檔（aoos010）；只有一列（aza01 = 0）；共用 schema';
COMMENT ON COLUMN system_parameter.parameter_key IS 'aza01 KEY VALUE（固定 0）';
COMMENT ON COLUMN system_parameter.base_currency IS 'aza17 本國幣別（ER2；幣別資料轉入前不加外鍵）';
COMMENT ON COLUMN system_parameter.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
CREATE TRIGGER system_parameter_version BEFORE UPDATE ON system_parameter
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER system_parameter_change_log AFTER INSERT OR UPDATE OR DELETE ON system_parameter
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('parameter_key');
INSERT INTO system_parameter (parameter_key, base_currency) VALUES ('0', 'TWD');

-- 每月匯率（azj_file）
CREATE TABLE monthly_exchange_rate (
    currency_code   varchar(4) NOT NULL REFERENCES currency (currency_code),
    period          char(6) NOT NULL CHECK (period ~ '^[0-9]{4}(0[1-9]|1[0-2])$'),
    bank_buy        numeric(20,10) NOT NULL DEFAULT 0 CHECK (bank_buy >= 0),
    bank_sell       numeric(20,10) NOT NULL DEFAULT 0 CHECK (bank_sell >= 0),
    bank_mid        numeric(20,10) NOT NULL DEFAULT 0 CHECK (bank_mid >= 0),
    customs_period  numeric(20,10) NOT NULL DEFAULT 0 CHECK (customs_period >= 0),
    customs_buy     numeric(20,10) NOT NULL DEFAULT 0 CHECK (customs_buy >= 0),
    customs_sell    numeric(20,10) NOT NULL DEFAULT 0 CHECK (customs_sell >= 0),
    revaluation     numeric(20,10) NOT NULL DEFAULT 0 CHECK (revaluation >= 0),
    active          boolean NOT NULL DEFAULT true,
    confirmed       boolean NOT NULL DEFAULT false,
    confirmed_by    varchar(10),
    confirmed_at    timestamptz,
    created_by      text NOT NULL,
    created_dept    text NOT NULL DEFAULT '',
    created_on      date NOT NULL DEFAULT CURRENT_DATE,
    updated_by      text,
    updated_on      date,
    version         integer NOT NULL DEFAULT 1,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz,
    PRIMARY KEY (currency_code, period)
);
COMMENT ON TABLE monthly_exchange_rate IS 'TipTop: azj_file 幣別每月匯率檔（aooi060 幣別每月匯率維護）；共用 schema（ER1）';
COMMENT ON COLUMN monthly_exchange_rate.currency_code IS 'azj01 幣別代碼（須存在且有效，MR5）';
COMMENT ON COLUMN monthly_exchange_rate.period IS 'azj02 年月 YYYYMM（MR2）';
COMMENT ON COLUMN monthly_exchange_rate.bank_buy IS 'azj03 銀行買入匯率（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN monthly_exchange_rate.bank_sell IS 'azj04 銀行賣出匯率（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN monthly_exchange_rate.bank_mid IS 'azj041 銀行中價匯率（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN monthly_exchange_rate.customs_period IS 'azj05 海關旬匯率（月平均，月底重評價寫入；.sch 為「月底採購匯率」，MR10）（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN monthly_exchange_rate.customs_buy IS 'azj051 海關買入匯率（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN monthly_exchange_rate.customs_sell IS 'azj052 海關賣出匯率（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN monthly_exchange_rate.active IS 'azjacti 資料有效碼（停用的不可修改，MR6）';
COMMENT ON COLUMN monthly_exchange_rate.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9）';
COMMENT ON COLUMN monthly_exchange_rate.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN monthly_exchange_rate.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN monthly_exchange_rate.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN monthly_exchange_rate.revaluation IS 'azj07 月底重評價匯率（月底重評價取當天銀行中價，MR7）';
COMMENT ON COLUMN monthly_exchange_rate.created_by IS 'azjuser 資料所有者';
COMMENT ON COLUMN monthly_exchange_rate.created_dept IS 'azjgrup 資料所有部門';
COMMENT ON COLUMN monthly_exchange_rate.updated_by IS 'azjmodu 資料修改者';
COMMENT ON COLUMN monthly_exchange_rate.updated_on IS 'azjdate 最近修改日';
CREATE TRIGGER monthly_exchange_rate_version BEFORE UPDATE ON monthly_exchange_rate
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER monthly_exchange_rate_change_log AFTER INSERT OR UPDATE OR DELETE ON monthly_exchange_rate
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('currency_code,period');

-- 每日匯率（azk_file；9999/12/31 最近匯率列不移植，ER4）
CREATE TABLE daily_exchange_rate (
    currency_code   varchar(4) NOT NULL REFERENCES currency (currency_code),
    rate_date       date NOT NULL,
    bank_buy        numeric(20,10) NOT NULL DEFAULT 0 CHECK (bank_buy >= 0),
    bank_sell       numeric(20,10) NOT NULL DEFAULT 0 CHECK (bank_sell >= 0),
    bank_mid        numeric(20,10) NOT NULL DEFAULT 0 CHECK (bank_mid >= 0),
    customs_period  numeric(20,10) NOT NULL DEFAULT 0 CHECK (customs_period >= 0),
    customs_buy     numeric(20,10) NOT NULL DEFAULT 0 CHECK (customs_buy >= 0),
    customs_sell    numeric(20,10) NOT NULL DEFAULT 0 CHECK (customs_sell >= 0),
    active          boolean NOT NULL DEFAULT true,
    confirmed       boolean NOT NULL DEFAULT false,
    confirmed_by    varchar(10),
    confirmed_at    timestamptz,
    created_by      text NOT NULL,
    created_dept    text NOT NULL DEFAULT '',
    created_on      date NOT NULL DEFAULT CURRENT_DATE,
    updated_by      text,
    updated_on      date,
    version         integer NOT NULL DEFAULT 1,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz,
    PRIMARY KEY (currency_code, rate_date)
);
COMMENT ON TABLE daily_exchange_rate IS 'TipTop: azk_file 幣別每日匯率檔（aooi070 幣別每日匯率維護、aoop010 每日匯率複製）；共用 schema（ER1）';
COMMENT ON COLUMN daily_exchange_rate.currency_code IS 'azk01 幣別代碼（須存在且有效）';
COMMENT ON COLUMN daily_exchange_rate.rate_date IS 'azk02 日期（9999/12/31 最近匯率列不移植，ER4）';
COMMENT ON COLUMN daily_exchange_rate.bank_buy IS 'azk03 銀行買入匯率（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN daily_exchange_rate.bank_sell IS 'azk04 銀行賣出匯率（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN daily_exchange_rate.bank_mid IS 'azk041 銀行中價匯率（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN daily_exchange_rate.customs_period IS 'azk05 海關旬匯率（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN daily_exchange_rate.customs_buy IS 'azk051 海關買入匯率（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN daily_exchange_rate.customs_sell IS 'azk052 海關賣出匯率（Decimal，不可小於 0；本國幣別一律 1）';
COMMENT ON COLUMN daily_exchange_rate.active IS 'TTRS 新增資料有效碼（azk_file 沒有；停用的不算進月平均、不被取用，DR3）';
COMMENT ON COLUMN daily_exchange_rate.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9）';
COMMENT ON COLUMN daily_exchange_rate.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN daily_exchange_rate.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN daily_exchange_rate.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN daily_exchange_rate.created_by IS 'TTRS 新增：資料所有者（azk_file 沒有）';
COMMENT ON COLUMN daily_exchange_rate.created_dept IS 'TTRS 新增：資料所有部門';
CREATE INDEX daily_exchange_rate_date ON daily_exchange_rate (rate_date);
CREATE TRIGGER daily_exchange_rate_version BEFORE UPDATE ON daily_exchange_rate
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER daily_exchange_rate_change_log AFTER INSERT OR UPDATE OR DELETE ON daily_exchange_rate
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('currency_code,rate_date');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊；側欄：整體管理之下

INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'monthly-exchange-rate-maintenance', 'AOO', 'I', 'aooi060', '1',
       'insert, query, modify, delete, reproduce, invalid, history, confirm, notconfirm, output, exporttoexcel, re_rate', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'monthly-exchange-rate-maintenance',
       ARRAY['insert', 'query', 'modify', 'delete', 'reproduce', 'invalid', 'history', 'confirm', 'notconfirm', 'output', 'exporttoexcel', 're_rate'],
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'monthly-exchange-rate-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('monthly-exchange-rate-maintenance', 'zh-TW', '每月匯率維護'), ('monthly-exchange-rate-maintenance', 'en', 'Monthly Exchange Rates'),
               ('monthly-exchange-rate-maintenance', 'zh-CN', '每月汇率维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'monthly-exchange-rate-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'monthly-exchange-rate-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;

INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'daily-exchange-rate-maintenance', 'AOO', 'I', 'aooi070', '1',
       'insert, query, modify, delete, reproduce, invalid, history, confirm, notconfirm, output, exporttoexcel, unify_rate', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'daily-exchange-rate-maintenance',
       ARRAY['insert', 'query', 'modify', 'delete', 'reproduce', 'invalid', 'history', 'confirm', 'notconfirm', 'output', 'exporttoexcel', 'unify_rate'],
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'daily-exchange-rate-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('daily-exchange-rate-maintenance', 'zh-TW', '每日匯率維護'), ('daily-exchange-rate-maintenance', 'en', 'Daily Exchange Rates'),
               ('daily-exchange-rate-maintenance', 'zh-CN', '每日汇率维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'daily-exchange-rate-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'daily-exchange-rate-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;

INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'daily-exchange-rate-copy', 'AOO', 'P', 'aoop010', '1',
       'run, output, exporttoexcel', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'daily-exchange-rate-copy',
       ARRAY['run', 'output', 'exporttoexcel'],
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'daily-exchange-rate-copy')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('daily-exchange-rate-copy', 'zh-TW', '每日匯率複製'), ('daily-exchange-rate-copy', 'en', 'Daily Exchange Rate Copy'),
               ('daily-exchange-rate-copy', 'zh-CN', '每日汇率复制')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'daily-exchange-rate-copy' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'daily-exchange-rate-copy')
ON CONFLICT (menu_no, program_no) DO NOTHING;
