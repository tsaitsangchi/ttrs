-- User Maintenance 使用者維護（TipTop p_zx）。規格：specs/features/azz/user_maintenance.feature；登入與權限：docs/adr/0006
-- 系統管理資料不分營運中心，不帶 plant_id（同 TipTop zx/zxy 為共用表）。
-- TipTop 密碼相關欄位（zx10、zx16、zx17、zx19、zx20）不移植，由公司 IdP 處理（ADR 0006 §3）。

CREATE TABLE user_account (
    user_id       varchar(10) PRIMARY KEY,
    name          varchar(80),
    dept_code     varchar(10),
    class_code    text NOT NULL REFERENCES permission_class (class_code),
    -- 選單是否為 MENU 程式由應用層檢核；不設外鍵，避免刪除程式時受阻（同 TipTop）
    menu_program  varchar(20) NOT NULL,
    multi_plant   boolean NOT NULL DEFAULT false,
    default_plant text NOT NULL REFERENCES plant (plant_id),
    email         varchar(80),
    idp_username  varchar(255),
    idp_subject   varchar(255) UNIQUE,
    lang          varchar(5) NOT NULL DEFAULT 'zh-TW' CHECK (lang IN ('zh-TW', 'en', 'zh-CN')),
    valid_until   date,
    active        boolean NOT NULL DEFAULT true,
    created_by    text NOT NULL,
    created_dept  text NOT NULL,
    created_on    date NOT NULL DEFAULT CURRENT_DATE,
    updated_by    text,
    updated_on    date
);
COMMENT ON TABLE user_account IS 'TipTop: zx_file 部門使用者檔 (p_zx)';
COMMENT ON COLUMN user_account.user_id IS 'TipTop: zx01 使用者代號；各表 created_by 存此值，建立後不可修改（U1）';
COMMENT ON COLUMN user_account.dept_code IS 'TipTop: zx03 部門代號（部門主檔移植前為自由輸入，U3）';
COMMENT ON COLUMN user_account.class_code IS 'TipTop: zx04 權限類別';
COMMENT ON COLUMN user_account.menu_program IS 'TipTop: zx05 自定目錄編號（模組 MENU 的程式）';
COMMENT ON COLUMN user_account.multi_plant IS 'TipTop: zx07 是否為多營運中心使用者';
COMMENT ON COLUMN user_account.default_plant IS 'TipTop: zx08 預設營運中心編號';
COMMENT ON COLUMN user_account.email IS 'TipTop: zx09 Email 位址';
COMMENT ON COLUMN user_account.idp_username IS '公司帳號（UPN，小寫），ADR 0006 §2';
COMMENT ON COLUMN user_account.idp_subject IS '公司 IdP 使用者識別碼（sub/oid），首次登入時綁定';
COMMENT ON COLUMN user_account.lang IS 'TipTop: zx06 使用語言別 0/1/2 → zh-TW/en/zh-CN';
COMMENT ON COLUMN user_account.valid_until IS 'TipTop: zx18 使用者可用截止日期';
COMMENT ON COLUMN user_account.active IS 'TipTop: zxacti 資料有效碼';
CREATE UNIQUE INDEX user_account_idp_username_key ON user_account (lower(idp_username));

CREATE TABLE user_plant (
    user_id   varchar(10) NOT NULL REFERENCES user_account (user_id) ON DELETE CASCADE,
    plant_id  text NOT NULL REFERENCES plant (plant_id),
    PRIMARY KEY (user_id, plant_id)
);
COMMENT ON TABLE user_plant IS 'TipTop: zxy_file 使用者可使用營運中心（zxy01 使用者、zxy03 營運中心）';
