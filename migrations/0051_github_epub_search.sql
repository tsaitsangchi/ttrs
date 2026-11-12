-- GitHub EPUB 搜尋（TTRS 新增的工具作業，TipTop 無對應；specs/features/azz/github_epub_search.feature GE1–GE9）
-- 不建資料表：搜尋結果不快取、檔案不保存（GE4、GE9）。只登錄作業、ADMIN 權限（query、run，GE7）與系統管理選單。
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'github-epub-search', 'AZZ', 'P', NULL, '1', 'query, run', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AZZ')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'github-epub-search', ARRAY['query', 'run'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'github-epub-search')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('github-epub-search', 'zh-TW', 'GitHub EPUB 搜尋'), ('github-epub-search', 'en', 'GitHub EPUB Search'),
               ('github-epub-search', 'zh-CN', 'GitHub EPUB 搜索')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-azz', COALESCE(MAX(seq), 0) + 10, 'github-epub-search' FROM menu_item WHERE menu_no = 'menu-azz'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-azz')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'github-epub-search')
ON CONFLICT (menu_no, program_no) DO NOTHING;
