-- 料件預設倉儲維護（TipTop aimi202；specs/features/aim/item_storage.feature IS1、IS11）：作業登錄與起始權限（ADMIN 全部功能）
-- 資料表在工廠 schema（migrations/plant/0017_item_storage.sql）。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊。
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'item-storage-maintenance', 'AIM', 'I', 'aimi202', '1',
       'query, detail, reproduce, output, exporttoexcel, history, batch_generate',
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AIM')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'item-storage-maintenance',
       ARRAY['query', 'detail', 'reproduce', 'output', 'exporttoexcel', 'history', 'batch_generate'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'item-storage-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('item-storage-maintenance', 'zh-TW', '料件預設倉儲維護'), ('item-storage-maintenance', 'en', 'Item Default Storage'),
               ('item-storage-maintenance', 'zh-CN', '料件默认仓储维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：庫存管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aim', COALESCE(MAX(seq), 0) + 10, 'item-storage-maintenance' FROM menu_item WHERE menu_no = 'menu-aim'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aim')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'item-storage-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
