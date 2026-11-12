//! EPUB 轉 TXT（TTRS 工具作業；specs/features/azz/epub_to_text.feature）
//! 選一個或多個 EPUB → 逐一上傳轉換 → 下載同名的 .txt（ET2）；下方列出每本的書名、作者、章節數或錯誤。

use dioxus::prelude::*;

use crate::api;
use crate::components::material::{Card, Notification, NotificationKind, Tone, icons};
use crate::components::permission::ProgramGate;
use crate::i18n::{t, tf};

const PROGRAM: &str = "epub-to-text";

#[component]
pub fn EpubToText() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, EpubPage {} }
    }
}

/// 一本的結果：檔名、書名、作者、章節數；或錯誤
#[derive(Clone, PartialEq)]
struct Done {
    file: String,
    title: Option<String>,
    author: Option<String>,
    chapters: Option<u32>,
    error: Option<String>,
}

#[component]
fn EpubPage() -> Element {
    let mut results = use_signal(Vec::<Done>::new);
    let mut running = use_signal(|| false);

    let on_files = move |e: FormEvent| async move {
        let files = e.files();
        if files.is_empty() {
            return;
        }
        running.set(true);
        for f in files {
            let name = f.name();
            let done = match f.read_bytes().await {
                Err(err) => Done {
                    file: name,
                    title: None,
                    author: None,
                    chapters: None,
                    error: Some(err.to_string()),
                },
                Ok(bytes) => match api::epub_to_text(&name, bytes.to_vec()).await {
                    Ok(r) => match api::download_text(&r.file_name, &r.text) {
                        Ok(()) => Done {
                            file: name,
                            title: r.title,
                            author: r.author,
                            chapters: Some(r.chapters),
                            error: None,
                        },
                        Err(err) => Done {
                            file: name,
                            title: None,
                            author: None,
                            chapters: None,
                            error: Some(err),
                        },
                    },
                    Err(err) => Done {
                        file: name,
                        title: None,
                        author: None,
                        chapters: None,
                        error: Some(err),
                    },
                },
            };
            results.write().insert(0, done);
        }
        running.set(false);
    };

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("epub-breadcrumb")} }
                h2 { {t("epub-title")} }
            }
            Card {
                div { class: "card-body",
                    p { {t("epub-hint")} }
                    div { class: "action-bar",
                        label { class: "md-button md-state md-button--filled md-button--with-icon",
                            svg {
                                class: "md-button__icon {Tone::Neutral.icon_class()}",
                                width: "18",
                                height: "18",
                                view_box: "0 0 24 24",
                                fill: "currentColor",
                                "aria-hidden": "true",
                                path { d: icons::UPLOAD }
                            }
                            {t("epub-action-choose")}
                            input {
                                id: "epub-files",
                                r#type: "file",
                                accept: ".epub,application/epub+zip",
                                multiple: true,
                                hidden: true,
                                disabled: running(),
                                onchange: on_files,
                            }
                        }
                        if running() {
                            span { class: "muted", {t("epub-msg-running")} }
                        }
                    }
                }
            }
            for r in results() {
                if let Some(err) = r.error.clone() {
                    Notification { kind: NotificationKind::Error, title: tf("epub-msg-failed", &[("file", r.file.as_str()), ("error", err.as_str())]), onclose: move |_| {} }
                } else {
                    Notification {
                        kind: NotificationKind::Success,
                        title: tf("epub-msg-done", &[
                            ("file", r.file.as_str()),
                            ("title", r.title.clone().unwrap_or_default().as_str()),
                            ("author", r.author.clone().unwrap_or_default().as_str()),
                            ("chapters", r.chapters.unwrap_or(0).to_string().as_str()),
                        ]),
                        onclose: move |_| {},
                    }
                }
            }
        }
    }
}
