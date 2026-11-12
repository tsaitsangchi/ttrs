//! GitHub EPUB 搜尋（TTRS 工具作業；specs/features/azz/github_epub_search.feature）
//! 關鍵字搜尋 repo（可只看開放授權）→ 選 repo 列出 .epub → 一次轉換一本並下載 TXT（GE2–GE4）。

use dioxus::prelude::*;
use ttrs_shared::{GithubEpubListDto, GithubRepoPage};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Notification, NotificationKind, TextField, Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::i18n::{t, tf};

const PROGRAM: &str = "github-epub-search";

#[component]
pub fn GithubEpubSearch() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, SearchPage {} }
    }
}

fn size_text(bytes: u64) -> String {
    // 以整數四捨五入到小數一位（禁用浮點數）
    let tenths = (bytes * 10 + 524_288) / 1_048_576;
    format!("{}.{} MB", tenths / 10, tenths % 10) // i18n-allow
}

/// 捲動到指定區塊（列出 EPUB 後清單在頁面下方）
fn scroll_to(id: &'static str) {
    document::eval(&format!(
        // 等畫面重繪出該區塊後再捲動
        "setTimeout(()=>document.getElementById('{id}')?.scrollIntoView({{block:'start'}}),100)"
    ));
}

#[component]
fn SearchPage() -> Element {
    let perm = use_context::<Perm>();
    let mut keyword = use_signal(|| "epub".to_owned());
    let mut open_only = use_signal(|| true);
    let mut page = use_signal(|| 1u32);
    let mut repos = use_signal(|| None::<GithubRepoPage>);
    let mut files = use_signal(|| None::<GithubEpubListDto>);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    // 轉換結果顯示在 EPUB 清單裡（清單在頁面下方，訊息放上方會看不到）
    let mut file_error = use_signal(|| None::<String>);

    let mut search = move |p: u32| {
        let (q, open) = (keyword(), open_only());
        if q.trim().is_empty() {
            return;
        }
        busy.set(true);
        spawn(async move {
            match api::github_epub_repos(q.trim(), open, p).await {
                Ok(r) => {
                    error.set(None);
                    page.set(p);
                    files.set(None);
                    repos.set(Some(r));
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    let mut list_files = move |repo: String| {
        busy.set(true);
        spawn(async move {
            match api::github_epub_files(&repo).await {
                Ok(f) => {
                    error.set(None);
                    message.set(None);
                    file_error.set(None);
                    files.set(Some(f));
                    scroll_to("github-files");
                }
                Err(e) => {
                    error.set(Some(e));
                    scroll_to("github-top");
                }
            }
            busy.set(false);
        });
    };

    let mut convert = move |(repo, path): (String, String)| {
        busy.set(true);
        message.set(None);
        file_error.set(None);
        spawn(async move {
            match api::github_epub_convert(&repo, &path).await {
                Ok(r) => match api::download_text(&r.file_name, &r.text) {
                    Ok(()) => {
                        message.set(Some(tf(
                            "github-msg-done",
                            &[
                                ("file", r.file_name.as_str()),
                                ("title", r.title.clone().unwrap_or_default().as_str()),
                                ("chapters", r.chapters.to_string().as_str()),
                            ],
                        )));
                    }
                    Err(e) => file_error.set(Some(e)),
                },
                Err(e) => file_error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    let result = repos();
    let pages = result.as_ref().map_or(1, |r| {
        u32::try_from(r.total.min(1000).div_ceil(30))
            .unwrap_or(1)
            .max(1)
    });

    rsx! {
        div { class: "page", id: "github-top",
            div { class: "page-header",
                div { class: "program-id", {t("github-breadcrumb")} }
                h2 { {t("github-title")} }
            }
            Card {
                div { class: "card-body",
                    p { class: "muted", {t("github-hint")} }
                    div { class: "action-bar",
                        TextField { id: "github-q", label: t("github-field-keyword"), value: keyword(), oninput: move |v| keyword.set(v) }
                        Checkbox { id: "github-open", label: t("github-field-open-only"), checked: open_only(), onchange: move |v| open_only.set(v) }
                        Button { icon: icons::SEARCH, tone: Tone::Primary, variant: ButtonVariant::Tonal, disabled: busy() || keyword().trim().is_empty(), onclick: move |_| search(1), {t("common-action-query")} }
                        if busy() {
                            span { class: "muted", {t("github-msg-busy")} }
                        }
                    }
                }
            }
            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(r) = result {
                Card {
                    div { class: "table-header",
                        h3 { {t("github-repos-title")} }
                        p { {tf("github-repos-count", &[("total", r.total.to_string().as_str()), ("page", page().to_string().as_str()), ("pages", pages.to_string().as_str())])} }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead { tr {
                                th { {t("github-col-repo")} }
                                th { {t("github-col-description")} }
                                th { {t("github-col-license")} }
                                th { {t("github-col-stars")} }
                                th {}
                            } }
                            tbody {
                                if r.items.is_empty() {
                                    tr { td { colspan: 5, class: "muted", {t("common-msg-no-data")} } }
                                }
                                for repo in r.items.clone() {
                                    tr { key: "{repo.full_name}",
                                        td { a { href: "{repo.html_url}", target: "_blank", rel: "noopener", "{repo.full_name}" } }
                                        td { div { class: "clamp-2", title: repo.description.clone().unwrap_or_default(), {repo.description.clone().unwrap_or_default()} } }
                                        td { {repo.license.clone().unwrap_or_else(|| t("github-license-none"))} }
                                        td { class: "num", "{repo.stars}" }
                                        td {
                                            Button { icon: icons::SEARCH, tone: Tone::Primary, variant: ButtonVariant::Text, disabled: busy(), onclick: { let n = repo.full_name.clone(); move |_| list_files(n.clone()) }, {t("github-action-files")} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "action-bar",
                        Button { icon: icons::ARROW_BACK, tone: Tone::Neutral, variant: ButtonVariant::Text, disabled: busy() || page() <= 1, onclick: move |_| search(page() - 1), {t("github-action-prev")} }
                        Button { icon: icons::ARROW_FORWARD, tone: Tone::Neutral, variant: ButtonVariant::Text, disabled: busy() || page() >= pages, onclick: move |_| search(page() + 1), {t("github-action-next")} }
                    }
                }
            }
            if let Some(f) = files() {
                Card {
                    div { class: "table-header", id: "github-files",
                        h3 { {tf("github-files-title", &[("repo", f.repo.as_str())])} }
                        p { {match &f.license {
                            Some(l) => tf("github-files-license", &[("license", l.as_str())]),
                            None => t("github-license-unknown"),
                        }} }
                        if f.truncated {
                            p { class: "query-hint", {t("github-files-truncated")} }
                        }
                    }
                    if let Some(msg) = file_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| file_error.set(None) }
                    }
                    if let Some(msg) = message() {
                        Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead { tr {
                                th { {t("github-col-path")} }
                                th { {t("github-col-size")} }
                                th {}
                            } }
                            tbody {
                                if f.files.is_empty() {
                                    tr { td { colspan: 3, class: "muted", {t("github-files-none")} } }
                                }
                                for file in f.files.clone() {
                                    tr { key: "{file.path}",
                                        td { a { href: "{file.html_url}", target: "_blank", rel: "noopener", class: "mono", "{file.path}" } }
                                        td { class: "num", {size_text(file.size)} }
                                        td {
                                            if perm.can("run") {
                                                Button { icon: icons::DOWNLOAD, tone: Tone::Success, variant: ButtonVariant::Tonal, disabled: busy(), onclick: { let (r, p) = (f.repo.clone(), file.path.clone()); move |_| convert((r.clone(), p.clone())) }, {t("github-action-convert")} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
