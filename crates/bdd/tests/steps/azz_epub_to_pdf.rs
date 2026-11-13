//! EPUB 轉 PDF、簡體轉繁體 — specs/features/azz/epub_to_pdf.feature（EP；TTRS 新增的工具作業）

use std::sync::Arc;

use cucumber::{given, then, when};
use printpdf::{Op, PdfDocument, PdfParseOptions, TextItem};
use ttrs_app::{Content, ConvertOptions, Format, epub_pdf::NoFonts};

use crate::{ErpWorld, steps::azz_epub_to_text::build};

fn options(format: &str, s2t: &str) -> ConvertOptions {
    ConvertOptions {
        format: Format::parse(format).expect("格式"),
        s2t: !s2t.is_empty(),
    }
}

fn book_bytes(w: &ErpWorld) -> Vec<u8> {
    match (&w.epub_bytes, &w.epub_book) {
        (Some(b), _) => b.clone(),
        (None, Some((t, a, ch, sp))) => build(t, a, ch, sp, false),
        _ => panic!("尚未建立 EPUB"),
    }
}

async fn run(w: &mut ErpWorld, name: &str, bytes: Vec<u8>, opts: ConvertOptions) {
    let r = w.epub.convert_with(&w.actor, name, &bytes, opts).await;
    set_result(w, r).await;
}

