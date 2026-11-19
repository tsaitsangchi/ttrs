-- 工廠層（docs/adr/0007-multi-plant.md）：單別（TipTop asmi300 / smy_file、smz_file、smu_file、smv_file；
-- specs/features/aim/document_type.feature）。本批只開放系統別 aim（DT3）。
CREATE TABLE document_type (
    doc_type_code     varchar(5) PRIMARY KEY CHECK (doc_type_code !~ '[[:space:]-]' AND char_length(doc_type_code) BETWEEN 3 AND 5),
    name              varchar(80) NOT NULL CHECK (char_length(trim(name)) >= 1),
    system            varchar(3) NOT NULL,
    kind              char(1) NOT NULL,
    auto_number       boolean NOT NULL DEFAULT true,
    numbering         char(1) NOT NULL DEFAULT '2' CHECK (numbering IN ('1', '2', '3', '4')),
    print_now         boolean NOT NULL DEFAULT false,
    confirm_now       boolean NOT NULL DEFAULT false,
    cost_enabled      boolean NOT NULL DEFAULT true,
    cost_class        char(1) NOT NULL CHECK (cost_class IN ('1', '2', '3', '4', '5', 'X')),
    affects_idle_date boolean NOT NULL DEFAULT true,
    internal_trade    boolean NOT NULL DEFAULT false,
    budget_control    boolean NOT NULL DEFAULT false,
    memo              varchar(255),
    restriction       char(1) NOT NULL DEFAULT '0' CHECK (restriction IN ('0', '1', '2', '3', '4')),
    active            boolean NOT NULL DEFAULT true,
    confirmed         boolean NOT NULL DEFAULT false,
    confirmed_by      varchar(10),
    confirmed_at      timestamptz,
    created_by        text NOT NULL,
    created_dept      text NOT NULL DEFAULT '',
    created_on        date NOT NULL DEFAULT CURRENT_DATE,
    updated_by        text,
    updated_on        date,
    version           integer NOT NULL DEFAULT 1,
    created_at        timestamptz NOT NULL DEFAULT now(),
    updated_at        timestamptz,
    CHECK (NOT (cost_enabled AND cost_class = 'X'))
);
COMMENT ON TABLE document_type IS 'TipTop: smy_file 製造管理系統使用單據性質檔（asmi300，document-type-maintenance）；工廠層（DT2）';
COMMENT ON COLUMN document_type.doc_type_code IS 'smyslip 單別（長度依 aza41，不可有空白或「-」，DT5）';
COMMENT ON COLUMN document_type.name IS 'smydesc 單據名稱（TTRS 必填，DT6(a)）';
COMMENT ON COLUMN document_type.system IS 'smysys 系統別（本批只開放 aim，DT3）';
COMMENT ON COLUMN document_type.kind IS 'smykind 單據性質（DT4）';
COMMENT ON COLUMN document_type.auto_number IS 'smyauno 自動編號（盤點單一律是，DT6(b)）';
COMMENT ON COLUMN document_type.numbering IS 'smydmy5 編號方式 1 流水號、2 年月、3 年週、4 年月日（1、3 意義待確認，DT6(b)）';
COMMENT ON COLUMN document_type.print_now IS 'smyprint 立即列印';
COMMENT ON COLUMN document_type.confirm_now IS 'smydmy4 立即確認';
COMMENT ON COLUMN document_type.cost_enabled IS 'smydmy1 計算成本（成會入項）';
COMMENT ON COLUMN document_type.cost_class IS 'smydmy2 成會單據類別 1 入庫、2 銷貨、3 領出、4 轉撥、5 調整、X 非庫存異動（DT7）';
COMMENT ON COLUMN document_type.affects_idle_date IS 'smy56 是否影響呆滯日期';
COMMENT ON COLUMN document_type.internal_trade IS 'smy53 兩交內部交易單據';
COMMENT ON COLUMN document_type.budget_control IS 'smy59 是否做預算控管';
COMMENT ON COLUMN document_type.memo IS 'smymemo1 備註';
COMMENT ON COLUMN document_type.restriction IS 'smyware 倉庫設限方式 0 不限、1/2 只能用、3/4 不能用列出的倉庫（儲位）（DT8）';
COMMENT ON COLUMN document_type.active IS 'smyacti 資料有效碼';
COMMENT ON COLUMN document_type.confirmed IS 'TTRS 標準確認欄位（ADR 0013 SA9；smy_file 無對應欄位）';
COMMENT ON COLUMN document_type.created_by IS 'smyuser 資料所有者';
COMMENT ON COLUMN document_type.created_dept IS 'smygrup 資料所有部門';
COMMENT ON COLUMN document_type.updated_by IS 'smymodu 資料修改者';
COMMENT ON COLUMN document_type.updated_on IS 'smydate 最近修改日';
COMMENT ON COLUMN document_type.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TABLE document_type_storage (
    doc_type_code varchar(5) NOT NULL REFERENCES document_type (doc_type_code) ON DELETE CASCADE,
    warehouse     varchar(10) NOT NULL CHECK (char_length(warehouse) >= 1),
    location      varchar(10) NOT NULL DEFAULT '',
    PRIMARY KEY (doc_type_code, warehouse, location)
);
COMMENT ON TABLE document_type_storage IS 'TipTop: smz_file 單別限定倉庫儲位明細檔（asmi300 倉庫單身，DT8）；可用 * ? 萬用字元';
COMMENT ON COLUMN document_type_storage.warehouse IS 'smz021 設限倉庫（使用者輸入值；smz02 的 % _ 轉換值不移植）';
COMMENT ON COLUMN document_type_storage.location IS 'smz031 設限儲位；空字串為儲位空白';

CREATE TABLE document_type_user (
    doc_type_code varchar(5) NOT NULL REFERENCES document_type (doc_type_code) ON DELETE CASCADE,
    user_id       varchar(10) NOT NULL,
    PRIMARY KEY (doc_type_code, user_id)
);
COMMENT ON TABLE document_type_user IS 'TipTop: smu_file 單別限定使用者明細檔（asmi300 使用者設限，DT9）；不再存系統別 smu03';
COMMENT ON COLUMN document_type_user.user_id IS 'smu02 設限使用者（zx01）';

CREATE TABLE document_type_dept (
    doc_type_code varchar(5) NOT NULL REFERENCES document_type (doc_type_code) ON DELETE CASCADE,
    dept_code     varchar(10) NOT NULL REFERENCES department (dept_code),
    PRIMARY KEY (doc_type_code, dept_code)
);
COMMENT ON TABLE document_type_dept IS 'TipTop: smv_file 單別限使用部門明細檔（asmi300 部門設限，DT9）；不再存系統別 smv03';
COMMENT ON COLUMN document_type_dept.dept_code IS 'smv02 設限使用部門';

CREATE TRIGGER document_type_version BEFORE UPDATE ON document_type
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER document_type_change_log AFTER INSERT OR UPDATE OR DELETE ON document_type
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_type_code');
CREATE TRIGGER document_type_storage_change_log AFTER INSERT OR UPDATE OR DELETE ON document_type_storage
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_type_code,warehouse,location');
CREATE TRIGGER document_type_user_change_log AFTER INSERT OR UPDATE OR DELETE ON document_type_user
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_type_code,user_id');
CREATE TRIGGER document_type_dept_change_log AFTER INSERT OR UPDATE OR DELETE ON document_type_dept
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('doc_type_code,dept_code');
