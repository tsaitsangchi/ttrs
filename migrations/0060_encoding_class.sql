-- 編碼類別維護（TipTop aooi402 / gei_file 單頭、gel_file 單身；specs/features/aoo/encoding_class.feature EC1–EC11）
-- 共用 schema（EC2）。gei04 總段數、gei05 總長度由單身即時算出不另存；gei06、gei07 為 No Use（EC10）。
-- 性質參照編碼性質、型態 2 的獨立段參照編碼獨立段（外鍵擋刪除，EC9）。

CREATE TABLE encoding_class (
    class_code    text PRIMARY KEY CHECK (char_length(class_code) BETWEEN 1 AND 10),
    name          varchar(80) NOT NULL,
    nature_code   text NOT NULL REFERENCES encoding_nature (nature_code),
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
COMMENT ON TABLE encoding_class IS 'TipTop: gei_file 編碼類別資料單頭檔（aooi402 編碼類別資料維護，encoding-class-maintenance）；共用 schema（EC2）';
COMMENT ON COLUMN encoding_class.class_code IS 'gei01 編碼類別（區分大小寫、最多 10 字、建立後不可修改，EC3）';
COMMENT ON COLUMN encoding_class.name IS 'gei02 名稱（必填）';
COMMENT ON COLUMN encoding_class.nature_code IS 'gei03 性質（編碼性質 encoding_nature，須存在且有效，aoo-131）';
COMMENT ON COLUMN encoding_class.active IS 'geiacti 資料有效碼';
COMMENT ON COLUMN encoding_class.confirmed IS 'TTRS 標準確認欄位（ADR 0013 SA9；gei_file 無對應欄位）';
COMMENT ON COLUMN encoding_class.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN encoding_class.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN encoding_class.created_by IS 'geiuser 資料所有者';
COMMENT ON COLUMN encoding_class.created_dept IS 'geigrup 資料所有部門';
COMMENT ON COLUMN encoding_class.updated_by IS 'geimodu 資料修改者（修改單身時一併更新，FUN-5B0136）';
COMMENT ON COLUMN encoding_class.updated_on IS 'geidate 最近修改日';
COMMENT ON COLUMN encoding_class.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TABLE encoding_class_segment (
    class_code           text NOT NULL REFERENCES encoding_class (class_code) ON DELETE CASCADE,
    seq_no               integer NOT NULL CHECK (seq_no > 0),
    description          varchar(80),
    length               smallint NOT NULL CHECK (length BETWEEN 1 AND 30),
    segment_type         text NOT NULL CHECK (segment_type IN ('1', '2', '3', '4', '5', '6', '7', '8', '9')),
    fixed_value          varchar(30) CHECK (fixed_value !~ '[*%_? ]'),
    independent_segment  text REFERENCES encoding_segment (segment_code),
    show_name            boolean NOT NULL DEFAULT false,
    PRIMARY KEY (class_code, seq_no),
    CHECK ((segment_type = '1') = (fixed_value IS NOT NULL)),
    CHECK ((segment_type = '2') = (independent_segment IS NOT NULL))
);
CREATE INDEX encoding_class_segment_independent ON encoding_class_segment (independent_segment);
COMMENT ON TABLE encoding_class_segment IS 'TipTop: gel_file 編碼類別資料單身檔（aooi402 單身）；一段一列';
COMMENT ON COLUMN encoding_class_segment.class_code IS 'gel01 編碼類別';
COMMENT ON COLUMN encoding_class_segment.seq_no IS 'gel02 段次（同一類別內不可重複，EC4）';
COMMENT ON COLUMN encoding_class_segment.description IS 'gel06 段次說明';
COMMENT ON COLUMN encoding_class_segment.length IS 'gel03 長度（1–30；獨立段取獨立段長度；季月週日固定 2，EC5）';
COMMENT ON COLUMN encoding_class_segment.segment_type IS 'gel04 型態：1 固定值、2 獨立段、3 流水號、4 欄位值、5 年、6 季、7 月、8 週、9 日';
COMMENT ON COLUMN encoding_class_segment.fixed_value IS 'gel05 固定值（型態 1）';
COMMENT ON COLUMN encoding_class_segment.independent_segment IS 'gel05 獨立段編號（型態 2；參照 encoding_segment）';
COMMENT ON COLUMN encoding_class_segment.show_name IS 'gel07 名稱顯示否';

CREATE TRIGGER encoding_class_version BEFORE UPDATE ON encoding_class
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER encoding_class_change_log AFTER INSERT OR UPDATE OR DELETE ON encoding_class
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('class_code');
CREATE TRIGGER encoding_class_segment_change_log AFTER INSERT OR UPDATE OR DELETE ON encoding_class_segment
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('class_code,seq_no');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'encoding-class-maintenance', 'AOO', 'I', 'aooi402', '1',
       'insert, query, modify, delete, invalid, reproduce, detail, history, confirm, notconfirm, output, exporttoexcel',
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'encoding-class-maintenance',
       ARRAY['insert', 'query', 'modify', 'delete', 'invalid', 'reproduce', 'detail', 'history', 'confirm', 'notconfirm',
             'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'encoding-class-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('encoding-class-maintenance', 'zh-TW', '編碼類別維護'), ('encoding-class-maintenance', 'en', 'Encoding Classes'),
               ('encoding-class-maintenance', 'zh-CN', '编码类别维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'encoding-class-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'encoding-class-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
