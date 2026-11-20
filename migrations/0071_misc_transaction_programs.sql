-- 整體管理系統參數：單號位數 aza42（misc_issue.feature MT4(a)；GP9 由延後改為移植）。預設 8 碼（TipTop aza42＝1）。
ALTER TABLE system_parameter ADD COLUMN doc_serial_length smallint NOT NULL DEFAULT 8 CHECK (doc_serial_length IN (8, 9, 10));
COMMENT ON COLUMN system_parameter.doc_serial_length IS 'aza42 單號位數：1→8、2→9、3→10 碼；已有單據時不可變更（MT4(a)）';
-- 雜項發料 aimt301、雜項收料 aimt302、雜項報廢 aimt303（三支獨立作業，MT1、MT13）：作業登錄與起始權限（ADMIN 全部功能）。
-- 資料表在工廠 schema（migrations/plant/0020_misc_transaction.sql）。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊。
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'misc-issue', 'AIM', 'T', 'aimt301', '1', 'insert, query, modify, delete, detail, reproduce, void, confirm, undo_confirm, history, output, exporttoexcel', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AIM')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'misc-issue', ARRAY['insert', 'query', 'modify', 'delete', 'detail', 'reproduce', 'void', 'confirm', 'undo_confirm', 'history', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'misc-issue')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('misc-issue', 'zh-TW', '雜項發料'), ('misc-issue', 'en', 'Misc Issue'), ('misc-issue', 'zh-CN', '杂项发料')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aim', COALESCE(MAX(seq), 0) + 10, 'misc-issue' FROM menu_item WHERE menu_no = 'menu-aim'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aim')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'misc-issue')
ON CONFLICT (menu_no, program_no) DO NOTHING;
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'misc-receipt', 'AIM', 'T', 'aimt302', '1', 'insert, query, modify, delete, detail, reproduce, void, confirm, undo_confirm, history, output, exporttoexcel', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AIM')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'misc-receipt', ARRAY['insert', 'query', 'modify', 'delete', 'detail', 'reproduce', 'void', 'confirm', 'undo_confirm', 'history', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'misc-receipt')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('misc-receipt', 'zh-TW', '雜項收料'), ('misc-receipt', 'en', 'Misc Receipt'), ('misc-receipt', 'zh-CN', '杂项收料')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aim', COALESCE(MAX(seq), 0) + 10, 'misc-receipt' FROM menu_item WHERE menu_no = 'menu-aim'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aim')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'misc-receipt')
ON CONFLICT (menu_no, program_no) DO NOTHING;
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'misc-scrap', 'AIM', 'T', 'aimt303', '1', 'insert, query, modify, delete, detail, reproduce, void, confirm, undo_confirm, history, output, exporttoexcel', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AIM')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'misc-scrap', ARRAY['insert', 'query', 'modify', 'delete', 'detail', 'reproduce', 'void', 'confirm', 'undo_confirm', 'history', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'misc-scrap')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('misc-scrap', 'zh-TW', '雜項報廢'), ('misc-scrap', 'en', 'Misc Scrap'), ('misc-scrap', 'zh-CN', '杂项报废')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aim', COALESCE(MAX(seq), 0) + 10, 'misc-scrap' FROM menu_item WHERE menu_no = 'menu-aim'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aim')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'misc-scrap')
ON CONFLICT (menu_no, program_no) DO NOTHING;
