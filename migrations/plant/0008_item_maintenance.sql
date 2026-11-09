-- 工廠層（docs/adr/0007-multi-plant.md）：料件基本資料第一階段（TipTop aimi100 / ima_file，以客製 topcust/cim 為準；
-- specs/features/aim/item_maintenance.feature IM1–IM31）。擴充早期骨架的 item 表（IM10），欄位對照見 docs/legacy-map/aim.yaml
ALTER TABLE item
    ADD COLUMN group_code             varchar(10) REFERENCES item_group (group_code),
    ADD COLUMN source_code            varchar(1) NOT NULL DEFAULT 'P',
    ADD COLUMN configurable_item_no   varchar(40),
    ADD COLUMN current_version        varchar(10),
    ADD COLUMN catalog_ref            varchar(20),
    ADD COLUMN supplier_control       varchar(1) NOT NULL DEFAULT '0' CHECK (supplier_control IN ('0', '1', '2', '3')),
    ADD COLUMN product_class          varchar(10) REFERENCES product_class (class_code),
    ADD COLUMN drawing_no             varchar(255),
    ADD COLUMN apply_dept             varchar(10),
    ADD COLUMN software               boolean NOT NULL DEFAULT false,
    ADD COLUMN engineering            boolean NOT NULL DEFAULT false,
    ADD COLUMN insert_position        boolean NOT NULL DEFAULT false,
    ADD COLUMN insert_qpa_check       boolean NOT NULL DEFAULT false,
    ADD COLUMN material_category      varchar(10),
    ADD COLUMN coproduct              boolean NOT NULL DEFAULT false,
    ADD COLUMN inspection             boolean NOT NULL DEFAULT false,
    ADD COLUMN repetitive             boolean NOT NULL DEFAULT false,
    ADD COLUMN pcn                    varchar(20),
    ADD COLUMN abc_code               varchar(1) NOT NULL DEFAULT 'A' CHECK (abc_code IN ('A', 'B', 'C')),
    ADD COLUMN consumable             boolean NOT NULL DEFAULT false,
    ADD COLUMN replenish_policy       varchar(1) CHECK (replenish_policy IN ('0', '1', '2', '3', '4', '5')),
    ADD COLUMN economic_order_qty     numeric(15,3) NOT NULL DEFAULT 1 CHECK (economic_order_qty > 0),
    ADD COLUMN average_order_qty      numeric(15,3) NOT NULL DEFAULT 1 CHECK (average_order_qty > 0),
    ADD COLUMN safety_stock           numeric(15,3) CHECK (safety_stock >= 0),
    ADD COLUMN safety_period          numeric(15,3) CHECK (safety_period >= 0),
    ADD COLUMN max_stock_qty          numeric(15,3) CHECK (max_stock_qty >= 0),
    ADD COLUMN shelf_life_days        integer CHECK (shelf_life_days >= 0),
    ADD COLUMN warehouse              varchar(10),
    ADD COLUMN location               varchar(10),
    ADD COLUMN stock_keeper           varchar(10),
    ADD COLUMN second_unit            varchar(4) REFERENCES public.unit (unit_code),
    ADD COLUMN pricing_unit           varchar(4) REFERENCES public.unit (unit_code),
    ADD COLUMN other_group1           varchar(10),
    ADD COLUMN other_group2           varchar(10),
    ADD COLUMN other_group3           varchar(10),
    ADD COLUMN other_group4           varchar(10),
    ADD COLUMN account                varchar(24),
    ADD COLUMN account2               varchar(24),
    ADD COLUMN bonded                 boolean NOT NULL DEFAULT false,
    ADD COLUMN full_name              varchar(100),
    ADD COLUMN english_name           varchar(80),
    ADD COLUMN planned_discontinue_on date,
    ADD COLUMN life_cycle             varchar(1) CHECK (life_cycle IN ('1', '2', '3', '4', '5', '6')),
    ADD COLUMN carton_barcode         varchar(40),
    ADD COLUMN active_code            varchar(1) NOT NULL DEFAULT 'P' CHECK (active_code IN ('P', 'Y', 'N')),
    ADD COLUMN status_code            varchar(1) NOT NULL DEFAULT '0' CHECK (status_code IN ('0', '1')),
    ADD COLUMN order_invalid          boolean NOT NULL DEFAULT true,
    ADD COLUMN phase_out              boolean NOT NULL DEFAULT false,
    ADD COLUMN unit_usage             varchar(1) NOT NULL DEFAULT '1',
    ADD COLUMN low_level_code         integer NOT NULL DEFAULT 99,
    ADD COLUMN bom_changed            varchar(1),
    ADD COLUMN coproduct_this_month   boolean NOT NULL DEFAULT false,
    ADD COLUMN new_part_status        varchar(8) NOT NULL DEFAULT 'NNNNNNNN',
    ADD COLUMN data_source            varchar(10) NOT NULL DEFAULT '',
    ADD COLUMN item_created_on        date NOT NULL DEFAULT CURRENT_DATE,
    ADD COLUMN first_shipped_on       date,
    ADD COLUMN first_received_on      date,
    ADD COLUMN slow_moving_on         date,
    ADD COLUMN last_period_purchase_on date,
    ADD COLUMN last_received_on       date,
    ADD COLUMN last_issued_on         date,
    ADD COLUMN last_txn_on            date,
    ADD COLUMN last_count_on          date,
    ADD COLUMN created_by             text NOT NULL DEFAULT '',
    ADD COLUMN created_dept           text NOT NULL DEFAULT '',
    ADD COLUMN created_on             date NOT NULL DEFAULT CURRENT_DATE,
    ADD COLUMN updated_by             text,
    ADD COLUMN updated_on             date,
    ADD COLUMN version                integer NOT NULL DEFAULT 1;

