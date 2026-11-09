-- 料件分群補欄位（specs/features/aim/item_maintenance.feature IM24）：新增料件時由分群帶入
-- imz131 產品分類、imz51 經濟訂購量、imz52 平均訂購量（aimi100.4gl i100_set_rel_ima06 :3842-3930）
ALTER TABLE item_group
    ADD COLUMN product_class      varchar(10) REFERENCES product_class (class_code),
    ADD COLUMN economic_order_qty numeric(15,3) CHECK (economic_order_qty >= 0),
    ADD COLUMN average_order_qty  numeric(15,3) CHECK (average_order_qty >= 0);
COMMENT ON COLUMN item_group.product_class IS 'imz131 產品分類編號（新增料件時帶入 ima131）';
COMMENT ON COLUMN item_group.economic_order_qty IS 'imz51 經濟訂購量';
COMMENT ON COLUMN item_group.average_order_qty IS 'imz52 平均訂購量';
