-- 工廠層（docs/adr/0007-multi-plant.md）：專案（TipTop aooi200 / gja_file；specs/features/aoo/project_maintenance.feature PJ1–PJ6）
CREATE TABLE project (
    project_code varchar(10) PRIMARY KEY CHECK (char_length(project_code) >= 1),
    name         varchar(80),
    start_on     date,
    end_on       date,
    active       boolean NOT NULL DEFAULT true,
    confirmed    boolean NOT NULL DEFAULT false,
    confirmed_by varchar(10),
    confirmed_at timestamptz,
    created_by   text NOT NULL,
    created_dept text NOT NULL DEFAULT '',
    created_on   date NOT NULL DEFAULT CURRENT_DATE,
    updated_by   text,
    updated_on   date,
    version      integer NOT NULL DEFAULT 1,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz,
    -- 結束日期不可早於起始日期（aooi200.4gl:318-336，apj-018，PJ3）
    CHECK (start_on IS NULL OR end_on IS NULL OR end_on >= start_on)
);
COMMENT ON TABLE project IS 'TipTop: gja_file 專案名稱檔（aooi200 專案名稱維護）；工廠層（PJ2）';
COMMENT ON COLUMN project.project_code IS 'gja01 專案編號（區分大小寫、建立後不可修改，PJ5）';
COMMENT ON COLUMN project.name IS 'gja02 專案名稱';
COMMENT ON COLUMN project.start_on IS 'gja03 專案起始日期（可空白）';
COMMENT ON COLUMN project.end_on IS 'gja04 專案結束日期（可空白；不可早於起始日期）';
COMMENT ON COLUMN project.active IS 'gjaacti 資料有效碼';
COMMENT ON COLUMN project.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9；gja_file 無對應欄位）';
COMMENT ON COLUMN project.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN project.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN project.created_by IS 'gjauser 資料所有者';
COMMENT ON COLUMN project.created_dept IS 'gjagrup 資料所有部門';
COMMENT ON COLUMN project.updated_by IS 'gjamodu 資料修改者';
COMMENT ON COLUMN project.updated_on IS 'gjadate 最近修改日';
COMMENT ON COLUMN project.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TRIGGER project_version BEFORE UPDATE ON project
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER project_change_log AFTER INSERT OR UPDATE OR DELETE ON project
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('project_code');
