-- 部門維護 aooi030、員工維護 aooi040 的程式與起始權限（同 0015：ADMIN 全部功能，authorization.feature AQ11）
-- 需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊；表本身在工廠 schema（migrations/plant/0002）
INSERT INTO program (program_no, module_code, program_type, run_command, report_ext_mode,
                     default_actions, created_by, created_dept)
VALUES
    ('aooi030', 'AOO', 'I', '$FGLRUN $AOOi/aooi030', '1', 'query, detail, account', 'tiptop', ''),
    ('aooi040', 'AOO', 'I', '$FGLRUN $AOOi/aooi040', '1', 'query, detail', 'tiptop', '')
ON CONFLICT (program_no) DO NOTHING;

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
 WHERE p.program_no IN ('aooi030', 'aooi040')
   AND EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
ON CONFLICT (class_code, program_no) DO NOTHING;
