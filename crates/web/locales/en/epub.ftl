# EPUB 轉換（epub_to_text.feature、epub_to_pdf.feature）
epub-breadcrumb = System Admin / EPUB Converter
epub-title = EPUB Converter
epub-hint = Choose one or more EPUB e-books; each is converted as selected below to TXT (UTF-8, Windows line endings) or PDF (A5 portrait with bookmarks and page numbers) and downloaded under the same name. Files are not kept on the server; DRM-protected books cannot be converted; each file may be up to 100 MB.
epub-action-choose = Choose EPUB Files
epub-msg-running = Converting…
epub-msg-done = { $file }: downloaded ({ $title } / { $author }, { $chapters } chapters)
epub-msg-failed = { $file }: conversion failed, { $error }
epub-field-format = Output format
epub-format-txt = TXT text file
epub-format-pdf = PDF e-book
epub-field-s2t = Simplified to Traditional Chinese (title, author and text)
epub-field-lang = Language
epub-lang-none = No translation (original)
epub-lang-translate = Translate English to Traditional Chinese
epub-field-consent = I understand the book text is sent to Claude for translation and I have the right to use this book
epub-hint-translate = Translation uses the subscription usage of the server's Claude account: a book can take several minutes to tens of minutes, up to 300,000 characters per book, one book at a time. If the usage limit is reached the job stops; translated parts are kept for 24 hours and uploading the same book again resumes. English books only.
epub-msg-translating = { $file }: translating, { $done } of { $total } paragraphs done
