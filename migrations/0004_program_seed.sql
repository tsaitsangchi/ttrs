-- 開發用種子資料，需與 ttrs_app::memory::InMemoryStore::seeded() 對齊
-- ADMIN 為 API 預設操作者的權限類別（TTRS_PERMISSION_CLASS）
INSERT INTO module (module_code, name) VALUES
    ('AIM', '庫存管理'), ('AGL', '總帳管理'), ('AZZ', '系統管理');
INSERT INTO permission_class (class_code, name) VALUES ('ADMIN', '系統管理員');
