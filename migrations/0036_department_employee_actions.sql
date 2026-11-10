-- 部門 aooi030、員工 aooi040 的標準功能鈕（docs/adr/0013-standard-maintenance-actions.md SA7）：
-- 複製、確認／取消確認，員工另加列印、匯出 Excel。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊；ADMIN 擁有全部功能
UPDATE program
   SET default_actions = default_actions || ', reproduce, confirm, notconfirm'
 WHERE program_no = 'department-maintenance' AND default_actions NOT LIKE '%notconfirm%';
UPDATE program_permission
   SET actions = actions || ARRAY['reproduce', 'confirm', 'notconfirm']
 WHERE class_code = 'ADMIN' AND program_no = 'department-maintenance' AND NOT ('notconfirm' = ANY(actions));
UPDATE program
   SET default_actions = default_actions || ', reproduce, confirm, notconfirm, output, exporttoexcel'
 WHERE program_no = 'employee-maintenance' AND default_actions NOT LIKE '%notconfirm%';
UPDATE program_permission
   SET actions = actions || ARRAY['reproduce', 'confirm', 'notconfirm', 'output', 'exporttoexcel']
 WHERE class_code = 'ADMIN' AND program_no = 'employee-maintenance' AND NOT ('notconfirm' = ANY(actions));
