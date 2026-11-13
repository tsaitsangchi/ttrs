-- EPUB 轉換（epub_to_pdf.feature EP1）：epub-to-text 加 PDF 輸出與簡體轉繁體，程式名稱由「EPUB 轉 TXT」改為「EPUB 轉換」
UPDATE program_name SET name = 'EPUB 轉換'      WHERE program_no = 'epub-to-text' AND lang = 'zh-TW';
UPDATE program_name SET name = 'EPUB Converter' WHERE program_no = 'epub-to-text' AND lang = 'en';
UPDATE program_name SET name = 'EPUB 转换'      WHERE program_no = 'epub-to-text' AND lang = 'zh-CN';
