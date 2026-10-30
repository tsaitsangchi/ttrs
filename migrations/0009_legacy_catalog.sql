-- TipTop 對照目錄（TipTop Catalog）：TipTop 資料表／欄位 → 英文名稱的唯讀對照。
-- 規格：specs/features/azz/legacy_catalog.feature；來源為 .sch 資料目錄＋命名檔 docs/legacy-map/tiptop-naming/*.yaml，
-- 由 make legacy-catalog 整批匯入（每次匯入整批取代）。不是業務表，不帶 plant_id。

CREATE TABLE legacy_table (
    legacy_name  varchar(30) PRIMARY KEY,
    name         varchar(63) NOT NULL UNIQUE,
    title        text NOT NULL DEFAULT '',
    purpose      text,
    module       varchar(10) NOT NULL
);
COMMENT ON TABLE legacy_table IS 'TipTop 對照目錄：TipTop 資料表（.sch，如 zx_file）→ 英文表名';
COMMENT ON COLUMN legacy_table.title IS 'TipTop .sch 表說明';

CREATE TABLE legacy_column (
    legacy_table  varchar(30) NOT NULL REFERENCES legacy_table (legacy_name) ON DELETE CASCADE,
    legacy_name   varchar(30) NOT NULL,
    position      integer NOT NULL,
    name          varchar(63) NOT NULL,
    data_type     varchar(40) NOT NULL,
    primary_key   boolean NOT NULL DEFAULT false,
    description   text NOT NULL DEFAULT '',
    notes         text,
    unused        boolean NOT NULL DEFAULT false,
    verify        boolean NOT NULL DEFAULT false,
    PRIMARY KEY (legacy_table, legacy_name),
    UNIQUE (legacy_table, name)
);
COMMENT ON TABLE legacy_column IS 'TipTop 對照目錄：TipTop 欄位（.sch，如 zx01）→ 英文欄位名';
COMMENT ON COLUMN legacy_column.unused IS 'TipTop .sch 標示 No Use';
COMMENT ON COLUMN legacy_column.verify IS '英文命名待確認';
