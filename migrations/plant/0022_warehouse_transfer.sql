-- 工廠層（docs/adr/0007-multi-plant.md）：庫存直接調撥（TipTop aimt324 / imm_file、imn_file 的一階段倉庫調撥；
-- specs/features/aim/warehouse_transfer.feature WT1）。兩階段調撥 aimt325 移植時有自己的表。
CREATE TABLE warehouse_transfer (
    doc_no        varchar(16) PRIMARY KEY CHECK (doc_no ~ '^[^[:space:]-]{3,5}-[^[:space:]]{8,10}$'),
    transfer_date date NOT NULL,
    dept_code     varchar(10) REFERENCES department (dept_code),
    applicant     varchar(10),
    memo          varchar(255),
    status        char(1) NOT NULL DEFAULT 'N' CHECK (status IN ('N', 'Y', 'X')),
    posted        boolean NOT NULL DEFAULT false,
    created_by    text NOT NULL,
    created_dept  text NOT NULL DEFAULT '',
    created_on    date NOT NULL DEFAULT CURRENT_DATE,
    updated_by    text,
    updated_on    date,
    version       integer NOT NULL DEFAULT 1,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz
);
COMMENT ON TABLE warehouse_transfer IS 'TipTop: imm_file 庫存調撥單單頭檔 imm10＝1（aimt324 庫存直接調撥，warehouse-transfer）；工廠層（WT1）';
COMMENT ON COLUMN warehouse_transfer.doc_no IS 'imm01 調撥單號';
COMMENT ON COLUMN warehouse_transfer.transfer_date IS 'imm02 調撥日期（編號、期間檢查、過帳都用它，WT2、WT3(a)）';
COMMENT ON COLUMN warehouse_transfer.dept_code IS 'imm14 部門';
COMMENT ON COLUMN warehouse_transfer.applicant IS 'imm16 申請人（員工代號）';
COMMENT ON COLUMN warehouse_transfer.memo IS 'imm09 備註';
COMMENT ON COLUMN warehouse_transfer.status IS 'immconf 確認碼 N 開立、Y 已確認、X 作廢';
COMMENT ON COLUMN warehouse_transfer.posted IS 'imm03 過帳否';
COMMENT ON COLUMN warehouse_transfer.created_by IS 'immuser 資料所有者';
COMMENT ON COLUMN warehouse_transfer.created_dept IS 'immgrup 資料所有部門';
COMMENT ON COLUMN warehouse_transfer.updated_by IS 'immmodu 資料修改者';
COMMENT ON COLUMN warehouse_transfer.updated_on IS 'immdate 最近修改日';
COMMENT ON COLUMN warehouse_transfer.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TABLE warehouse_transfer_line (
    doc_no         varchar(16) NOT NULL REFERENCES warehouse_transfer (doc_no) ON DELETE CASCADE,
    line_no        integer NOT NULL CHECK (line_no > 0),
    item_no        varchar(40) NOT NULL REFERENCES item (item_no),
    from_warehouse varchar(10) NOT NULL REFERENCES warehouse (warehouse_code),
    from_location  varchar(10) NOT NULL DEFAULT '',
    from_lot       varchar(24) NOT NULL DEFAULT '',
    to_warehouse   varchar(10) NOT NULL REFERENCES warehouse (warehouse_code),
    to_location    varchar(10) NOT NULL DEFAULT '',
    to_lot         varchar(24) NOT NULL DEFAULT '',
    unit           varchar(4) NOT NULL,
    qty            numeric(15, 3) NOT NULL CHECK (qty > 0),
    reason         varchar(10),
    PRIMARY KEY (doc_no, line_no),
    CHECK ((from_warehouse, from_location, from_lot) IS DISTINCT FROM (to_warehouse, to_location, to_lot))
);
COMMENT ON TABLE warehouse_transfer_line IS 'TipTop: imn_file 庫存調撥單單身檔（aimt324）';
COMMENT ON COLUMN warehouse_transfer_line.line_no IS 'imn02 項次';
COMMENT ON COLUMN warehouse_transfer_line.item_no IS 'imn03 料號';
COMMENT ON COLUMN warehouse_transfer_line.from_warehouse IS 'imn04 撥出倉庫';
COMMENT ON COLUMN warehouse_transfer_line.from_location IS 'imn05 撥出儲位（空白為空字串）';
COMMENT ON COLUMN warehouse_transfer_line.from_lot IS 'imn06 撥出批號';
COMMENT ON COLUMN warehouse_transfer_line.to_warehouse IS 'imn15 撥入倉庫';
COMMENT ON COLUMN warehouse_transfer_line.to_location IS 'imn16 撥入儲位';
COMMENT ON COLUMN warehouse_transfer_line.to_lot IS 'imn17 撥入批號（空白時帶撥出批號，WT5(e)）';
COMMENT ON COLUMN warehouse_transfer_line.unit IS 'imn09 庫存單位（撥入單位 imn20 相同，WT6）';
COMMENT ON COLUMN warehouse_transfer_line.qty IS 'imn10 調撥數量（轉換後數量 imn22 相同）';
COMMENT ON COLUMN warehouse_transfer_line.reason IS 'imn28 調撥理由（分類碼類別 2）';

CREATE TRIGGER warehouse_transfer_version BEFORE UPDATE ON warehouse_transfer
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER warehouse_transfer_change_log AFTER INSERT OR UPDATE OR DELETE ON warehouse_transfer
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_no');
CREATE TRIGGER warehouse_transfer_line_change_log AFTER INSERT OR UPDATE OR DELETE ON warehouse_transfer_line
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_no,line_no');

-- 調撥每筆單身寫撥出、撥入兩筆異動記錄（WT9(d)）：唯一鍵加上方向
DROP INDEX inventory_txn_doc_line_uq;
CREATE UNIQUE INDEX inventory_txn_doc_line_uq ON inventory_txn (program, doc_no, doc_line, direction) WHERE program <> '';
