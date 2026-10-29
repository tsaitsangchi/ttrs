-- Data Dictionary Maintenance 資料字典維護（TipTop p_zta）。舊表對照見 docs/legacy-map/azz.yaml
-- 只描述資料表與欄位；資料表結構本身仍只由 migration 變更。
-- 長度依 D8：資料表/欄位代碼 63（PostgreSQL 識別字上限），其餘依 TipTop .sch。

CREATE TABLE table_dictionary (
    table_name   varchar(63) PRIMARY KEY,
    module_code  varchar(10) NOT NULL REFERENCES module (module_code),
    table_type   varchar(1) CHECK (table_type IN ('P', 'M', 'T', 'S', 'B')),
    upstream     varchar(80),
    downstream   varchar(60),
    created_by   text NOT NULL,
    created_on   date NOT NULL DEFAULT CURRENT_DATE,
    updated_at   timestamptz NOT NULL DEFAULT now()
);
COMMENT ON TABLE table_dictionary IS 'TipTop: zta_file 檔案名稱檔＋gat_file 語系共用欄位 (p_zta)';
COMMENT ON COLUMN table_dictionary.table_type IS 'TipTop: gat07 P 參數 M 主檔 T 交易 S 統計 B 基本';
COMMENT ON COLUMN table_dictionary.upstream IS 'TipTop: zta11（4fd 上游檔案）';
COMMENT ON COLUMN table_dictionary.downstream IS 'TipTop: zta12（4fd 下游檔案）';

-- 以下說明隨字典刪除（D3）
CREATE TABLE table_dictionary_text (
    table_name  varchar(63) NOT NULL REFERENCES table_dictionary (table_name) ON DELETE CASCADE,
    lang        varchar(5) NOT NULL CHECK (lang IN ('zh-TW', 'en', 'zh-CN')),
    name        varchar(120),
    purpose     varchar(255),
    remark      varchar(255),
    PRIMARY KEY (table_name, lang)
);
COMMENT ON TABLE table_dictionary_text IS 'TipTop: gat_file 檔案名稱多語言對照檔（gat03 名稱、gat04 目的、gat05 備註）';

CREATE TABLE column_dictionary_text (
    table_name   varchar(63) NOT NULL REFERENCES table_dictionary (table_name) ON DELETE CASCADE,
    column_name  varchar(63) NOT NULL,
    lang         varchar(5) NOT NULL CHECK (lang IN ('zh-TW', 'en', 'zh-CN')),
    name         varchar(255),
    description  varchar(500),
    extra        varchar(2000),
    updated_on   date NOT NULL DEFAULT CURRENT_DATE,
    PRIMARY KEY (table_name, column_name, lang)
);
COMMENT ON TABLE column_dictionary_text IS 'TipTop: gaq_file 欄位多語言說明檔；鍵值改為資料表＋欄位（D2）';
