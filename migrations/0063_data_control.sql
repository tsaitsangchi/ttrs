-- 資料控管原則維護（TipTop aooi601 / gev_file 中 gev03＝N 的列、gey_file；specs/features/aoo/data_control.feature DC1–DC11）
-- 集團層，放共用 schema（DC2）。資料中心的營運中心沿用 data_center_type（0062），兩張表合起來「資料類別＋營運中心」只能出現一次。

CREATE TABLE data_control_plant (
    data_type     char(1) NOT NULL CHECK (data_type BETWEEN '1' AND '8'),
    plant_id      text NOT NULL REFERENCES plant (plant_id),
    created_by    text NOT NULL,
    created_dept  text NOT NULL DEFAULT '',
    created_on    date NOT NULL DEFAULT CURRENT_DATE,
    updated_by    text,
    updated_on    date,
    version       integer NOT NULL DEFAULT 1,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz,
    PRIMARY KEY (data_type, plant_id)
);
COMMENT ON TABLE data_control_plant IS 'TipTop: gev_file 資料控管維護檔中 gev03＝N 的列（aooi601 資料控管原則維護，data-control-maintenance）：受控營運中心；共用 schema、集團層（DC2）';
COMMENT ON COLUMN data_control_plant.data_type IS 'gev01 資料類別（1–8；0 基本資料不可維護，DC3）';
COMMENT ON COLUMN data_control_plant.plant_id IS 'gev02 營運中心（須存在；同資料類別不可已是資料中心，DC2）';
COMMENT ON COLUMN data_control_plant.created_by IS '資料所有者（TTRS 新增，ADR 0012；gev_file 無對應欄位）';
COMMENT ON COLUMN data_control_plant.created_dept IS '資料所有部門（TTRS 新增，ADR 0012）';
COMMENT ON COLUMN data_control_plant.updated_by IS '最後修改者（修改欄位條件時一併更新，DC10）';
COMMENT ON COLUMN data_control_plant.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TABLE data_control_rule (
    data_type     char(1) NOT NULL,
    plant_id      text NOT NULL,
    field_code    varchar(20) NOT NULL,
    condition     smallint NOT NULL CHECK (condition BETWEEN 1 AND 11),
    field_values  varchar(255) NOT NULL CHECK (field_values !~ '[''")+><=*]'),
    PRIMARY KEY (data_type, plant_id, field_code),
    FOREIGN KEY (data_type, plant_id) REFERENCES data_control_plant (data_type, plant_id) ON DELETE CASCADE
);
COMMENT ON TABLE data_control_rule IS 'TipTop: gey_file 資料控管欄位條件（aooi601 condition_detail）：受控營運中心自己新增資料時欄位值須符合的條件（s_field_chk，DC6）';
COMMENT ON COLUMN data_control_rule.data_type IS 'gey01 資料類別';
COMMENT ON COLUMN data_control_rule.plant_id IS 'gey02 營運中心';
COMMENT ON COLUMN data_control_rule.field_code IS 'gey03 欄位代號（存 TTRS 欄位名稱，如 ima01 → item_no，DC11）';
COMMENT ON COLUMN data_control_rule.condition IS 'gey04 允許新增的條件：1 不等於、2 大於、3 大於等於、4 小於、5 小於等於、6 開頭是、7 開頭不是、8 結尾是、9 結尾不是、10 包含、11 不包含';
COMMENT ON COLUMN data_control_rule.field_values IS 'gey05 允許新增值（以 ; 分隔；不可含 '' " ) + > < = *，aoo-031）';

-- 同一資料類別＋營運中心不可同時是資料中心與受控營運中心（TipTop gev_file 的主鍵，DC2）
CREATE FUNCTION ttrs_data_control_exclusive() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_TABLE_NAME = 'data_control_plant' THEN
        IF EXISTS (SELECT 1 FROM data_center_type WHERE data_type = NEW.data_type AND plant_id = NEW.plant_id) THEN
            RAISE unique_violation USING MESSAGE = 'data type and plant already belong to a data center';
        END IF;
    ELSE
        IF EXISTS (SELECT 1 FROM data_control_plant WHERE data_type = NEW.data_type AND plant_id = NEW.plant_id) THEN
            RAISE unique_violation USING MESSAGE = 'data type and plant are already a controlled plant';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER data_control_plant_exclusive BEFORE INSERT OR UPDATE ON data_control_plant
    FOR EACH ROW EXECUTE FUNCTION ttrs_data_control_exclusive();
CREATE TRIGGER data_center_type_exclusive BEFORE INSERT OR UPDATE ON data_center_type
    FOR EACH ROW EXECUTE FUNCTION ttrs_data_control_exclusive();

CREATE TRIGGER data_control_plant_version BEFORE UPDATE ON data_control_plant
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER data_control_plant_change_log AFTER INSERT OR UPDATE OR DELETE ON data_control_plant
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('data_type,plant_id');
CREATE TRIGGER data_control_rule_change_log AFTER INSERT OR UPDATE OR DELETE ON data_control_rule
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('data_type,plant_id,field_code');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'data-control-maintenance', 'AOO', 'I', 'aooi601', '1',
       'query, detail, condition_detail, reproduce, history, output, exporttoexcel',
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'data-control-maintenance',
       ARRAY['query', 'detail', 'condition_detail', 'reproduce', 'history', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'data-control-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('data-control-maintenance', 'zh-TW', '資料控管原則維護'), ('data-control-maintenance', 'en', 'Data Control Rules'),
               ('data-control-maintenance', 'zh-CN', '资料控管原则维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'data-control-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'data-control-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
