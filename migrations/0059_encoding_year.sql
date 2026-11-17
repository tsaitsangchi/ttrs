-- 編碼年對照維護（TipTop aooi404 / gff_file；specs/features/aoo/encoding_year.feature EY1–EY8）
-- 共用 schema（EY2）。主鍵為年＋長度（TQC-950117）；長度＝編碼編號的字數（:229-234）。
-- gff_file 沒有資料有效、所有者等欄位，TTRS 依 ADR 0012、0013 補上（EY5）。

CREATE TABLE encoding_year_code (
    year          integer NOT NULL CHECK (year BETWEEN 1 AND 9999),
    length        smallint NOT NULL CHECK (length BETWEEN 1 AND 10),
    code_value    varchar(10) NOT NULL CHECK (code_value !~ '[*%_? ]'),
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
    updated_at    timestamptz,
    PRIMARY KEY (year, length),
    CHECK (char_length(code_value) = length)
);
COMMENT ON TABLE encoding_year_code IS 'TipTop: gff_file 編碼年對照表（aooi404 年－編碼維護，encoding-year-maintenance）；共用 schema（EY2）';
COMMENT ON COLUMN encoding_year_code.year IS 'gff01 年（1–9999，EY3）';
COMMENT ON COLUMN encoding_year_code.length IS 'gff02 長度：編碼編號的字數，自動帶出（EY3）';
COMMENT ON COLUMN encoding_year_code.code_value IS 'gff03 編碼編號（不可含 * % _ ? 與空白；修改時字數不可變，EY3、EY4）';
COMMENT ON COLUMN encoding_year_code.active IS 'TTRS 新增：資料有效（EY5；gff_file 無對應欄位）';
COMMENT ON COLUMN encoding_year_code.confirmed IS 'TTRS 標準確認欄位（ADR 0013 SA9；gff_file 無對應欄位）';
COMMENT ON COLUMN encoding_year_code.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN encoding_year_code.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN encoding_year_code.created_by IS 'TTRS 新增：資料所有者（EY5）';
COMMENT ON COLUMN encoding_year_code.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TRIGGER encoding_year_code_version BEFORE UPDATE ON encoding_year_code
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER encoding_year_code_change_log AFTER INSERT OR UPDATE OR DELETE ON encoding_year_code
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('year,length');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'encoding-year-maintenance', 'AOO', 'I', 'aooi404', '1',
       'query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'encoding-year-maintenance',
       ARRAY['query', 'detail', 'history', 'reproduce', 'confirm', 'notconfirm', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'encoding-year-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('encoding-year-maintenance', 'zh-TW', '編碼年對照維護'), ('encoding-year-maintenance', 'en', 'Encoding Year Codes'),
               ('encoding-year-maintenance', 'zh-CN', '编码年对照维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'encoding-year-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'encoding-year-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
