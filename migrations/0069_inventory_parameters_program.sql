-- 庫存參數（TipTop asms230；specs/features/aim/inventory_parameters.feature IP1、IP9）：作業登錄與起始權限（ADMIN 全部功能）
-- 資料表在工廠 schema（migrations/plant/0018_manufacturing_parameter.sql）。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊。
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'inventory-parameters', 'AIM', 'S', 'asms230', '1',
       'query, modify, history, output, exporttoexcel',
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AIM')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'inventory-parameters',
       ARRAY['query', 'modify', 'history', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'inventory-parameters')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('inventory-parameters', 'zh-TW', '庫存參數'), ('inventory-parameters', 'en', 'Inventory Parameters'),
               ('inventory-parameters', 'zh-CN', '库存参数')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：庫存管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aim', COALESCE(MAX(seq), 0) + 10, 'inventory-parameters' FROM menu_item WHERE menu_no = 'menu-aim'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aim')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'inventory-parameters')
ON CONFLICT (menu_no, program_no) DO NOTHING;
