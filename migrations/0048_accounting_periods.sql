-- 會計期間（TipTop aoos020 / azm_file；specs/features/aoo/accounting_periods.feature PD1–PD12）
-- 共用 schema、全公司一份（PD2）。azm_file 一年一列 41 欄改為表頭 accounting_year＋明細 accounting_period（一期一列）。
-- 每日明細 azn_file 不另存，由程式即時計算（PD3）。標準欄位與觸發器（ADR 0012）、確認欄位（ADR 0013 SA9）；沒有資料有效（PD9）。

CREATE TABLE accounting_year (
    year            integer PRIMARY KEY,
    period_count    smallint NOT NULL CHECK (period_count IN (12, 13)),
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
COMMENT ON TABLE accounting_year IS 'TipTop: azm_file 會計期間參數設定檔（aoos020，accounting-period-maintenance）表頭；共用 schema';
COMMENT ON COLUMN accounting_year.year IS 'azm01 會計年度';
COMMENT ON COLUMN accounting_year.period_count IS 'azm02 會計期間分類方式：12 或 13 期（TipTop 1 = 12 期、2 = 13 期；新增時取 aza02，PD6）';
COMMENT ON COLUMN accounting_year.confirmed IS 'TTRS 標準確認欄位（ADR 0013 SA9；azm_file 無對應欄位）；已確認不可修改、刪除（PD8）';
COMMENT ON COLUMN accounting_year.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TABLE accounting_period (
    year            integer NOT NULL REFERENCES accounting_year (year) ON DELETE CASCADE,
    period_no       smallint NOT NULL CHECK (period_no BETWEEN 1 AND 13),
    start_date      date NOT NULL,
    end_date        date NOT NULL CHECK (end_date >= start_date),
    quarter         smallint NOT NULL CHECK (quarter BETWEEN 1 AND 4),
    PRIMARY KEY (year, period_no)
);
COMMENT ON TABLE accounting_period IS 'TipTop: azm_file 各期（azm0n1 起始日、azm0n2 截止日、azm0n3 季別，n = 01–13）；一期一列';
COMMENT ON COLUMN accounting_period.period_no IS '期別 1–13';
COMMENT ON COLUMN accounting_period.start_date IS 'azm0n1 第 n 期起始日期';
COMMENT ON COLUMN accounting_period.end_date IS 'azm0n2 第 n 期截止日期';
COMMENT ON COLUMN accounting_period.quarter IS 'azm0n3 季別';
CREATE INDEX accounting_period_dates ON accounting_period (start_date, end_date);

CREATE TRIGGER accounting_year_version BEFORE UPDATE ON accounting_year
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER accounting_year_change_log AFTER INSERT OR UPDATE OR DELETE ON accounting_year
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('year');
CREATE TRIGGER accounting_period_change_log AFTER INSERT OR UPDATE OR DELETE ON accounting_period
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('year,period_no');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'accounting-period-maintenance', 'AOO', 'S', 'aoos020', '1',
       'insert, query, modify, delete, reproduce, confirm, notconfirm, output, exporttoexcel, history', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'accounting-period-maintenance',
       ARRAY['insert', 'query', 'modify', 'delete', 'reproduce', 'confirm', 'notconfirm', 'output', 'exporttoexcel', 'history'],
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'accounting-period-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('accounting-period-maintenance', 'zh-TW', '會計期間維護'),
               ('accounting-period-maintenance', 'en', 'Accounting Periods'),
               ('accounting-period-maintenance', 'zh-CN', '会计期间维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'accounting-period-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'accounting-period-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
