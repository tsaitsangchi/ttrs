-- 資料異動記錄第二階段：共用層的表（docs/adr/0012-change-history.md；change_history.feature CH1）
-- public.change_log 與工廠的 change_log 同結構；觸發器函式沿用 0024（寫入 TG_TABLE_SCHEMA.change_log）。

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
COMMENT ON TABLE change_log IS 'TipTop: azo_file 系統重要資料修改記錄檔（ADR 0012）；共用層的表（使用者、程式、權限、營運中心、資料字典），只能新增';

-- 共用 schema 由應用程式的登入角色擁有，無法以權限限制：以觸發器擋下修改與刪除（§5）。
-- 歸檔或測試清除時，須在交易內 set_config('ttrs.allow_purge', 'on', true)
CREATE FUNCTION ttrs_change_log_guard() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    IF current_setting('ttrs.allow_purge', true) = 'on' THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION '異動記錄不可修改或刪除' USING ERRCODE = 'insufficient_privilege';
END $$;
CREATE TRIGGER change_log_guard BEFORE UPDATE OR DELETE ON change_log
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log_guard();

-- 月份分割：共用 schema 沒有工廠角色，不做 REVOKE（由上面的觸發器保護）
CREATE OR REPLACE FUNCTION ttrs_change_log_partitions(p_schema text) RETURNS void
    LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    m date;
    part text;
    has_rows boolean;
BEGIN
    IF to_regclass(format('%I.change_log', p_schema)) IS NULL THEN
        RETURN;
    END IF;
    FOR i IN 0..1 LOOP
        m := (date_trunc('month', now()) + make_interval(months => i))::date;
        part := 'change_log_' || to_char(m, 'YYYYMM');
        IF to_regclass(format('%I.%I', p_schema, part)) IS NULL THEN
            EXECUTE format('SELECT EXISTS (SELECT 1 FROM %I.change_log_default WHERE occurred_at >= %L AND occurred_at < %L)',
                           p_schema, m, (m + interval '1 month')::date) INTO has_rows;
            IF NOT has_rows THEN
                EXECUTE format('CREATE TABLE %I.%I PARTITION OF %I.change_log FOR VALUES FROM (%L) TO (%L)',
                               p_schema, part, p_schema, m, (m + interval '1 month')::date);
            END IF;
        END IF;
    END LOOP;
    IF p_schema = 'public' THEN
        RETURN;
    END IF;
    FOR part IN
        SELECT c.relname
          FROM pg_inherits i
          JOIN pg_class c ON c.oid = i.inhrelid
          JOIN pg_class p ON p.oid = i.inhparent
          JOIN pg_namespace n ON n.oid = p.relnamespace
         WHERE n.nspname = p_schema AND p.relname = 'change_log'
    LOOP
        EXECUTE format('REVOKE UPDATE, DELETE, TRUNCATE ON %I.%I FROM %I', p_schema, part, ttrs_plant_role(p_schema));
    END LOOP;
    EXECUTE format('REVOKE UPDATE, DELETE, TRUNCATE ON %I.change_log FROM %I', p_schema, ttrs_plant_role(p_schema));
END $$;
SELECT ttrs_change_log_partitions('public');

-- 版本號與建立、最後修改時間（§1、§4）：主檔加 version／created_at／updated_at（已有者略過）
ALTER TABLE user_account       ADD COLUMN version integer NOT NULL DEFAULT 1,
                               ADD COLUMN created_at timestamptz NOT NULL DEFAULT now(),
                               ADD COLUMN updated_at timestamptz;
ALTER TABLE permission_class   ADD COLUMN version integer NOT NULL DEFAULT 1,
                               ADD COLUMN created_at timestamptz NOT NULL DEFAULT now(),
                               ADD COLUMN updated_at timestamptz;
ALTER TABLE plant              ADD COLUMN version integer NOT NULL DEFAULT 1,
                               ADD COLUMN updated_at timestamptz;
ALTER TABLE program            ADD COLUMN version integer NOT NULL DEFAULT 1;
ALTER TABLE program_permission ADD COLUMN version integer NOT NULL DEFAULT 1,
                               ADD COLUMN updated_at timestamptz;
