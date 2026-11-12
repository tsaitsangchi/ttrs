-- 整體管理系統參數（TipTop aoos010 / aza_file；specs/features/aoo/general_parameters.feature GP1–GP10）
-- 沿用 migration 0045 建立的 system_parameter（共用 schema、只有一筆，GP2）；本批只加會計期間 aza02、匯率採用 aza19（GP3）
-- 與 ADR 0012 標準欄位。本國幣別 base_currency 不加外鍵（幣別資料轉入前，同 ER2），由程式檢查（GP5）。

ALTER TABLE system_parameter
    ADD COLUMN accounting_periods smallint NOT NULL DEFAULT 12 CHECK (accounting_periods IN (12, 13)),
    ADD COLUMN rate_basis         varchar(7) NOT NULL DEFAULT 'daily' CHECK (rate_basis IN ('monthly', 'daily')),
    ADD COLUMN created_by         text NOT NULL DEFAULT 'tiptop',
    ADD COLUMN created_dept       text NOT NULL DEFAULT '',
    ADD COLUMN created_on         date NOT NULL DEFAULT CURRENT_DATE,
    ADD COLUMN updated_by         text,
    ADD COLUMN updated_on         date;

COMMENT ON TABLE system_parameter IS 'TipTop: aza_file 整體管理系統參數檔（aoos010，general-management-parameters）；只有一列（aza01 = 0）；共用 schema';
COMMENT ON COLUMN system_parameter.accounting_periods IS 'aza02 會計期間：12 或 13 期（TipTop 1 = 12 期、2 = 13 期；GP4）';
COMMENT ON COLUMN system_parameter.rate_basis IS 'aza19 匯率採用：monthly 每月（TipTop 1 月平均）、daily 每日（TipTop 2；GP6）';
COMMENT ON COLUMN system_parameter.created_by IS 'azauser 資料所有者';
COMMENT ON COLUMN system_parameter.created_dept IS 'azagrup 資料所有群';
COMMENT ON COLUMN system_parameter.created_on IS '建立日期（TTRS 新增）';
COMMENT ON COLUMN system_parameter.updated_by IS 'azamodu 資料修改者';
COMMENT ON COLUMN system_parameter.updated_on IS 'azadate 最近修改日';

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'general-management-parameters', 'AOO', 'S', 'aoos010', '1',
       'query, modify, history, output, exporttoexcel', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'general-management-parameters',
       ARRAY['query', 'modify', 'history', 'output', 'exporttoexcel'],
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'general-management-parameters')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('general-management-parameters', 'zh-TW', '整體管理系統參數'),
               ('general-management-parameters', 'en', 'General Management Parameters'),
               ('general-management-parameters', 'zh-CN', '整体管理系统参数')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'general-management-parameters' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'general-management-parameters')
ON CONFLICT (menu_no, program_no) DO NOTHING;
