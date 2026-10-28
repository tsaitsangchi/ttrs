-- 基礎結構。舊表對照見 docs/legacy-map/*.yaml
-- 命名規則：新表用可讀英文；每張表 COMMENT 註明對應的 TipTop 資料表。

CREATE TABLE plant (
    plant_id    TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
COMMENT ON TABLE plant IS 'TipTop: azp_file 營運中心';

CREATE TABLE unit (
    unit_code   TEXT PRIMARY KEY,
    name        TEXT NOT NULL
);
COMMENT ON TABLE unit IS 'TipTop: gfe_file 單位';

CREATE TABLE item (
    plant_id    TEXT NOT NULL REFERENCES plant (plant_id),
    item_no     TEXT NOT NULL,
    name        TEXT NOT NULL,
    spec        TEXT,
    stock_unit  TEXT NOT NULL REFERENCES unit (unit_code),
    status      TEXT NOT NULL DEFAULT 'unconfirmed'
                CHECK (status IN ('unconfirmed', 'confirmed', 'voided')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (plant_id, item_no)
);
COMMENT ON TABLE item IS 'TipTop: ima_file 料件主檔 (aimi100)';

-- 庫存異動明細：所有進出庫都寫這張（TipTop: tlf_file）
CREATE TABLE inventory_txn (
    txn_id      BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    plant_id    TEXT NOT NULL,
    item_no     TEXT NOT NULL,
    txn_date    DATE NOT NULL,
    doc_no      TEXT NOT NULL,
    doc_line    INTEGER NOT NULL,
    qty         NUMERIC(18, 4) NOT NULL,
    unit_code   TEXT NOT NULL REFERENCES unit (unit_code),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (plant_id, item_no) REFERENCES item (plant_id, item_no)
);
COMMENT ON TABLE inventory_txn IS 'TipTop: tlf_file 庫存異動明細';
CREATE INDEX inventory_txn_item_idx ON inventory_txn (plant_id, item_no, txn_date);
