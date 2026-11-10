-- 基本代碼（共用 schema）：區域別 aooi100 / gea_file、國別 aooi090 / geb_file、地區 aooi110 / geo_file、
-- 交運方式 aooi160 / ged_file、報表抬頭 aooi210 / azh_file
-- 規格：specs/features/aoo/region_country_area_maintenance.feature（RC1–RC7）、shipping_method_maintenance.feature（SM1–SM6）、
--       report_title_maintenance.feature（RH1–RH6）；標準欄位與觸發器（ADR 0012）、確認欄位（ADR 0013 SA9）

-- ----- 區域別 -----
CREATE TABLE region (
    region_code  varchar(10) PRIMARY KEY CHECK (char_length(region_code) >= 1),
    name         varchar(80),
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
    updated_at   timestamptz
);
COMMENT ON TABLE region IS 'TipTop: gea_file 區域別檔（aooi100 區域別資料維護）；共用 schema（RC2）';
COMMENT ON COLUMN region.region_code IS 'gea01 區域編號（區分大小寫、建立後不可修改，RC5）';
COMMENT ON COLUMN region.name IS 'gea02 區域名稱';
COMMENT ON COLUMN region.active IS 'geaacti 資料有效碼';
COMMENT ON COLUMN region.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9；gea_file 無對應欄位）';
COMMENT ON COLUMN region.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN region.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN region.created_by IS 'geauser 資料所有者';
COMMENT ON COLUMN region.created_dept IS 'geagrup 資料所有部門';
COMMENT ON COLUMN region.updated_by IS 'geamodu 資料修改者';
COMMENT ON COLUMN region.updated_on IS 'geadate 最近修改日';
COMMENT ON COLUMN region.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

-- ----- 國別 -----
CREATE TABLE country (
    country_code varchar(10) PRIMARY KEY CHECK (char_length(country_code) >= 1),
    name         varchar(80),
    -- 被國別引用的區域不可刪除（RC4）
    region_code  varchar(10) REFERENCES region (region_code) ON DELETE RESTRICT,
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
    updated_at   timestamptz
);
CREATE INDEX country_region ON country (region_code);
COMMENT ON TABLE country IS 'TipTop: geb_file 國別檔（aooi090 國別資料維護）；共用 schema（RC2）';
COMMENT ON COLUMN country.country_code IS 'geb01 國別編號（區分大小寫、建立後不可修改，RC5）';
COMMENT ON COLUMN country.name IS 'geb02 國別名稱';
COMMENT ON COLUMN country.region_code IS 'geb03 隸屬區域編號（可空白；須存在且有效，aoo-061、9028，RC3）';
COMMENT ON COLUMN country.active IS 'gebacti 資料有效碼';
COMMENT ON COLUMN country.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9；geb_file 無對應欄位）';
COMMENT ON COLUMN country.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN country.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN country.created_by IS 'gebuser 資料所有者';
COMMENT ON COLUMN country.created_dept IS 'gebgrup 資料所有部門';
COMMENT ON COLUMN country.updated_by IS 'gebmodu 資料修改者';
COMMENT ON COLUMN country.updated_on IS 'gebdate 最近修改日';
COMMENT ON COLUMN country.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

-- ----- 地區 -----
CREATE TABLE area (
    area_code    varchar(10) PRIMARY KEY CHECK (char_length(area_code) >= 1),
    name         varchar(80),
    -- 被地區引用的國別不可刪除（RC4）
    country_code varchar(10) REFERENCES country (country_code) ON DELETE RESTRICT,
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
    updated_at   timestamptz
);
CREATE INDEX area_country ON area (country_code);
COMMENT ON TABLE area IS 'TipTop: geo_file 地區資料檔（aooi110 地區資料維護）；共用 schema（RC2）';
COMMENT ON COLUMN area.area_code IS 'geo01 地區編號（區分大小寫、建立後不可修改，RC5）';
COMMENT ON COLUMN area.name IS 'geo02 地區名稱';
COMMENT ON COLUMN area.country_code IS 'geo03 隸屬國別（可空白；須存在且有效，aoo-061、9028，RC3）';
COMMENT ON COLUMN area.active IS 'geoacti 資料有效碼';
COMMENT ON COLUMN area.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9；geo_file 無對應欄位）';
COMMENT ON COLUMN area.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN area.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN area.created_by IS 'geouser 資料所有者';
COMMENT ON COLUMN area.created_dept IS 'geogrup 資料所有部門';
COMMENT ON COLUMN area.updated_by IS 'geomodu 資料修改者';
COMMENT ON COLUMN area.updated_on IS 'geodate 最近修改日';
COMMENT ON COLUMN area.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

-- ----- 交運方式 -----
CREATE TABLE shipping_method (
    method_code  varchar(10) PRIMARY KEY CHECK (char_length(method_code) >= 1),
    name         varchar(80),
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
    updated_at   timestamptz
);
COMMENT ON TABLE shipping_method IS 'TipTop: ged_file 交運方式維護檔（aooi160 交運方式維護）；共用 schema（SM2）';
COMMENT ON COLUMN shipping_method.method_code IS 'ged01 交運方式（區分大小寫、建立後不可修改，SM5）';
COMMENT ON COLUMN shipping_method.name IS 'ged02 交運方式說明';
COMMENT ON COLUMN shipping_method.active IS '資料有效碼（TTRS 新增，ged_file 無此欄；轉入資料一律有效，SM3）';
COMMENT ON COLUMN shipping_method.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9）';
COMMENT ON COLUMN shipping_method.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN shipping_method.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN shipping_method.created_by IS '資料所有者（TTRS 新增，ADR 0012）';
COMMENT ON COLUMN shipping_method.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

