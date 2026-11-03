-- 程式代號改用英文（docs/adr/0008-english-program-codes.md；對照 docs/legacy-map/programs.yaml）
-- program.program_no 存英文代號（小寫英文、數字、連字號，最多 40 字）；TipTop 代號存 tiptop_program_no（zz01）。
-- 引用 program 的外鍵都是 ON UPDATE CASCADE，改代號時程式名稱、權限、特殊權限、選單項目跟著改。

ALTER TABLE program ALTER COLUMN program_no TYPE varchar(40);
ALTER TABLE program ADD COLUMN tiptop_program_no varchar(20);
ALTER TABLE program ADD CONSTRAINT program_tiptop_program_no_key UNIQUE (tiptop_program_no);
COMMENT ON COLUMN program.program_no IS '英文程式代號（ADR 0008）';
COMMENT ON COLUMN program.tiptop_program_no IS 'zz01 TipTop 程式代號：只用於轉檔與對照，不可重複';
ALTER TABLE user_account ALTER COLUMN menu_program TYPE varchar(40);

-- 已移植作業（E2）
UPDATE program p
   SET tiptop_program_no = p.program_no,
       program_no = m.english
  FROM (VALUES ('p_zz', 'program-maintenance'),
               ('p_zta', 'data-dictionary'),
               ('p_zx', 'user-maintenance'),
               ('p_zxw', 'user-permission-maintenance'),
               ('p_zw', 'permission-class-maintenance'),
               ('p_zy', 'program-permission-maintenance'),
               ('aooi901', 'plant-maintenance'),
               ('aooi030', 'department-maintenance'),
               ('aooi040', 'employee-maintenance'),
               ('aimi100', 'item-maintenance')) AS m(tiptop, english)
 WHERE p.program_no = m.tiptop;

-- 其他不符英文格式的代號（開發期資料、選單 menu_main 等）：轉小寫、底線改連字號（E5）
UPDATE program
   SET tiptop_program_no = program_no,
       program_no = replace(lower(program_no), '_', '-')
 WHERE program_no !~ '^[a-z][a-z0-9-]*$'
   AND replace(lower(program_no), '_', '-') ~ '^[a-z][a-z0-9-]*$'
   AND NOT EXISTS (SELECT 1 FROM program q WHERE q.program_no = replace(lower(program.program_no), '_', '-'));

-- 沒有外鍵、以文字記錄選單程式代號的欄位
UPDATE menu_item SET menu_no = replace(lower(menu_no), '_', '-') WHERE menu_no !~ '^[a-z][a-z0-9-]*$';
UPDATE user_account SET menu_program = replace(lower(menu_program), '_', '-')
 WHERE menu_program !~ '^[a-z][a-z0-9-]*$';
UPDATE permission_class SET default_menu = replace(lower(default_menu), '_', '-')
 WHERE default_menu IS NOT NULL AND default_menu !~ '^[a-z][a-z0-9-]*$';
