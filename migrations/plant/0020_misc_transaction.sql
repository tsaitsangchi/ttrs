-- 工廠層（docs/adr/0007-multi-plant.md）：庫存明細與雜項發料／收料／報廢（TipTop img_file；ina_file、inb_file；
-- specs/features/aim/misc_issue.feature、misc_receipt.feature、misc_scrap.feature）。
-- 三支作業各自一組單頭、單身資料表（Key User 2026-11-20；MT2），欄位相同。

CREATE TABLE inventory_balance (
    item_no   varchar(40) NOT NULL REFERENCES item (item_no),
    warehouse varchar(10) NOT NULL REFERENCES warehouse (warehouse_code),
    location  varchar(10) NOT NULL DEFAULT '',
    lot       varchar(24) NOT NULL DEFAULT '',
    unit      varchar(4) NOT NULL,
    qty       numeric(15, 3) NOT NULL DEFAULT 0,
    expiry    date,
    PRIMARY KEY (item_no, warehouse, location, lot)
);
COMMENT ON TABLE inventory_balance IS 'TipTop: img_file 庫存資料明細檔；本批只建檢查用欄位（MT17），由各作業的過帳維護（第二份規格）';
COMMENT ON COLUMN inventory_balance.item_no IS 'img01 料件編號';
COMMENT ON COLUMN inventory_balance.warehouse IS 'img02 倉庫編號';
COMMENT ON COLUMN inventory_balance.location IS 'img03 儲位（空白為空字串）';
COMMENT ON COLUMN inventory_balance.lot IS 'img04 批號（空白為空字串）';
COMMENT ON COLUMN inventory_balance.unit IS 'img09 庫存單位';
COMMENT ON COLUMN inventory_balance.qty IS 'img10 庫存數量';
COMMENT ON COLUMN inventory_balance.expiry IS 'img18 有效日期';
CREATE TRIGGER inventory_balance_change_log AFTER INSERT OR UPDATE OR DELETE ON inventory_balance
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('item_no,warehouse,location,lot');

CREATE TABLE misc_issue (
    doc_no       varchar(16) PRIMARY KEY CHECK (doc_no ~ '^[^[:space:]-]{3,5}-[^[:space:]]{8,10}$'),
    input_date   date NOT NULL,
    posting_date date NOT NULL,
    dept_code    varchar(10) REFERENCES department (dept_code),
    applicant    varchar(10),
    memo         varchar(255) NOT NULL CHECK (char_length(trim(memo)) >= 1),
    status       char(1) NOT NULL DEFAULT 'N' CHECK (status IN ('N', 'Y', 'X')),
    posted       boolean NOT NULL DEFAULT false,
    created_by   text NOT NULL,
    created_dept text NOT NULL DEFAULT '',
    created_on   date NOT NULL DEFAULT CURRENT_DATE,
    updated_by   text,
    updated_on   date,
    version      integer NOT NULL DEFAULT 1,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz
);
COMMENT ON TABLE misc_issue IS 'TipTop: ina_file 庫存異動單據單頭檔 ina00＝1（aimt301 雜項發料，misc-issue）；工廠層（MT2）';
COMMENT ON COLUMN misc_issue.doc_no IS 'ina01 單據編號（單別-流水號，MT4）';
COMMENT ON COLUMN misc_issue.input_date IS 'ina03 輸入日期';
COMMENT ON COLUMN misc_issue.posting_date IS 'ina02 扣帳日期';
COMMENT ON COLUMN misc_issue.dept_code IS 'ina04 部門編號';
COMMENT ON COLUMN misc_issue.applicant IS 'ina11 申請人（員工代號）';
COMMENT ON COLUMN misc_issue.memo IS 'ina07 備註（客製 aim-372 必填，MT14）';
COMMENT ON COLUMN misc_issue.status IS 'inaconf 確認碼 N 開立、Y 已確認、X 作廢';
COMMENT ON COLUMN misc_issue.posted IS 'inapost 過帳碼（第二份規格）';
COMMENT ON COLUMN misc_issue.created_by IS 'inauser 資料所有者';
COMMENT ON COLUMN misc_issue.created_dept IS 'inagrup 資料所有群';
COMMENT ON COLUMN misc_issue.updated_by IS 'inamodu 資料更改者';
COMMENT ON COLUMN misc_issue.updated_on IS 'inadate 最近修改日';
COMMENT ON COLUMN misc_issue.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TABLE misc_issue_line (
    doc_no    varchar(16) NOT NULL REFERENCES misc_issue (doc_no) ON DELETE CASCADE,
    line_no   integer NOT NULL CHECK (line_no > 0),
    item_no   varchar(40) NOT NULL REFERENCES item (item_no),
    warehouse varchar(10) NOT NULL REFERENCES warehouse (warehouse_code),
    location  varchar(10) NOT NULL DEFAULT '',
    lot       varchar(24) NOT NULL DEFAULT '',
    unit      varchar(4) NOT NULL,
    qty       numeric(15, 3) NOT NULL CHECK (qty > 0),
    reason    varchar(10),
    PRIMARY KEY (doc_no, line_no)
);
COMMENT ON TABLE misc_issue_line IS 'TipTop: inb_file 庫存異動單據單身檔（aimt301 雜項發料）';
COMMENT ON COLUMN misc_issue_line.line_no IS 'inb03 項次';
COMMENT ON COLUMN misc_issue_line.item_no IS 'inb04 料號';
COMMENT ON COLUMN misc_issue_line.warehouse IS 'inb05 倉庫';
COMMENT ON COLUMN misc_issue_line.location IS 'inb06 儲位（空白為空字串）';
COMMENT ON COLUMN misc_issue_line.lot IS 'inb07 批號（空白為空字串）';
COMMENT ON COLUMN misc_issue_line.unit IS 'inb08 單位（一律為庫存單位，MT7(d)）';
COMMENT ON COLUMN misc_issue_line.qty IS 'inb09 異動數量';
COMMENT ON COLUMN misc_issue_line.reason IS 'inb15 理由碼（分類碼類別 2）';

