-- 工廠層（docs/adr/0007-multi-plant.md）：倉庫與儲位（TipTop aimi201、aimi200 / imd_file、ime_file；specs/features/aim/warehouse.feature）
CREATE TABLE warehouse (
    warehouse_code  varchar(10) PRIMARY KEY CHECK (char_length(warehouse_code) >= 1),
    name            varchar(80),
    address         varchar(255),
    contact         varchar(40),
    phone           varchar(40),
    account         varchar(24),
    account2        varchar(24),
    cost_group      varchar(1),
    cost_center     varchar(10) REFERENCES department (dept_code),
    storage_class   char(1) NOT NULL CHECK (storage_class IN ('S', 'W')),
    usable          boolean NOT NULL DEFAULT true,
    mrp_usable      boolean NOT NULL DEFAULT true CHECK (usable OR NOT mrp_usable),
    bonded          boolean NOT NULL DEFAULT false,
    issue_priority  integer NOT NULL DEFAULT 0 CHECK (issue_priority >= 0),
    ship_priority   integer NOT NULL DEFAULT 0 CHECK (ship_priority >= 0),
    active          boolean NOT NULL DEFAULT true,
    confirmed       boolean NOT NULL DEFAULT false,
    confirmed_by    varchar(10),
    confirmed_at    timestamptz,
    created_by      text NOT NULL,
    created_dept    text NOT NULL DEFAULT '',
    created_on      date NOT NULL DEFAULT CURRENT_DATE,
    updated_by      text,
    updated_on      date,
    version         integer NOT NULL DEFAULT 1,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz
);
COMMENT ON TABLE warehouse IS 'TipTop: imd_file 倉庫別資料主檔（aimi201、aimi200，warehouse-maintenance）；工廠層（WH2）';
COMMENT ON COLUMN warehouse.warehouse_code IS 'imd01 倉庫別（區分大小寫、建立後不可修改，WH3(a)）';
COMMENT ON COLUMN warehouse.name IS 'imd02 倉庫名稱';
COMMENT ON COLUMN warehouse.address IS 'imd03 地址一（imd04、imd05 NO USE 不移植）';
COMMENT ON COLUMN warehouse.contact IS 'imd06 負責人';
COMMENT ON COLUMN warehouse.phone IS 'imd07 電話';
COMMENT ON COLUMN warehouse.account IS 'imd08 存貨科目（會計科目未移植，不檢查）';
COMMENT ON COLUMN warehouse.account2 IS 'imd081 存貨科目二';
COMMENT ON COLUMN warehouse.cost_group IS 'imd09 成本庫別（用途待確認，WH3(d)）';
COMMENT ON COLUMN warehouse.cost_center IS 'imd16 成本中心（成本／利潤中心部門，WH3(f)）';
COMMENT ON COLUMN warehouse.storage_class IS 'imd10 倉儲類別：S 一般性倉庫、W 在製品倉庫';
COMMENT ON COLUMN warehouse.usable IS 'imd11 是否為可用倉儲';
COMMENT ON COLUMN warehouse.mrp_usable IS 'imd12 是否為 MRP 可用倉儲（不可用倉儲必定 MRP 不可用，WH3(b)）';
COMMENT ON COLUMN warehouse.bonded IS 'imd13 保稅與否';
COMMENT ON COLUMN warehouse.issue_priority IS 'imd14 生產發料優先順序';
COMMENT ON COLUMN warehouse.ship_priority IS 'imd15 銷售領料優先順序';
COMMENT ON COLUMN warehouse.active IS 'imdacti 資料有效碼';
COMMENT ON COLUMN warehouse.confirmed IS 'TTRS 標準確認欄位（ADR 0013 SA9；imd_file 無對應欄位）';
COMMENT ON COLUMN warehouse.created_by IS 'imduser 資料所有者';
COMMENT ON COLUMN warehouse.created_dept IS 'imdgrup 資料所有群';
COMMENT ON COLUMN warehouse.updated_by IS 'imdmodu 資料更改者（修改儲位時一併更新，WH10）';
COMMENT ON COLUMN warehouse.updated_on IS 'imddate 最近修改日';
COMMENT ON COLUMN warehouse.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TABLE warehouse_location (
    warehouse_code  varchar(10) NOT NULL REFERENCES warehouse (warehouse_code) ON DELETE CASCADE,
    location_code   varchar(10) NOT NULL CHECK (char_length(location_code) >= 1),
    name            varchar(80),
    storage_class   char(1) NOT NULL CHECK (storage_class IN ('S', 'W')),
    usable          boolean NOT NULL DEFAULT true,
    mrp_usable      boolean NOT NULL DEFAULT true CHECK (usable OR NOT mrp_usable),
    bonded          boolean NOT NULL DEFAULT false,
    issue_priority  integer NOT NULL DEFAULT 0 CHECK (issue_priority >= 0),
    ship_priority   integer NOT NULL DEFAULT 0 CHECK (ship_priority >= 0),
    account_method  char(1) NOT NULL DEFAULT '0' CHECK (account_method IN ('0', '1', '2')),
    account         varchar(24),
    account2        varchar(24),
    PRIMARY KEY (warehouse_code, location_code)
);
COMMENT ON TABLE warehouse_location IS 'TipTop: ime_file 倉庫／存放位置資料主檔（aimi201 單身）；不建空白儲位 ime02＝'' ''（WH4(b)）';
COMMENT ON COLUMN warehouse_location.warehouse_code IS 'ime01 倉庫編號';
COMMENT ON COLUMN warehouse_location.location_code IS 'ime02 儲位（建立後不可修改，WH4(e)）';
COMMENT ON COLUMN warehouse_location.name IS 'ime03 儲位名稱';
COMMENT ON COLUMN warehouse_location.storage_class IS 'ime04 倉儲類別（新增時從倉庫帶入，倉庫修改時同步，WH4(c)、WH5）';
COMMENT ON COLUMN warehouse_location.usable IS 'ime05 是否為可用倉儲';
COMMENT ON COLUMN warehouse_location.mrp_usable IS 'ime06 是否為 MRP 可用倉儲';
COMMENT ON COLUMN warehouse_location.bonded IS 'ime07 保稅與否';
COMMENT ON COLUMN warehouse_location.issue_priority IS 'ime10 工單發料優先順序';
COMMENT ON COLUMN warehouse_location.ship_priority IS 'ime11 銷售出貨優先順序';
COMMENT ON COLUMN warehouse_location.account_method IS 'ime08 會計科目自動顯示方式：0 自行輸入、1 料件科目、2 儲位科目';
COMMENT ON COLUMN warehouse_location.account IS 'ime09 倉儲所屬會計科目（方式 2 才有）';
COMMENT ON COLUMN warehouse_location.account2 IS 'ime091 存貨科目二';

CREATE TRIGGER warehouse_version BEFORE UPDATE ON warehouse
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER warehouse_change_log AFTER INSERT OR UPDATE OR DELETE ON warehouse
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('warehouse_code');
CREATE TRIGGER warehouse_location_change_log AFTER INSERT OR UPDATE OR DELETE ON warehouse_location
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('warehouse_code,location_code');
