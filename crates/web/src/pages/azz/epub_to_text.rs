//! EPUB 轉換（TTRS 工具作業；specs/features/azz/epub_to_text.feature、epub_to_pdf.feature）
//! 選一個或多個 EPUB → 逐一上傳轉換 → 下載同名的 .txt（ET2）；下方列出每本的書名、作者、章節數或錯誤。

use dioxus::prelude::*;

use crate::api;
use crate::components::material::{
    Card, Checkbox, Notification, NotificationKind, RadioGroup, Tone, icons,
};
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

impl Done {
    fn failed(file: String, error: String) -> Self {
        Self {
            file,
            title: None,
            author: None,
            chapters: None,
            error: Some(error),
        }
    }
}

/// 輸出格式（TXT／PDF）與簡體轉繁體選項；EPUB 轉換與 GitHub EPUB 搜尋共用（EP1、EP11）
#[component]
pub fn EpubOptions(format: Signal<String>, s2t: Signal<bool>, disabled: bool) -> Element {
    rsx! {
        div { class: "action-bar",
            RadioGroup {
                name: "epub-format",
                legend: t("epub-field-format"),
                options: vec![("txt".to_owned(), t("epub-format-txt")), ("pdf".to_owned(), t("epub-format-pdf"))],
                value: Some(format()),
                disabled,
                onchange: move |v: String| format.set(v),
            }
            Checkbox { id: "epub-s2t", label: t("epub-field-s2t"), checked: s2t(), disabled, onchange: move |v: bool| s2t.set(v) }
        }
    }
}

#[component]
fn EpubPage() -> Element {
    let mut results = use_signal(Vec::<Done>::new);
    let mut running = use_signal(|| false);
    let format = use_signal(|| "txt".to_owned());
    let s2t = use_signal(|| false);

    let on_files = move |e: FormEvent| async move {
        let files = e.files();
        if files.is_empty() {
            return;
        }
        running.set(true);
        // 同一組選項套用到每一本（EP10）
        let (fmt, to_trad) = (format(), s2t());
        for f in files {
            let name = f.name();
            let outcome = match f.read_bytes().await {
                Err(err) => Err(err.to_string()),
                Ok(bytes) => api::epub_convert(&name, bytes.to_vec(), &fmt, to_trad).await,
            };
            let done = match outcome {
                Ok(r) => match api::download_file(&r.file_name, &r.bytes, r.mime) {
                    Ok(()) => Done {
                        file: name,
                        title: r.title,
                        author: r.author,
                        chapters: Some(r.chapters),
                        error: None,
                    },
                    Err(err) => Done::failed(name, err),
                },
                Err(err) => Done::failed(name, err),
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
                    EpubOptions { format, s2t, disabled: running() }
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
