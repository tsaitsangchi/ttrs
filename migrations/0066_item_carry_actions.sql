-- 料件拋轉（TipTop aimi100 carry、qry_carry_history；specs/features/aim/item_carry.feature IC1、IC11）
-- 料件維護加功能 carry（拋轉）、qry_carry_history（拋轉歷史）；ADMIN 給全部功能。
UPDATE program
   SET default_actions = default_actions || ', carry, qry_carry_history'
 WHERE program_no = 'item-maintenance' AND default_actions NOT LIKE '%qry_carry_history%';
UPDATE program_permission
   SET actions = actions || ARRAY['carry', 'qry_carry_history']
 WHERE class_code = 'ADMIN' AND program_no = 'item-maintenance' AND NOT ('qry_carry_history' = ANY(actions));
