-- 部門、員工的「確認」功能鈕（docs/adr/0013-standard-maintenance-actions.md SA9；
-- department_employee_standard_actions.feature）。TipTop gem_file、gen_file 沒有確認欄位，TTRS 加上標準確認欄位（SA9-1）；
-- 既有資料一律視為已確認（SA9-3），新增由應用程式寫入未確認。
ALTER TABLE department
    ADD COLUMN confirmed    boolean NOT NULL DEFAULT true,
    ADD COLUMN confirmed_by varchar(10),
    ADD COLUMN confirmed_at timestamptz;
ALTER TABLE department ALTER COLUMN confirmed SET DEFAULT false;
COMMENT ON COLUMN department.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9；TipTop gem_file 無對應欄位）';
COMMENT ON COLUMN department.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN department.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
ALTER TABLE employee
    ADD COLUMN confirmed    boolean NOT NULL DEFAULT true,
    ADD COLUMN confirmed_by varchar(10),
    ADD COLUMN confirmed_at timestamptz;
ALTER TABLE employee ALTER COLUMN confirmed SET DEFAULT false;
COMMENT ON COLUMN employee.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9；TipTop gen_file 無對應欄位）';
COMMENT ON COLUMN employee.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN employee.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
