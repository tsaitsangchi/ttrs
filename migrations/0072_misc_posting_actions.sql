-- 雜項發料、收料、報廢的庫存過帳 stock_post 與過帳還原 undo_post（misc_issue_posting.feature PT1）：
-- 補進作業的功能清單與 ADMIN 起始權限。需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊。
UPDATE program
   SET default_actions = default_actions || ', stock_post, undo_post'
 WHERE program_no IN ('misc-issue', 'misc-receipt', 'misc-scrap')
   AND default_actions NOT LIKE '%stock_post%';
UPDATE program_permission
   SET actions = actions || ARRAY['stock_post', 'undo_post']
 WHERE class_code = 'ADMIN'
   AND program_no IN ('misc-issue', 'misc-receipt', 'misc-scrap')
   AND NOT ('stock_post' = ANY (actions));
