-- 依 tiptop/azz/sch/zz_file.sch、gaz_file.sch 更正欄位名稱並加上 TipTop 定義的長度上限（以字元計）。
-- 0003/0005 的命名依 4gl/4fd 推測，與資料字典說明不符：
--   zz05 列印選擇條件否、zz21 固定列印條件、zz22 固定列印方式、zz25 Set explain on
ALTER TABLE program RENAME COLUMN print_option TO print_conditions;
ALTER TABLE program RENAME COLUMN fixed_run_condition TO fixed_print_condition;
ALTER TABLE program RENAME COLUMN print_method TO fixed_print_method;
ALTER TABLE program RENAME COLUMN help_enabled TO sql_explain;
COMMENT ON COLUMN program.print_conditions IS 'TipTop: zz05 列印選擇條件否（報表結束時列印 QBE 條件）';
COMMENT ON COLUMN program.fixed_print_condition IS 'TipTop: zz21 固定列印條件';
COMMENT ON COLUMN program.fixed_print_method IS 'TipTop: zz22 固定列印方式';
COMMENT ON COLUMN program.sql_explain IS 'TipTop: zz25 Set explain on（SQL 執行計畫）';

ALTER TABLE program
    ALTER COLUMN program_no TYPE varchar(20),
    ALTER COLUMN module_code TYPE varchar(10),
    ALTER COLUMN program_type TYPE varchar(1),
    ALTER COLUMN default_actions TYPE varchar(3000),
    ALTER COLUMN run_command TYPE varchar(60),
    ALTER COLUMN program_group TYPE varchar(10),
    ALTER COLUMN fixed_print_condition TYPE varchar(80),
    ALTER COLUMN fixed_print_method TYPE varchar(1),
    ALTER COLUMN window_style TYPE varchar(20),
    ALTER COLUMN remark TYPE varchar(255);

ALTER TABLE program_name
    ALTER COLUMN name TYPE varchar(500),
    ALTER COLUMN report_title TYPE varchar(255);
