-- 工廠層（docs/adr/0007-multi-plant.md）：分類碼，每個工廠 schema 各一份（classification_code.feature CC8）

-- 分類碼（TipTop: azf_file，aooi300 共用程式；specs/features/aoo/classification_code.feature）
-- 第一階段只開放料件用到的類別（CC2）：8 材料類別、D／E／F／G 其他分群碼一～四
CREATE TABLE classification_code (
    kind               char(1) NOT NULL CHECK (kind IN ('8', 'D', 'E', 'F', 'G')),
    code               varchar(10) NOT NULL,
    description        varchar(80),
    variance_account   varchar(24),
    variance_account2  varchar(24),
    cost_nature        integer,
    active             boolean NOT NULL DEFAULT true,
    created_by         text NOT NULL,
    created_dept       text NOT NULL,
    created_on         date NOT NULL DEFAULT CURRENT_DATE,
    updated_by         text,
    updated_on         date,
    PRIMARY KEY (kind, code)
);
COMMENT ON TABLE classification_code IS 'TipTop: azf_file 分類碼（aooi300；aooi305＝8 材料類別、aooi309–aooi312＝D／E／F／G 其他分群碼一～四）；工廠層';
COMMENT ON COLUMN classification_code.kind IS 'azf02 碼類別';
COMMENT ON COLUMN classification_code.code IS 'azf01 碼別代號（同一類別內不可重複）';
COMMENT ON COLUMN classification_code.description IS 'azf03 說明內容';
COMMENT ON COLUMN classification_code.variance_account IS 'azf05 存貨差異科目代號（類別 G；會計科目未移植，暫不檢查，CC3）';
COMMENT ON COLUMN classification_code.variance_account2 IS 'azf051 存貨差異科目代號二（類別 G）';
COMMENT ON COLUMN classification_code.cost_nature IS 'azf06 成本性質（類別 G）';
COMMENT ON COLUMN classification_code.active IS 'azfacti 資料有效碼';
COMMENT ON COLUMN classification_code.created_by IS 'azfuser 原始資料建立者';
COMMENT ON COLUMN classification_code.created_dept IS 'azfgrup 使用者部門';
COMMENT ON COLUMN classification_code.updated_by IS 'azfmodu 最後修改資料者';
COMMENT ON COLUMN classification_code.updated_on IS 'azfdate 最後修改日期';
