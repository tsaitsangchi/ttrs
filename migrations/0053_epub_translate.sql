-- EPUB 英文翻譯成繁體中文（TTRS 新增的工具作業，TipTop 無對應；specs/features/azz/epub_translate.feature EL1–EL13）
-- translation_job：翻譯工作。原文、切段、已翻譯的批次與結果只保留 24 小時（expires_at），之後清除（EL4）；
-- 暫存性資料不是維護作業的主檔，不掛版本與異動記錄觸發器（ADR 0012 不適用）。
CREATE TABLE translation_job (
    job_id           TEXT PRIMARY KEY,
    user_id          TEXT        NOT NULL,
    source_sha256    TEXT        NOT NULL,
    file_name        TEXT        NOT NULL,
    format           TEXT        NOT NULL CHECK (format IN ('txt', 'pdf')),
    status           TEXT        NOT NULL CHECK (status IN ('queued', 'running', 'done', 'failed')),
    total_batches    INTEGER     NOT NULL,
    done_batches     INTEGER     NOT NULL,
    total_lines      INTEGER     NOT NULL,
    done_lines       INTEGER     NOT NULL,
    error_code       TEXT,
    error_message    TEXT,
    doc              TEXT        NOT NULL,
    result           BYTEA,
    result_file_name TEXT,
    result_title     TEXT,
    result_author    TEXT,
    result_chapters  INTEGER     NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ NOT NULL,
    updated_at       TIMESTAMPTZ NOT NULL,
    expires_at       TIMESTAMPTZ NOT NULL
);
CREATE INDEX translation_job_resume_idx ON translation_job (user_id, source_sha256);
CREATE INDEX translation_job_expires_idx ON translation_job (expires_at);
COMMENT ON TABLE translation_job IS 'TTRS 新增（TipTop 無對應）：EPUB 英文翻譯工作（epub-to-text 的 translate 功能）；doc 為原文、切段與已翻譯批次的 JSON，24 小時後清除';
COMMENT ON COLUMN translation_job.doc IS '原文、切段與已翻譯的批次（JSON），續翻用';
COMMENT ON COLUMN translation_job.source_sha256 IS '上傳檔案的 SHA-256；同一人同一檔案失敗的工作可續翻（EL13）';

-- 作業 epub-to-text 新增功能 translate（EL10）：ADMIN 預設有
UPDATE program SET default_actions = 'run, translate' WHERE program_no = 'epub-to-text';
UPDATE program_permission SET actions = array_append(actions, 'translate')
 WHERE program_no = 'epub-to-text' AND NOT ('translate' = ANY (actions));
