-- 料件分群維護（TipTop aimi110；specs/features/aim/item_group.feature）的作業登錄與起始權限
-- 表在工廠 schema（migrations/plant/0006）。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
VALUES ('item-group-maintenance', 'AIM', 'I', 'aimi110', '1',
        'insert, query, modify, delete, reproduce, invalid, history', 'tiptop', '')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'item-group-maintenance',
       ARRAY['insert', 'query', 'modify', 'delete', 'reproduce', 'invalid', 'history'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('item-group-maintenance', 'zh-TW', '料件分群維護'),
               ('item-group-maintenance', 'en', 'Item Groups'),
               ('item-group-maintenance', 'zh-CN', '料件分群维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：庫存管理之下，排在料件基本資料之前
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aim', 5, 'item-group-maintenance'
 WHERE EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aim')
   AND NOT EXISTS (SELECT 1 FROM menu_item WHERE menu_no = 'menu-aim' AND seq = 5)
ON CONFLICT (menu_no, program_no) DO NOTHING;
