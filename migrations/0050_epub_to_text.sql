-- EPUB 轉 TXT（TTRS 新增的工具作業，TipTop 無對應；specs/features/azz/epub_to_text.feature ET1–ET8）
-- 不建資料表：檔案只在記憶體轉換、不保存（ET3）。只登錄作業、ADMIN 權限（功能 run，ET7）與系統管理選單。
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'epub-to-text', 'AZZ', 'P', NULL, '1', 'run', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AZZ')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'epub-to-text', ARRAY['run'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'epub-to-text')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('epub-to-text', 'zh-TW', 'EPUB 轉 TXT'), ('epub-to-text', 'en', 'EPUB to Text'),
               ('epub-to-text', 'zh-CN', 'EPUB 转 TXT')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-azz', COALESCE(MAX(seq), 0) + 10, 'epub-to-text' FROM menu_item WHERE menu_no = 'menu-azz'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-azz')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'epub-to-text')
ON CONFLICT (menu_no, program_no) DO NOTHING;
