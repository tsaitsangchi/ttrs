-- 整體管理系統參數：單別位數 aza41（document_type.feature DT5(a)；GP9 由延後改為移植）。預設 3 碼（TipTop aza41＝1）。
ALTER TABLE system_parameter ADD COLUMN doc_code_length smallint NOT NULL DEFAULT 3 CHECK (doc_code_length IN (3, 4, 5));
COMMENT ON COLUMN system_parameter.doc_code_length IS 'aza41 單別位數：1→3、2→4、3→5 碼；已有單別時不可變更（DT5(a)）';
-- 單別維護（TipTop asmi300；specs/features/aim/document_type.feature DT1、DT14）；另加整體參數單別位數 aza41（DT5(a)）：作業登錄與起始權限（ADMIN 全部功能）
-- 資料表在工廠 schema（migrations/plant/0019_document_type.sql）。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊。
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'document-type-maintenance', 'AIM', 'I', 'asmi300', '1',
       'insert, query, modify, delete, invalid, detail, authorization, dept_authorization, reproduce, confirm, notconfirm, history, output, exporttoexcel',
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AIM')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'document-type-maintenance',
       ARRAY['insert', 'query', 'modify', 'delete', 'invalid', 'detail', 'authorization', 'dept_authorization', 'reproduce', 'confirm', 'notconfirm', 'history', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'document-type-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('document-type-maintenance', 'zh-TW', '單別維護'), ('document-type-maintenance', 'en', 'Document Types'),
               ('document-type-maintenance', 'zh-CN', '单别维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：庫存管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aim', COALESCE(MAX(seq), 0) + 10, 'document-type-maintenance' FROM menu_item WHERE menu_no = 'menu-aim'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aim')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'document-type-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
