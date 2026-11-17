-- 編碼性質維護（TipTop aooi401 / geh_file；specs/features/aoo/encoding_nature.feature EN1–EN8）
-- 共用 schema（EN2）。geh03 檔案名稱、geh05 索引值是由類別決定的固定值，不移植（EN5）。

CREATE TABLE encoding_nature (
    nature_code   text PRIMARY KEY CHECK (char_length(nature_code) BETWEEN 1 AND 2),
    name          varchar(80),
    category      text NOT NULL CHECK (category IN ('1', '2', '3', '4', '5', '6')),
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
COMMENT ON TABLE encoding_nature IS 'TipTop: geh_file 編碼性質檔（aooi401 編碼性質說明，encoding-nature-maintenance）；共用 schema（EN2）';
COMMENT ON COLUMN encoding_nature.nature_code IS 'geh01 性質（區分大小寫、最多 2 字、建立後不可修改，EN3）';
COMMENT ON COLUMN encoding_nature.name IS 'geh02 說明';
COMMENT ON COLUMN encoding_nature.category IS 'geh04 類別：1 料件編號、2 客戶編號、3 供應商編號、4 固定資產編號、5 批號、6 序號（EN4）';
COMMENT ON COLUMN encoding_nature.active IS 'gehacti 資料有效碼';
COMMENT ON COLUMN encoding_nature.confirmed IS 'TTRS 標準確認欄位（ADR 0013 SA9；geh_file 無對應欄位）';
COMMENT ON COLUMN encoding_nature.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN encoding_nature.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN encoding_nature.created_by IS 'gehuser 資料所有者';
COMMENT ON COLUMN encoding_nature.created_dept IS 'gehgrup 資料所有部門';
COMMENT ON COLUMN encoding_nature.updated_by IS 'gehmodu 資料修改者';
COMMENT ON COLUMN encoding_nature.updated_on IS 'gehdate 最近修改日';
COMMENT ON COLUMN encoding_nature.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TRIGGER encoding_nature_version BEFORE UPDATE ON encoding_nature
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER encoding_nature_change_log AFTER INSERT OR UPDATE OR DELETE ON encoding_nature
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('nature_code');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'encoding-nature-maintenance', 'AOO', 'I', 'aooi401', '1',
       'query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'encoding-nature-maintenance',
       ARRAY['query', 'detail', 'history', 'reproduce', 'confirm', 'notconfirm', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'encoding-nature-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('encoding-nature-maintenance', 'zh-TW', '編碼性質維護'), ('encoding-nature-maintenance', 'en', 'Encoding Natures'),
               ('encoding-nature-maintenance', 'zh-CN', '编码性质维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'encoding-nature-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'encoding-nature-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
