-- 我的 GitHub 書庫（TTRS 新增的工具作業，TipTop 無對應；specs/features/azz/epub_github_library.feature ML1–ML8）
-- 每位使用者自己指定要納入 EPUB 搜尋的 GitHub repo（最多 20 個，由程式檢查）；共用 schema，不分營運中心（ML6）。

CREATE TABLE epub_github_library (
    user_id     text NOT NULL,
    repo        text NOT NULL CHECK (repo ~ '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$'),
    version     integer NOT NULL DEFAULT 1,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz,
    PRIMARY KEY (user_id, repo)
);
COMMENT ON TABLE epub_github_library IS 'TTRS 新增（TipTop 無對應）：我的 GitHub 書庫（github-epub-search）；每位使用者納入 EPUB 搜尋的 GitHub repo';
COMMENT ON COLUMN epub_github_library.user_id IS '使用者代號（zx01）';
COMMENT ON COLUMN epub_github_library.repo IS 'GitHub repo「擁有者/repo」（GitHub 回傳的正式名稱）';
COMMENT ON COLUMN epub_github_library.version IS '版本號（樂觀鎖，ADR 0012 §4）';

CREATE TRIGGER epub_github_library_version BEFORE UPDATE ON epub_github_library
    FOR EACH ROW EXECUTE FUNCTION ttrs_bump_version();
CREATE TRIGGER epub_github_library_change_log AFTER INSERT OR UPDATE OR DELETE ON epub_github_library
    FOR EACH ROW EXECUTE FUNCTION ttrs_change_log('user_id,repo');

-- 作業 github-epub-search 新增功能 insert（加入）、delete（移除）（ML5）：ADMIN 預設有
UPDATE program SET default_actions = 'insert, query, delete, run' WHERE program_no = 'github-epub-search';
UPDATE program_permission SET actions = array_append(actions, 'insert')
 WHERE program_no = 'github-epub-search' AND class_code = 'ADMIN' AND NOT ('insert' = ANY (actions));
UPDATE program_permission SET actions = array_append(actions, 'delete')
 WHERE program_no = 'github-epub-search' AND class_code = 'ADMIN' AND NOT ('delete' = ANY (actions));
