//! 我的 GitHub 書庫 — specs/features/azz/epub_github_library.feature（ML；假 GitHub，不連外網 ML8）

use cucumber::{given, then, when};
use ttrs_app::{Actor, ports::GithubRepo};

use crate::ErpWorld;

async fn add(w: &mut ErpWorld, actor: &Actor, repo: &str) -> Result<(), ttrs_app::AppError> {
    w.epub_search.add_library(actor, repo).await.map(drop)
}

#[given(regex = r#"^使用者已把 "([^"]+)" 加入我的 GitHub 書庫$"#)]
async fn added(w: &mut ErpWorld, repo: String) {
    let actor = w.actor.clone();
    add(w, &actor, &repo).await.expect("加入書庫");
}

#[given(regex = r#"^使用者 "([^"]+)" 已把 "([^"]+)" 加入他的 GitHub 書庫$"#)]
async fn other_added(w: &mut ErpWorld, user: String, repo: String) {
    let actor = Actor {
        user_id: user,
        ..w.actor.clone()
    };
    add(w, &actor, &repo).await.expect("加入書庫");
}

#[given(regex = r"^使用者的 GitHub 書庫已有 (\d+) 個 repo$")]
async fn has_n(w: &mut ErpWorld, n: usize) {
    let actor = w.actor.clone();
    for i in 1..=n {
        let repo = format!("filler/r{i:02}");
        w.github_feed.add_github_repo(GithubRepo {
            full_name: repo.clone(),
            description: None,
            license: None,
            stars: 0,
            updated_at: None,
            default_branch: "main".into(),
        });
        add(w, &actor, &repo).await.expect("加入書庫");
    }
}

#[when(regex = r#"^使用者把 "([^"]+)" 加入我的 GitHub 書庫$"#)]
async fn add_when(w: &mut ErpWorld, repo: String) {
    let actor = w.actor.clone();
    let r = add(w, &actor, &repo).await;
    w.last = Some(r);
}

#[when(regex = r#"^使用者把 "([^"]+)" 從我的 GitHub 書庫移除$"#)]
async fn remove(w: &mut ErpWorld, repo: String) {
    let list = w.epub_search.library(&w.actor).await.expect("列出書庫");
    let version = list
        .iter()
        .find(|e| e.repo == repo)
        .map(|e| e.version)
        .expect("書庫中沒有這個 repo");
    let r = w.epub_search.remove_library(&w.actor, &repo, version).await;
    w.last = Some(r);
}

#[then(regex = r#"^我的 GitHub 書庫應為 "([^"]*)"$"#)]
async fn list_is(w: &mut ErpWorld, expected: String) {
    let got: Vec<String> = w
        .epub_search
        .library(&w.actor)
        .await
        .expect("列出書庫")
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
