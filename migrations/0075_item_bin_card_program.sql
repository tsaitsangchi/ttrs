-- 料件 BIN 卡查詢（TipTop aimq231；specs/features/aim/item_bin_card.feature BN1、BN7）：作業登錄與起始權限（ADMIN 全部功能）。
-- 唯讀查詢，讀工廠 schema 的 item、inventory_balance、inventory_txn。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊。
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT 'item-bin-card', 'AIM', 'Q', 'aimq231', '1', 'query, output, exporttoexcel', 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AIM')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'item-bin-card', ARRAY['query', 'output', 'exporttoexcel'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'item-bin-card')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('item-bin-card', 'zh-TW', '料件 BIN 卡查詢'), ('item-bin-card', 'en', 'Item Bin Card'),
               ('item-bin-card', 'zh-CN', '料件 BIN 卡查询')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aim', COALESCE(MAX(seq), 0) + 10, 'item-bin-card' FROM menu_item WHERE menu_no = 'menu-aim'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aim')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'item-bin-card')
ON CONFLICT (menu_no, program_no) DO NOTHING;
