-- 分類碼維護（TipTop aooi300；specs/features/aoo/classification_code.feature）的程式與起始權限
-- 同 0017：ADMIN 全部功能（authorization.feature AQ11）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊。
-- 表本身在工廠 schema（migrations/plant/0003）
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
VALUES ('classification-code-maintenance', 'AOO', 'I', 'aooi300', '1', 'query, detail', 'tiptop', '')
ON CONFLICT (program_no) DO NOTHING;

INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', p.program_no, ARRAY['query', 'detail'], 'tiptop', ''
  FROM program p
 WHERE p.program_no = 'classification-code-maintenance'
   AND EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
ON CONFLICT (class_code, program_no) DO NOTHING;

INSERT INTO program_name (program_no, lang, name)
VALUES ('classification-code-maintenance', 'zh-TW', '分類碼維護'),
       ('classification-code-maintenance', 'en', 'Classification Code Maintenance'),
       ('classification-code-maintenance', 'zh-CN', '分类码维护')
ON CONFLICT (program_no, lang) DO NOTHING;
