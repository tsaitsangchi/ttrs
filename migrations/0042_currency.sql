-- 幣別維護（TipTop aooi050 / azi_file；specs/features/aoo/currency_maintenance.feature CY1–CY8）
-- 共用 schema（CY2）；標準欄位與觸發器（ADR 0012）、確認欄位（ADR 0013 SA9）

CREATE TABLE currency (
    currency_code   varchar(4) PRIMARY KEY CHECK (char_length(currency_code) >= 1),
    name            varchar(80),
    price_decimals  smallint NOT NULL DEFAULT 4 CHECK (price_decimals BETWEEN 0 AND 6),
    amount_decimals smallint NOT NULL DEFAULT 2 CHECK (amount_decimals BETWEEN 0 AND 6),
    total_decimals  smallint NOT NULL DEFAULT 2 CHECK (total_decimals BETWEEN 0 AND 6),
    rate_decimals   smallint NOT NULL DEFAULT 6 CHECK (rate_decimals BETWEEN 0 AND 10),
    rate_method     char(1) NOT NULL DEFAULT '1' CHECK (rate_method IN ('1', '2')),
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
    updated_at      timestamptz
);
COMMENT ON TABLE currency IS 'TipTop: azi_file 幣別檔（aooi050 幣別資料維護）；共用 schema（CY2）';
COMMENT ON COLUMN currency.currency_code IS 'azi01 幣別代碼（區分大小寫、建立後不可修改，CY5）';
COMMENT ON COLUMN currency.name IS 'azi02 幣別說明';
COMMENT ON COLUMN currency.price_decimals IS 'azi03 單價（成本）小數位數（0–6，預設 4，CY3）';
COMMENT ON COLUMN currency.amount_decimals IS 'azi04 金額小數位數（0–6，預設 2）';
COMMENT ON COLUMN currency.total_decimals IS 'azi05 小計、總計小數位數（0–6，預設 2）';
COMMENT ON COLUMN currency.rate_decimals IS 'azi07 匯率小數位數（0–10，預設 6）';
COMMENT ON COLUMN currency.rate_method IS 'azi10 計算方式：1 乘、2 除（預設 1，CY4）';
COMMENT ON COLUMN currency.active IS 'aziacti 資料有效碼';
COMMENT ON COLUMN currency.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9；azi_file 無對應欄位）';
COMMENT ON COLUMN currency.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN currency.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN currency.created_by IS 'aziuser 資料所有者';
COMMENT ON COLUMN currency.created_dept IS 'azigrup 資料所有部門';
COMMENT ON COLUMN currency.updated_by IS 'azimodu 資料修改者';
COMMENT ON COLUMN currency.updated_on IS 'azidate 最近修改日';
COMMENT ON COLUMN currency.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TRIGGER currency_version BEFORE UPDATE ON currency FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER currency_change_log AFTER INSERT OR UPDATE OR DELETE ON currency
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('currency_code');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'currency-maintenance', 'AOO', 'I', 'aooi050', '1',
       'query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'currency-maintenance',
       ARRAY['query', 'detail', 'history', 'reproduce', 'confirm', 'notconfirm', 'output', 'exporttoexcel'],
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'currency-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('currency-maintenance', 'zh-TW', '幣別維護'), ('currency-maintenance', 'en', 'Currencies'),
               ('currency-maintenance', 'zh-CN', '币别维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'currency-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'currency-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
