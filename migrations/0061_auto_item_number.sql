-- 料件自動編號（TipTop aimi100 自動編碼＋aoos010 aza28；specs/features/aim/item_auto_number.feature AN1–AN7）
-- 整體管理系統參數加「料件編號自動編碼否」（general_parameters.feature GP9 由延後改為移植）。預設否。
ALTER TABLE system_parameter ADD COLUMN auto_item_number boolean NOT NULL DEFAULT false;
COMMENT ON COLUMN system_parameter.auto_item_number IS 'aza28 料件編號自動編碼否：是時料件新增、複製可依編碼類別取料號（item_auto_number.feature AN1）';
