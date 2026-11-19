-- 工廠層（docs/adr/0007-multi-plant.md）：製造管理系統參數（TipTop sma_file；每個營運中心一筆）
-- 本批只放 asms230 庫存參數的欄位（specs/features/aim/inventory_parameters.feature IP2、IP10）；其他 asms 參數作業移植時再加欄位。
-- 不預先建立資料：第一次修改時由 app 以預設值建立（IP2）。
CREATE TABLE manufacturing_parameter (
    parameter_key               char(1) PRIMARY KEY DEFAULT '0' CHECK (parameter_key = '0'),
    storage_control             char(1) NOT NULL DEFAULT '3' CHECK (storage_control IN ('1', '2', '3')),
    fast_turnover               numeric(9, 4) NOT NULL DEFAULT 0 CHECK (fast_turnover BETWEEN 0 AND 100),
    average_turnover            numeric(9, 4) NOT NULL DEFAULT 0 CHECK (average_turnover BETWEEN 0 AND 100),
    posting_date_basis          char(1) NOT NULL DEFAULT '1' CHECK (posting_date_basis IN ('1', '2')),
    misc_receipt_new_location   boolean NOT NULL DEFAULT false,
    stock_query_window          boolean NOT NULL DEFAULT false,
    plant_transfer_same_no      boolean NOT NULL DEFAULT false,
    warehouse_transfer_same_no  boolean NOT NULL DEFAULT false,
    misc_issue_dept             boolean NOT NULL DEFAULT false,
    misc_receipt_dept           boolean NOT NULL DEFAULT false,
    add_storage_freely          boolean NOT NULL DEFAULT false,
    created_by                  text NOT NULL,
    created_dept                text NOT NULL DEFAULT '',
    created_on                  date NOT NULL DEFAULT CURRENT_DATE,
    updated_by                  text,
    updated_on                  date,
    version                     integer NOT NULL DEFAULT 1,
    created_at                  timestamptz NOT NULL DEFAULT now(),
    updated_at                  timestamptz,
    CHECK (fast_turnover >= average_turnover)
);
COMMENT ON TABLE manufacturing_parameter IS 'TipTop: sma_file 製造管理系統參數檔（asms230 庫存參數，inventory-parameters）；工廠層、每廠一筆（IP2）';
COMMENT ON COLUMN manufacturing_parameter.parameter_key IS 'sma00 KEY VALUE(0)';
COMMENT ON COLUMN manufacturing_parameter.storage_control IS 'sma42 料件存放位置管制：1 預設倉庫、2 預設倉庫＋儲位、3 自行決定（IP4）';
COMMENT ON COLUMN manufacturing_parameter.fast_turnover IS 'sma68 快速移動料件週轉率（0–100，不小於平速，IP5）';
COMMENT ON COLUMN manufacturing_parameter.average_turnover IS 'sma69 平速移動料件週轉率（0–100，IP5）';
COMMENT ON COLUMN manufacturing_parameter.posting_date_basis IS 'sma892 第 1 碼 庫存帳務基礎日期：1 單據日期、2 產生日期（IP6(a)）';
COMMENT ON COLUMN manufacturing_parameter.misc_receipt_new_location IS 'sma892 第 2 碼（4fd：雜收時可新增儲位；.sch：新增 img 時是否維護相關資料；意義待確認 IP6(b)）';
COMMENT ON COLUMN manufacturing_parameter.stock_query_window IS 'sma892 第 3 碼（4fd：開窗查詢庫存明細；意義待確認 IP6(b)）';
COMMENT ON COLUMN manufacturing_parameter.plant_transfer_same_no IS 'sma88 兩階段營運中心調撥，撥出單號與撥入單號相同（IP7）';
COMMENT ON COLUMN manufacturing_parameter.warehouse_transfer_same_no IS 'sma884 兩階段倉庫調撥，撥出單號與撥入單號相同（IP7）';
COMMENT ON COLUMN manufacturing_parameter.misc_issue_dept IS 'sma847 第 1 碼 雜發單是否要輸入部門（IP8）';
COMMENT ON COLUMN manufacturing_parameter.misc_receipt_dept IS 'sma847 第 2 碼 雜收單是否要輸入部門（IP8）';
COMMENT ON COLUMN manufacturing_parameter.add_storage_freely IS 'sma39 可不經倉儲維護新增倉儲（與 sma24 的連動延到成本參數，IP8）';
COMMENT ON COLUMN manufacturing_parameter.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TRIGGER manufacturing_parameter_version BEFORE UPDATE ON manufacturing_parameter
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER manufacturing_parameter_change_log AFTER INSERT OR UPDATE OR DELETE ON manufacturing_parameter
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('parameter_key');
