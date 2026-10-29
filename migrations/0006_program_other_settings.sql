-- p_zz.4fd 其餘欄位：其他設定、設計日期、最後修改日期、報表抬頭
ALTER TABLE program
    ADD COLUMN default_actions  TEXT,
    ADD COLUMN print_option     BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN allow_key_update BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN remark           TEXT,
    ADD COLUMN designed_on      DATE NOT NULL DEFAULT CURRENT_DATE,
    ADD COLUMN updated_on       DATE NOT NULL DEFAULT CURRENT_DATE;
COMMENT ON COLUMN program.default_actions IS 'TipTop: zz04 可執行功能（格式待確認）';
COMMENT ON COLUMN program.print_option IS 'TipTop: zz05 列印選項';
COMMENT ON COLUMN program.allow_key_update IS 'TipTop: zz13 維護作業可修改鍵值';
COMMENT ON COLUMN program.remark IS 'TipTop: zz31 其他備註';
COMMENT ON COLUMN program.designed_on IS 'TipTop: zz09 設計日期';
COMMENT ON COLUMN program.updated_on IS 'TipTop: zzdate 最後建立/修改日期';

ALTER TABLE program_name ADD COLUMN report_title TEXT;
COMMENT ON COLUMN program_name.report_title IS 'TipTop: gaz06 報表抬頭';
