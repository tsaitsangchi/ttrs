-- 權限檢查起始資料（specs/features/azz/authorization.feature AQ11；docs/adr/0006 §4）
-- 登錄 TTRS 已移植、經 Authorizer 檢查的作業（TipTop zz_file），並給權限類別 ADMIN 全部功能、
-- 資料權限 0／0／0、單身 0（zy_file），否則新系統沒有人能操作。
-- 需與 ttrs_app::authorization::PORTED_PROGRAMS、ttrs_app::memory::InMemoryStore::seeded() 對齊；
-- 日後匯入 TipTop 的 zz_file、zy_file 時以匯入資料為準。已存在的程式與權限不變。

-- 營運中心維護 aooi901 的模組；名稱待匯入 TipTop gao_file
INSERT INTO module (module_code, name) VALUES ('AOO', 'AOO') ON CONFLICT DO NOTHING;

-- 標準命名的程式（第 4 碼為程式類別）同程式維護自動帶入程式類別與執行指令（p_zz.4gl:917-926）
INSERT INTO program (program_no, module_code, program_type, run_command, report_ext_mode,
                     default_actions, created_by, created_dept)
VALUES
    ('p_zz', 'AZZ', NULL, NULL, '1',
     'insert, query, modify, delete, reproduce, authorization', 'tiptop', ''),
    ('p_zta', 'AZZ', NULL, NULL, '1',
     'insert, query, modify, delete, maintain_table_description, maintain_field_description, exporttoexcel, zta_import',
     'tiptop', ''),
    ('p_zx', 'AZZ', NULL, NULL, '1',
     'insert, query, modify, delete, reproduce, invalid', 'tiptop', ''),
    ('p_zw', 'AZZ', NULL, NULL, '1', 'query, detail', 'tiptop', ''),
    ('aooi901', 'AOO', 'I', '$FGLRUN $AOOi/aooi901', '1',
     'insert, query, modify, delete, reproduce', 'tiptop', ''),
    ('p_zy', 'AZZ', NULL, NULL, '1',
     'query, delete, reproduce, detail, batch_gen_by_prog, batch_gen_by_menu, batch_del_by_menu',
     'tiptop', ''),
    ('p_zxw', 'AZZ', NULL, NULL, '1', 'query, delete, reproduce, detail', 'tiptop', ''),
    ('aimi100', 'AIM', 'I', '$FGLRUN $AIMi/aimi100', '1',
     'insert, query, delete, confirm, notconfirm', 'tiptop', '')
ON CONFLICT (program_no) DO NOTHING;

-- 允許執行功能為程式的可執行功能（逗號分隔、去空白、轉小寫、去重複、依原順序，同 0013 的轉換）
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN',
       p.program_no,
       COALESCE(ARRAY(
           SELECT t.action
             FROM (SELECT lower(btrim(u.item)) AS action, min(u.ord) AS ord
                     FROM unnest(string_to_array(p.default_actions, ',')) WITH ORDINALITY AS u(item, ord)
                    WHERE btrim(u.item) <> ''
                    GROUP BY 1) t
            ORDER BY t.ord), '{}'),
       'tiptop',
       ''
  FROM program p
 WHERE p.program_no IN ('p_zz', 'p_zta', 'p_zx', 'p_zw', 'aooi901', 'p_zy', 'p_zxw', 'aimi100')
   AND EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
ON CONFLICT (class_code, program_no) DO NOTHING;