CREATE TRIGGER misc_issue_version BEFORE UPDATE ON misc_issue
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER misc_issue_change_log AFTER INSERT OR UPDATE OR DELETE ON misc_issue
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_no');
CREATE TRIGGER misc_issue_line_change_log AFTER INSERT OR UPDATE OR DELETE ON misc_issue_line
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_no,line_no');

CREATE TABLE misc_receipt (
    doc_no       varchar(16) PRIMARY KEY CHECK (doc_no ~ '^[^[:space:]-]{3,5}-[^[:space:]]{8,10}$'),
    input_date   date NOT NULL,
    posting_date date NOT NULL,
    dept_code    varchar(10) REFERENCES department (dept_code),
    applicant    varchar(10),
    memo         varchar(255) NOT NULL CHECK (char_length(trim(memo)) >= 1),
    status       char(1) NOT NULL DEFAULT 'N' CHECK (status IN ('N', 'Y', 'X')),
    posted       boolean NOT NULL DEFAULT false,
    created_by   text NOT NULL,
    created_dept text NOT NULL DEFAULT '',
    created_on   date NOT NULL DEFAULT CURRENT_DATE,
    updated_by   text,
    updated_on   date,
    version      integer NOT NULL DEFAULT 1,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz
);
COMMENT ON TABLE misc_receipt IS 'TipTop: ina_file 庫存異動單據單頭檔 ina00＝3（aimt302 雜項收料，misc-receipt）；工廠層（MT2）';
COMMENT ON COLUMN misc_receipt.doc_no IS 'ina01 單據編號（單別-流水號，MT4）';
COMMENT ON COLUMN misc_receipt.input_date IS 'ina03 輸入日期';
COMMENT ON COLUMN misc_receipt.posting_date IS 'ina02 扣帳日期';
COMMENT ON COLUMN misc_receipt.dept_code IS 'ina04 部門編號';
COMMENT ON COLUMN misc_receipt.applicant IS 'ina11 申請人（員工代號）';
COMMENT ON COLUMN misc_receipt.memo IS 'ina07 備註（客製 aim-372 必填，MT14）';
COMMENT ON COLUMN misc_receipt.status IS 'inaconf 確認碼 N 開立、Y 已確認、X 作廢';
COMMENT ON COLUMN misc_receipt.posted IS 'inapost 過帳碼（第二份規格）';
COMMENT ON COLUMN misc_receipt.created_by IS 'inauser 資料所有者';
COMMENT ON COLUMN misc_receipt.created_dept IS 'inagrup 資料所有群';
COMMENT ON COLUMN misc_receipt.updated_by IS 'inamodu 資料更改者';
COMMENT ON COLUMN misc_receipt.updated_on IS 'inadate 最近修改日';
COMMENT ON COLUMN misc_receipt.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TABLE misc_receipt_line (
    doc_no    varchar(16) NOT NULL REFERENCES misc_receipt (doc_no) ON DELETE CASCADE,
    line_no   integer NOT NULL CHECK (line_no > 0),
    item_no   varchar(40) NOT NULL REFERENCES item (item_no),
    warehouse varchar(10) NOT NULL REFERENCES warehouse (warehouse_code),
    location  varchar(10) NOT NULL DEFAULT '',
    lot       varchar(24) NOT NULL DEFAULT '',
    unit      varchar(4) NOT NULL,
    qty       numeric(15, 3) NOT NULL CHECK (qty > 0),
    reason    varchar(10),
    PRIMARY KEY (doc_no, line_no)
);
COMMENT ON TABLE misc_receipt_line IS 'TipTop: inb_file 庫存異動單據單身檔（aimt302 雜項收料）';
COMMENT ON COLUMN misc_receipt_line.line_no IS 'inb03 項次';
COMMENT ON COLUMN misc_receipt_line.item_no IS 'inb04 料號';
COMMENT ON COLUMN misc_receipt_line.warehouse IS 'inb05 倉庫';
COMMENT ON COLUMN misc_receipt_line.location IS 'inb06 儲位（空白為空字串）';
COMMENT ON COLUMN misc_receipt_line.lot IS 'inb07 批號（空白為空字串）';
COMMENT ON COLUMN misc_receipt_line.unit IS 'inb08 單位（一律為庫存單位，MT7(d)）';
COMMENT ON COLUMN misc_receipt_line.qty IS 'inb09 異動數量';
COMMENT ON COLUMN misc_receipt_line.reason IS 'inb15 理由碼（分類碼類別 2）';

