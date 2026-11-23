-- 倉庫料件數量查詢（TipTop aimq401；specs/features/aim/warehouse_stock_query.feature WK1、WK7）：作業登錄與起始權限（ADMIN 全部功能）。
-- 唯讀查詢，讀工廠 schema 的 warehouse、inventory_balance、item。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊。
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'warehouse-stock-query', 'AIM', 'Q', 'aimq401', '1', 'query, output, exporttoexcel', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AIM')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'warehouse-stock-query', ARRAY['query', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'warehouse-stock-query')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('warehouse-stock-query', 'zh-TW', '倉庫料件數量查詢'), ('warehouse-stock-query', 'en', 'Warehouse Stock Query'),
               ('warehouse-stock-query', 'zh-CN', '仓库料件数量查询')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aim', COALESCE(MAX(seq), 0) + 10, 'warehouse-stock-query' FROM menu_item WHERE menu_no = 'menu-aim'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aim')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'warehouse-stock-query')
ON CONFLICT (menu_no, program_no) DO NOTHING;
