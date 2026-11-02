-- aooi901 營運中心維護：補齊 azp_file 欄位。規格 specs/features/aoo/plant_maintenance.feature；
-- 舊表對照見 docs/legacy-map/aoo.yaml。資料庫連線欄位（azp03、azp04、azp20）與上線人數（azp09、azp10）不移植（P1、P8）。

ALTER TABLE plant
    ADD COLUMN time_zone     TEXT NOT NULL DEFAULT 'Asia/Taipei',
    ADD COLUMN address       TEXT,
    ADD COLUMN phone         TEXT,
    ADD COLUMN fax           TEXT,
    ADD COLUMN created_by    TEXT NOT NULL DEFAULT '',
    ADD COLUMN created_dept  TEXT NOT NULL DEFAULT '',
    ADD COLUMN created_on    DATE NOT NULL DEFAULT CURRENT_DATE,
    ADD COLUMN updated_by    TEXT,
    ADD COLUMN updated_on    DATE;

-- 既有資料的建立日期沿用 created_at（0001 建立；之後以 created_on 為準）
UPDATE plant SET created_on = created_at::date;

COMMENT ON TABLE plant IS 'TipTop: azp_file 營運中心 (aooi901)；TipTop 一個營運中心一個 schema，新系統為一筆資料';
COMMENT ON COLUMN plant.plant_id IS 'azp01 營運中心代碼（大寫、建立後不可修改，P3、P4）';
COMMENT ON COLUMN plant.name IS 'azp02 營運中心名稱';
COMMENT ON COLUMN plant.active IS '資料有效（TipTop azp_file 無此欄，P9）';
COMMENT ON COLUMN plant.created_at IS '建立時間（0001 建立；以 created_on 為準）';
COMMENT ON COLUMN plant.time_zone IS 'azp052 時區，改存 IANA 名稱（例 Asia/Taipei，P6）';
COMMENT ON COLUMN plant.address IS 'azp051 地址';
COMMENT ON COLUMN plant.phone IS 'azp06 營運中心電話';
COMMENT ON COLUMN plant.fax IS 'azp07 營運中心傳真';
COMMENT ON COLUMN plant.created_by IS '資料所有者（TipTop 無此欄，P12）';
COMMENT ON COLUMN plant.created_dept IS '資料所有部門（TipTop 無此欄，P12）';
COMMENT ON COLUMN plant.created_on IS '建立日期';
COMMENT ON COLUMN plant.updated_by IS '最後修改者';
COMMENT ON COLUMN plant.updated_on IS '最後修改日期';
