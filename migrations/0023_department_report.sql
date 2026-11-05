-- 部門維護的列印與匯出 Excel（specs/features/aoo/department_report.feature DR5、DR11；docs/adr/0011）

-- 稽核紀錄補充說明：報表的條件、格式、筆數（ADR 0011 §4）
ALTER TABLE audit_log ADD COLUMN detail text;
COMMENT ON COLUMN audit_log.detail IS 'TTRS 新增：補充說明，例如報表的條件、格式、筆數（ADR 0011）';

-- 部門維護新增功能 output（列印）、exporttoexcel（匯出 Excel），沿用 TipTop aooi030 功能代號（DR5）；
-- 須與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
UPDATE program
   SET default_actions = 'query, detail, account, output, exporttoexcel'
 WHERE program_no = 'department-maintenance';

UPDATE program_permission
   SET actions = actions || ARRAY['output', 'exporttoexcel']
 WHERE class_code = 'ADMIN'
   AND program_no = 'department-maintenance'
   AND NOT ('output' = ANY (actions));
