-- 專案維護（TipTop aooi200 / gja_file；specs/features/aoo/project_maintenance.feature PJ1–PJ6）
-- 資料表在各工廠 schema（migrations/plant/0013_project.sql，PJ2）；這裡只登錄作業與起始權限。
-- TipTop 為單筆作業：insert、modify、delete、query、reproduce、invalid、output；依 ADR 0013 補上 exporttoexcel、history、confirm、notconfirm
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'project-maintenance', 'AOO', 'I', 'aooi200', '1',
       'insert, query, modify, delete, reproduce, invalid, history, confirm, notconfirm, output, exporttoexcel',
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'project-maintenance',
       ARRAY['insert', 'query', 'modify', 'delete', 'reproduce', 'invalid', 'history', 'confirm', 'notconfirm',
             'output', 'exporttoexcel'],
       'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'project-maintenance')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('project-maintenance', 'zh-TW', '專案維護'), ('project-maintenance', 'en', 'Projects'),
               ('project-maintenance', 'zh-CN', '专案维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'project-maintenance' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'project-maintenance')
ON CONFLICT (menu_no, program_no) DO NOTHING;
