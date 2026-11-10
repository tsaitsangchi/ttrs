-- 稅別維護（TipTop aooi150 / gec_file；specs/features/aoo/tax_type_maintenance.feature TX1–TX9）
-- 共用 schema（TX2）；只做台灣版（TX3）；標準欄位與觸發器（ADR 0012）、確認欄位（ADR 0013 SA9）

CREATE TABLE tax_type (
    tax_code       varchar(4) NOT NULL CHECK (char_length(tax_code) >= 1),
    direction      char(1) NOT NULL CHECK (direction IN ('1', '2')),
    name           varchar(80),
    account        varchar(24),
    account2       varchar(24),
    invoice_copies char(1) CHECK (invoice_copies IN ('0', '2', '3', '4', 'X')),
    tax_class      char(1) CHECK (tax_class IN ('1', '2', '3')),
    rate           numeric(9,4) NOT NULL DEFAULT 0 CHECK (rate BETWEEN 0 AND 100),
    include_tax    boolean NOT NULL DEFAULT false,
    filing_format  varchar(2) NOT NULL DEFAULT 'XX',
    active         boolean NOT NULL DEFAULT true,
    confirmed      boolean NOT NULL DEFAULT false,
    confirmed_by   varchar(10),
    confirmed_at   timestamptz,
    created_by     text NOT NULL,
    created_dept   text NOT NULL DEFAULT '',
    created_on     date NOT NULL DEFAULT CURRENT_DATE,
    updated_by     text,
    updated_on     date,
    version        integer NOT NULL DEFAULT 1,
    created_at     timestamptz NOT NULL DEFAULT now(),
    updated_at     timestamptz,
    PRIMARY KEY (tax_code, direction),
    -- 申報格式依進銷項（aooi150.4gl:423-447）
    CHECK (filing_format = 'XX'
        OR (direction = '1' AND filing_format IN ('21', '22', '23', '24', '25', '26', '27', '28', '29'))
        OR (direction = '2' AND filing_format IN ('31', '32', '33', '34', '35', '36', '37', '38')))
);
COMMENT ON TABLE tax_type IS 'TipTop: gec_file 稅別檔（aooi150 稅別資料維護）；共用 schema（TX2）';
COMMENT ON COLUMN tax_type.tax_code IS 'gec01 稅別碼（區分大小寫、建立後不可修改，TX8）';
COMMENT ON COLUMN tax_type.direction IS 'gec011 進／銷項：1 進項、2 銷項';
COMMENT ON COLUMN tax_type.name IS 'gec02 稅別名稱';
COMMENT ON COLUMN tax_type.account IS 'gec03 會計科目編號（主帳套；會計科目移植前不檢查，TX4）';
COMMENT ON COLUMN tax_type.account2 IS 'gec031 會計科目二（第二帳套）';
COMMENT ON COLUMN tax_type.invoice_copies IS 'gec05 發票聯數：0 園區收據、2 二聯、3 三聯、4 收據、X 不申報（台灣版，TX3）';
COMMENT ON COLUMN tax_type.tax_class IS 'gec06 課稅別：1 應稅、2 零稅率、3 免稅';
COMMENT ON COLUMN tax_type.rate IS 'gec04 稅率（%，0–100，TX5）；零稅率、免稅為 0';
COMMENT ON COLUMN tax_type.include_tax IS 'gec07 單價含稅否（二聯式且申報格式不是 28 時須含稅）';
COMMENT ON COLUMN tax_type.filing_format IS 'gec08 媒體申報格式：進項 21–29、銷項 31–38、XX 不申報（預設，TX6）';
COMMENT ON COLUMN tax_type.active IS 'gecacti 資料有效碼';
COMMENT ON COLUMN tax_type.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9；gec_file 無對應欄位）';
COMMENT ON COLUMN tax_type.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN tax_type.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN tax_type.created_by IS 'gecuser 資料所有者';
COMMENT ON COLUMN tax_type.created_dept IS 'gecgrup 資料所有部門';
COMMENT ON COLUMN tax_type.updated_by IS 'gecmodu 資料修改者';
COMMENT ON COLUMN tax_type.updated_on IS 'gecdate 最近修改日';
COMMENT ON COLUMN tax_type.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TRIGGER tax_type_version BEFORE UPDATE ON tax_type FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER tax_type_change_log AFTER INSERT OR UPDATE OR DELETE ON tax_type
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('tax_code,direction');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'tax-type-maintenance', 'AOO', 'I', 'aooi150', '1',
       'query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'tax-type-maintenance',
       ARRAY['query', 'detail', 'history', 'reproduce', 'confirm', 'notconfirm', 'output', 'exporttoexcel'],
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'tax-type-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('tax-type-maintenance', 'zh-TW', '稅別維護'), ('tax-type-maintenance', 'en', 'Tax Types'),
               ('tax-type-maintenance', 'zh-CN', '税别维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'tax-type-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'tax-type-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
