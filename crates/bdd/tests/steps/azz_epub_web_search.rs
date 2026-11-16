//! EPUB 多來源搜尋 — specs/features/azz/epub_web_search.feature（WS；假來源，不連外網 WS10）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::{
    ConvertOptions, Format,
    memory::FakeEpubSource,
    ports::{EpubBook, EpubBookKind},
};

use crate::{
    ErpWorld,
    steps::{azz_epub_to_pdf::set_result, azz_epub_to_text::build, common::rows},
};

fn source<'a>(w: &'a ErpWorld, name: &str) -> &'a FakeEpubSource {
    // FakeEpubSource 的名稱要用 EpubSource::name 取得
    use ttrs_app::ports::EpubSource;
    w.fake_sources
        .iter()
        .find(|s| s.name() == name)
        .unwrap_or_else(|| panic!("沒有來源 {name}"))
}

#[given("有下列來源與書:")]
async fn sources_and_books(w: &mut ErpWorld, step: &Step) {
    w.fake_sources.clear();
    for r in rows(step) {
        // 來源、書名、作者、語言、授權、下載次數、標籤
        let (src, title, author, lang, license, downloads, tag) =
            (&r[0], &r[1], &r[2], &r[3], &r[4], &r[5], &r[6]);
        if !w.fake_sources.iter().any(|s| {
            use ttrs_app::ports::EpubSource;
            s.name() == *src
        }) {
            w.fake_sources.push(FakeEpubSource::new(src));
        }
        let ch = vec![("c1.xhtml".to_owned(), "<p>內容</p>".to_owned())];
        let bytes = build(title, author, &ch, &["c1.xhtml".to_owned()], false);
        source(w, src).add_book(
            EpubBook {
                source: src.clone(),
                id: title.clone(),
                title: title.clone(),
                author: Some(author.clone()),
                language: Some(lang.clone()),
                license: Some(license.clone()),
                downloads: downloads.parse().ok(),
                kind: EpubBookKind::Book,
                url: None,
            },
            tag,
            bytes,
        );
    }
    let list = w
        .fake_sources
        .iter()
        .map(|s| std::sync::Arc::new(s.clone()) as std::sync::Arc<dyn ttrs_app::ports::EpubSource>)
        .collect();
    w.epub_search = w.epub_search.clone().with_sources(list);
}

#[given(regex = r#"^來源 "([^"]+)" 暫時無法查詢$"#)]
async fn source_down(w: &mut ErpWorld, name: String) {
    source(w, &name).set_failing(true);
}

#[given(regex = r#"^"([^"]+)" 的 "([^"]+)" 檔案大小為 (\d+) MB$"#)]
async fn source_size(w: &mut ErpWorld, name: String, title: String, mb: u64) {
    source(w, &name).set_size(&title, mb * 1024 * 1024);
}

