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

/// 提交翻譯工作、每 3 秒查一次進度，完成後下載結果（EL4）
async fn translate_one(
    name: &str,
    bytes: Vec<u8>,
    format: &str,
    consent: bool,
    progress: &mut Signal<Option<(String, u32, u32)>>,
) -> Result<api::EpubFile, String> {
    let id = api::epub_translate_submit(name, bytes, format, consent).await?;
    loop {
        let s = api::epub_translate_status(&id).await?;
        progress.set(Some((name.to_owned(), s.done_lines, s.total_lines)));
        match s.status.as_str() {
            "done" => return api::epub_translate_download(&id, format).await,
            "failed" => {
                let code = s.error_code.unwrap_or_default();
                let msg = s.error_message.unwrap_or_default();
                return Err(crate::i18n::error_text(&code, &msg));
            }
            _ => api::sleep_ms(3000).await,
        }
    }
}

/// 輸出格式（TXT／PDF）、簡體轉繁體與（EPUB 轉換頁才有的）英文翻譯選項（EP1、EL1）
/// `translate` 為 (語言, 同意)：語言 "none"／"translate"；GitHub EPUB 搜尋頁不放翻譯（EL12）
#[component]
pub fn EpubOptions(
    format: Signal<String>,
    s2t: Signal<bool>,
    #[props(default)] translate: Option<(Signal<String>, Signal<bool>)>,
    disabled: bool,
) -> Element {
    let translating = translate.is_some_and(|(lang, _)| lang() == "translate");
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
            // 英文翻譯的結果本來就是繁體，不需要再簡轉繁
            Checkbox { id: "epub-s2t", label: t("epub-field-s2t"), checked: s2t() && !translating, disabled: disabled || translating, onchange: move |v: bool| s2t.set(v) }
            if let Some((mut lang, mut consent)) = translate {
                RadioGroup {
                    name: "epub-lang",
                    legend: t("epub-field-lang"),
                    options: vec![("none".to_owned(), t("epub-lang-none")), ("translate".to_owned(), t("epub-lang-translate"))],
                    value: Some(lang()),
                    disabled,
                    onchange: move |v: String| lang.set(v),
                }
                if translating {
                    Checkbox { id: "epub-consent", label: t("epub-field-consent"), checked: consent(), disabled, onchange: move |v: bool| consent.set(v) }
                }
            }
        }
        if translating {
            p { class: "query-hint", {t("epub-hint-translate")} }
        }
    }
}

#[component]
fn EpubPage() -> Element {
    let mut results = use_signal(Vec::<Done>::new);
    let mut running = use_signal(|| false);
    let format = use_signal(|| "txt".to_owned());
    let s2t = use_signal(|| false);
    let lang = use_signal(|| "none".to_owned());
    let consent = use_signal(|| false);
    // 翻譯進度：(檔名, 已翻譯段數, 全部段數)
    let mut progress = use_signal(|| None::<(String, u32, u32)>);

    let on_files = move |e: FormEvent| async move {
        let files = e.files();
        if files.is_empty() {
            return;
        }
        running.set(true);
        // 同一組選項套用到每一本（EP10）
        let (fmt, to_trad, translating, agreed) =
            (format(), s2t(), lang() == "translate", consent());
        for f in files {
            let name = f.name();
            let outcome = match f.read_bytes().await {
                Err(err) => Err(err.to_string()),
                Ok(bytes) if translating => {
                    translate_one(&name, bytes.to_vec(), &fmt, agreed, &mut progress).await
                }
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
            progress.set(None);
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
                    EpubOptions { format, s2t, translate: Some((lang, consent)), disabled: running() }
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
                        if let Some((file, done, total)) = progress() {
                            span { class: "muted", {tf("epub-msg-translating", &[("file", file.as_str()), ("done", done.to_string().as_str()), ("total", total.to_string().as_str())])} }
                        } else if running() {
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
