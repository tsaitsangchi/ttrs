-- p_zy 程式執行權限維護：補齊 zy_file 欄位。規格 specs/features/azz/program_permission_maintenance.feature；
-- 舊表對照見 docs/legacy-map/azz.yaml。資料權限沿用 TipTop 代碼（Y4）；zy06 列印方式不移植。

ALTER TABLE program_permission
    ADD COLUMN actions          TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN other_user_data  TEXT NOT NULL DEFAULT '0' CHECK (other_user_data IN ('0', '1', '2', '3', '4')),
    ADD COLUMN other_dept_data  TEXT NOT NULL DEFAULT '0'
        CHECK (other_dept_data IN ('0', '1', '2', '3', '4', '5', '6', '7', '8')),
    ADD COLUMN detail_rows      TEXT NOT NULL DEFAULT '0' CHECK (detail_rows IN ('0', '1', '2', '3')),
    ADD COLUMN created_by       TEXT NOT NULL DEFAULT '',
    ADD COLUMN created_dept     TEXT NOT NULL DEFAULT '',
    ADD COLUMN created_on       DATE NOT NULL DEFAULT CURRENT_DATE,
    ADD COLUMN updated_by       TEXT,
    ADD COLUMN updated_on       DATE;

-- 既有授權（由程式維護建立）：允許執行功能帶入程式的可執行功能（逗號分隔、去空白、轉小寫、去重複，Y1、Y3）
UPDATE program_permission pp
   SET created_on = pp.created_at::date,
       actions = COALESCE(ARRAY(
           SELECT t.action
             FROM (SELECT lower(btrim(u.item)) AS action, min(u.ord) AS ord
                     FROM unnest(string_to_array(p.default_actions, ',')) WITH ORDINALITY AS u(item, ord)
                    WHERE btrim(u.item) <> ''
                    GROUP BY 1) t
            ORDER BY t.ord), '{}')
  FROM program p
 WHERE p.program_no = pp.program_no;

COMMENT ON TABLE program_permission IS 'TipTop: zy_file 權限設定檔 (p_zy)：權限類別＋程式一筆，有此筆才可執行該程式';
COMMENT ON COLUMN program_permission.class_code IS 'zy01 權限類別';
COMMENT ON COLUMN program_permission.program_no IS 'zy02 程式代號';
COMMENT ON COLUMN program_permission.created_at IS '建立時間（0003 建立；以 created_on 為準）';
COMMENT ON COLUMN program_permission.actions IS 'zy03 允許執行功能：TipTop action id，只能取自程式的可執行功能 zz04（Y1）';
COMMENT ON COLUMN program_permission.other_user_data IS 'zy04 他人資料權限（查詢/修改/刪除）：0 全可、1 查改、2 查刪、3 只查、4 皆不可';
COMMENT ON COLUMN program_permission.other_dept_data IS 'zy05 他部門資料權限：0–4 同 zy04；5 相關部門查改、6 查刪、7 只查、8 全可（部門群組 zyw）';
COMMENT ON COLUMN program_permission.detail_rows IS 'zy07 單身權限：0 可新增刪除、1 只可新增、2 只可刪除、3 皆不可';
COMMENT ON COLUMN program_permission.created_by IS 'zyuser 資料所有者';
COMMENT ON COLUMN program_permission.created_dept IS 'zygrup 資料所有部門';
COMMENT ON COLUMN program_permission.created_on IS 'zydate 建立日期';
COMMENT ON COLUMN program_permission.updated_by IS 'zymodu 最後修改者';
COMMENT ON COLUMN program_permission.updated_on IS '最後修改日期';
