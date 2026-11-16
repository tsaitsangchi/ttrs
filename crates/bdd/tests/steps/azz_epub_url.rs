//! 以網址下載 EPUB — specs/features/azz/epub_url_download.feature（UD；假網路，不連外網、位址解析也是假的 UD8）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::{ConvertOptions, Format};

use crate::{
    ErpWorld,
    steps::{azz_epub_to_pdf::set_result, azz_epub_to_text::build, common::rows},
};

fn epub(title: &str) -> Vec<u8> {
    let ch = vec![("c1.xhtml".to_owned(), "<p>內容</p>".to_owned())];
    build(title, "某", &ch, &["c1.xhtml".to_owned()], false)
}

#[given(regex = r#"^網址 "([^"]+)" 回傳一本 EPUB，書名 "([^"]+)"$"#)]
async fn url_epub(w: &mut ErpWorld, url: String, title: String) {
    w.fake_web.add_epub(&url, epub(&title));
}

#[given(regex = r#"^網址 "([^"]+)" 是網頁，含連結:$"#)]
async fn url_page(w: &mut ErpWorld, url: String, step: &Step) {
    let links: String = rows(step)
        .into_iter()
        .map(|r| format!("<a href=\"{}\">link</a>", r[0]))
        .collect();
    w.fake_web
        .add_html(&url, &format!("<html><body>{links}</body></html>"));
}

#[given(regex = r#"^網址 "([^"]+)" 回傳內容 "([^"]*)"$"#)]
async fn url_body(w: &mut ErpWorld, url: String, body: String) {
    w.fake_web
        .add_body(&url, "application/epub+zip", body.into_bytes());
}

#[given(regex = r#"^網址 "([^"]+)" 的內容有 (\d+) MB$"#)]
async fn url_big(w: &mut ErpWorld, url: String, mb: u64) {
    w.fake_web.add_sized(&url, mb * 1024 * 1024);
}

#[given(regex = r#"^網域 "([^"]+)" 解析到 "([^"]+)"$"#)]
async fn dns(w: &mut ErpWorld, host: String, ip: String) {
    w.fake_web.set_dns(&host, &ip);
}

#[given(regex = r#"^網址 "([^"]+)" 轉址到 "([^"]+)"$"#)]
async fn redirect(w: &mut ErpWorld, url: String, to: String) {
    w.fake_web.add_redirect(&url, &to);
}

#[given(regex = r#"^網址 "([^"]+)" 一直轉址到自己$"#)]
async fn redirect_loop(w: &mut ErpWorld, url: String) {
    w.fake_web.add_redirect(&url, &url);
}

#[given(regex = r#"^網站 "([^"]+)" 的 robots.txt 禁止所有路徑$"#)]
async fn robots(w: &mut ErpWorld, origin: String) {
    w.fake_web.add_body(
        &format!("{origin}/robots.txt"),
        "text/plain",
        b"User-agent: *\nDisallow: /".to_vec(),
    );
}

#[when(regex = r#"^使用者讀取網址 "([^"]+)"$"#)]
async fn read(w: &mut ErpWorld, url: String) {
    let r = w.epub_url.read(&w.actor, &url).await;
    w.url_read = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

#[then(regex = r#"^應視為一本書，書名 "([^"]+)"$"#)]
async fn is_book(w: &mut ErpWorld, title: String) {
    if let Some(Err(e)) = &w.last {
        panic!("讀取失敗：{e}");
    }
    let r = w.url_read.as_ref().expect("尚未讀取");
    assert_eq!(r.kind, "book");
    assert_eq!(r.title.as_deref(), Some(title.as_str()));
}

#[then("應列出 EPUB 連結:")]
async fn links_are(w: &mut ErpWorld, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("讀取失敗：{e}");
    }
    let r = w.url_read.as_ref().expect("尚未讀取");
    assert_eq!(r.kind, "links");
    let got: Vec<Vec<String>> = r
        .links
        .iter()
        .map(|l| vec![l.name.clone(), l.url.clone()])
        .collect();
    assert_eq!(got, rows(step));
}

#[when(regex = r#"^使用者轉換網址 "([^"]+)"，輸出 "(txt|pdf)"$"#)]
async fn convert(w: &mut ErpWorld, url: String, format: String) {
    let r = w
        .epub_url
        .convert(
            &w.actor,
            &url,
            ConvertOptions {
                format: Format::parse(&format).expect("格式"),
                s2t: false,
            },
        )
        .await;
    set_result(w, r).await;
}

#[then(regex = r#"^稽核紀錄應有網址 "([^"]+)" 的 "(\S+)" "(\S+)" 一筆，且內容不含 "([^"]+)"$"#)]
async fn audit(w: &mut ErpWorld, target: String, program: String, action: String, secret: String) {
    let entries = w.programs.audit_entries(&w.actor, &target).await.unwrap();
    let hit: Vec<_> = entries
        .iter()
        .filter(|e| e.program == program && e.action == action)
        .collect();
    assert!(!hit.is_empty(), "應有稽核紀錄：{entries:?}");
    assert!(
        hit.iter().all(
            |e| !e.detail.as_deref().unwrap_or_default().contains(&secret)
                && !e.key.contains(&secret)
        ),
        "稽核紀錄不應含「{secret}」：{hit:?}"
    );
}
