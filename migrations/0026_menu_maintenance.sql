-- 選單維護（TipTop p_zm / zm_file；specs/features/azz/menu_maintenance.feature MN1–MN16）與側欄樹狀選單

-- MN2：序號 zm03；主鍵改為 (目錄, 序號)，同一目錄內程式不可重複（TipTop 允許重複，TTRS 不允許）
ALTER TABLE menu_item ADD COLUMN seq integer;
UPDATE menu_item m
   SET seq = x.rn * 10
  FROM (SELECT menu_no, program_no,
               row_number() OVER (PARTITION BY menu_no ORDER BY program_no COLLATE "C") AS rn
          FROM menu_item) x
 WHERE x.menu_no = m.menu_no AND x.program_no = m.program_no;
ALTER TABLE menu_item ALTER COLUMN seq SET NOT NULL;
ALTER TABLE menu_item ADD CONSTRAINT menu_item_seq_positive CHECK (seq > 0);
ALTER TABLE menu_item DROP CONSTRAINT menu_item_pkey;
-- 可延後檢查：上移、下移、重新編號時在同一交易內交換序號
ALTER TABLE menu_item ADD PRIMARY KEY (menu_no, seq) DEFERRABLE INITIALLY IMMEDIATE;
ALTER TABLE menu_item ADD CONSTRAINT menu_item_program_once UNIQUE (menu_no, program_no);
COMMENT ON COLUMN menu_item.seq IS 'zm03 序號（同一目錄內依序號排列，cl_create_4sm.4gl:203）';

-- 異動記錄（ADR 0012）：選單改由使用者維護
CREATE TRIGGER menu_item_change_log AFTER INSERT OR UPDATE OR DELETE ON menu_item
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('menu_no,program_no');

-- 選單維護作業與起始權限（ADMIN 全部功能）；需與 ttrs_app::authorization::PORTED_PROGRAMS 對齊
INSERT INTO module (module_code, name) VALUES ('MENU', 'MENU') ON CONFLICT DO NOTHING;
INSERT INTO program (program_no, module_code, program_type, tiptop_program_no, report_ext_mode,
                     default_actions, created_by, created_dept)
VALUES ('menu-maintenance', 'AZZ', 'I', 'p_zm', '1',
        'insert, query, delete, detail, reproduce, history', 'tiptop', '')
ON CONFLICT (program_no) DO NOTHING;
INSERT INTO program_permission (class_code, program_no, actions, created_by, created_dept)
SELECT 'ADMIN', 'menu-maintenance', ARRAY['insert', 'query', 'delete', 'detail', 'reproduce', 'history'], 'tiptop', ''
 WHERE EXISTS (SELECT 1 FROM permission_class WHERE class_code = 'ADMIN')
ON CONFLICT (class_code, program_no) DO NOTHING;

-- MN14：起始目錄（與原本寫死的側欄相同）。目錄本身是模組 MENU 的程式
INSERT INTO program (program_no, module_code, program_type, report_ext_mode, default_actions, created_by, created_dept)
VALUES ('menu-main', 'MENU', NULL, '1', NULL, 'tiptop', ''),
       ('menu-aim', 'MENU', NULL, '1', NULL, 'tiptop', ''),
       ('menu-aoo', 'MENU', NULL, '1', NULL, 'tiptop', ''),
       ('menu-azz', 'MENU', NULL, '1', NULL, 'tiptop', '')
ON CONFLICT (program_no) DO NOTHING;

