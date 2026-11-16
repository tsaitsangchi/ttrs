//! GitHub 依檔名搜尋 EPUB — specs/features/azz/epub_github_filename.feature（GF；假 GitHub，不連外網 GF7）

use std::sync::Arc;

use cucumber::{gherkin::Step, given, then};
use ttrs_app::{
    github_epub_source::{GithubFiles, GithubRepos},
    ports::{EpubSource, GithubRepo, GithubSource},
};

use crate::{
    ErpWorld,
    steps::{azz_epub_to_text::build, common::rows},
};

#[given("GitHub 上有下列 EPUB 檔:")]
async fn epub_files(w: &mut ErpWorld, step: &Step) {
    let mut repos: Vec<String> = Vec::new();
    for r in rows(step) {
        let (repo, path, license) = (&r[0], &r[1], &r[2]);
        if !repos.contains(repo) {
            repos.push(repo.clone());
            w.github_feed.add_github_repo(GithubRepo {
                full_name: repo.clone(),
                description: None,
                license: Some(license.clone()).filter(|s| !s.is_empty()),
                stars: 0,
                updated_at: None,
                default_branch: "main".into(),
            });
        }
        let file = path.rsplit('/').next().unwrap_or(path);
        let title = file.trim_end_matches(".epub");
        let ch = vec![("c1.xhtml".to_owned(), "<p>內容</p>".to_owned())];
        let bytes = build(title, "某", &ch, &["c1.xhtml".to_owned()], false);
        let size = bytes.len() as u64;
        w.github_feed.add_github_file(repo, path, size, bytes);
    }
    let gh: Arc<dyn GithubSource> = Arc::new(w.github_feed.clone());
    let sources: Vec<Arc<dyn EpubSource>> =
        vec![Arc::new(GithubRepos(gh.clone())), Arc::new(GithubFiles(gh))];
    w.epub_search = w.epub_search.clone().with_sources(sources);
}

#[given("已設定 GitHub token")]
async fn with_token(w: &mut ErpWorld) {
    w.github_feed.set_github_token(true);
}

#[given("未設定 GitHub token")]
async fn without_token(w: &mut ErpWorld) {
    w.github_feed.set_github_token(false);
}

#[given("GitHub 程式碼搜尋被限流")]
async fn code_limited(w: &mut ErpWorld) {
    w.github_feed.set_github_code_limited(true);
}

fn page(w: &ErpWorld) -> &ttrs_shared::EpubSearchPage {
    if let Some(Err(e)) = &w.last {
        panic!("搜尋失敗：{e}");
    }
    w.search_page.as_ref().expect("尚未搜尋")
}

#[then(regex = r#"^結果應有 "([^"]+)"，來源為 "([^"]+)"$"#)]
async fn has_from(w: &mut ErpWorld, title: String, source: String) {
    assert!(
        page(w)
            .items
            .iter()
            .any(|b| b.title == title && b.source == source),
        "沒有 {title}（{source}）：{:?}",
        page(w).items.iter().map(|b| &b.title).collect::<Vec<_>>()
    );
}

#[then(regex = r#"^結果不應有 "([^"]+)"$"#)]
async fn has_not(w: &mut ErpWorld, title: String) {
    assert!(
        !page(w).items.iter().any(|b| b.title == title),
        "不應有 {title}"
    );
}

#[then(regex = r#"^應提示 "([A-Z_]+)"$"#)]
async fn notice(w: &mut ErpWorld, code: String) {
    assert!(page(w).notices.contains(&code), "{:?}", page(w).notices);
}

#[then(regex = r#"^這次下載的稽核紀錄內容應含 "([^"]+)" 與 "([^"]+)"$"#)]
async fn download_audit(w: &mut ErpWorld, d1: String, d2: String) {
    let id = w.search_id.clone().expect("尚未轉換搜尋結果");
    let key = format!("GitHub/{id}");
    let entries = w.programs.audit_entries(&w.actor, &key).await.unwrap();
    assert!(
        entries.iter().any(|e| e.program == "github-epub-search"
            && e.action == "run"
            && e.detail
                .as_deref()
                .is_some_and(|d| d.contains(&d1) && d.contains(&d2))),
        "{entries:?}"
    );
}
