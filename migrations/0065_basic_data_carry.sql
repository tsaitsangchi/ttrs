-- 基本資料拋轉（TipTop aoop603）與拋轉歷史查詢（aooq604）/ gex_file 資料中心拋轉記錄檔
-- specs/features/aoo/basic_data_carry.feature BC1–BC12。拋轉紀錄放共用 schema（集團層）。

CREATE TABLE carry_log (
    log_id       bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    center_code  text NOT NULL,
    data_type    char(1) NOT NULL CHECK (data_type BETWEEN '0' AND '8'),
    plant_id     text NOT NULL,
    program_no   varchar(20) NOT NULL,
    record_key   varchar(80) NOT NULL,
    action       varchar(10) NOT NULL CHECK (action IN ('insert', 'update')),
    carried_at   timestamptz NOT NULL DEFAULT now(),
    carried_by   text NOT NULL
);
CREATE INDEX carry_log_center ON carry_log (center_code, data_type);
CREATE INDEX carry_log_at ON carry_log (carried_at DESC);
COMMENT ON TABLE carry_log IS 'TipTop: gex_file 資料中心拋轉記錄檔（aoop603 等拋轉寫入、aooq604 查詢）；每拋一筆一列（BC7）；共用 schema';
COMMENT ON COLUMN carry_log.center_code IS 'gex01 資料中心代碼（有紀錄時集團中心、拋轉設定不可刪除，BC11）';
COMMENT ON COLUMN carry_log.data_type IS 'gex02 資料類別';
COMMENT ON COLUMN carry_log.plant_id IS 'gex03 拋轉營運中心';
COMMENT ON COLUMN carry_log.program_no IS 'gex04 程式代號（TipTop 作業代號，如 aooi030）';
COMMENT ON COLUMN carry_log.record_key IS 'gex05 拋轉主鍵值';
COMMENT ON COLUMN carry_log.action IS 'TTRS 新增：insert 新增、update 覆蓋（略過的不記錄）';
COMMENT ON COLUMN carry_log.carried_at IS 'gex07 拋轉日期＋gex08 拋轉時間';
COMMENT ON COLUMN carry_log.carried_by IS 'gex09 拋轉人員代號';

-- 作業登錄與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
SELECT v.program_no, 'AOO', v.program_type, v.tiptop, '1', v.actions, 'tiptop', ''
  FROM (VALUES ('basic-data-carry', 'P', 'aoop603', 'query, carry'),
               ('carry-history-query', 'Q', 'aooq604', 'query, output, exporttoexcel')) AS v(program_no, program_type, tiptop, actions)
 WHERE EXISTS (SELECT 1 FROM module WHERE module_code = 'AOO')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', v.program_no, v.actions, 'tiptop', ''
  FROM (VALUES ('basic-data-carry', ARRAY['query', 'carry']),
               ('carry-history-query', ARRAY['query', 'output', 'exporttoexcel'])) AS v(program_no, actions)
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
   AND EXISTS (SELECT 1 FROM program p WHERE p.program_no = v.program_no)
ON CONFLICT (class_code, program_no) DO NOTHING;
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('basic-data-carry', 'zh-TW', '基本資料拋轉'), ('basic-data-carry', 'en', 'Basic Data Carry'),
               ('basic-data-carry', 'zh-CN', '基本资料抛转'),
               ('carry-history-query', 'zh-TW', '拋轉歷史查詢'), ('carry-history-query', 'en', 'Carry History'),
               ('carry-history-query', 'zh-CN', '抛转历史查询')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;
-- 側欄：整體管理之下
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'basic-data-carry' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'basic-data-carry')
ON CONFLICT (menu_no, program_no) DO NOTHING;
INSERT INTO menu_item (menu_no, seq, program_no)
SELECT 'menu-aoo', COALESCE(MAX(seq), 0) + 10, 'carry-history-query' FROM menu_item WHERE menu_no = 'menu-aoo'
HAVING EXISTS (SELECT 1 FROM program WHERE program_no = 'menu-aoo')
   AND EXISTS (SELECT 1 FROM program WHERE program_no = 'carry-history-query')
ON CONFLICT (menu_no, program_no) DO NOTHING;
