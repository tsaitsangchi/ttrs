-- 工廠層（docs/adr/0007-multi-plant.md）：產品分類（TipTop axmi110 / oba_file；specs/features/axm/product_classification.feature）
CREATE TABLE product_class (
    class_code        varchar(10) PRIMARY KEY,
    name              varchar(80),
    period_format     varchar(1),
    periods           integer CHECK (periods >= 0),
    cost_center       varchar(10) REFERENCES department (dept_code),
    revenue_account   varchar(24),
    revenue_account2  varchar(24),
    active            boolean NOT NULL DEFAULT true,
    created_by        text NOT NULL,
    created_dept      text NOT NULL,
    created_on        date NOT NULL DEFAULT CURRENT_DATE,
    updated_by        text,
    updated_on        date,
    version           integer NOT NULL DEFAULT 1,
    created_at        timestamptz NOT NULL DEFAULT now(),
    updated_at        timestamptz
);
COMMENT ON TABLE product_class IS 'TipTop: oba_file 產品分類檔（axmi110）；工廠層（PC2）';
COMMENT ON COLUMN product_class.class_code IS 'oba01 產品分類碼（區分大小寫、建立後不可修改，PC3）';
COMMENT ON COLUMN product_class.name IS 'oba02 分類名稱';
COMMENT ON COLUMN product_class.period_format IS 'oba08 計劃期別格式（代碼意義待確認，PC6）';
COMMENT ON COLUMN product_class.periods IS 'oba09 期數（不可小於 0，axm-096）';
COMMENT ON COLUMN product_class.cost_center IS 'oba10 成本中心（有效、管理類別 1／2 的部門，PC4）';
COMMENT ON COLUMN product_class.revenue_account IS 'oba11 銷貨收入科目（會計科目未移植，不檢查，PC5）';
COMMENT ON COLUMN product_class.revenue_account2 IS 'oba111 銷貨收入科目二';
COMMENT ON COLUMN product_class.active IS '資料有效（TTRS 新增，PC7；oba_file 無此欄）';
COMMENT ON COLUMN product_class.created_by IS '資料所有者（TTRS 新增，PC7）';
COMMENT ON COLUMN product_class.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TRIGGER product_class_version BEFORE UPDATE ON product_class
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER product_class_change_log AFTER INSERT OR UPDATE OR DELETE ON product_class
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('class_code');
