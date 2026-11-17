-- 編碼獨立段維護（TipTop aooi403 / gej_file 單頭、gek_file 單身；specs/features/aoo/encoding_segment.feature IS1–IS8）
-- 共用 schema（IS2）。gej04–06、gek05–08 為 No Use，不移植（IS7）。

CREATE TABLE encoding_segment (
    segment_code  text PRIMARY KEY CHECK (char_length(segment_code) BETWEEN 1 AND 10),
    name          varchar(80),
    length        smallint NOT NULL CHECK (length BETWEEN 1 AND 30),
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
COMMENT ON TABLE encoding_segment IS 'TipTop: gej_file 獨立段資料單頭檔（aooi403 編碼獨立段資料維護，encoding-segment-maintenance）；共用 schema（IS2）';
COMMENT ON COLUMN encoding_segment.segment_code IS 'gej01 獨立段編號（區分大小寫、最多 10 字、建立後不可修改，IS3）';
COMMENT ON COLUMN encoding_segment.name IS 'gej02 名稱';
COMMENT ON COLUMN encoding_segment.length IS 'gej03 長度（1–30；已有單身時不可修改，IS3、IS5）';
COMMENT ON COLUMN encoding_segment.active IS 'gejacti 資料有效碼';
COMMENT ON COLUMN encoding_segment.confirmed IS 'TTRS 標準確認欄位（ADR 0013 SA9；gej_file 無對應欄位）';
COMMENT ON COLUMN encoding_segment.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN encoding_segment.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN encoding_segment.created_by IS 'gejuser 資料所有者';
COMMENT ON COLUMN encoding_segment.created_dept IS 'gejgrup 資料所有部門';
COMMENT ON COLUMN encoding_segment.updated_by IS 'gejmodu 資料修改者（修改單身時一併更新，FUN-5B0136）';
COMMENT ON COLUMN encoding_segment.updated_on IS 'gejdate 最近修改日';
COMMENT ON COLUMN encoding_segment.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TABLE encoding_segment_code (
    segment_code  text NOT NULL REFERENCES encoding_segment (segment_code) ON DELETE CASCADE,
    line_no       integer NOT NULL CHECK (line_no > 0),
    code_value    varchar(30) NOT NULL CHECK (code_value !~ '[*%_? ]'),
    name          varchar(80),
    PRIMARY KEY (segment_code, line_no),
    UNIQUE (segment_code, code_value)
);
COMMENT ON TABLE encoding_segment_code IS 'TipTop: gek_file 獨立段資料單身檔（aooi403 單身）；一個編碼編號一列';
COMMENT ON COLUMN encoding_segment_code.segment_code IS 'gek01 獨立段編號';
COMMENT ON COLUMN encoding_segment_code.line_no IS 'gek02 項次（同一獨立段內不可重複，IS4）';
COMMENT ON COLUMN encoding_segment_code.code_value IS 'gek03 編碼編號（長度等於單頭長度、不可含 * % _ ? 與空白、同一獨立段內不可重複，IS4）';
COMMENT ON COLUMN encoding_segment_code.name IS 'gek04 編碼名稱';

CREATE TRIGGER encoding_segment_version BEFORE UPDATE ON encoding_segment
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER encoding_segment_change_log AFTER INSERT OR UPDATE OR DELETE ON encoding_segment
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('segment_code');
CREATE TRIGGER encoding_segment_code_change_log AFTER INSERT OR UPDATE OR DELETE ON encoding_segment_code
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('segment_code,line_no');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'encoding-segment-maintenance', 'AOO', 'I', 'aooi403', '1',
       'insert, query, modify, delete, invalid, reproduce, detail, history, confirm, notconfirm, output, exporttoexcel',
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'encoding-segment-maintenance',
       ARRAY['insert', 'query', 'modify', 'delete', 'invalid', 'reproduce', 'detail', 'history', 'confirm', 'notconfirm',
             'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'encoding-segment-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('encoding-segment-maintenance', 'zh-TW', '編碼獨立段維護'), ('encoding-segment-maintenance', 'en', 'Encoding Segments'),
               ('encoding-segment-maintenance', 'zh-CN', '编码独立段维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'encoding-segment-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'encoding-segment-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
