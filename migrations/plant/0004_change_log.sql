-- 工廠層（docs/adr/0007-multi-plant.md）：資料異動記錄（docs/adr/0012-change-history.md；change_history.feature）
-- 取代 TipTop azo_file 系統重要資料修改記錄檔。觸發器函式在 public（migrations/0024_change_log.sql）。

CREATE TABLE change_log (
    change_id       bigint GENERATED ALWAYS AS IDENTITY,
    occurred_at     timestamptz NOT NULL DEFAULT now(),
    table_name      text NOT NULL,
    record_key      jsonb NOT NULL,
    operation       text NOT NULL,
    before          jsonb,
    after           jsonb,
    changed_fields  text[] NOT NULL DEFAULT '{}',
    version         integer,
    user_id         text NOT NULL,
    user_dept       text,
    actor_kind      text NOT NULL CHECK (actor_kind IN ('user', 'ai', 'service', 'db')),
    on_behalf_of    text,
    program         text,
    action          text,
    request_id      uuid,
    client_ip       inet,
    user_agent      text,
    reason          text,
    PRIMARY KEY (change_id, occurred_at)
) PARTITION BY RANGE (occurred_at);
CREATE TABLE change_log_default PARTITION OF change_log DEFAULT;
CREATE INDEX change_log_record ON change_log (table_name, record_key);
COMMENT ON TABLE change_log IS 'TipTop: azo_file 系統重要資料修改記錄檔（ADR 0012；改為所有異動自動記錄改前改後）；工廠層，只能新增';
COMMENT ON COLUMN change_log.occurred_at IS 'azo03 異動日期＋azo04 異動時間（交易時間）';
COMMENT ON COLUMN change_log.table_name IS '資料表';
COMMENT ON COLUMN change_log.record_key IS 'azo05 異動資料的 KEY VALUE（主鍵欄位與值）';
COMMENT ON COLUMN change_log.operation IS 'insert／update／delete 或業務動作（activate、deactivate…）；取代 azo06 異動說明';
COMMENT ON COLUMN change_log.before IS '改前整筆內容（個資欄位遮蔽）';
COMMENT ON COLUMN change_log.after IS '改後整筆內容（個資欄位遮蔽）';
COMMENT ON COLUMN change_log.changed_fields IS '有變動的欄位（不含版本、修改者、修改時間等管理欄位）';
COMMENT ON COLUMN change_log.version IS '異動後的版本號';
COMMENT ON COLUMN change_log.user_id IS 'azo02 異動人員；直接 SQL 為 db:<資料庫角色>';
COMMENT ON COLUMN change_log.program IS 'azo01 程式編號';

-- 應用程式的資料庫角色只能新增與查詢（§5）；之後每次啟動由 ttrs_change_log_partitions 再確認
SELECT public.ttrs_change_log_partitions(current_schema());

-- 版本號與建立、最後修改時間（§1、§4）
ALTER TABLE department
    ADD COLUMN version     integer NOT NULL DEFAULT 1,
    ADD COLUMN created_at  timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN updated_at  timestamptz;
ALTER TABLE employee
    ADD COLUMN version     integer NOT NULL DEFAULT 1,
    ADD COLUMN created_at  timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN updated_at  timestamptz;
ALTER TABLE classification_code
    ADD COLUMN version     integer NOT NULL DEFAULT 1,
    ADD COLUMN created_at  timestamptz NOT NULL DEFAULT now(),
    ADD COLUMN updated_at  timestamptz;
COMMENT ON COLUMN department.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN department.created_at IS '建立時間（到秒）；TTRS 新增';
COMMENT ON COLUMN department.updated_at IS '最後修改時間（到秒）；gemdate 只到日';
COMMENT ON COLUMN employee.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN employee.created_at IS '建立時間（到秒）；TTRS 新增';
COMMENT ON COLUMN employee.updated_at IS '最後修改時間（到秒）；gendate 只到日';
COMMENT ON COLUMN classification_code.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN classification_code.created_at IS '建立時間（到秒）；TTRS 新增';
COMMENT ON COLUMN classification_code.updated_at IS '最後修改時間（到秒）；azfdate 只到日';

CREATE TRIGGER department_version BEFORE UPDATE ON department
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER department_change_log AFTER INSERT OR UPDATE OR DELETE ON department
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('dept_code');
CREATE TRIGGER employee_version BEFORE UPDATE ON employee
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
-- email 為個資，只記「有修改」（CH6）
CREATE TRIGGER employee_change_log AFTER INSERT OR UPDATE OR DELETE ON employee
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('employee_id', 'email');
CREATE TRIGGER classification_code_version BEFORE UPDATE ON classification_code
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_bump_version();
CREATE TRIGGER classification_code_change_log AFTER INSERT OR UPDATE OR DELETE ON classification_code
    FOR EACH ROW EXECUTE FUNCTION public.ttrs_change_log('kind,code');
