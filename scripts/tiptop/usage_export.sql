-- TipTop usage statistics export (TTRS migration Phase 0: scope and porting order)
-- Usage: the DBA runs this read-only in TipTop (TOPPROD, or a recent copy of it); every query is a SELECT and changes nothing.
-- Export each result as UTF-8 CSV with a header row, named as shown in each section.
-- Everything is aggregated: no user IDs, names or business data are exported.
-- Table and column definitions: tiptop/azz/sch/{zu,zz,zx,zy,zxw,gaz}_file.sch, tiptop/aoo/sch/{azo,azp}_file.sch
--
-- Where to run:
--   Q1, Q2, Q3, Q5, Q7: the system schema (default ds). If zu_file is empty in ds, also try each plant's schema.
--   Q4: each plant's schema (azp03 from Q1), once per plant; put the schema name in the file name.
--   Q6: once, as an account that can read all_tables, listing the schemas of every plant.

-- ---------------------------------------------------------------------------
-- Q1 Plant list → q1_plants.csv
SELECT azp01 AS plant, azp02 AS plant_name, azp03 AS db_schema, azp053 AS is_erp_db
FROM azp_file
ORDER BY azp01;

-- ---------------------------------------------------------------------------
-- Q2 Program registry (every registered program, Traditional Chinese name, customization code) → q2_programs.csv
SELECT zz01 AS program, zz011 AS module, zz03 AS program_type, gaz03 AS name_zh_tw, gaz05 AS custom_code
FROM zz_file
LEFT JOIN gaz_file ON gaz01 = zz01 AND gaz02 = '0'
ORDER BY zz01;

-- ---------------------------------------------------------------------------
-- Q3 Program runs over the last 12 months (zu_file run log) → q3_program_runs.csv
--     active_months: in how many months the program ran (detects month-end and year-end programs)
SELECT zu01                             AS program,
       COUNT(*)                         AS runs,
       COUNT(DISTINCT zu04)             AS users,
       COUNT(DISTINCT TO_CHAR(zu02, 'YYYYMM')) AS active_months,
       ROUND(SUM(zu05), 1)              AS total_minutes,
       MIN(zu02)                        AS first_run,
       MAX(zu02)                        AS last_run
FROM zu_file
WHERE zu02 >= ADD_MONTHS(TRUNC(SYSDATE), -12)
GROUP BY zu01
ORDER BY runs DESC;

-- Check that zu_file is actually being written (if the latest date is old or the count is 0, rely on Q4 and Q5) → q3_check.csv
SELECT COUNT(*) AS total_rows, MIN(zu02) AS earliest, MAX(zu02) AS latest FROM zu_file;

-- ---------------------------------------------------------------------------
-- Q4 Data changes by program over the last 12 months (azo_file change log; run once per plant schema) → q4_changes_<schema>.csv
SELECT azo01                AS program,
       COUNT(*)             AS changes,
       COUNT(DISTINCT azo02) AS users,
       MAX(azo03)           AS last_change
FROM azo_file
WHERE azo03 >= ADD_MONTHS(TRUNC(SYSDATE), -12)
GROUP BY azo01
ORDER BY changes DESC;

-- ---------------------------------------------------------------------------
-- Q5 Programs granted to active users (permission class zy_file + user permissions zxw_file) → q5_granted.csv
--     Active user: zxacti = 'Y' and the expiry date zx18 is blank or not yet reached
WITH active_users AS (
    SELECT zx01, zx04 FROM zx_file
    WHERE zxacti = 'Y' AND (zx18 IS NULL OR zx18 >= TRUNC(SYSDATE))
),
user_classes AS (
    SELECT zx01, zx04 AS class_code FROM active_users
    UNION
    SELECT w.zxw01, w.zxw04 FROM zxw_file w
    JOIN active_users u ON u.zx01 = w.zxw01
    WHERE w.zxw03 = '1'                      -- 1: grant by permission class
),
grants AS (
    SELECT c.zx01, y.zy02 AS program FROM user_classes c
    JOIN zy_file y ON y.zy01 = c.class_code
    UNION
    SELECT w.zxw01, w.zxw04 FROM zxw_file w
    JOIN active_users u ON u.zx01 = w.zxw01
    WHERE w.zxw03 = '2'                      -- 2: grant by program
)
SELECT program, COUNT(DISTINCT zx01) AS active_users
FROM grants
GROUP BY program
ORDER BY active_users DESC;

-- ---------------------------------------------------------------------------
-- Q6 Data volume per table (estimated from statistics; for migration sizing) → q6_table_rows.csv
--     Replace the IN list below with the db_schema values of every plant from Q1 (uppercase)
SELECT owner, table_name, num_rows, last_analyzed
FROM all_tables
WHERE owner IN ('DS' /* , 'DS1', 'DS2' ... */)
ORDER BY owner, num_rows DESC NULLS LAST;

-- ---------------------------------------------------------------------------
-- Q7 Active user count (by department, counts only) → q7_active_users.csv
SELECT zx03 AS dept, COUNT(*) AS active_users
FROM zx_file
WHERE zxacti = 'Y' AND (zx18 IS NULL OR zx18 >= TRUNC(SYSDATE))
GROUP BY zx03
ORDER BY active_users DESC;