-- 早期骨架的狀態轉成資料有效碼＋狀態碼（IM27）：未確認 → P／0、已確認 → Y／1、作廢 → N／0
UPDATE item SET active_code = CASE status WHEN 'confirmed' THEN 'Y' WHEN 'voided' THEN 'N' ELSE 'P' END,
                status_code = CASE status WHEN 'confirmed' THEN '1' ELSE '0' END,
                item_created_on = created_at::date,
                created_on = created_at::date;
ALTER TABLE item DROP COLUMN status;
-- 最後修改時間只在修改後才有（ADR 0012 §1）
ALTER TABLE item ALTER COLUMN updated_at DROP NOT NULL, ALTER COLUMN updated_at DROP DEFAULT;
ALTER TABLE item ADD CONSTRAINT item_no_length CHECK (char_length(item_no) <= 40) NOT VALID;

COMMENT ON TABLE item IS 'TipTop: ima_file 料件資料主檔（aimi100，客製 topcust/cim）；工廠層（IM9）';
COMMENT ON COLUMN item.item_no IS 'ima01 料件編號（區分大小寫、最多 40 字，IM21）';
COMMENT ON COLUMN item.name IS 'ima02 品名（zh-TW；其他語系在 item_text）';
COMMENT ON COLUMN item.spec IS 'ima021 規格';
COMMENT ON COLUMN item.stock_unit IS 'ima25 庫存單位';
COMMENT ON COLUMN item.group_code IS 'ima06 分群碼';
COMMENT ON COLUMN item.source_code IS 'ima08 來源碼（C/T/D/A/M/P/X/K/U/V/R/Z/S）';
COMMENT ON COLUMN item.configurable_item_no IS 'ima13 規格組件料件編號';
COMMENT ON COLUMN item.current_version IS 'ima05 目前使用版本';
COMMENT ON COLUMN item.catalog_ref IS 'ima03 目錄參考號碼';
COMMENT ON COLUMN item.supplier_control IS 'ima915 是否需做供應商的管制（0 不管制、1 請購、2 採購、3 二者）';
COMMENT ON COLUMN item.product_class IS 'ima131 產品分類編號（客製必填）';
COMMENT ON COLUMN item.drawing_no IS 'ima04 工程圖號';
COMMENT ON COLUMN item.apply_dept IS 'ta_ima08 申請部門（客製）';
COMMENT ON COLUMN item.software IS 'ima105 是否為軟體物件';
COMMENT ON COLUMN item.engineering IS 'ima14 是否為工程料件';
COMMENT ON COLUMN item.insert_position IS 'ima107 插件位置';
COMMENT ON COLUMN item.insert_qpa_check IS 'ima147 插件位置與 QPA 是否要勾稽';
COMMENT ON COLUMN item.material_category IS 'ima109 材料類別（分類碼類別 8）';
COMMENT ON COLUMN item.coproduct IS 'ima903 可否做聯產品入庫';
COMMENT ON COLUMN item.inspection IS 'ima24 檢驗碼';
COMMENT ON COLUMN item.repetitive IS 'ima911 是否為重覆性生產料件';
COMMENT ON COLUMN item.pcn IS 'ta_ima02 PCN（客製）';
COMMENT ON COLUMN item.abc_code IS 'ima07 ABC 碼';
COMMENT ON COLUMN item.consumable IS 'ima70 消耗料件';
COMMENT ON COLUMN item.replenish_policy IS 'ima37 補貨策略碼（0–5，IM23）';
COMMENT ON COLUMN item.economic_order_qty IS 'ima51 經濟訂購量';
COMMENT ON COLUMN item.average_order_qty IS 'ima52 平均訂購量';
COMMENT ON COLUMN item.safety_stock IS 'ima27 安全庫存量';
COMMENT ON COLUMN item.safety_period IS 'ima28 安全庫存期間';
COMMENT ON COLUMN item.max_stock_qty IS 'ima271 最高儲存數量';
COMMENT ON COLUMN item.shelf_life_days IS 'ima71 儲存有效天數';
COMMENT ON COLUMN item.warehouse IS 'ima35 主要倉庫別（倉庫未移植，不檢查）';
COMMENT ON COLUMN item.location IS 'ima36 主要儲位別';
COMMENT ON COLUMN item.stock_keeper IS 'ima23 倉管員';
COMMENT ON COLUMN item.second_unit IS 'ima907 第二單位';
COMMENT ON COLUMN item.pricing_unit IS 'ima908 計價單位';
COMMENT ON COLUMN item.other_group1 IS 'ima09 其他分群碼一（分類碼類別 D）';
COMMENT ON COLUMN item.other_group2 IS 'ima10 其他分群碼二（類別 E）';
COMMENT ON COLUMN item.other_group3 IS 'ima11 其他分群碼三（類別 F）';
COMMENT ON COLUMN item.other_group4 IS 'ima12 其他分群碼四（類別 G）';
COMMENT ON COLUMN item.account IS 'ima39 料件所屬會計科目（未移植，不檢查）';
COMMENT ON COLUMN item.account2 IS 'ima391 會計科目二';
COMMENT ON COLUMN item.bonded IS 'ima15 保稅與否';
COMMENT ON COLUMN item.full_name IS 'ima1001 中文名稱（全稱）';
COMMENT ON COLUMN item.english_name IS 'ima1002 英文名稱';
COMMENT ON COLUMN item.planned_discontinue_on IS 'ima1015 預計停產日';
COMMENT ON COLUMN item.life_cycle IS 'ima1014 產品週期狀態（1–6）';
COMMENT ON COLUMN item.carton_barcode IS 'ima1016 箱條碼';
COMMENT ON COLUMN item.active_code IS 'imaacti 資料有效碼（P 處理中、Y 已確認、N 無效）';
COMMENT ON COLUMN item.status_code IS 'ima1010 狀態碼（0 開立、1 確認）';
COMMENT ON COLUMN item.order_invalid IS 'ta_ima01 訂單無效否（客製，新增預設是）';
COMMENT ON COLUMN item.phase_out IS 'ima140 Phase Out 否';
COMMENT ON COLUMN item.unit_usage IS 'ima906 單位使用方式（固定 1，IM14）';
COMMENT ON COLUMN item.low_level_code IS 'ima16 低階碼（新增 99）';
COMMENT ON COLUMN item.bom_changed IS 'ima146 BOM 異動否';
COMMENT ON COLUMN item.coproduct_this_month IS 'ima905 當月是否入聯產品';
COMMENT ON COLUMN item.new_part_status IS 'ima93 新料件處理狀況';
COMMENT ON COLUMN item.data_source IS 'ima916 資料來源（營運中心）';
COMMENT ON COLUMN item.item_created_on IS 'ima901 料件建檔日期';
COMMENT ON COLUMN item.first_shipped_on IS 'ima1012 首次出貨日';
COMMENT ON COLUMN item.first_received_on IS 'ima1013 首次入庫日';
COMMENT ON COLUMN item.slow_moving_on IS 'ima902 呆滯日期';
COMMENT ON COLUMN item.last_period_purchase_on IS 'ima881 期間採購最近採購日期';
COMMENT ON COLUMN item.last_received_on IS 'ima73 最近入庫日';
COMMENT ON COLUMN item.last_issued_on IS 'ima74 最近出庫日';
COMMENT ON COLUMN item.last_txn_on IS 'ima29 最近異動日期';
COMMENT ON COLUMN item.last_count_on IS 'ima30 最近盤點日期';
COMMENT ON COLUMN item.created_by IS 'imauser 資料所有者';
COMMENT ON COLUMN item.created_dept IS 'imagrup 資料所有群';
COMMENT ON COLUMN item.updated_by IS 'imamodu 資料更改者';
COMMENT ON COLUMN item.updated_on IS 'imadate 最近修改日';
COMMENT ON COLUMN item.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN item.created_at IS '建立時間（到秒）；TTRS 新增';
COMMENT ON COLUMN item.updated_at IS '最後修改時間（到秒）；imadate 只到日';

CREATE TRIGGER item_version BEFORE UPDATE ON item
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER item_change_log AFTER INSERT OR UPDATE OR DELETE ON item
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('item_no');

-- 料件的多語言品名、規格（ADR 0005；IM7，TipTop「更新多語系名稱資料」）；zh-TW 為 item.name／spec
CREATE TABLE item_text (
    item_no  text NOT NULL REFERENCES item (item_no) ON DELETE CASCADE,
    lang     varchar(5) NOT NULL CHECK (lang IN ('en', 'zh-CN')),
    name     varchar(120),
    spec     varchar(120),
    PRIMARY KEY (item_no, lang)
);
COMMENT ON TABLE item_text IS 'TipTop: aimi100 更新多語系名稱資料（品名 ima02、規格 ima021 的其他語系）；工廠層';
CREATE TRIGGER item_text_change_log AFTER INSERT OR UPDATE OR DELETE ON item_text
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('item_no,lang');
