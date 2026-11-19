-- 工廠層（docs/adr/0007-multi-plant.md）：料件預設倉儲（TipTop aimi202 / imf_file；specs/features/aim/item_storage.feature）
CREATE TABLE item_storage (
    item_no         text NOT NULL REFERENCES item (item_no) ON DELETE CASCADE,
    warehouse_code  varchar(10) NOT NULL REFERENCES warehouse (warehouse_code),
    location_code   varchar(10) NOT NULL DEFAULT '',
    max_qty         numeric(15, 3) CHECK (max_qty >= 0),
    version         integer NOT NULL DEFAULT 1,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz,
    PRIMARY KEY (item_no, warehouse_code, location_code)
);
CREATE INDEX item_storage_place ON item_storage (warehouse_code, location_code);
COMMENT ON TABLE item_storage IS 'TipTop: imf_file 料件預設倉庫/存放位置資料檔（aimi202，item-storage-maintenance）；工廠層（IS2）；刪除料件時連同刪除';
COMMENT ON COLUMN item_storage.item_no IS 'imf01 料件編號';
COMMENT ON COLUMN item_storage.warehouse_code IS 'imf02 倉庫編號（新增、修改時須為有效倉庫，IS3(a)）';
COMMENT ON COLUMN item_storage.location_code IS 'imf03 存放位置；空字串為整個倉庫（TipTop 存 '' ''，IS3(b)）；非空白須為該倉庫的儲位（由 app 層檢查）';
COMMENT ON COLUMN item_storage.max_qty IS 'imf04 最高存量限制；NULL 為不限制（TipTop 999999999 轉為 NULL，IS3(d)）';
COMMENT ON COLUMN item_storage.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TRIGGER item_storage_version BEFORE UPDATE ON item_storage
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER item_storage_change_log AFTER INSERT OR UPDATE OR DELETE ON item_storage
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('item_no,warehouse_code,location_code');
