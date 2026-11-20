-- 庫存直接調撥（TipTop aimt324；specs/features/aim/warehouse_transfer.feature WT1、WT12）：作業登錄與起始權限（ADMIN 全部功能）。
-- 資料表在工廠 schema（migrations/plant/0022_warehouse_transfer.sql）。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊。
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'warehouse-transfer', 'AIM', 'T', 'aimt324', '1',
       'insert, query, modify, delete, detail, reproduce, void, confirm, undo_confirm, post, undo_post, history, output, exporttoexcel',
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AIM')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'warehouse-transfer',
       ARRAY['insert', 'query', 'modify', 'delete', 'detail', 'reproduce', 'void', 'confirm', 'undo_confirm', 'post', 'undo_post',
             'history', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'warehouse-transfer')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('warehouse-transfer', 'zh-TW', '庫存直接調撥'), ('warehouse-transfer', 'en', 'Warehouse Transfer'),
               ('warehouse-transfer', 'zh-CN', '库存直接调拨')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aim', COALESCE(MAX(seq), 0) + 10, 'warehouse-transfer' FROM menu_item WHERE menu_no = 'menu-aim'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aim')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'warehouse-transfer')
ON CONFLICT (menu_no, program_no) DO NOTHING;
