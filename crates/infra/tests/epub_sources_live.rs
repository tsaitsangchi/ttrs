//! 多來源 EPUB 搜尋的實測（需要網路，預設不執行）：
//! `cargo test -p ttrs-infra --test epub_sources_live -- --ignored --nocapture`
//! epub_web_search.feature WS10

use ttrs_infra::default_epub_sources;

fn keywords() -> Vec<String> {
    vec!["紅樓夢".to_owned(), "红楼梦".to_owned()]
}

async fn search_ok(name: &str, expect_results: bool) -> Vec<ttrs_app::ports::EpubBook> {
    let sources = default_epub_sources();
    let s = sources.iter().find(|s| s.name() == name).expect("來源");
    let books = s.search(&keywords(), true, 10).await.expect(name);
    println!("{name}: {} 筆", books.len());
    for b in books.iter().take(3) {
        println!(
            "  {} | {:?} | {:?} | {:?}",
            b.title, b.language, b.license, b.downloads
        );
    }
    if expect_results {
        assert!(!books.is_empty(), "{name} 應有結果");
    }
    books
}

#[tokio::test]
#[ignore = "需要網路"]
async fn internet_archive_search_and_download() {
    let books = search_ok("Internet Archive", true).await;
    // 下載一本小的（上限 5 MB），確認能取得 EPUB（zip）
    let s = default_epub_sources()
        .into_iter()
        .find(|s| s.name() == "Internet Archive")
        .unwrap();
    for b in books.iter().take(5) {
        if let Ok(bytes) = s.download(&b.id, 5 * 1024 * 1024).await {
            assert!(bytes.starts_with(b"PK"));
            println!("下載 {} 成功 {} bytes", b.id, bytes.len());
            return;
        }
    }
    panic!("前 5 筆都下載不到 5 MB 以內的 EPUB");
}

#[tokio::test]
#[ignore = "需要網路"]
async fn gutenberg_search_and_download() {
    let books = search_ok("Gutenberg", true).await;
    let s = default_epub_sources()
        .into_iter()
        .find(|s| s.name() == "Gutenberg")
        .unwrap();
    let bytes = s
        .download(&books[0].id, 20 * 1024 * 1024)
        .await
        .expect("下載");
    assert!(bytes.starts_with(b"PK"));
    println!("下載 {} 成功 {} bytes", books[0].title, bytes.len());
}

#[tokio::test]
#[ignore = "需要網路"]
async fn wikisource_search_and_download() {
    let books = search_ok("Wikisource", true).await;
    let s = default_epub_sources()
        .into_iter()
        .find(|s| s.name() == "Wikisource")
        .unwrap();
    let bytes = s
        .download(&books[0].id, 50 * 1024 * 1024)
        .await
        .expect("下載");
    assert!(bytes.starts_with(b"PK"));
    println!("下載 {} 成功 {} bytes", books[0].title, bytes.len());
}

#[tokio::test]
#[ignore = "需要網路"]
async fn github_search() {
    search_ok("GitHub", false).await;
}

/// GitHub 依檔名搜尋（epub_github_filename.feature GF7）：需要 TTRS_GITHUB_TOKEN 與網路
#[tokio::test]
#[ignore = "需要網路與 TTRS_GITHUB_TOKEN"]
async fn github_filename_search_and_download() {
    use std::sync::Arc;
    use ttrs_app::{
        github_epub_source::GithubFiles,
        ports::{EpubBookKind, EpubSource},
    };
    let s = GithubFiles(Arc::new(ttrs_infra::GithubHttp));
    let books = s
        .search(&["慶餘年".to_owned(), "庆余年".to_owned()], false, 10)
        .await
        .expect("GitHub 依檔名");
    for b in &books {
        println!("{}｜{}｜{:?}", b.title, b.id, b.license);
    }
    let b = books
        .iter()
        .find(|b| b.kind == EpubBookKind::Book)
        .expect("至少一筆");
    let bytes = s.download(&b.id, 100 * 1024 * 1024).await.expect("下載");
    assert!(bytes.starts_with(b"PK"));
    println!("下載 {} 成功 {} bytes", b.title, bytes.len());
}
