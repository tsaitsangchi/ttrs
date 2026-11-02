-- p_zw 權限類別維護：補齊 zw_file 欄位。規格 specs/features/azz/permission_class_maintenance.feature；
-- 舊表對照見 docs/legacy-map/azz.yaml。列舉欄位沿用 TipTop 原始代碼（'1'、'2'、'3'）。

ALTER TABLE permission_class
    -- 選單本身也是程式（模組 MENU），比照 user_account.menu_program 不設外鍵
    ADD COLUMN default_menu  TEXT,
    ADD COLUMN idle_control  TEXT NOT NULL DEFAULT '1' CHECK (idle_control IN ('1', '2', '3')),
    ADD COLUMN idle_action   TEXT CHECK (idle_action IN ('1', '2', '3')),
    ADD COLUMN idle_seconds  INTEGER CHECK (idle_seconds BETWEEN 1 AND 99999),
    ADD COLUMN active        BOOLEAN NOT NULL DEFAULT TRUE,
    -- TipTop 的 zwuser/zwgrup/zwdate 從未寫入，轉入的舊資料留空白
    ADD COLUMN created_by    TEXT NOT NULL DEFAULT '',
    ADD COLUMN created_dept  TEXT NOT NULL DEFAULT '',
    ADD COLUMN created_on    DATE NOT NULL DEFAULT CURRENT_DATE,
    ADD COLUMN updated_by    TEXT,
    ADD COLUMN updated_on    DATE,
    -- 只有「依權限類別」才有閒置處理與秒數，且兩者都要有（W5）
    ADD CONSTRAINT permission_class_idle_by_class CHECK (
        (idle_control = '2' AND idle_action IS NOT NULL AND idle_seconds IS NOT NULL)
        OR (idle_control <> '2' AND idle_action IS NULL AND idle_seconds IS NULL)
    );

COMMENT ON TABLE permission_class IS 'TipTop: zw_file 權限類別 (p_zw)';
COMMENT ON COLUMN permission_class.class_code IS 'zw01 權限類別（建立後不可修改，W1）';
COMMENT ON COLUMN permission_class.name IS 'zw02 權限說明';
COMMENT ON COLUMN permission_class.default_menu IS 'zw03 使用目錄編號（預設選單，模組 MENU 的程式）';
COMMENT ON COLUMN permission_class.idle_control IS 'zw04 閒置設定：1 不控制、2 依權限類別、3 依系統';
COMMENT ON COLUMN permission_class.idle_action IS 'zw05 閒置處理：1 僅警告、2 警告後關閉、3 直接關閉';
COMMENT ON COLUMN permission_class.idle_seconds IS 'zw06 閒置時間（秒）';
COMMENT ON COLUMN permission_class.active IS 'zwacti 資料有效';
COMMENT ON COLUMN permission_class.created_by IS 'zwuser 資料所有者';
COMMENT ON COLUMN permission_class.created_dept IS 'zwgrup 資料所有部門';
COMMENT ON COLUMN permission_class.updated_by IS 'zwmodu 最後修改者';
COMMENT ON COLUMN permission_class.updated_on IS 'zwdate 最後修改日期';
