//! EPUB 轉 TXT — specs/features/azz/epub_to_text.feature（ET；TTRS 新增的工具作業）

use std::io::Write;

use cucumber::{gherkin::Step, given, then, when};

use crate::{ErpWorld, steps::common::rows};

/// 組出一本 EPUB：mimetype、container.xml、OPF（書名、作者、manifest、spine）與各章
pub(crate) fn build(
    title: &str,
    author: &str,
    chapters: &[(String, String)],
    spine: &[String],
    encrypted: bool,
) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opt = zip::write::SimpleFileOptions::default();
        z.start_file("mimetype", opt).unwrap();
        z.write_all(b"application/epub+zip").unwrap();
        z.start_file("META-INF/container.xml", opt).unwrap();
        z.write_all(br#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#).unwrap();
        if encrypted {
            z.start_file("META-INF/encryption.xml", opt).unwrap();
            z.write_all(b"<encryption/>").unwrap();
        }
        let items: String = chapters
            .iter()
            .map(|(f, _)| {
                format!(r#"<item id="{f}" href="text/{f}" media-type="application/xhtml+xml"/>"#)
            })
            .collect();
        let refs: String = spine
            .iter()
            .map(|f| format!(r#"<itemref idref="{f}"/>"#))
            .collect();
        let opf = format!(
            r#"<?xml version="1.0" encoding="utf-8"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{title}</dc:title><dc:creator>{author}</dc:creator></metadata><manifest>{items}</manifest><spine>{refs}</spine></package>"#
        );
        z.start_file("OEBPS/content.opf", opt).unwrap();
        z.write_all(opf.as_bytes()).unwrap();
        for (f, body) in chapters {
            z.start_file(format!("OEBPS/text/{f}"), opt).unwrap();
            let html = format!(
                r#"<?xml version="1.0" encoding="utf-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>{f}</title></head><body>{body}</body></html>"#
            );
            z.write_all(html.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

#[given(regex = r#"^有一本 EPUB，書名 "(\S+)"，作者 "(\S+)"，閱讀順序為:$"#)]
async fn has_epub(w: &mut ErpWorld, title: String, author: String, step: &Step) {
    let chapters: Vec<(String, String)> = rows(step)
        .into_iter()
        .map(|r| (r[0].clone(), r[1].clone()))
        .collect();
    let spine: Vec<String> = chapters.iter().map(|(f, _)| f.clone()).collect();
    w.epub_book = Some((title, author, chapters, spine));
    w.epub_bytes = None;
}

#[given(expr = "閱讀順序為 {string}")]
async fn spine(w: &mut ErpWorld, order: String) {
    let book = w.epub_book.as_mut().expect("尚未建立 EPUB");
    book.3 = order.split(',').map(|s| s.trim().to_owned()).collect();
}

#[given("有一本有加密的 EPUB")]
async fn encrypted(w: &mut ErpWorld) {
    let ch = vec![("c1.xhtml".to_owned(), "<p>x</p>".to_owned())];
    w.epub_bytes = Some(build("書", "某", &ch, &["c1.xhtml".to_owned()], true));
}

async fn convert(w: &mut ErpWorld, name: &str, bytes: Vec<u8>) {
    let r = w.epub.convert(&w.actor, name, &bytes).await;
    w.epub_result = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

#[when("使用者將這本 EPUB 轉成文字")]
async fn convert_book(w: &mut ErpWorld) {
    let bytes = match (&w.epub_bytes, &w.epub_book) {
        (Some(b), _) => b.clone(),
        (None, Some((t, a, ch, sp))) => build(t, a, ch, sp, false),
        _ => panic!("尚未建立 EPUB"),
    };
    convert(w, "book.epub", bytes).await;
}

#[when(expr = "使用者將內容為 {string} 的檔案轉成文字")]
async fn convert_raw(w: &mut ErpWorld, content: String) {
    convert(w, "x.epub", content.into_bytes()).await;
}

#[when(regex = r"^使用者將 (\d+) MB 的檔案轉成文字$")]
async fn convert_big(w: &mut ErpWorld, mb: usize) {
    convert(w, "big.epub", vec![0u8; mb * 1024 * 1024]).await;
}

fn result(w: &ErpWorld) -> ttrs_shared::EpubTextDto {
    if let Some(Err(e)) = &w.last {
        panic!("轉換失敗：{e}");
    }
    w.epub_result.clone().expect("尚未轉換")
}

#[then(regex = r#"^書名應為 "(\S+)"，作者 "(\S+)"，共 (\d+) 章$"#)]
async fn book_info(w: &mut ErpWorld, title: String, author: String, n: u32) {
    let r = result(w);
    assert_eq!(
        (r.title.as_deref(), r.author.as_deref(), r.chapters),
        (Some(title.as_str()), Some(author.as_str()), n)
    );
}

#[then("轉出的文字應為:")]
async fn text_is(w: &mut ErpWorld, step: &Step) {
    let r = result(w);
    let expected = step
        .docstring
        .as_ref()
        .expect("需要文字")
        .trim_matches('\n')
        .replace('\n', "\r\n");
    assert_eq!(r.text, expected);
}

#[then("下載的文字檔應為 UTF-8 不含 BOM，換行為 CRLF")]
async fn crlf(w: &mut ErpWorld) {
    let r = result(w);
    assert!(!r.text.starts_with('\u{feff}'));
    assert!(r.text.contains("\r\n"));
    assert!(!r.text.replace("\r\n", "").contains('\n'));
    assert_eq!(r.file_name, "book.txt");
}