-- 節點名稱取程式名稱（cl_create_4sm.4gl:257-285）；只寫入已登錄的程式，已存在的名稱不變
INSERT INTO program_name (program_no, lang, name)
SELECT v.program_no, v.lang, v.name
  FROM (VALUES ('menu-main', 'zh-TW', '主選單'), ('menu-main', 'en', 'Main Menu'), ('menu-main', 'zh-CN', '主选单'),
       ('menu-aim', 'zh-TW', '庫存管理'), ('menu-aim', 'en', 'Inventory'), ('menu-aim', 'zh-CN', '库存管理'),
       ('menu-aoo', 'zh-TW', '整體管理'), ('menu-aoo', 'en', 'General Management'), ('menu-aoo', 'zh-CN', '整体管理'),
       ('menu-azz', 'zh-TW', '系統管理'), ('menu-azz', 'en', 'System Admin'), ('menu-azz', 'zh-CN', '系统管理'),
       ('menu-maintenance', 'zh-TW', '選單維護'), ('menu-maintenance', 'en', 'Menu Maintenance'),
       ('menu-maintenance', 'zh-CN', '选单维护'),
       ('item-maintenance', 'zh-TW', '料件基本資料'), ('item-maintenance', 'en', 'Item Master'),
       ('item-maintenance', 'zh-CN', '料件基本资料'),
       ('plant-maintenance', 'zh-TW', '營運中心維護'), ('plant-maintenance', 'en', 'Plants'),
       ('plant-maintenance', 'zh-CN', '营运中心维护'),
       ('department-maintenance', 'zh-TW', '部門維護'), ('department-maintenance', 'en', 'Departments'),
       ('department-maintenance', 'zh-CN', '部门维护'),
       ('employee-maintenance', 'zh-TW', '員工維護'), ('employee-maintenance', 'en', 'Employees'),
       ('employee-maintenance', 'zh-CN', '员工维护'),
       ('program-maintenance', 'zh-TW', '程式維護'), ('program-maintenance', 'en', 'Program Maintenance'),
       ('program-maintenance', 'zh-CN', '程序维护'),
       ('data-dictionary', 'zh-TW', '資料字典維護'), ('data-dictionary', 'en', 'Data Dictionary'),
       ('data-dictionary', 'zh-CN', '数据字典维护'),
       ('user-maintenance', 'zh-TW', '使用者維護'), ('user-maintenance', 'en', 'User Maintenance'),
       ('user-maintenance', 'zh-CN', '用户维护'),
       ('user-permission-maintenance', 'zh-TW', '使用者特殊權限'), ('user-permission-maintenance', 'en', 'User Special Permissions'),
       ('user-permission-maintenance', 'zh-CN', '用户特殊权限'),
       ('permission-class-maintenance', 'zh-TW', '權限類別維護'), ('permission-class-maintenance', 'en', 'Permission Classes'),
       ('permission-class-maintenance', 'zh-CN', '权限类别维护'),
       ('program-permission-maintenance', 'zh-TW', '程式執行權限'), ('program-permission-maintenance', 'en', 'Program Permissions'),
       ('program-permission-maintenance', 'zh-CN', '程序执行权限')) AS v(program_no, lang, name)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (program_no, lang) DO NOTHING;

INSERT INTO menu_item (menu_no, seq, program_no)
SELECT v.menu_no, v.seq, v.program_no
  FROM (VALUES ('menu-main', 10, 'menu-aim'), ('menu-main', 20, 'menu-aoo'), ('menu-main', 30, 'menu-azz'),
               ('menu-aim', 10, 'item-maintenance'),
               ('menu-aoo', 10, 'plant-maintenance'), ('menu-aoo', 20, 'department-maintenance'),
               ('menu-aoo', 30, 'employee-maintenance'), ('menu-aoo', 40, 'classification-code-maintenance'),
               ('menu-azz', 10, 'program-maintenance'), ('menu-azz', 20, 'data-dictionary'),
               ('menu-azz', 30, 'user-maintenance'), ('menu-azz', 40, 'user-permission-maintenance'),
               ('menu-azz', 50, 'permission-class-maintenance'), ('menu-azz', 60, 'program-permission-maintenance'),
               ('menu-azz', 70, 'menu-maintenance')) AS v(menu_no, seq, program_no)
  JOIN program p ON p.program_no = v.program_no
ON CONFLICT (menu_no, program_no) DO NOTHING;