async fn set_result(w: &mut ErpWorld, r: Result<ttrs_app::EpubOutput, ttrs_app::AppError>) {
    w.epub_output = r.as_ref().ok().cloned();
    w.epub_result = match &w.epub_output {
        Some(o) => match &o.content {
            Content::Text(t) => Some(ttrs_shared::EpubTextDto {
                file_name: o.file_name.clone(),
                title: o.title.clone(),
                author: o.author.clone(),
                chapters: o.chapters,
                text: t.clone(),
            }),
            Content::Pdf(_) => None,
        },
        None => None,
    };
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者將這本 EPUB 轉成 "(txt|pdf)"(，並簡體轉繁體)?$"#)]
async fn convert_book(w: &mut ErpWorld, format: String, s2t: String) {
    let bytes = book_bytes(w);
    run(w, "book.epub", bytes, options(&format, &s2t)).await;
}

#[when(regex = r#"^使用者將這個檔案轉成 "(txt|pdf)"$"#)]
async fn convert_file(w: &mut ErpWorld, format: String) {
    let bytes = w.epub_file.clone().expect("尚未建立檔案");
    run(w, "x.epub", bytes, options(&format, "")).await;
}

#[when(regex = r#"^使用者將 (\d+) MB 的檔案轉成 "(txt|pdf)"$"#)]
async fn convert_big(w: &mut ErpWorld, mb: usize, format: String) {
    run(
        w,
        "big.epub",
        vec![0u8; mb * 1024 * 1024],
        options(&format, ""),
    )
    .await;
}

#[given(expr = "有一個 {string} 檔案")]
async fn file_kind(w: &mut ErpWorld, kind: String) {
    w.epub_file = Some(match kind.as_str() {
        "加密的 EPUB" => {
            let ch = vec![("c1.xhtml".to_owned(), "<p>x</p>".to_owned())];
            build("書", "某", &ch, &["c1.xhtml".to_owned()], true)
        }
        "不是 EPUB" => b"hello".to_vec(),
        other => panic!("未知的檔案：{other}"),
    });
}

#[given(regex = r#"^有一本 EPUB，書名 "(\S+)"，作者 "(\S+)"，其中一章有 (\d+) 個段落$"#)]
async fn long_book(w: &mut ErpWorld, title: String, author: String, n: usize) {
    let body: String = (1..=n).map(|i| format!("<p>段落{i}。</p>")).collect();
    let ch = vec![("c1.xhtml".to_owned(), format!("<h1>第一章</h1>{body}"))];
    let spine = vec!["c1.xhtml".to_owned()];
    w.epub_book = Some((title, author, ch, spine));
    w.epub_bytes = None;
}

#[given("伺服器沒有可用的中文字型")]
async fn no_fonts(w: &mut ErpWorld) {
    w.epub = w.epub.clone().with_fonts(Arc::new(NoFonts));
}

#[then(expr = "轉成 {string} 仍應成功")]
async fn still_ok(w: &mut ErpWorld, format: String) {
    let bytes = book_bytes(w);
    let r = w
        .epub
        .convert_with(&w.actor, "book.epub", &bytes, options(&format, ""))
        .await;
    assert!(r.is_ok(), "應成功：{:?}", r.err().map(|e| e.to_string()));
}

fn output(w: &ErpWorld) -> ttrs_app::EpubOutput {
    if let Some(Err(e)) = &w.last {
        panic!("轉換失敗：{e}");
    }
    w.epub_output.clone().expect("尚未轉換")
}

#[then(expr = "書名應為 {string}，作者 {string}")]
async fn title_author(w: &mut ErpWorld, title: String, author: String) {
    let o = output(w);
    assert_eq!(
        (o.title.as_deref(), o.author.as_deref()),
        (Some(title.as_str()), Some(author.as_str()))
    );
}

#[then(expr = "轉出的檔名應為 {string}")]
async fn file_name(w: &mut ErpWorld, name: String) {
    assert_eq!(output(w).file_name, name);
}

/// 解析 PDF：每頁的文字（依序串接）與書籤
pub(crate) struct Pdf {
    pub(crate) pages: Vec<String>,
    pub(crate) bookmarks: Vec<String>,
}

fn parse_pdf(w: &ErpWorld) -> Pdf {
    let Content::Pdf(bytes) = output(w).content else {
        panic!("轉出的不是 PDF");
    };
    parse_pdf_bytes(&bytes)
}

/// 解析 PDF 位元組：每頁文字與書籤（翻譯結果也用）
pub(crate) fn parse_pdf_bytes(bytes: &[u8]) -> Pdf {
    assert!(bytes.starts_with(b"%PDF-"), "不是 PDF 檔頭");
    let mut warnings = Vec::new();
    let doc = PdfDocument::parse(bytes, &PdfParseOptions::default(), &mut warnings)
        .expect("PDF 無法解析");
    let pages = doc
        .pages
        .iter()
        .map(|p| {
            let mut s = String::new();
            for op in &p.ops {
                if let Op::ShowText { items } = op {
                    for it in items {
                        match it {
                            TextItem::Text(t) => s.push_str(t),
                            TextItem::GlyphIds(gs) => {
                                for g in gs {
                                    if let Some(c) = &g.cid {
                                        s.push_str(c);
                                    }
                                }
                            }
                            TextItem::Offset(_) => {}
                        }
                    }
                }
            }
            s
        })
        .collect();
    let mut marks: Vec<_> = doc.bookmarks.map.values().collect();
    marks.sort_by_key(|b| b.page);
    Pdf {
        pages,
        bookmarks: marks.iter().map(|b| b.name.clone()).collect(),
    }
}

#[then(regex = r"^轉出的檔案應為 PDF，共 (\d+) 頁$")]
async fn pdf_pages(w: &mut ErpWorld, n: usize) {
    assert_eq!(parse_pdf(w).pages.len(), n);
}

#[then(regex = r"^PDF 頁數應大於 (\d+)$")]
async fn pdf_pages_more(w: &mut ErpWorld, n: usize) {
    let pages = parse_pdf(w).pages.len();
    assert!(pages > n, "頁數 {pages}");
}

fn quoted(list: &str) -> Vec<String> {
    list.split('、')
        .map(|s| s.trim().trim_matches('"').to_owned())
        .collect()
}

#[then(regex = r"^PDF 內容應包含 (.+)$")]
async fn pdf_has(w: &mut ErpWorld, list: String) {
    let all = parse_pdf(w).pages.join("");
    for t in quoted(&list) {
        assert!(all.contains(&t), "PDF 應包含「{t}」，實際：{all}");
    }
}

#[then(regex = r"^PDF 內容不應包含 (.+)$")]
async fn pdf_has_not(w: &mut ErpWorld, list: String) {
    let all = parse_pdf(w).pages.join("");
    for t in quoted(&list) {
        assert!(!all.contains(&t), "PDF 不應包含「{t}」");
    }
}

#[then(regex = r"^PDF 應有 (\d+) 個書籤：(.+)$")]
async fn pdf_bookmarks(w: &mut ErpWorld, n: usize, list: String) {
    let b = parse_pdf(w).bookmarks;
    assert_eq!(b.len(), n);
    assert_eq!(b, quoted(&list));
}

#[then("每一頁底部應有頁碼")]
async fn page_numbers(w: &mut ErpWorld) {
    let pages = parse_pdf(w).pages;
    for (i, text) in pages.iter().enumerate() {
        assert!(
            text.ends_with(&(i + 1).to_string()),
            "第 {} 頁底部應有頁碼，實際結尾：{}",
            i + 1,
            text.chars().rev().take(6).collect::<String>()
        );
    }
}

#[then(
    regex = r#"^稽核紀錄應有檔案 "(\S+)" 的 "(\S+)" "(\S+)" 一筆，內容含 "([^"]+)" 與 "([^"]+)"$"#
)]
async fn audit(
    w: &mut ErpWorld,
    file: String,
    program: String,
    action: String,
    d1: String,
    d2: String,
) {
    let entries = w.programs.audit_entries(&w.actor, &file).await.unwrap();
    assert!(
        entries.iter().any(|e| e.program == program
            && e.action == action
            && e.detail
                .as_deref()
                .is_some_and(|d| d.contains(&d1) && d.contains(&d2))),
        "應有稽核紀錄：{entries:?}"
    );
}

#[when(regex = r#"^使用者轉換 GitHub 檔案 "(\S+)" "(\S+)"，輸出 "(txt|pdf)"(，並簡體轉繁體)?$"#)]
async fn github_convert(w: &mut ErpWorld, repo: String, path: String, format: String, s2t: String) {
    let r = w
        .github
        .convert_with(&w.actor, &repo, &path, options(&format, &s2t))
        .await;
    set_result(w, r).await;
}
