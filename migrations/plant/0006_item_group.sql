-- 工廠層（docs/adr/0007-multi-plant.md）：料件分群（TipTop aimi110 / imz_file；specs/features/aim/item_group.feature）
-- 第一階段 41 欄（IG3）；單位外鍵到共用 schema 的 unit（UN5：被引用的單位不可刪除）
CREATE TABLE item_group (
    group_code         varchar(10) PRIMARY KEY,
    description        varchar(80),
    source_code        varchar(1),
    stock_unit         varchar(4) REFERENCES public.unit (unit_code),
    catalog_ref        varchar(20),
    supplier_control   varchar(1) NOT NULL DEFAULT '0',
    drawing_no         varchar(20),
    software           boolean NOT NULL DEFAULT false,
    engineering        boolean NOT NULL DEFAULT false,
    insert_qpa_check   boolean NOT NULL DEFAULT false,
    insert_position    boolean NOT NULL DEFAULT false,
    material_category  varchar(10),
    coproduct          boolean NOT NULL DEFAULT false,
    inspection         boolean NOT NULL DEFAULT false,
    repetitive         boolean NOT NULL DEFAULT false,
    abc_code           varchar(1),
    consumable         boolean NOT NULL DEFAULT false,
    replenish_policy   varchar(1),
    reorder_point      numeric(15,3) CHECK (reorder_point >= 0),
    reorder_qty        numeric(15,3) CHECK (reorder_qty >= 0),
    safety_stock       numeric(15,3) CHECK (safety_stock >= 0),
    safety_period      numeric(15,3) CHECK (safety_period >= 0),
    shelf_life_days    integer CHECK (shelf_life_days >= 0),
    mrp_bucket_days    integer CHECK (mrp_bucket_days >= 0),
    warehouse          varchar(10),
    location           varchar(10),
    stock_keeper       varchar(10),
    sales_stat_unit    varchar(4) REFERENCES public.unit (unit_code),
    second_unit        varchar(4) REFERENCES public.unit (unit_code),
    pricing_unit       varchar(4) REFERENCES public.unit (unit_code),
    other_group1       varchar(10),
    other_group2       varchar(10),
    other_group3       varchar(10),
    other_group4       varchar(10),
    account            varchar(24),
    account2           varchar(24),
    bonded             boolean NOT NULL DEFAULT false,
    bonded_class       varchar(4),
    tariff_no          varchar(4),
    bonded_type        varchar(1),
    active             boolean NOT NULL DEFAULT true,
    created_by         text NOT NULL,
    created_dept       text NOT NULL,
    created_on         date NOT NULL DEFAULT CURRENT_DATE,
    updated_by         text,
    updated_on         date,
    version            integer NOT NULL DEFAULT 1,
    created_at         timestamptz NOT NULL DEFAULT now(),
    updated_at         timestamptz
);
COMMENT ON TABLE item_group IS 'TipTop: imz_file 料件分類基本資料檔（aimi110 基本／庫存資料）；工廠層（IG2）';
COMMENT ON COLUMN item_group.group_code IS 'imz01 料件分群碼（區分大小寫、建立後不可修改）';
COMMENT ON COLUMN item_group.description IS 'imz02 說明';
COMMENT ON COLUMN item_group.source_code IS 'imz08 來源碼（C／T／D／A／M／P／X／K／U／V／R／Z／S）';
COMMENT ON COLUMN item_group.stock_unit IS 'imz25 庫存單位';
COMMENT ON COLUMN item_group.catalog_ref IS 'imz03 目錄參考號碼';
COMMENT ON COLUMN item_group.supplier_control IS 'imz72 是否需做供應商的管制';
COMMENT ON COLUMN item_group.drawing_no IS 'imz04 工程圖號';
COMMENT ON COLUMN item_group.software IS 'imz105 是否為軟體物件';
COMMENT ON COLUMN item_group.engineering IS 'imz14 是否為工程料件';
COMMENT ON COLUMN item_group.insert_qpa_check IS 'imz147 插件位置與 QPA 是否要勾稽';
COMMENT ON COLUMN item_group.insert_position IS 'imz107 插件位置';
COMMENT ON COLUMN item_group.material_category IS 'imz109 材料類別（分類碼類別 8）';
COMMENT ON COLUMN item_group.coproduct IS 'imz903 可否做聯產品入庫';
COMMENT ON COLUMN item_group.inspection IS 'imz24 檢驗否';
COMMENT ON COLUMN item_group.repetitive IS 'imz911 是否為重覆性生產料件';
COMMENT ON COLUMN item_group.abc_code IS 'imz07 ABC 碼';
COMMENT ON COLUMN item_group.consumable IS 'imz70 消耗料件';
COMMENT ON COLUMN item_group.replenish_policy IS 'imz37 補貨策略碼（0–5）';
COMMENT ON COLUMN item_group.reorder_point IS 'imz38 再補貨點';
COMMENT ON COLUMN item_group.reorder_qty IS 'imz99 再補貨量';
COMMENT ON COLUMN item_group.safety_stock IS 'imz27 安全庫存量';
COMMENT ON COLUMN item_group.safety_period IS 'imz28 安全庫存期間';
COMMENT ON COLUMN item_group.shelf_life_days IS 'imz71 儲存有效天數';
COMMENT ON COLUMN item_group.mrp_bucket_days IS 'imz909 MRP 匯總時距（天）';
COMMENT ON COLUMN item_group.warehouse IS 'imz35 主要倉庫別（倉庫未移植，不檢查）';
COMMENT ON COLUMN item_group.location IS 'imz36 主要儲位別';
COMMENT ON COLUMN item_group.stock_keeper IS 'imz23 倉管員（本營運中心的員工）';
COMMENT ON COLUMN item_group.sales_stat_unit IS 'imz17 銷售統計單位';
COMMENT ON COLUMN item_group.second_unit IS 'imz907 第二單位';
COMMENT ON COLUMN item_group.pricing_unit IS 'imz908 計價單位';
COMMENT ON COLUMN item_group.other_group1 IS 'imz09 其他分群碼一（分類碼類別 D）';
COMMENT ON COLUMN item_group.other_group2 IS 'imz10 其他分群碼二（分類碼類別 E）';
COMMENT ON COLUMN item_group.other_group3 IS 'imz11 其他分群碼三（分類碼類別 F）';
COMMENT ON COLUMN item_group.other_group4 IS 'imz12 其他分群碼四（分類碼類別 G）';
COMMENT ON COLUMN item_group.account IS 'imz39 料件所屬會計科目（會計科目未移植，不檢查）';
COMMENT ON COLUMN item_group.account2 IS 'imz391 會計科目二';
COMMENT ON COLUMN item_group.bonded IS 'imz15 保稅與否';
COMMENT ON COLUMN item_group.bonded_class IS 'imz19 保稅料件進出口分類統計用類';
COMMENT ON COLUMN item_group.tariff_no IS 'imz21 保稅料件稅則編號';
COMMENT ON COLUMN item_group.bonded_type IS 'imz106 保稅料件型態';
COMMENT ON COLUMN item_group.active IS 'imzacti 資料有效碼';
COMMENT ON COLUMN item_group.created_by IS 'imzuser 資料所有者';
COMMENT ON COLUMN item_group.created_dept IS 'imzgrup 資料所有群';
COMMENT ON COLUMN item_group.updated_by IS 'imzmodu 資料更改者';
COMMENT ON COLUMN item_group.updated_on IS 'imzdate 最近修改日';

CREATE TRIGGER item_group_version BEFORE UPDATE ON item_group
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER item_group_change_log AFTER INSERT OR UPDATE OR DELETE ON item_group
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('group_code');
