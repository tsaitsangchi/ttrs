-- 單位換算（TipTop aooi102 / smc_file；specs/features/aoo/unit_conversion.feature UC1–UC12）
-- 共用 schema（UC2）。一組單位只存一筆，A→B 與 B→A 視為同一組（UC3，由程式檢查）。料件換算 smd_file 在工廠 schema（migrations/plant/0015）。

CREATE TABLE unit_conversion (
    from_unit       text NOT NULL REFERENCES unit (unit_code),
    to_unit         text NOT NULL REFERENCES unit (unit_code),
    from_quantity   numeric(15,3) NOT NULL CHECK (from_quantity > 0),
    to_quantity     numeric(15,3) NOT NULL CHECK (to_quantity > 0),
    note            varchar(80),
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
    PRIMARY KEY (from_unit, to_unit),
    CHECK (from_unit <> to_unit)
);
COMMENT ON TABLE unit_conversion IS 'TipTop: smc_file 單位換算資料檔（aooi102，unit-conversion-maintenance）；共用 schema；一組單位一筆（UC3）';
COMMENT ON COLUMN unit_conversion.from_unit IS 'smc01 來源計算單位';
COMMENT ON COLUMN unit_conversion.to_unit IS 'smc02 目的計算單位';
COMMENT ON COLUMN unit_conversion.from_quantity IS 'smc03 來源單位兌換數量（> 0，UC4）';
COMMENT ON COLUMN unit_conversion.to_quantity IS 'smc04 目的單位兌換數量（> 0，UC4）';
COMMENT ON COLUMN unit_conversion.note IS 'smc05 說明';
COMMENT ON COLUMN unit_conversion.active IS 'smcacti 資料有效碼';
COMMENT ON COLUMN unit_conversion.confirmed IS 'TTRS 標準確認欄位（ADR 0013 SA9；smc_file 無對應欄位）';
COMMENT ON COLUMN unit_conversion.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TRIGGER unit_conversion_version BEFORE UPDATE ON unit_conversion
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER unit_conversion_change_log AFTER INSERT OR UPDATE OR DELETE ON unit_conversion
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('from_unit,to_unit');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT v.program_no, 'AOO', 'I', v.tiptop, '1', v.actions, 'tiptop', ''
  FROM (VALUES ('unit-conversion-maintenance', 'aooi102',
                'query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel'),
               ('item-unit-conversion-maintenance', 'aooi103',
                'query, detail, history, output, exporttoexcel')) AS v(program_no, tiptop, actions)
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', v.program_no, v.actions, 'tiptop', ''
  FROM (VALUES ('unit-conversion-maintenance',
                ARRAY['query', 'detail', 'history', 'reproduce', 'confirm', 'notconfirm', 'output', 'exporttoexcel']),
               ('item-unit-conversion-maintenance',
                ARRAY['query', 'detail', 'history', 'output', 'exporttoexcel'])) AS v(program_no, actions)
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program p WHERE p.program_no = v.program_no)
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('unit-conversion-maintenance', 'zh-TW', '單位換算維護'),
               ('unit-conversion-maintenance', 'en', 'Unit Conversions'),
               ('unit-conversion-maintenance', 'zh-CN', '单位换算维护'),
               ('item-unit-conversion-maintenance', 'zh-TW', '料件單位換算維護'),
               ('item-unit-conversion-maintenance', 'en', 'Item Unit Conversions'),
               ('item-unit-conversion-maintenance', 'zh-CN', '料件单位换算维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'unit-conversion-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'unit-conversion-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'item-unit-conversion-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'item-unit-conversion-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
