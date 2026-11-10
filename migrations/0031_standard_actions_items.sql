-- 標準功能鈕（docs/adr/0013-standard-maintenance-actions.md SA7）：料件基本資料（aimi100）、料件分群（aimi110）
-- 加上列印 output、匯出 Excel exporttoexcel（item_maintenance_report.feature、item_group_report.feature）。
-- 需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊；ADMIN 擁有全部功能
UPDATE program
   SET default_actions = 'insert, query, modify, delete, invalid, reproduce, confirm, notconfirm, order_invalid, update_item, output, exporttoexcel, history'
 WHERE program_no = 'item-maintenance';
UPDATE program_permission
   SET actions = ARRAY['insert', 'query', 'modify', 'delete', 'invalid', 'reproduce', 'confirm', 'notconfirm',
                       'order_invalid', 'update_item', 'output', 'exporttoexcel', 'history']
 WHERE class_code = 'ADMIN' AND program_no = 'item-maintenance';
UPDATE program
   SET default_actions = 'insert, query, modify, delete, reproduce, invalid, output, exporttoexcel, history'
 WHERE program_no = 'item-group-maintenance';
UPDATE program_permission
   SET actions = ARRAY['insert', 'query', 'modify', 'delete', 'reproduce', 'invalid', 'output', 'exporttoexcel', 'history']
 WHERE class_code = 'ADMIN' AND program_no = 'item-group-maintenance';
