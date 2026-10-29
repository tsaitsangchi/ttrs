-- 依 p_zz.4fd 畫面標籤更正欄位名稱（0003 依程式碼推測命名有誤）
--   zz10 Program Group（開窗 q_gaw）、zz22 Print Method、zz21 Fixed Executed Condition
ALTER TABLE program RENAME COLUMN print_setting TO program_group;
ALTER TABLE program RENAME COLUMN print_default TO print_method;
ALTER TABLE program RENAME COLUMN quick_run_data TO fixed_run_condition;
COMMENT ON COLUMN program.program_group IS 'TipTop: zz10 程式群組';
COMMENT ON COLUMN program.print_method IS 'TipTop: zz22 列印方式';
COMMENT ON COLUMN program.fixed_run_condition IS 'TipTop: zz21 固定執行條件';