-- ----- 報表抬頭 -----
CREATE TABLE report_title (
    title_code   varchar(10) PRIMARY KEY CHECK (char_length(title_code) >= 1),
    name         varchar(80),
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
    updated_at   timestamptz
);
COMMENT ON TABLE report_title IS 'TipTop: azh_file 報表抬頭檔（aooi210 報表抬頭維護；agl 財務報表列印時選用，RH2）；共用 schema（RH3）';
COMMENT ON COLUMN report_title.title_code IS 'azh01 抬頭代碼（區分大小寫、建立後不可修改）';
COMMENT ON COLUMN report_title.name IS 'azh02 抬頭名稱（繁中，翻譯來源；其他語系在 report_title_text，RH4）';
COMMENT ON COLUMN report_title.active IS '資料有效碼（TTRS 新增，azh_file 無此欄；轉入資料一律有效，RH5）';
COMMENT ON COLUMN report_title.confirmed IS 'TTRS 標準確認欄位：已確認否（ADR 0013 SA9）';
COMMENT ON COLUMN report_title.confirmed_by IS 'TTRS 標準確認欄位：確認人（使用者代號 zx01）';
COMMENT ON COLUMN report_title.confirmed_at IS 'TTRS 標準確認欄位：確認時間';
COMMENT ON COLUMN report_title.created_by IS '資料所有者（TTRS 新增，ADR 0012）';
COMMENT ON COLUMN report_title.version IS '版本號（樂觀鎖，ADR 0012 §4）；TTRS 新增';

CREATE TABLE report_title_text (
    title_code varchar(10) NOT NULL REFERENCES report_title (title_code) ON DELETE CASCADE,
    lang       varchar(5) NOT NULL CHECK (lang IN ('en', 'zh-CN')),
    name       varchar(80) NOT NULL,
    PRIMARY KEY (title_code, lang)
);
COMMENT ON TABLE report_title_text IS '報表抬頭名稱的其他語系（ADR 0005；azh_file 無語言別欄位，RH4）';
COMMENT ON COLUMN report_title_text.lang IS '語系（en、zh-CN；zh-TW 存在 report_title.name）';
COMMENT ON COLUMN report_title_text.name IS '抬頭名稱';

-- 版本號與異動記錄（ADR 0012）
CREATE TRIGGER region_version BEFORE UPDATE ON region FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER region_change_log AFTER INSERT OR UPDATE OR DELETE ON region
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('region_code');
CREATE TRIGGER country_version BEFORE UPDATE ON country FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER country_change_log AFTER INSERT OR UPDATE OR DELETE ON country
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('country_code');
CREATE TRIGGER area_version BEFORE UPDATE ON area FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER area_change_log AFTER INSERT OR UPDATE OR DELETE ON area
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('area_code');
CREATE TRIGGER shipping_method_version BEFORE UPDATE ON shipping_method FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER shipping_method_change_log AFTER INSERT OR UPDATE OR DELETE ON shipping_method
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('method_code');
CREATE TRIGGER report_title_version BEFORE UPDATE ON report_title FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER report_title_change_log AFTER INSERT OR UPDATE OR DELETE ON report_title
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('title_code');
CREATE TRIGGER report_title_text_change_log AFTER INSERT OR UPDATE OR DELETE ON report_title_text
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('title_code,lang');

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT v.program_no, 'AOO', 'I', v.tiptop, '1',
       'query, detail, history, reproduce, confirm, notconfirm, output, exporttoexcel', 'tiptop', ''
  FROM (VALUES ('region-maintenance', 'aooi100'), ('country-maintenance', 'aooi090'),
               ('area-maintenance', 'aooi110'), ('shipping-method-maintenance', 'aooi160'),
               ('report-title-maintenance', 'aooi210')) AS v(program_no, tiptop)
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', p.program_no,
       ARRAY['query', 'detail', 'history', 'reproduce', 'confirm', 'notconfirm', 'output', 'exporttoexcel'],
       'tiptop', ''
  FROM program p
 WHERE p.program_no IN ('region-maintenance', 'country-maintenance', 'area-maintenance',
                        'shipping-method-maintenance', 'report-title-maintenance')
   AND EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('region-maintenance', 'zh-TW', '區域別維護'), ('region-maintenance', 'en', 'Regions'),
               ('region-maintenance', 'zh-CN', '区域别维护'),
               ('country-maintenance', 'zh-TW', '國別維護'), ('country-maintenance', 'en', 'Countries'),
               ('country-maintenance', 'zh-CN', '国别维护'),
               ('area-maintenance', 'zh-TW', '地區維護'), ('area-maintenance', 'en', 'Areas'),
               ('area-maintenance', 'zh-CN', '地区维护'),
               ('shipping-method-maintenance', 'zh-TW', '交運方式維護'),
               ('shipping-method-maintenance', 'en', 'Shipping Methods'),
               ('shipping-method-maintenance', 'zh-CN', '交运方式维护'),
               ('report-title-maintenance', 'zh-TW', '報表抬頭維護'),
               ('report-title-maintenance', 'en', 'Report Titles'),
               ('report-title-maintenance', 'zh-CN', '报表抬头维护')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', (SELECT COALESCE(MAX(seq), 0) FROM menu_item WHERE menu_no = 'menu-aoo') + v.n * 10, v.program_no
  FROM (VALUES (1, 'region-maintenance'), (2, 'country-maintenance'), (3, 'area-maintenance'),
               (4, 'shipping-method-maintenance'), (5, 'report-title-maintenance')) AS v(n, program_no)
 WHERE EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = v.program_no)
ON CONFLICT (menu_no, program_no) DO NOTHING;