CREATE TRIGGER misc_receipt_version BEFORE UPDATE ON misc_receipt
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER misc_receipt_change_log AFTER INSERT OR UPDATE OR DELETE ON misc_receipt
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_no');
CREATE TRIGGER misc_receipt_line_change_log AFTER INSERT OR UPDATE OR DELETE ON misc_receipt_line
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_no,line_no');

CREATE TABLE misc_scrap (
    doc_no       varchar(16) PRIMARY KEY CHECK (doc_no ~ '^[^[:space:]-]{3,5}-[^[:space:]]{8,10}$'),
    input_date   date NOT NULL,
    posting_date date NOT NULL,
    dept_code    varchar(10) REFERENCES department (dept_code),
    applicant    varchar(10),
    memo         varchar(255) NOT NULL CHECK (char_length(trim(memo)) >= 1),
    status       char(1) NOT NULL DEFAULT 'N' CHECK (status IN ('N', 'Y', 'X')),
    posted       boolean NOT NULL DEFAULT false,
    created_by   text NOT NULL,
    created_dept text NOT NULL DEFAULT '',
    created_on   date NOT NULL DEFAULT CURRENT_DATE,
    updated_by   text,
    updated_on   date,
    version      integer NOT NULL DEFAULT 1,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz
);
COMMENT ON TABLE misc_scrap IS 'TipTop: ina_file 庫存異動單據單頭檔 ina00＝5（aimt303 雜項報廢，misc-scrap）；工廠層（MT2）';
COMMENT ON COLUMN misc_scrap.doc_no IS 'ina01 單據編號（單別-流水號，MT4）';
COMMENT ON COLUMN misc_scrap.input_date IS 'ina03 輸入日期';
COMMENT ON COLUMN misc_scrap.posting_date IS 'ina02 扣帳日期';
COMMENT ON COLUMN misc_scrap.dept_code IS 'ina04 部門編號';
COMMENT ON COLUMN misc_scrap.applicant IS 'ina11 申請人（員工代號）';
COMMENT ON COLUMN misc_scrap.memo IS 'ina07 備註（客製 aim-372 必填，MT14）';
COMMENT ON COLUMN misc_scrap.status IS 'inaconf 確認碼 N 開立、Y 已確認、X 作廢';
COMMENT ON COLUMN misc_scrap.posted IS 'inapost 過帳碼（第二份規格）';
COMMENT ON COLUMN misc_scrap.created_by IS 'inauser 資料所有者';
COMMENT ON COLUMN misc_scrap.created_dept IS 'inagrup 資料所有群';
COMMENT ON COLUMN misc_scrap.updated_by IS 'inamodu 資料更改者';
COMMENT ON COLUMN misc_scrap.updated_on IS 'inadate 最近修改日';
COMMENT ON COLUMN misc_scrap.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TABLE misc_scrap_line (
    doc_no    varchar(16) NOT NULL REFERENCES misc_scrap (doc_no) ON DELETE CASCADE,
    line_no   integer NOT NULL CHECK (line_no > 0),
    item_no   varchar(40) NOT NULL REFERENCES item (item_no),
    warehouse varchar(10) NOT NULL REFERENCES warehouse (warehouse_code),
    location  varchar(10) NOT NULL DEFAULT '',
    lot       varchar(24) NOT NULL DEFAULT '',
    unit      varchar(4) NOT NULL,
    qty       numeric(15, 3) NOT NULL CHECK (qty > 0),
    reason    varchar(10),
    PRIMARY KEY (doc_no, line_no)
);
COMMENT ON TABLE misc_scrap_line IS 'TipTop: inb_file 庫存異動單據單身檔（aimt303 雜項報廢）';
COMMENT ON COLUMN misc_scrap_line.line_no IS 'inb03 項次';
COMMENT ON COLUMN misc_scrap_line.item_no IS 'inb04 料號';
COMMENT ON COLUMN misc_scrap_line.warehouse IS 'inb05 倉庫';
COMMENT ON COLUMN misc_scrap_line.location IS 'inb06 儲位（空白為空字串）';
COMMENT ON COLUMN misc_scrap_line.lot IS 'inb07 批號（空白為空字串）';
COMMENT ON COLUMN misc_scrap_line.unit IS 'inb08 單位（一律為庫存單位，MT7(d)）';
COMMENT ON COLUMN misc_scrap_line.qty IS 'inb09 異動數量';
COMMENT ON COLUMN misc_scrap_line.reason IS 'inb15 理由碼（分類碼類別 2）';

CREATE TRIGGER misc_scrap_version BEFORE UPDATE ON misc_scrap
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER misc_scrap_change_log AFTER INSERT OR UPDATE OR DELETE ON misc_scrap
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_no');
CREATE TRIGGER misc_scrap_line_change_log AFTER INSERT OR UPDATE OR DELETE ON misc_scrap_line
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_no,line_no');
