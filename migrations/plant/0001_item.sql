-- 工廠層（docs/adr/0007-multi-plant.md）：每個工廠 schema 各一份，表上不帶 plant_id。
-- 共用 schema 的表一律寫明 public.

-- 料件主檔（TipTop: ima_file，aimi100）
CREATE TABLE item (
    item_no     TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    spec        TEXT,
    stock_unit  TEXT NOT NULL REFERENCES public.unit (unit_code),
    status      TEXT NOT NULL DEFAULT 'unconfirmed'
                CHECK (status IN ('unconfirmed', 'confirmed', 'voided')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
COMMENT ON TABLE item IS 'TipTop: ima_file 料件主檔 (aimi100)；工廠層';

-- 庫存異動明細（TipTop: tlf_file）；正式移植庫存異動時依年度分區（ADR 0007 §10）
CREATE TABLE inventory_txn (
    txn_id      BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    item_no     TEXT NOT NULL REFERENCES item (item_no),
    txn_date    DATE NOT NULL,
    doc_no      TEXT NOT NULL,
    doc_line    INTEGER NOT NULL,
    qty         NUMERIC(18, 4) NOT NULL,
    unit_code   TEXT NOT NULL REFERENCES public.unit (unit_code),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
COMMENT ON TABLE inventory_txn IS 'TipTop: tlf_file 庫存異動明細；工廠層';
CREATE INDEX inventory_txn_item_idx ON inventory_txn (item_no, txn_date);
