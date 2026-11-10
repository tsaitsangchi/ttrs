-- 工廠層：分類碼加入其他類別（客製 topcust/coo/4gl/aooi080.4gl；specs/features/aoo/classification_code_other_kinds.feature OC1–OC7）
-- 2 理由碼、6 檢驗碼、A 保稅原料領用、R 業務請求狀態、S 業務處理結果；3、5、H、Z 不開放（OC2）
ALTER TABLE classification_code DROP CONSTRAINT classification_code_kind_check;
ALTER TABLE classification_code ADD CONSTRAINT classification_code_kind_check
    CHECK (kind IN ('8', 'D', 'E', 'F', 'G', '2', '6', 'A', 'R', 'S'));
COMMENT ON COLUMN classification_code.kind IS 'azf02 碼類別：8、D、E、F、G（CC2）；2、6、A、R、S（OC1）';