#[when(regex = r#"^使用者搜尋 "([^"]*)"(，只看開放授權)?$"#)]
async fn search(w: &mut ErpWorld, keyword: String, open_only: String) {
    let r = w
        .epub_search
        .search(&w.actor, &keyword, !open_only.is_empty(), 1)
        .await;
    w.search_page = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

fn page(w: &ErpWorld) -> &ttrs_shared::EpubSearchPage {
    if let Some(Err(e)) = &w.last {
        panic!("搜尋失敗：{e}");
    }
    w.search_page.as_ref().expect("尚未搜尋")
}

#[then("結果順序應為:")]
async fn order_is(w: &mut ErpWorld, step: &Step) {
    let got: Vec<Vec<String>> = page(w)
        .items
        .iter()
        .map(|b| vec![b.title.clone(), b.source.clone()])
        .collect();
    assert_eq!(got, rows(step));
}

#[then(regex = r#"^繁體中文結果的書名順序應為 "([^"]+)"$"#)]
async fn traditional_order(w: &mut ErpWorld, list: String) {
    let got: Vec<String> = page(w)
        .items
        .iter()
        .filter(|b| b.script == "traditional")
        .map(|b| b.title.clone())
        .collect();
    assert_eq!(got.join("、"), list);
}

#[then(regex = r#"^繁體中文結果中 "([^"]+)" 應只有一筆，來源為 "([^"]+)"$"#)]
async fn dedup(w: &mut ErpWorld, title: String, src: String) {
    let hits: Vec<_> = page(w)
        .items
        .iter()
        .filter(|b| b.script == "traditional" && b.title == title)
        .collect();
    assert_eq!(hits.len(), 1, "應只有一筆");
    assert_eq!(hits[0].source, src);
}

#[then(regex = r#"^其他來源應列出 "([^"]+)"$"#)]
async fn other_sources(w: &mut ErpWorld, name: String) {
    assert!(
        page(w)
            .items
            .iter()
            .any(|b| b.other_sources.contains(&name)),
        "其他來源沒有 {name}"
    );
}

#[then("結果應為空")]
async fn empty(w: &mut ErpWorld) {
    assert!(page(w).items.is_empty(), "{:?}", page(w).items);
}

#[then(regex = r#"^結果應有 "([^"]+)"，授權為 "([^"]+)"$"#)]
async fn has_with_license(w: &mut ErpWorld, title: String, license: String) {
    let b = page(w)
        .items
        .iter()
        .find(|b| b.title == title)
        .unwrap_or_else(|| panic!("沒有 {title}"));
    assert_eq!(b.license.as_deref(), Some(license.as_str()));
    assert!(!b.open_license);
}

#[then(regex = r#"^結果應仍有 "([^"]+)"$"#)]
async fn still_has(w: &mut ErpWorld, title: String) {
    assert!(page(w).items.iter().any(|b| b.title == title));
}

#[then(regex = r#"^應提示來源 "([^"]+)" 暫時無法查詢$"#)]
async fn warned(w: &mut ErpWorld, name: String) {
    assert!(page(w).warnings.contains(&name), "{:?}", page(w).warnings);
}

#[when(regex = r#"^使用者下載 "([^"]+)" 的 "([^"]+)" 原始 EPUB$"#)]
async fn download_raw(w: &mut ErpWorld, src: String, title: String) {
    let r = w
        .epub_search
        .download_raw(&w.actor, &src, &title, Some(&title))
        .await;
    w.raw_epub = r.as_ref().ok().map(|(_, b)| b.clone());
    w.last = Some(r.map(drop));
}

#[then("取得的檔案應為 EPUB")]
async fn is_epub(w: &mut ErpWorld) {
    let bytes = w.raw_epub.clone().expect("尚未下載");
    assert!(bytes.starts_with(b"PK"), "不是 zip／EPUB");
    assert!(ttrs_app::epub_service::convert(&bytes).is_ok());
}

#[when(regex = r#"^使用者轉換 "([^"]+)" 的 "([^"]+)"，輸出 "(txt|pdf)"$"#)]
async fn convert(w: &mut ErpWorld, src: String, title: String, format: String) {
    // 搜尋過就用結果中的識別碼（GitHub 依檔名搜尋的識別碼不是書名），否則以書名當識別碼
    let id = w
        .search_page
        .as_ref()
        .and_then(|p| {
            p.items
                .iter()
                .find(|b| b.source == src && b.title == title)
                .map(|b| b.id.clone())
        })
        .unwrap_or_else(|| title.clone());
    w.search_id = Some(id.clone());
    let r = w
        .epub_search
        .convert(
            &w.actor,
            &src,
            &id,
            Some(&title),
            ConvertOptions {
                format: Format::parse(&format).expect("格式"),
                s2t: false,
            },
        )
        .await;
    set_result(w, r).await;
}

#[then(regex = r#"^稽核紀錄應有 "(\S+)" "(\S+)" 一筆，內容含 "([^"]+)" 與 "([^"]+)"$"#)]
async fn audit(w: &mut ErpWorld, program: String, action: String, d1: String, d2: String) {
    let entries = w.programs.audit_entries(&w.actor, &d1).await.unwrap();
    assert!(
        entries.iter().any(|e| e.program == program
            && e.action == action
            && e.detail
                .as_deref()
                .is_some_and(|d| d.contains(&d1) && d.contains(&d2))),
        "應有稽核紀錄：{entries:?}"
    );
}

/// epub_search_relevance.feature：第 N 筆的書名
#[then(regex = r#"^第(一|二|三|四|五)筆應為 "([^"]+)"$"#)]
async fn nth_is(w: &mut ErpWorld, nth: String, title: String) {
    let i = ["一", "二", "三", "四", "五"]
        .iter()
        .position(|n| *n == nth)
        .unwrap();
    let got = page(w).items.get(i).map(|b| b.title.clone());
    assert_eq!(got.as_deref(), Some(title.as_str()));
}
