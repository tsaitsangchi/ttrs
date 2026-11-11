-- 匯率自動匯入（TipTop aooi070 ★ auto_import；specs/features/aoo/exchange_rate_import.feature IM1–IM9）
-- 幣別加「匯入代號」（IM6）；每日匯率維護加功能 auto_import（IM9）

ALTER TABLE currency ADD COLUMN import_code varchar(4);
COMMENT ON COLUMN currency.import_code IS 'TTRS 新增：匯率自動匯入的來源幣別代號（如 MCY 的匯入代號 CNY；空白表示同幣別代號，IM6）';
-- TipTop 寫死的對照（aooi070.4gl:1642 CNY → MCY）
UPDATE currency SET import_code = 'CNY' WHERE currency_code = 'MCY' AND import_code IS NULL;

UPDATE program
   SET default_actions = default_actions || ', auto_import'
 WHERE program_no = 'daily-exchange-rate-maintenance' AND default_actions NOT LIKE '%auto_import%';
UPDATE program_permission
   SET actions = array_append(actions, 'auto_import')
 WHERE class_code = 'ADMIN' AND program_no = 'daily-exchange-rate-maintenance'
   AND NOT ('auto_import' = ANY (actions));
