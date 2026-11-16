//! EPUB 搜尋（TTRS 工具作業；specs/features/azz/epub_web_search.feature、github_epub_search.feature）
//! 關鍵字同時查 Internet Archive、Project Gutenberg、Wikisource、GitHub，繁體中文排最前面（WS3）；
//! 書可直接下載原始 EPUB、或轉成 TXT／PDF（可簡轉繁、英文翻譯）；GitHub 結果是 repo，按「列出 EPUB」再選檔案（GE3）。

use dioxus::prelude::*;
use ttrs_shared::{EpubBookDto, EpubSearchPage, GithubEpubListDto};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Notification, NotificationKind, TextField, Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::i18n::{t, tf};
use crate::pages::azz::epub_to_text::{EpubOptions, wait_translation};

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

/// 文字種類的顯示（WS3、WS4）
fn script_label(script: &str) -> String {
    t(match script {
        "traditional" => "github-script-traditional",
        "simplified" => "github-script-simplified",
        "other_chinese" => "github-script-other-chinese",
        _ => "github-script-other",
    })
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
    let mut keyword = use_signal(String::new);
    let mut open_only = use_signal(|| false);
    let mut page = use_signal(|| 1u32);
    let mut results = use_signal(|| None::<EpubSearchPage>);
    let mut files = use_signal(|| None::<GithubEpubListDto>);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut book_message = use_signal(|| None::<String>);
    // 轉換結果顯示在 EPUB 清單裡（清單在頁面下方，訊息放上方會看不到）
    let mut file_error = use_signal(|| None::<String>);
    let format = use_signal(|| "txt".to_owned());
    let s2t = use_signal(|| false);
    let lang = use_signal(|| "none".to_owned());
    let consent = use_signal(|| false);
    // 翻譯進度：(書名, 已翻譯段數, 全部段數)
    let mut progress = use_signal(|| None::<(String, u32, u32)>);

    let mut search = move |p: u32| {
        let (q, open) = (keyword(), open_only());
        if q.trim().is_empty() {
            return;
        }
        busy.set(true);
        spawn(async move {
            match api::epub_search(q.trim(), open, p).await {
                Ok(r) => {
                    error.set(None);
                    page.set(p);
                    files.set(None);
                    results.set(Some(r));
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
        let (fmt, to_trad) = (format(), s2t());
        spawn(async move {
            match api::github_epub_convert(&repo, &path, &fmt, to_trad).await {
                Ok(r) => match api::download_file(&r.file_name, &r.bytes, r.mime) {
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

    // 下載原始 EPUB（WS8）
    let mut download_epub = move |book: EpubBookDto| {
        busy.set(true);
        book_message.set(None);
        error.set(None);
        spawn(async move {
            match api::epub_search_download(&book.source, &book.id, &book.title).await {
                Ok(r) => match api::download_file(&r.file_name, &r.bytes, r.mime) {
                    Ok(()) => book_message.set(Some(tf(
                        "github-msg-downloaded",
                        &[("file", r.file_name.as_str())],
                    ))),
                    Err(e) => error.set(Some(e)),
                },
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    // 轉換搜尋結果中的一本；選了英文翻譯時提交翻譯工作並等它完成（EL4）
    let mut convert_book = move |book: EpubBookDto| {
        busy.set(true);
        book_message.set(None);
        error.set(None);
        let (fmt, to_trad, translating, agreed) =
            (format(), s2t(), lang() == "translate", consent());
        spawn(async move {
            let outcome =
                match api::epub_search_convert(&book, &fmt, to_trad, translating, agreed).await {
                    Ok(api::Converted::File(f)) => Ok(f),
                    Ok(api::Converted::Job(id)) => {
                        wait_translation(&id, &book.title, &fmt, &mut progress).await
                    }
                    Err(e) => Err(e),
                };
            progress.set(None);
            match outcome {
                Ok(r) => match api::download_file(&r.file_name, &r.bytes, r.mime) {
                    Ok(()) => book_message.set(Some(tf(
                        "github-msg-done",
                        &[
                            ("file", r.file_name.as_str()),
                            ("title", r.title.clone().unwrap_or_default().as_str()),
                            ("chapters", r.chapters.to_string().as_str()),
                        ],
                    ))),
                    Err(e) => error.set(Some(e)),
                },
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    let result = results();
    let pages = result.as_ref().map_or(1, |r| {
        u32::try_from(r.total.div_ceil(30)).unwrap_or(1).max(1)
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
            // 轉換選項：搜尋結果與貼上網址共用
            if perm.can("run") {
                Card {
                    div { class: "card-body",
                        EpubOptions { format, s2t, translate: Some((lang, consent)), disabled: busy() }
                    }
                }
            }
            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = book_message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| book_message.set(None) }
            }
            if let Some((book, done, total)) = progress() {
                p { class: "query-hint", {tf("epub-msg-translating", &[("file", book.as_str()), ("done", done.to_string().as_str()), ("total", total.to_string().as_str())])} }
            }
            if let Some(r) = result {
                for w in r.warnings.clone() {
                    p { class: "query-hint", {tf("github-warn-source", &[("source", w.as_str())])} }
                }
                Card {
                    div { class: "table-header",
                        h3 { {t("github-repos-title")} }
                        p { {tf("github-repos-count", &[("total", r.total.to_string().as_str()), ("page", page().to_string().as_str()), ("pages", pages.to_string().as_str())])} }
                    }
                    div { class: "table-scroll",
                        table { class: "md-table",
                            thead { tr {
                                th { {t("github-col-title")} }
                                th { {t("github-col-script")} }
                                th { {t("github-col-source")} }
                                th { {t("github-col-license")} }
                                th { {t("github-col-downloads")} }
                                th {}
                            } }
                            tbody {
                                if r.items.is_empty() {
                                    tr { td { colspan: 6, class: "muted", {t("common-msg-no-data")} } }
                                }
                                for book in r.items.clone() {
                                    tr { key: "{book.source}/{book.id}",
                                        td {
                                            if let Some(u) = book.url.clone() {
                                                a { href: "{u}", target: "_blank", rel: "noopener", "{book.title}" }
                                            } else {
                                                "{book.title}"
                                            }
                                            if let Some(a) = book.author.clone() {
                                                div { class: "muted", "{a}" }
                                            }
                                        }
                                        td { {script_label(&book.script)} }
                                        td {
                                            "{book.source}"
                                            if !book.other_sources.is_empty() {
                                                div { class: "muted", {tf("github-others", &[("sources", book.other_sources.join(", ").as_str())])} }
                                            }
                                        }
                                        td {
                                            {book.license.clone().unwrap_or_else(|| t("github-license-none"))}
                                            if !book.open_license {
                                                div { class: "query-hint", {t("github-license-check")} }
                                            }
                                        }
                                        td { class: "num", {book.downloads.map_or_else(String::new, |n| n.to_string())} }
                                        td {
                                            if book.kind == "repo" {
                                                Button { icon: icons::SEARCH, tone: Tone::Primary, variant: ButtonVariant::Text, disabled: busy(), onclick: { let n = book.id.clone(); move |_| list_files(n.clone()) }, {t("github-action-files")} }
                                            } else if perm.can("run") {
                                                Button { icon: icons::DOWNLOAD, tone: Tone::Neutral, variant: ButtonVariant::Text, disabled: busy(), onclick: { let b = book.clone(); move |_| download_epub(b.clone()) }, {t("github-action-download")} }
                                                Button { icon: icons::DOWNLOAD, tone: Tone::Success, variant: ButtonVariant::Tonal, disabled: busy(), onclick: { let b = book.clone(); move |_| convert_book(b.clone()) }, {t("github-action-convert")} }
                                            }
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
