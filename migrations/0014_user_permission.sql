-- p_zxw 使用者特殊權限維護。規格 specs/features/azz/user_permission_maintenance.feature；舊表對照見 docs/legacy-map/azz.yaml。
-- 全公司共用一份、不帶 plant_id（X9：待 DBA 確認 TipTop 是否各營運中心不同）；ta_zxw01 確認否不移植（X10）。

CREATE TABLE user_permission (
    -- 刪除使用者時連同特殊權限（p_zx.4gl:1246-1249，X6）
    user_id          VARCHAR(10) NOT NULL REFERENCES user_account (user_id) ON DELETE CASCADE,
    line_no          INTEGER NOT NULL CHECK (line_no > 0),
    kind             TEXT NOT NULL CHECK (kind IN ('1', '2')),
    class_code       TEXT REFERENCES permission_class (class_code),
    -- 程式改代號、刪除時連動（同 program_permission）
    program_no       TEXT REFERENCES program (program_no) ON UPDATE CASCADE ON DELETE CASCADE,
    actions          TEXT[],
    other_user_data  TEXT CHECK (other_user_data IN ('0', '1', '2', '3', '4')),
    other_dept_data  TEXT CHECK (other_dept_data IN ('0', '1', '2', '3', '4', '5', '6', '7', '8')),
    detail_rows      TEXT CHECK (detail_rows IN ('0', '1', '2', '3')),
    created_by       TEXT NOT NULL,
    created_dept     TEXT NOT NULL,
    created_on       DATE NOT NULL,
    updated_by       TEXT,
    updated_on       DATE,
    PRIMARY KEY (user_id, line_no),
    -- 同一使用者不重複同一權限類別或程式（X3）
    UNIQUE (user_id, class_code),
    UNIQUE (user_id, program_no),
    -- 權限類別列不帶功能與資料權限（TQC-5A0094）；程式列一定有
    CONSTRAINT user_permission_kind CHECK (
        (kind = '1' AND class_code IS NOT NULL AND program_no IS NULL AND actions IS NULL
             AND other_user_data IS NULL AND other_dept_data IS NULL AND detail_rows IS NULL)
        OR (kind = '2' AND program_no IS NOT NULL AND class_code IS NULL AND actions IS NOT NULL
             AND other_user_data IS NOT NULL AND other_dept_data IS NOT NULL AND detail_rows IS NOT NULL)
    )
);

COMMENT ON TABLE user_permission IS 'TipTop: zxw_file 部門使用者權限類別檔 (p_zxw)：有特殊權限的使用者只用特殊權限，不用使用者維護的權限類別';
COMMENT ON COLUMN user_permission.user_id IS 'zxw01 使用者代號';
COMMENT ON COLUMN user_permission.line_no IS 'zxw02 項次（自動給號，只決定順序）';
COMMENT ON COLUMN user_permission.kind IS 'zxw03 權限分類：1 權限類別、2 程式編號';
COMMENT ON COLUMN user_permission.class_code IS 'zxw04（kind = 1）權限類別：取得該類別的程式執行權限';
COMMENT ON COLUMN user_permission.program_no IS 'zxw04（kind = 2）程式代號';
COMMENT ON COLUMN user_permission.actions IS 'zxw05 允許執行功能（程式列；只能取自程式的可執行功能）';
COMMENT ON COLUMN user_permission.other_user_data IS 'zxw06 資料所有人權限（同 program_permission.other_user_data）';
COMMENT ON COLUMN user_permission.other_dept_data IS 'zxw07 資料部門權限（同 program_permission.other_dept_data）';
COMMENT ON COLUMN user_permission.detail_rows IS 'zxw08 單身權限（同 program_permission.detail_rows）';
COMMENT ON COLUMN user_permission.created_by IS '資料所有者（TipTop zxw_file 無此欄）';
COMMENT ON COLUMN user_permission.created_dept IS '資料所有部門';
COMMENT ON COLUMN user_permission.created_on IS '建立日期';
COMMENT ON COLUMN user_permission.updated_by IS '最後修改者';
COMMENT ON COLUMN user_permission.updated_on IS '最後修改日期';
