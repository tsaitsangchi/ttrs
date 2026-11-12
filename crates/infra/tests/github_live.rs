//! GitHub 實測（github_epub_search.feature GE8）：需要網路，預設不執行。
//! 執行：cargo test -p ttrs-infra --test github_live -- --ignored

use ttrs_app::ports::GithubSource;

#[tokio::test]
#[ignore = "需要網路，連 GitHub API"]
async fn search_and_list_epubs() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused@localhost/unused")
        .unwrap();
    let store = ttrs_infra::PgStore::new(pool);
    let (repos, total) = store
        .search_repos("standardebooks epub", 1, 5)
        .await
        .unwrap();
    assert!(total > 0 && !repos.is_empty(), "{repos:?}");
    let r = &repos[0];
    let (files, _) = store.tree(&r.full_name, &r.default_branch).await.unwrap();
    assert!(!files.is_empty());
}
