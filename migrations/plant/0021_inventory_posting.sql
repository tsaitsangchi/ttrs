-- 工廠層（docs/adr/0007-multi-plant.md）：雜項發料／收料／報廢的庫存過帳（misc_issue_posting.feature、
-- misc_receipt_posting.feature、misc_scrap_posting.feature）。庫存明細補欄位、異動記錄表由骨架擴充、庫存參數加期間參數。

-- img_file 其餘移植的欄位（PT14）
ALTER TABLE inventory_balance
    ADD COLUMN last_received_on date,
    ADD COLUMN last_issued_on   date,
    ADD COLUMN last_txn_on      date,
    ADD COLUMN usable           boolean NOT NULL DEFAULT true,
    ADD COLUMN mrp_usable       boolean NOT NULL DEFAULT true,
    ADD COLUMN bonded           boolean NOT NULL DEFAULT false,
    ADD COLUMN idle_on          date,
    ADD COLUMN ref_no           varchar(16),
    ADD COLUMN ref_line         integer,
    ADD CONSTRAINT inventory_balance_qty_non_negative CHECK (qty >= 0);
COMMENT ON COLUMN inventory_balance.last_received_on IS 'img15 最近收料日';
COMMENT ON COLUMN inventory_balance.last_issued_on IS 'img16 最近發料日（報廢不改，PS1）';
COMMENT ON COLUMN inventory_balance.last_txn_on IS 'img17 最近異動日';
COMMENT ON COLUMN inventory_balance.usable IS 'img23 是否為可用倉儲（新建時取儲位，沒有儲位時取倉庫）';
COMMENT ON COLUMN inventory_balance.mrp_usable IS 'img24 是否為 MRP 可用倉儲';
COMMENT ON COLUMN inventory_balance.bonded IS 'img25 保稅與否';
COMMENT ON COLUMN inventory_balance.idle_on IS 'img37 呆滯日期（新建時為扣帳日）';
COMMENT ON COLUMN inventory_balance.ref_no IS 'img05 參考號碼（建立它的單號）';
COMMENT ON COLUMN inventory_balance.ref_line IS 'img06 參考序號（建立它的項次）';

-- tlf_file 異動記錄（PT6）：骨架表 inventory_txn 擴充；txn_date＝tlf06、doc_no＝tlf905、doc_line＝tlf906、qty＝tlf10、unit_code＝tlf11
ALTER TABLE inventory_txn
    ADD COLUMN program       varchar(30) NOT NULL DEFAULT '',
    ADD COLUMN doc_type      varchar(5) NOT NULL DEFAULT '',
    ADD COLUMN warehouse     varchar(10) NOT NULL DEFAULT '',
    ADD COLUMN location      varchar(10) NOT NULL DEFAULT '',
    ADD COLUMN lot           varchar(24) NOT NULL DEFAULT '',
    ADD COLUMN direction     smallint NOT NULL DEFAULT 0 CHECK (direction IN (-1, 0, 1)),
    ADD COLUMN source        smallint,
    ADD COLUMN dest          smallint,
    ADD COLUMN balance_after numeric(15, 3),
    ADD COLUMN reason        varchar(10),
    ADD COLUMN memo          varchar(255),
    ADD COLUMN dept_code     varchar(10),
    ADD COLUMN cost_group    varchar(1),
    ADD COLUMN created_by    text NOT NULL DEFAULT '';
CREATE UNIQUE INDEX inventory_txn_doc_line_uq ON inventory_txn (program, doc_no, doc_line) WHERE program <> '';
COMMENT ON COLUMN inventory_txn.txn_date IS 'tlf06 單據日期（扣帳日）';
COMMENT ON COLUMN inventory_txn.doc_no IS 'tlf905 單號（tlf026／tlf036 同）';
COMMENT ON COLUMN inventory_txn.doc_line IS 'tlf906 項次';
COMMENT ON COLUMN inventory_txn.qty IS 'tlf10 異動數量';
COMMENT ON COLUMN inventory_txn.unit_code IS 'tlf11 異動單位（換算率 tlf12、tlf60 固定 1）';
COMMENT ON COLUMN inventory_txn.created_at IS 'tlf07、tlf08 異動資料產生日期時間';
COMMENT ON COLUMN inventory_txn.program IS 'tlf13 異動命令代號（TTRS 程式代號，如 misc-issue）';
COMMENT ON COLUMN inventory_txn.doc_type IS 'tlf61 單別';
COMMENT ON COLUMN inventory_txn.warehouse IS 'tlf902 倉庫';
COMMENT ON COLUMN inventory_txn.location IS 'tlf903 儲位（空白為空字串）';
COMMENT ON COLUMN inventory_txn.lot IS 'tlf904 批號（空白為空字串）';
COMMENT ON COLUMN inventory_txn.direction IS 'tlf907 入出庫碼：-1 出庫、1 入庫';
COMMENT ON COLUMN inventory_txn.source IS 'tlf02 來源狀況（50 庫存、90 雜項）';
COMMENT ON COLUMN inventory_txn.dest IS 'tlf03 目的狀況（50 庫存、90 雜項、40 報廢）';
COMMENT ON COLUMN inventory_txn.balance_after IS 'tlf024／tlf034 異動後庫存數量';
COMMENT ON COLUMN inventory_txn.reason IS 'tlf14、tlf43 理由碼';
COMMENT ON COLUMN inventory_txn.memo IS 'tlf17 備註';
COMMENT ON COLUMN inventory_txn.dept_code IS 'tlf19 部門';
COMMENT ON COLUMN inventory_txn.cost_group IS 'tlf901 成本庫別（倉庫 imd09）';
COMMENT ON COLUMN inventory_txn.created_by IS 'tlf09 異動資料發出者';

-- 期間參數（PT3）：sma51、sma52、sma53；未設定時不檢查
ALTER TABLE manufacturing_parameter
    ADD COLUMN current_year   integer CHECK (current_year BETWEEN 1900 AND 9999),
    ADD COLUMN current_period smallint CHECK (current_period BETWEEN 1 AND 13),
    ADD COLUMN closing_date   date,
    ADD CONSTRAINT manufacturing_parameter_current_period CHECK ((current_year IS NULL) = (current_period IS NULL));
COMMENT ON COLUMN manufacturing_parameter.current_year IS 'sma51 目前單據輸入會計年度（PT3）';
COMMENT ON COLUMN manufacturing_parameter.current_period IS 'sma52 目前單據輸入會計期別（PT3）';
COMMENT ON COLUMN manufacturing_parameter.closing_date IS 'sma53 成本關帳日期（PT3）';
