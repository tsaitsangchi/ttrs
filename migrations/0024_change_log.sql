-- 資料異動記錄（docs/adr/0012-change-history.md；specs/features/azz/change_history.feature）
-- 取代 TipTop azo_file（系統重要資料修改記錄檔）與 cl_log 文字記錄檔。
-- 共用的觸發器函式放 public；各工廠 schema 的 change_log 由工廠 migration（migrations/plant/0004）建立。

-- 版本號與最後修改時間（§1、§4）：每次 UPDATE 版本加 1。use case 以 WHERE version = 讀取時的版本 做樂觀鎖
CREATE FUNCTION ttrs_bump_version() RETURNS trigger
    LANGUAGE plpgsql AS $$
BEGIN
    NEW.version := OLD.version + 1;
    NEW.updated_at := now();
    RETURN NEW;
END $$;

-- 異動記錄（§2、§3）：AFTER INSERT/UPDATE/DELETE，寫入同一 schema 的 change_log。
-- 參數：1 主鍵欄位（逗號分隔）、2 遮蔽欄位（個資，只記「有修改」，§7）。
-- 操作情境由應用程式在交易開始時以 set_config('ttrs.*', …, true) 設定；沒有時記為 db（直接修改資料，AU8）。
-- SECURITY DEFINER：應用程式的資料庫角色對 change_log 沒有 UPDATE／DELETE（§5）
CREATE FUNCTION ttrs_change_log() RETURNS trigger
    LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, public AS $$
DECLARE
    b jsonb;
    a jsonb;
    k jsonb := '{}';
    col text;
    changed text[];
    op text := lower(TG_OP);
    uid text := nullif(current_setting('ttrs.user_id', true), '');
    meta text[] := ARRAY['version', 'updated_by', 'updated_on', 'updated_at', 'created_at'];
BEGIN
    IF TG_OP <> 'INSERT' THEN b := to_jsonb(OLD); END IF;
    IF TG_OP <> 'DELETE' THEN a := to_jsonb(NEW); END IF;
    FOREACH col IN ARRAY string_to_array(TG_ARGV[0], ',') LOOP
        k := k || jsonb_build_object(col, coalesce(a, b) ->> col);
    END LOOP;
    IF TG_OP = 'UPDATE' THEN
        SELECT coalesce(array_agg(n.key ORDER BY n.key), '{}') INTO changed
          FROM jsonb_each(a) n
         WHERE n.value IS DISTINCT FROM b -> n.key AND NOT n.key = ANY (meta);
        op := coalesce(nullif(current_setting('ttrs.operation', true), ''), 'update');
    ELSE
        SELECT coalesce(array_agg(n.key ORDER BY n.key), '{}') INTO changed
          FROM jsonb_each(coalesce(a, b)) n
         WHERE NOT n.key = ANY (meta);
    END IF;
    IF TG_NARGS > 1 THEN
        FOREACH col IN ARRAY string_to_array(TG_ARGV[1], ',') LOOP
            IF b ->> col IS NOT NULL THEN b := jsonb_set(b, ARRAY[col], '"***"'); END IF;
            IF a ->> col IS NOT NULL THEN a := jsonb_set(a, ARRAY[col], '"***"'); END IF;
        END LOOP;
    END IF;
    EXECUTE format(
        'INSERT INTO %I.change_log (table_name, record_key, operation, before, after, changed_fields, version,
                                    user_id, user_dept, actor_kind, on_behalf_of, program, action,
                                    request_id, client_ip, user_agent, reason)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)', TG_TABLE_SCHEMA)
    USING TG_TABLE_NAME, k, op, b, a, changed, (coalesce(a, b) ->> 'version')::integer,
          coalesce(uid, 'db:' || session_user),
          nullif(current_setting('ttrs.user_dept', true), ''),
          CASE WHEN uid IS NULL THEN 'db'
               ELSE coalesce(nullif(current_setting('ttrs.actor_kind', true), ''), 'user') END,
          nullif(current_setting('ttrs.on_behalf_of', true), ''),
          nullif(current_setting('ttrs.program', true), ''),
          nullif(current_setting('ttrs.action', true), ''),
          nullif(current_setting('ttrs.request_id', true), '')::uuid,
          nullif(current_setting('ttrs.client_ip', true), '')::inet,
          nullif(current_setting('ttrs.user_agent', true), ''),
          nullif(current_setting('ttrs.reason', true), '');
    RETURN NULL;
END $$;

-- 月份分割（CH7）：建立本月與下個月的分割表（預設分割表已有該月資料時略過），並確保應用程式角色只能新增與查詢
CREATE FUNCTION ttrs_change_log_partitions(p_schema text) RETURNS void
    LANGUAGE plpgsql SET search_path = pg_catalog, public AS $$
DECLARE
    r text := ttrs_plant_role(p_schema);
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
    FOR part IN
        SELECT c.relname
          FROM pg_inherits i
          JOIN pg_class c ON c.oid = i.inhrelid
          JOIN pg_class p ON p.oid = i.inhparent
          JOIN pg_namespace n ON n.oid = p.relnamespace
         WHERE n.nspname = p_schema AND p.relname = 'change_log'
    LOOP
        EXECUTE format('REVOKE UPDATE, DELETE, TRUNCATE ON %I.%I FROM %I', p_schema, part, r);
    END LOOP;
    EXECUTE format('REVOKE UPDATE, DELETE, TRUNCATE ON %I.change_log FROM %I', p_schema, r);
END $$;

-- 建立（或補齊）工廠 schema 與角色：同 0016，最後補 change_log 的分割表與權限（啟動時每次執行）
CREATE OR REPLACE FUNCTION ttrs_ensure_plant_schema(p_schema text) RETURNS void
    LANGUAGE plpgsql AS $$
DECLARE
    r text := ttrs_plant_role(p_schema);
BEGIN
    IF p_schema !~ '^ds[0-9]{2}$' THEN
        RAISE EXCEPTION '工廠 schema 名稱不符：%', p_schema;
    END IF;
    EXECUTE format('CREATE SCHEMA IF NOT EXISTS %I', p_schema);
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
        EXECUTE format('CREATE ROLE %I NOLOGIN', r);
    END IF;
    EXECUTE format('GRANT %I TO %I', r, session_user);
    EXECUTE format('GRANT USAGE ON SCHEMA %I TO %I', p_schema, r);
    EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA %I TO %I', p_schema, r);
    EXECUTE format('GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA %I TO %I', p_schema, r);
    EXECUTE format('ALTER DEFAULT PRIVILEGES IN SCHEMA %I GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO %I',
                   p_schema, r);
    EXECUTE format('ALTER DEFAULT PRIVILEGES IN SCHEMA %I GRANT USAGE, SELECT ON SEQUENCES TO %I', p_schema, r);
    PERFORM ttrs_change_log_partitions(p_schema);
END $$;

-- 功能 history「異動記錄」（AU11，TTRS 新增）：已套用異動記錄的作業；須與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
UPDATE program SET default_actions = 'query, detail, account, output, exporttoexcel, history'
 WHERE program_no = 'department-maintenance';
UPDATE program SET default_actions = 'query, detail, history'
 WHERE program_no IN ('employee-maintenance', 'classification-code-maintenance');

UPDATE program_permission
   SET actions = actions || ARRAY['history']
 WHERE class_code = 'ADMIN'
   AND program_no IN ('department-maintenance', 'employee-maintenance', 'classification-code-maintenance')
   AND NOT ('history' = ANY (actions));