ALTER TABLE user_permission    ADD COLUMN version integer NOT NULL DEFAULT 1,
                               ADD COLUMN created_at timestamptz NOT NULL DEFAULT now(),
                               ADD COLUMN updated_at timestamptz;
ALTER TABLE table_dictionary   ADD COLUMN version integer NOT NULL DEFAULT 1,
                               ADD COLUMN created_at timestamptz NOT NULL DEFAULT now();
ALTER TABLE account_policy     ADD COLUMN version integer NOT NULL DEFAULT 1,
                               ADD COLUMN created_at timestamptz NOT NULL DEFAULT now(),
                               ADD COLUMN updated_at timestamptz;
COMMENT ON COLUMN user_account.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN permission_class.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN plant.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN program.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN program_permission.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN user_permission.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN table_dictionary.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';
COMMENT ON COLUMN account_policy.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TRIGGER user_account_version BEFORE UPDATE ON user_account FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER permission_class_version BEFORE UPDATE ON permission_class FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER plant_version BEFORE UPDATE ON plant FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER program_version BEFORE UPDATE ON program FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER program_permission_version BEFORE UPDATE ON program_permission FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER user_permission_version BEFORE UPDATE ON user_permission FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER table_dictionary_version BEFORE UPDATE ON table_dictionary FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER account_policy_version BEFORE UPDATE ON account_policy FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();

-- 異動記錄（CH1）：主檔與子表；user_account.email 為個資，只記「有修改」（CH6）
CREATE TRIGGER user_account_change_log AFTER INSERT OR UPDATE OR DELETE ON user_account
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('user_id', 'email');
CREATE TRIGGER user_plant_change_log AFTER INSERT OR UPDATE OR DELETE ON user_plant
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('user_id,plant_id');
CREATE TRIGGER permission_class_change_log AFTER INSERT OR UPDATE OR DELETE ON permission_class
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('class_code');
CREATE TRIGGER plant_change_log AFTER INSERT OR UPDATE OR DELETE ON plant
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('plant_id');
CREATE TRIGGER program_change_log AFTER INSERT OR UPDATE OR DELETE ON program
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('program_no');
CREATE TRIGGER program_name_change_log AFTER INSERT OR UPDATE OR DELETE ON program_name
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('program_no,lang');
CREATE TRIGGER program_permission_change_log AFTER INSERT OR UPDATE OR DELETE ON program_permission
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('class_code,program_no');
CREATE TRIGGER user_permission_change_log AFTER INSERT OR UPDATE OR DELETE ON user_permission
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('user_id,line_no');
CREATE TRIGGER account_policy_change_log AFTER INSERT OR UPDATE OR DELETE ON account_policy
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('policy_key');
CREATE TRIGGER table_dictionary_change_log AFTER INSERT OR UPDATE OR DELETE ON table_dictionary
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('table_name');
CREATE TRIGGER table_dictionary_text_change_log AFTER INSERT OR UPDATE OR DELETE ON table_dictionary_text
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('table_name,lang');
CREATE TRIGGER column_dictionary_text_change_log AFTER INSERT OR UPDATE OR DELETE ON column_dictionary_text
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('table_name,column_name,lang');

-- 功能 history「異動記錄」（AU11）：共用層的維護作業；須與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
UPDATE program SET default_actions = default_actions || ', history'
 WHERE program_no IN ('program-maintenance', 'data-dictionary', 'user-maintenance', 'permission-class-maintenance',
                      'plant-maintenance', 'program-permission-maintenance', 'user-permission-maintenance')
   AND default_actions NOT LIKE '%history%';

UPDATE program_permission
   SET actions = actions || ARRAY['history']
 WHERE class_code = 'ADMIN'
   AND program_no IN ('program-maintenance', 'data-dictionary', 'user-maintenance', 'permission-class-maintenance',
                      'plant-maintenance', 'program-permission-maintenance', 'user-permission-maintenance')
   AND NOT ('history' = ANY (actions));
