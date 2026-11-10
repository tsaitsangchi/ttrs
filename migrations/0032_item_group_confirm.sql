-- 料件分群的「確認」功能鈕（docs/adr/0013-standard-maintenance-actions.md SA9；item_group_confirm.feature）：
-- 功能代號 confirm、notconfirm。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊；ADMIN 擁有全部功能
UPDATE program
   SET default_actions = 'insert, query, modify, delete, reproduce, confirm, notconfirm, invalid, output, exporttoexcel, history'
 WHERE program_no = 'item-group-maintenance';
UPDATE program_permission
   SET actions = ARRAY['insert', 'query', 'modify', 'delete', 'reproduce', 'confirm', 'notconfirm', 'invalid', 'output',
                       'exporttoexcel', 'history']
 WHERE class_code = 'ADMIN' AND program_no = 'item-group-maintenance';
