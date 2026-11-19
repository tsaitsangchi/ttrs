-- 資料中心拋轉設定（TipTop aooi602 / gew_file 資料拋轉原則、gez_file 拋轉預設資料；specs/features/aoo/carry_setting.feature CS1–CS12）
-- 集團層，放共用 schema（CS2）。gew_file 拆成單頭 carry_setting（gew01、gew02、gew03）、單身 carry_target（gew04、gew06–gew08）、
-- 拋轉條件 carry_condition（gew09 一列一個欄位；gew05 SQL 字串不移植，CS12）；gez_file → carry_default。

CREATE TABLE carry_setting (
    center_code   text NOT NULL REFERENCES group_center (center_code) ON DELETE CASCADE,
    data_type     char(1) NOT NULL CHECK (data_type BETWEEN '0' AND '8'),
    mode          char(1) NOT NULL DEFAULT '3' CHECK (mode IN ('1', '2', '3')),
    created_by    text NOT NULL,
    created_dept  text NOT NULL DEFAULT '',
    created_on    date NOT NULL DEFAULT CURRENT_DATE,
    updated_by    text,
    updated_on    date,
    version       integer NOT NULL DEFAULT 1,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz,
    PRIMARY KEY (center_code, data_type)
);
COMMENT ON TABLE carry_setting IS 'TipTop: gew_file 資料拋轉原則的單頭（aooi602 資料中心拋轉設定，carry-setting-maintenance）；共用 schema、集團層（CS2）';
COMMENT ON COLUMN carry_setting.center_code IS 'gew01 資料中心代碼（有效、類別 1，CS3(a)）';
COMMENT ON COLUMN carry_setting.data_type IS 'gew02 資料類別（須為該資料中心負責的資料類別 aoo-094，CS3(b)）';
COMMENT ON COLUMN carry_setting.mode IS 'gew03 拋轉方式：1 自動拋轉、2 詢問後拋轉、3 手動拋轉（TipTop 存在每列單身，TTRS 移到單頭，CS12）';
COMMENT ON COLUMN carry_setting.created_by IS '資料所有者（TTRS 新增，ADR 0012，CS10）';
COMMENT ON COLUMN carry_setting.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TABLE carry_target (
    center_code      text NOT NULL,
    data_type        char(1) NOT NULL,
    plant_id         text NOT NULL REFERENCES plant (plant_id),
    deletable        boolean NOT NULL DEFAULT true,
    update_existing  boolean NOT NULL DEFAULT true,
    notify           varchar(1500),
    PRIMARY KEY (center_code, data_type, plant_id),
    FOREIGN KEY (center_code, data_type) REFERENCES carry_setting (center_code, data_type) ON DELETE CASCADE
);
COMMENT ON TABLE carry_target IS 'TipTop: gew_file 資料拋轉原則的單身：拋轉營運中心（CS4）';
COMMENT ON COLUMN carry_target.plant_id IS 'gew04 拋轉營運中心（須存在、不可是來源營運中心，CS4）';
COMMENT ON COLUMN carry_target.deletable IS 'gew06 子公司可刪除資料否（基本資料固定 Y，CS6(d)）';
COMMENT ON COLUMN carry_target.update_existing IS 'gew07 更新已存在資料否（拋轉時覆蓋；子公司能否修改收到的資料，CS11）';
COMMENT ON COLUMN carry_target.notify IS 'gew08 拋轉通知人員（電子郵件，以 ; 分隔，CS5）';

CREATE TABLE carry_condition (
    center_code   text NOT NULL,
    data_type     char(1) NOT NULL,
    plant_id      text NOT NULL,
    field_code    varchar(20) NOT NULL,
    condition     smallint NOT NULL CHECK (condition BETWEEN 1 AND 11),
    field_values  varchar(255) NOT NULL CHECK (field_values !~ '[''")+><=*]'),
    PRIMARY KEY (center_code, data_type, plant_id, field_code),
    FOREIGN KEY (center_code, data_type, plant_id) REFERENCES carry_target (center_code, data_type, plant_id) ON DELETE CASCADE
);
COMMENT ON TABLE carry_condition IS 'TipTop: gew_file.gew09 拋轉條件（結構化，一列一個欄位；gew05 SQL 字串不移植，CS6、CS12）';
COMMENT ON COLUMN carry_condition.field_code IS 'gew09 欄位（TTRS 欄位名稱，如 ima06 → group_code）';
COMMENT ON COLUMN carry_condition.condition IS 'gew09 條件 1–11（s_get_option，同資料控管原則）';
COMMENT ON COLUMN carry_condition.field_values IS 'gew09 值（以 ; 分隔）';

CREATE TABLE carry_default (
    center_code   text NOT NULL,
    data_type     char(1) NOT NULL,
    plant_id      text NOT NULL,
    field_code    varchar(20) NOT NULL,
    field_value   varchar(255) NOT NULL,
    PRIMARY KEY (center_code, data_type, plant_id, field_code),
    FOREIGN KEY (center_code, data_type, plant_id) REFERENCES carry_target (center_code, data_type, plant_id) ON DELETE CASCADE
);
COMMENT ON TABLE carry_default IS 'TipTop: gez_file 拋轉預設資料維護：拋到該營運中心時把欄位換成固定值（CS7）';
COMMENT ON COLUMN carry_default.center_code IS 'gez01 資料中心代碼';
COMMENT ON COLUMN carry_default.data_type IS 'gez02 資料類別';
COMMENT ON COLUMN carry_default.plant_id IS 'gez03 拋轉營運中心';
COMMENT ON COLUMN carry_default.field_code IS 'gez04 欄位代號（TTRS 欄位名稱；料件不開放 item_no，CS7(b)）';
COMMENT ON COLUMN carry_default.field_value IS 'gez05 拋轉欄位值';

CREATE TRIGGER carry_setting_version BEFORE UPDATE ON carry_setting
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER carry_setting_change_log AFTER INSERT OR UPDATE OR DELETE ON carry_setting
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('center_code,data_type');
CREATE TRIGGER carry_target_change_log AFTER INSERT OR UPDATE OR DELETE ON carry_target
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('center_code,data_type,plant_id');
CREATE TRIGGER carry_condition_change_log AFTER INSERT OR UPDATE OR DELETE ON carry_condition
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('center_code,data_type,plant_id,field_code');
CREATE TRIGGER carry_default_change_log AFTER INSERT OR UPDATE OR DELETE ON carry_default
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('center_code,data_type,plant_id,field_code');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'carry-setting-maintenance', 'AOO', 'I', 'aooi602', '1',
       'insert, query, modify, delete, detail, set_default_value, reproduce, history, output, exporttoexcel',
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'carry-setting-maintenance',
       ARRAY['insert', 'query', 'modify', 'delete', 'detail', 'set_default_value', 'reproduce', 'history', 'output',
             'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'carry-setting-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('carry-setting-maintenance', 'zh-TW', '資料中心拋轉設定'), ('carry-setting-maintenance', 'en', 'Carry Settings'),
               ('carry-setting-maintenance', 'zh-CN', '资料中心抛转设置')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'carry-setting-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'carry-setting-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
