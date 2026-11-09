-- 料件基本資料第一階段（TipTop aimi100；specs/features/aim/item_maintenance.feature）的功能代號
-- 需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊；ADMIN 擁有全部功能
UPDATE program
   SET default_actions = 'insert, query, modify, delete, invalid, reproduce, confirm, notconfirm, order_invalid, update_item, history'
 WHERE program_no = 'item-maintenance';
UPDATE program_permission
   SET actions = ARRAY['insert', 'query', 'modify', 'delete', 'invalid', 'reproduce', 'confirm', 'notconfirm',
                       'order_invalid', 'update_item', 'history']
 WHERE class_code = 'ADMIN' AND program_no = 'item-maintenance';
