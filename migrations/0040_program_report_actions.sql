-- 程式維護 p_zz、程式執行權限 p_zy、使用者特殊權限 p_zxw 的列印、匯出 Excel（docs/adr/0013-standard-maintenance-actions.md SA3、SA7）。
-- 失效、確認不適用（SA4、SA9-4 例外）。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊；ADMIN 擁有全部功能
UPDATE program
   SET default_actions = default_actions || ', output, exporttoexcel'
 WHERE program_no IN ('program-maintenance', 'program-permission-maintenance', 'user-permission-maintenance')
   AND default_actions NOT LIKE '%exporttoexcel%';
UPDATE program_permission
   SET actions = actions || ARRAY['output', 'exporttoexcel']
 WHERE class_code = 'ADMIN'
   AND program_no IN ('program-maintenance', 'program-permission-maintenance', 'user-permission-maintenance')
   AND NOT ('exporttoexcel' = ANY(actions));

-- 資料字典：列印（PDF）；匯出 Excel 沿用 exporttoexcel（原本為 CSV，改提供 XLSX，CSV 保留）
UPDATE program
   SET default_actions = default_actions || ', output'
 WHERE program_no = 'data-dictionary' AND default_actions NOT LIKE '%output%';
UPDATE program_permission
   SET actions = actions || ARRAY['output']
 WHERE class_code = 'ADMIN' AND program_no = 'data-dictionary' AND NOT ('output' = ANY(actions));
