//! GitHub EPUB 搜尋 — specs/features/azz/github_epub_search.feature（GE；假 GitHub 來源，不連外網 GE8）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::ports::GithubRepo;

use crate::{ErpWorld, steps::azz_epub_to_text::build, steps::common::rows};

#[given("GitHub 上有 repo:")]
async fn repos(w: &mut ErpWorld, step: &Step) {
    for r in rows(step) {
        w.github_feed.add_github_repo(GithubRepo {
            full_name: r[0].clone(),
            description: Some(r[1].clone()).filter(|s| !s.is_empty()),
            license: Some(r[2].clone()).filter(|s| !s.is_empty()),
            stars: r[3].parse().unwrap(),
            updated_at: None,
            default_branch: "main".into(),
        });
    }
}

#[given(expr = "GitHub repo {string} 有檔案:")]
async fn files(w: &mut ErpWorld, repo: String, step: &Step) {
    for r in rows(step) {
        w.github_feed
            .add_github_file(&repo, &r[0], r[1].parse().unwrap(), Vec::new());
    }
}

#[given(expr = "GitHub 檔案 {string} {string} 是一本 EPUB，書名 {string}，內容 {string}")]
async fn epub_file(w: &mut ErpWorld, repo: String, path: String, title: String, body: String) {
    let ch = vec![("c1.xhtml".to_owned(), body)];
    let bytes = build(&title, "某", &ch, &["c1.xhtml".to_owned()], false);
    let size = bytes.len() as u64;
    w.github_feed.add_github_file(&repo, &path, size, bytes);
}

#[given(regex = r#"^GitHub repo "(\S+)" 的檔案 "(\S+)" 大小為 (\d+) MB$"#)]
async fn big_file(w: &mut ErpWorld, repo: String, path: String, mb: u64) {
    w.github_feed
        .add_github_file(&repo, &path, mb * 1024 * 1024, Vec::new());
}

#[given("GitHub 查詢次數已用完")]
async fn limited(w: &mut ErpWorld) {
    w.github_feed.set_github_rate_limited(true);
}

#[when(regex = r#"^使用者在 GitHub 搜尋 "(\S+)"(，只看開放授權)?$"#)]
async fn search(w: &mut ErpWorld, keyword: String, open: String) {
    let r = w
        .github
        .search(&w.actor, &keyword, !open.is_empty(), 1)
        .await;
    w.github_repos = r.as_ref().ok().map(|p| p.items.clone());
    w.last = Some(r.map(drop));
}

#[then("搜尋結果應為:")]
async fn search_is(w: &mut ErpWorld, step: &Step) {
    if let Some(Err(e)) = &w.last {
        panic!("搜尋失敗：{e}");
    }
    let got: Vec<Vec<String>> = w
        .github_repos
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|r| vec![r.full_name, r.license.unwrap_or_default()])
        .collect();
    assert_eq!(got, rows(step));
}

#[when(expr = "使用者列出 GitHub repo {string} 的 EPUB")]
async fn list(w: &mut ErpWorld, repo: String) {
    let r = w.github.files(&w.actor, &repo).await;
    w.github_files = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

#[then("EPUB 清單應為:")]
async fn list_is(w: &mut ErpWorld, step: &Step) {
    let got: Vec<Vec<String>> = w
        .github_files
        .clone()
        .expect("尚未列出")
        .files
        .into_iter()
        .map(|f| vec![f.path, f.size.to_string()])
        .collect();
    assert_eq!(got, rows(step));
}

#[when(expr = "使用者轉換 GitHub 檔案 {string} {string}")]
async fn convert(w: &mut ErpWorld, repo: String, path: String) {
    let r = w.github.convert(&w.actor, &repo, &path).await;
    w.epub_result = r.as_ref().ok().cloned();
    w.last = Some(r.map(drop));
}

#[then(expr = "轉出的檔名應為 {string}，書名 {string}")]
async fn converted(w: &mut ErpWorld, name: String, title: String) {
    let r = w.epub_result.clone().expect("尚未轉換");
    assert_eq!((r.file_name, r.title), (name, Some(title)));
}
