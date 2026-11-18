-- 集團中心維護（TipTop aooi600 / geu_file 集團中心資料檔、gev_file 資料控管維護檔；specs/features/aoo/group_center.feature GC1–GC11）
-- 集團層，放共用 schema（GC2，ADR 0007 §8）。gev_file 只移入 gev03＝Y 的列（資料中心負責的資料類別）；
-- gev03＝N 的列（各營運中心受哪個資料中心管控）在 aooi601 移植時另外決定放法。

CREATE TABLE group_center (
    center_code   text PRIMARY KEY CHECK (char_length(center_code) BETWEEN 1 AND 10),
    name          varchar(80),
    category      char(1) NOT NULL CHECK (category BETWEEN '1' AND '7'),
    active        boolean NOT NULL DEFAULT true,
    confirmed     boolean NOT NULL DEFAULT false,
    confirmed_by  varchar(10),
    confirmed_at  timestamptz,
    created_by    text NOT NULL,
    created_dept  text NOT NULL DEFAULT '',
    created_on    date NOT NULL DEFAULT CURRENT_DATE,
    updated_by    text,
    updated_on    date,
    version       integer NOT NULL DEFAULT 1,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz
);
COMMENT ON TABLE group_center IS 'TipTop: geu_file 集團中心資料檔（aooi600 集團中心資料維護，group-center-maintenance）；共用 schema、集團層（GC2）';
COMMENT ON COLUMN group_center.center_code IS 'geu01 集團中心代碼（區分大小寫、最多 10 字、建立後不可修改，GC3）';
COMMENT ON COLUMN group_center.name IS 'geu02 集團中心名稱';
COMMENT ON COLUMN group_center.category IS 'geu00 集團中心類別：1 資料中心、2 預測中心、3 訂單中心、4 採購中心、5 信用控管中心、6 帳務中心、7 資金中心（GC4）';
COMMENT ON COLUMN group_center.active IS 'geuacti 資料有效碼';
COMMENT ON COLUMN group_center.confirmed IS 'TTRS 標準確認欄位（ADR 0013 SA9；geu_file 無對應欄位）';
COMMENT ON COLUMN group_center.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN group_center.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN group_center.created_by IS 'geuuser 資料所有者';
COMMENT ON COLUMN group_center.created_dept IS 'geugrup 資料所有群';
COMMENT ON COLUMN group_center.updated_by IS 'geumodu 資料更改者';
COMMENT ON COLUMN group_center.updated_on IS 'geudate 最近修改日';
COMMENT ON COLUMN group_center.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TABLE data_center_type (
    data_type    char(1) NOT NULL CHECK (data_type BETWEEN '0' AND '8'),
    plant_id     text NOT NULL REFERENCES plant (plant_id),
    center_code  text NOT NULL REFERENCES group_center (center_code) ON DELETE CASCADE,
    PRIMARY KEY (data_type, plant_id)
);
CREATE INDEX data_center_type_center ON data_center_type (center_code);
COMMENT ON TABLE data_center_type IS 'TipTop: gev_file 資料控管維護檔中 gev03＝Y 的列（aooi600 資料類別設定 aooi600_1）：資料中心負責的資料類別與資料來源營運中心；同一資料類別＋營運中心全集團只能屬於一個資料中心（GC5）';
COMMENT ON COLUMN data_center_type.data_type IS 'gev01 資料類別：0 基本資料、1 料件、2 BOM、3 ECN、4 客戶、5 供應商、6 會計科目、7 價格、8 匯率';
COMMENT ON COLUMN data_center_type.plant_id IS 'gev02 營運中心（須存在於 plant，aooi600.4gl:805-811）';
COMMENT ON COLUMN data_center_type.center_code IS 'gev04 資料中心代碼（gev03 資料中心否＝Y 以存在於本表表示）';

CREATE TRIGGER group_center_version BEFORE UPDATE ON group_center
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER group_center_change_log AFTER INSERT OR UPDATE OR DELETE ON group_center
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('center_code');
CREATE TRIGGER data_center_type_change_log AFTER INSERT OR UPDATE OR DELETE ON data_center_type
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('center_code,data_type,plant_id');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'group-center-maintenance', 'AOO', 'I', 'aooi600', '1',
       'insert, query, modify, delete, invalid, reproduce, data_type_setting, history, confirm, notconfirm, output, exporttoexcel',
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'group-center-maintenance',
       ARRAY['insert', 'query', 'modify', 'delete', 'invalid', 'reproduce', 'data_type_setting', 'history', 'confirm',
             'notconfirm', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'group-center-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('group-center-maintenance', 'zh-TW', '集團中心維護'), ('group-center-maintenance', 'en', 'Group Centers'),
               ('group-center-maintenance', 'zh-CN', '集团中心维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'group-center-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'group-center-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
