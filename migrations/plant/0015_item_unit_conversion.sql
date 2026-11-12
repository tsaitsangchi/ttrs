-- 工廠層（docs/adr/0007-multi-plant.md）：料件單位換算（TipTop aooi103 / smd_file；specs/features/aoo/unit_conversion.feature UC2）
-- 同料件 A→B 與 B→A 視為同一組（UC3，由程式檢查）
CREATE TABLE item_unit_conversion (
    item_no         text NOT NULL REFERENCES item (item_no) ON DELETE CASCADE,
    from_unit       text NOT NULL REFERENCES public.unit (unit_code),
    to_unit         text NOT NULL REFERENCES public.unit (unit_code),
    from_quantity   numeric(15,3) NOT NULL CHECK (from_quantity > 0),
    to_quantity     numeric(15,3) NOT NULL CHECK (to_quantity > 0),
    note            varchar(80),
    active          boolean NOT NULL DEFAULT true,
    created_by      text NOT NULL,
    created_dept    text NOT NULL DEFAULT '',
    created_on      date NOT NULL DEFAULT CURRENT_DATE,
    updated_by      text,
    updated_on      date,
    version         integer NOT NULL DEFAULT 1,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz,
    PRIMARY KEY (item_no, from_unit, to_unit),
    CHECK (from_unit <> to_unit)
);
COMMENT ON TABLE item_unit_conversion IS 'TipTop: smd_file 料件單位換算資料檔（aooi103，item-unit-conversion-maintenance）；工廠層（UC2）';
COMMENT ON COLUMN item_unit_conversion.item_no IS 'smd01 料件編號';
COMMENT ON COLUMN item_unit_conversion.from_unit IS 'smd02 來源計算單位';
COMMENT ON COLUMN item_unit_conversion.to_unit IS 'smd03 目的計算單位';
COMMENT ON COLUMN item_unit_conversion.from_quantity IS 'smd04 來源單位兌換數量（> 0，UC4）';
COMMENT ON COLUMN item_unit_conversion.to_quantity IS 'smd06 目的單位兌換數量（> 0，UC4）';
COMMENT ON COLUMN item_unit_conversion.note IS 'smd05 說明';
COMMENT ON COLUMN item_unit_conversion.active IS 'smdacti 資料有效碼（停用的不採用，UC7）';
COMMENT ON COLUMN item_unit_conversion.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TRIGGER item_unit_conversion_version BEFORE UPDATE ON item_unit_conversion
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER item_unit_conversion_change_log AFTER INSERT OR UPDATE OR DELETE ON item_unit_conversion
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('item_no,from_unit,to_unit');
