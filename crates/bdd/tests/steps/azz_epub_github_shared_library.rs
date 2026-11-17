//! 共用 GitHub 書庫 — specs/features/azz/epub_github_shared_library.feature（SL；假 GitHub，不連外網 SL8）

use cucumber::{given, then, when};
use ttrs_app::ports::GithubRepo;

use crate::ErpWorld;

async fn add(w: &ErpWorld, repo: &str) -> Result<(), ttrs_app::AppError> {
    w.epub_search
        .add_shared_library(&w.actor, repo)
        .await
        .map(drop)
}

#[given(regex = r#"^共用 GitHub 書庫已有 "([^"]+)"$"#)]
async fn added(w: &mut ErpWorld, repo: String) {
    add(w, &repo).await.expect("加入共用書庫");
}

#[given(regex = r"^共用 GitHub 書庫已有 (\d+) 個 repo$")]
async fn has_n(w: &mut ErpWorld, n: usize) {
    for i in 1..=n {
        let repo = format!("filler/s{i:02}");
        w.github_feed.add_github_repo(GithubRepo {
            full_name: repo.clone(),
            description: None,
            license: None,
            stars: 0,
            updated_at: None,
            default_branch: "main".into(),
        });
        add(w, &repo).await.expect("加入共用書庫");
    }
}

#[when(regex = r#"^使用者把 "([^"]+)" 加入共用 GitHub 書庫$"#)]
async fn add_when(w: &mut ErpWorld, repo: String) {
    let r = add(w, &repo).await;
    w.last = Some(r);
}

#[when(regex = r#"^使用者把 "([^"]+)" 從共用 GitHub 書庫移除$"#)]
async fn remove(w: &mut ErpWorld, repo: String) {
    let list = w
        .epub_search
        .shared_library(&w.actor)
        .await
        .expect("列出共用書庫");
    let version = list
        .iter()
        .find(|e| e.repo == repo)
        .map(|e| e.version)
        .expect("共用書庫中沒有這個 repo");
    let r = w
        .epub_search
        .remove_shared_library(&w.actor, &repo, version)
        .await;
    w.last = Some(r);
}

#[then(regex = r#"^共用 GitHub 書庫應為 "([^"]*)"$"#)]
async fn list_is(w: &mut ErpWorld, expected: String) {
    let got: Vec<String> = w
        .epub_search
        .shared_library(&w.actor)
        .await
        .expect("列出共用書庫")
        .into_iter()
        .map(|e| e.repo)
        .collect();
    let want: Vec<String> = expected
        .split('、')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    assert_eq!(got, want);
}

#[then(regex = r#"^結果中 "([^"]+)" 應只有一筆$"#)]
async fn only_one(w: &mut ErpWorld, title: String) {
    if let Some(Err(e)) = &w.last {
        panic!("搜尋失敗：{e}");
    }
    let page = w.search_page.as_ref().expect("尚未搜尋");
    let n = page.items.iter().filter(|b| b.title == title).count();
    assert_eq!(n, 1, "應只有一筆");
}
