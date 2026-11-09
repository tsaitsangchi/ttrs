-- 單位維護（TipTop aooi101 / gfe_file；specs/features/aoo/unit_maintenance.feature UN1–UN7）
-- 沿用 0001 的 unit 表（共用 schema，UN2），補齊 gfe_file 欄位與標準欄位（ADR 0012）

ALTER TABLE unit
    ALTER COLUMN name DROP NOT NULL,
    ADD COLUMN decimals     smallint NOT NULL DEFAULT 0 CHECK (decimals BETWEEN 0 AND 6),
    ADD COLUMN active       boolean NOT NULL DEFAULT true,
    ADD COLUMN created_by   text NOT NULL DEFAULT '',
    ADD COLUMN created_dept text NOT NULL DEFAULT '',
    ADD COLUMN created_on   date NOT NULL DEFAULT CURRENT_DATE,
    ADD COLUMN updated_by   text,
    ADD COLUMN updated_on   date,
    ADD COLUMN version      integer NOT NULL DEFAULT 1,
    ADD COLUMN created_at   timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN updated_at   timestamptz;
-- UN3：最多 4 字（既有資料不檢查）
ALTER TABLE unit ADD CONSTRAINT unit_code_length CHECK (char_length(unit_code) BETWEEN 1 AND 4) NOT VALID;
COMMENT ON TABLE unit IS 'TipTop: gfe_file 單位檔（aooi101 單位資料維護）；共用 schema（UN2）';
COMMENT ON COLUMN unit.unit_code IS 'gfe01 單位（區分大小寫、建立後不可修改，UN3）';
COMMENT ON COLUMN unit.name IS 'gfe02 說明';
COMMENT ON COLUMN unit.decimals IS 'gfe03 小數位數（0–6，UN4）';
COMMENT ON COLUMN unit.active IS 'gfeacti 資料有效碼';
COMMENT ON COLUMN unit.created_by IS 'gfeuser 資料所有者';
COMMENT ON COLUMN unit.created_dept IS 'gfegrup 資料所有部門';
COMMENT ON COLUMN unit.updated_by IS 'gfemodu 資料修改者';
COMMENT ON COLUMN unit.updated_on IS 'gfedate 最近修改日';
COMMENT ON COLUMN unit.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TRIGGER unit_version BEFORE UPDATE ON unit FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER unit_change_log AFTER INSERT OR UPDATE OR DELETE ON unit
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('unit_code');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
VALUES ('unit-maintenance', 'AOO', 'I', 'aooi101', '1', 'query, detail, history', 'tiptop', '')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'unit-maintenance', ARRAY['query', 'detail', 'history'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('unit-maintenance', 'zh-TW', '單位維護'), ('unit-maintenance', 'en', 'Units'),
               ('unit-maintenance', 'zh-CN', '单位维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'unit-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'unit-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
