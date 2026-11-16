-- EPUB 多來源搜尋（epub_web_search.feature WS1）：github-epub-search 的畫面名稱由「GitHub EPUB 搜尋」改為「EPUB 搜尋」
UPDATE program_name SET name = 'EPUB 搜尋'   WHERE program_no = 'github-epub-search' AND lang = 'zh-TW';
UPDATE program_name SET name = 'EPUB Search' WHERE program_no = 'github-epub-search' AND lang = 'en';
UPDATE program_name SET name = 'EPUB 搜索'   WHERE program_no = 'github-epub-search' AND lang = 'zh-CN';
