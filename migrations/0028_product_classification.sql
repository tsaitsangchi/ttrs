-- 產品分類維護（TipTop axmi110；specs/features/axm/product_classification.feature）的作業登錄與起始權限
-- 表在工廠 schema（migrations/plant/0005）。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO module (module_code, name) VALUES ('AXM', 'AXM') ON CONFLICT DO NOTHING;
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
VALUES ('product-classification-maintenance', 'AXM', 'I', 'axmi110', '1', 'query, detail, history', 'tiptop', '')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'product-classification-maintenance', ARRAY['query', 'detail', 'history'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('product-classification-maintenance', 'zh-TW', '產品分類維護'),
               ('product-classification-maintenance', 'en', 'Product Classifications'),
               ('product-classification-maintenance', 'zh-CN', '产品分类维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下（產品分類屬銷售模組 axm，料件會引用；暫放整體管理）
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'product-classification-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'product-classification-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
