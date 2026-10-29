-- p_zz 程式登錄。舊表對照見 docs/legacy-map/azz.yaml
-- 系統管理資料不分營運中心，因此不帶 plant_id（TipTop 的 zz/gaz/zy/zw/zm 同樣是共用表）。
-- 列舉欄位沿用 TipTop 原始代碼（'1'、'2'…），方便舊資料直接轉入。

CREATE TABLE module (
    module_code TEXT PRIMARY KEY,
    name        TEXT NOT NULL
);
COMMENT ON TABLE module IS 'TipTop: gao_file 模組';

CREATE TABLE permission_class (
    class_code  TEXT PRIMARY KEY,
    name        TEXT NOT NULL
);
COMMENT ON TABLE permission_class IS 'TipTop: zw_file 權限類別';

CREATE TABLE program (
    program_no           TEXT PRIMARY KEY,
    -- MENU 為固定選項，不在 module 表，故不設外鍵
    module_code          TEXT NOT NULL,
    program_type         TEXT,
    report_type          TEXT NOT NULL DEFAULT '5' CHECK (report_type IN ('1', '2', '3', '4', '5')),
    idle_control         TEXT NOT NULL DEFAULT '3' CHECK (idle_control IN ('1', '2', '3')),
    idle_seconds         INTEGER,
    idle_action          TEXT CHECK (idle_action IN ('1', '2', '3')),
    pass_book_no         BOOLEAN NOT NULL DEFAULT FALSE,
    deferred_print_count INTEGER NOT NULL DEFAULT 0,
    current_print_count  INTEGER NOT NULL DEFAULT 0,
    report_width         INTEGER,
    run_command          TEXT,
    window_style         TEXT,
    report_ext_mode      TEXT CHECK (report_ext_mode IN ('1', '2')),
    print_setting        TEXT,
    print_default        TEXT,
    quick_run_data       TEXT,
    help_enabled         BOOLEAN NOT NULL DEFAULT FALSE,
    log_errors           BOOLEAN NOT NULL DEFAULT FALSE,
    created_by           TEXT NOT NULL,
    created_dept         TEXT NOT NULL,
    updated_by           TEXT,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
COMMENT ON TABLE program IS 'TipTop: zz_file 程式登錄 (p_zz)';

-- 以下子表隨程式代號連動（Q2 改代號）、隨程式刪除（Q3 一律連帶刪除）
CREATE TABLE program_name (
    program_no  TEXT NOT NULL REFERENCES program (program_no) ON UPDATE CASCADE ON DELETE CASCADE,
    lang        TEXT NOT NULL,
    name        TEXT NOT NULL,
    PRIMARY KEY (program_no, lang)
);
COMMENT ON TABLE program_name IS 'TipTop: gaz_file 程式名稱（多語系）';

CREATE TABLE program_permission (
    class_code  TEXT NOT NULL REFERENCES permission_class (class_code),
    program_no  TEXT NOT NULL REFERENCES program (program_no) ON UPDATE CASCADE ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (class_code, program_no)
);
COMMENT ON TABLE program_permission IS 'TipTop: zy_file 群組權限';

CREATE TABLE menu_item (
    -- 選單本身也是程式（模組 MENU），但選單可先建立項目再登錄，故不設外鍵
    menu_no     TEXT NOT NULL,
    program_no  TEXT NOT NULL REFERENCES program (program_no) ON UPDATE CASCADE ON DELETE CASCADE,
    PRIMARY KEY (menu_no, program_no)
);
COMMENT ON TABLE menu_item IS 'TipTop: zm_file 選單（zm01 選單代號、zm04 子項程式）';

CREATE TABLE query_definition (
    query_no    TEXT PRIMARY KEY
);
COMMENT ON TABLE query_definition IS 'TipTop: zai_file p_query 查詢定義（目前只用於存在檢查）';

CREATE TABLE audit_log (
    audit_id    BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    program     TEXT NOT NULL,
    user_id     TEXT NOT NULL,
    key         TEXT NOT NULL,
    action      TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
COMMENT ON TABLE audit_log IS 'TipTop: azo_file 異動紀錄';
CREATE INDEX audit_log_key_idx ON audit_log (key);
