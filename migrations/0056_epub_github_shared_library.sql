-- 共用 GitHub 書庫（TTRS 新增的工具作業，TipTop 無對應；specs/features/azz/epub_github_shared_library.feature SL1–SL8）
-- 全公司一份納入 EPUB 搜尋的 GitHub repo 清單（最多 20 個，由程式檢查）；共用 schema，不分營運中心（SL6）。

CREATE TABLE epub_github_shared_library (
    repo          text PRIMARY KEY CHECK (repo ~ '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$'),
    created_by    text NOT NULL,
    created_dept  text NOT NULL DEFAULT '',
    version       integer NOT NULL DEFAULT 1,
    created_at    timestamptz NOT NULL DEFAULT now(),
    updated_at    timestamptz
);
COMMENT ON TABLE epub_github_shared_library IS 'TTRS 新增（TipTop 無對應）：共用 GitHub 書庫（github-epub-search 的 share 功能）；全公司納入 EPUB 搜尋的 GitHub repo';
COMMENT ON COLUMN epub_github_shared_library.repo IS 'GitHub repo「擁有者/repo」（GitHub 回傳的正式名稱）';
COMMENT ON COLUMN epub_github_shared_library.created_by IS '加入者的使用者代號（zx01）';
COMMENT ON COLUMN epub_github_shared_library.created_dept IS '加入者的部門';
COMMENT ON COLUMN epub_github_shared_library.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TRIGGER epub_github_shared_library_version BEFORE UPDATE ON epub_github_shared_library
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER epub_github_shared_library_change_log AFTER INSERT OR UPDATE OR DELETE ON epub_github_shared_library
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('repo');

-- 作業 github-epub-search 新增功能 share（管理共用書庫，SL2）：ADMIN 預設有
UPDATE program SET default_actions = 'insert, query, delete, run, share' WHERE program_no = 'github-epub-search';
UPDATE program_permission SET actions = array_append(actions, 'share')
 WHERE program_no = 'github-epub-search' AND class_code = 'ADMIN' AND NOT ('share' = ANY (actions));
