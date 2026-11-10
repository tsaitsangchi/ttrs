-- 單位 aooi101 的標準功能鈕（docs/adr/0013-standard-maintenance-actions.md；unit_maintenance_standard_actions.feature）：
-- 查詢、複製、確認／取消確認、列印、匯出 Excel。TipTop gfe_file 沒有確認欄位，TTRS 加上標準確認欄位（SA9-1）；
-- 既有資料一律視為已確認（SA9-3），新增由應用程式寫入未確認。
ALTER TABLE unit
    ADD COLUMN confirmed    boolean NOT NULL DEFAULT true,
    ADD COLUMN confirmed_by varchar(10),
    ADD COLUMN confirmed_at timestamptz;
ALTER TABLE unit ALTER COLUMN confirmed SET DEFAULT false;
COMMENT ON COLUMN unit.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9；TipTop gfe_file 無對應欄位）';
COMMENT ON COLUMN unit.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN unit.confirmed_at IS 'TTRS 標準確認欄位：確認時間';

-- 功能代號（ADR 0013 SA7）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊；ADMIN 擁有全部功能
UPDATE program
   SET default_actions = default_actions || ', reproduce, confirm, notconfirm, output, exporttoexcel'
 WHERE program_no = 'unit-maintenance' AND default_actions NOT LIKE '%notconfirm%';
UPDATE program_permission
   SET actions = actions || ARRAY['reproduce', 'confirm', 'notconfirm', 'output', 'exporttoexcel']
 WHERE class_code = 'ADMIN' AND program_no = 'unit-maintenance' AND NOT ('notconfirm' = ANY(actions));
