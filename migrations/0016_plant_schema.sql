-- 一廠一個 PostgreSQL schema（docs/adr/0007-multi-plant.md；plant_maintenance.feature P13–P15）
-- 營運中心的資料庫代碼（TipTop azp03）即工廠 schema 名稱：ds 加兩位數字，小寫。
-- 工廠層的表放在各工廠 schema（migrations/plant/，由 ttrs_infra 對每個工廠 schema 套用）。

ALTER TABLE plant ADD COLUMN schema_name text;
UPDATE plant p
   SET schema_name = 'ds' || lpad(x.n::text, 2, '0')
  FROM (SELECT plant_id, row_number() OVER (ORDER BY plant_id COLLATE "C") AS n FROM plant) x
 WHERE p.plant_id = x.plant_id;
ALTER TABLE plant
    ALTER COLUMN schema_name SET NOT NULL,
    ADD CONSTRAINT plant_schema_name_key UNIQUE (schema_name),
    ADD CONSTRAINT plant_schema_name_check CHECK (schema_name ~ '^ds[0-9]{2}$');
COMMENT ON COLUMN plant.schema_name IS 'azp03 資料庫代碼：工廠 schema 名稱（ds01…，P13）；建立後不可修改';

-- 料件、庫存異動改為工廠層（migrations/plant/0001_item.sql）。開發期資料，不保留
DROP TABLE inventory_txn;
DROP TABLE item;

-- 工廠 schema 的資料庫角色：<資料庫名稱>_<schema>（角色整台伺服器共用，名稱帶資料庫名稱）
CREATE FUNCTION ttrs_plant_role(p_schema text) RETURNS text
    LANGUAGE sql STABLE AS $$ SELECT current_database() || '_' || p_schema $$;

-- 建立工廠 schema 與角色（已存在則略過）；角色只能存取自己的 schema（ADR 0007 §3）
CREATE FUNCTION ttrs_ensure_plant_schema(p_schema text) RETURNS void
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
END $$;

-- 工廠 schema 內是否有任何資料（刪除營運中心前檢查，P10）
CREATE FUNCTION ttrs_plant_schema_has_data(p_schema text) RETURNS boolean
    LANGUAGE plpgsql AS $$
DECLARE
    t record;
    found boolean;
BEGIN
    FOR t IN SELECT table_name FROM information_schema.tables
              WHERE table_schema = p_schema AND table_type = 'BASE TABLE' AND table_name <> '_sqlx_migrations'
    LOOP
        EXECUTE format('SELECT EXISTS (SELECT 1 FROM %I.%I)', p_schema, t.table_name) INTO found;
        IF found THEN
            RETURN true;
        END IF;
    END LOOP;
    RETURN false;
END $$;

-- 移除工廠 schema 與角色（P15）
CREATE FUNCTION ttrs_drop_plant_schema(p_schema text) RETURNS void
    LANGUAGE plpgsql AS $$
DECLARE
    r text := ttrs_plant_role(p_schema);
BEGIN
    IF p_schema !~ '^ds[0-9]{2}$' THEN
        RAISE EXCEPTION '工廠 schema 名稱不符：%', p_schema;
    END IF;
    EXECUTE format('DROP SCHEMA IF EXISTS %I CASCADE', p_schema);
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
        EXECUTE format('DROP OWNED BY %I', r);
        EXECUTE format('DROP ROLE %I', r);
    END IF;
END $$;
