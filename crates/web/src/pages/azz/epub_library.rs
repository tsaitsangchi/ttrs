//! 我的 GitHub 書庫（EPUB 搜尋頁的一區；specs/features/azz/epub_github_library.feature ML2）
//! 使用者自己指定要納入搜尋的 GitHub repo：加入（insert）、移除（delete）、列出（query）。

use dioxus::prelude::*;
use ttrs_shared::EpubLibraryRepoDto;

use crate::api;
use crate::components::history::local_time;
use crate::components::material::{
    Button, ButtonVariant, Notification, NotificationKind, TextField, Tone, icons,
};
use crate::components::permission::Perm;
use crate::i18n::{t, tf};

#[component]
pub fn EpubLibrary() -> Element {
    let perm = use_context::<Perm>();
    let mut input = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut list = use_resource(api::epub_library);

    let add = move |_| {
        let repo = input();
        if repo.trim().is_empty() {
            return;
        }
        busy.set(true);
        spawn(async move {
            match api::epub_library_add(repo.trim()).await {
                Ok(r) => {
                    error.set(None);
                    input.set(String::new());
                    message.set(Some(tf("library-msg-added", &[("repo", r.repo.as_str())])));
                    list.restart();
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    let mut remove = move |e: EpubLibraryRepoDto| {
        busy.set(true);
        spawn(async move {
            match api::epub_library_remove(&e.repo, e.version).await {
                Ok(()) => {
                    error.set(None);
                    message.set(Some(tf(
                        "library-msg-removed",
                        &[("repo", e.repo.as_str())],
                    )));
                    list.restart();
                }
                Err(err) => error.set(Some(err)),
            }
            busy.set(false);
        });
    };

    let entries = list.read().clone();
    rsx! {
        div { class: "card-body",
            h3 { {t("library-title")} }
            p { class: "muted", {t("library-hint")} }
            if perm.can("insert") {
                div { class: "action-bar",
                    TextField { id: "library-repo", label: t("library-field-repo"), value: input(), oninput: move |v| input.set(v) }
                    Button { icon: icons::ADD, tone: Tone::Primary, variant: ButtonVariant::Tonal, disabled: busy() || input().trim().is_empty(), onclick: add, {t("library-action-add")} }
                }
            }
            match entries {
                None => rsx! { p { class: "muted", {t("github-msg-busy")} } },
                Some(Err(e)) => rsx! { p { class: "query-hint", {e} } },
                Some(Ok(rows)) => rsx! {
                    table { class: "data-table",
                        thead {
                            tr {
                                th { {t("library-col-repo")} }
                                th { {t("github-col-license")} }
                                th { {t("library-col-added")} }
                                th {}
                            }
                        }
                        tbody {
                            if rows.is_empty() {
                                tr { td { colspan: 4, class: "muted", {t("library-msg-empty")} } }
                            }
                            for e in rows {
                                tr { key: "{e.repo}",
                                    td { a { href: "https://github.com/{e.repo}", target: "_blank", rel: "noopener", "{e.repo}" } }
                                    td { {e.license.clone().unwrap_or_else(|| t("github-license-none"))} }
                                    td { {local_time(e.created_at)} }
                                    td {
                                        if perm.can("delete") {
                                            Button { icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Text, disabled: busy(), onclick: { let e = e.clone(); move |_| remove(e.clone()) }, {t("library-action-remove")} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
            }
            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }
        }
    }
}
