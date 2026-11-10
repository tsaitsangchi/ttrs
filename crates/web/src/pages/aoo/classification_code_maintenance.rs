//! Classification Code Maintenance 分類碼維護（TipTop aooi300 分類碼共用程式）
//! 版面比照 tiptop/aoo/4fd/aooi300.4fd（CC7）：多筆單身表格（代號、說明、資料有效；類別 G 另有存貨差異科目與成本性質），
//! 上方切換碼類別（CC1）。新增／修改以小視窗編輯。工廠層：用登入的營運中心（ADR 0007）。規則在後端 ClassificationCodeService。

use dioxus::prelude::*;
use ttrs_shared::{
    ClassificationCodeDto, ClassificationCodeInput, QbeCondition, QueryRequest,
    classification_max_len as max_len,
};

use crate::api;
use crate::components::history::HistoryDialog;
use crate::components::material::{
    Button, ButtonVariant, Card, Dialog, Notification, NotificationKind, Select, Tabs, TextField,
    Tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::plant::PlantPicker;
use crate::components::qbe::{
    HINT_NUMBER, ListPager, PAGE_SIZE, Qbe, RecordNav, conditions_of, use_row_nav,
};
use crate::components::status::{
    self, ConfirmCells, ConfirmHeaders, ConfirmQbeCells, HasStatus, StatusCells, StatusHeaders,
    StatusQbeCells, TAB_DATA, TAB_STATUS,
};
use crate::components::toolbar::{Act, MaintenanceToolbar};
use crate::i18n::{t, tf};

/// 碼類別（CC2：料件用到的類別）：（代碼, 翻譯 ID）
const KINDS: [(&str, &str); 10] = [
    ("8", "code-kind-8"),
    ("D", "code-kind-d"),
    ("E", "code-kind-e"),
    ("F", "code-kind-f"),
    ("G", "code-kind-g"),
    // 客製 aooi080 的其他類別（classification_code_other_kinds.feature OC1）
    ("2", "code-kind-2"),
    ("6", "code-kind-6"),
    ("A", "code-kind-a"),
    ("R", "code-kind-r"),
    ("S", "code-kind-s"),
];

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

#[derive(Clone, PartialEq)]
enum Editor {
    New,
    Edit(String),
}

/// 查詢列的欄位：（標題翻譯 ID, 查詢欄位鍵值, 查詢提示翻譯 ID）；後三欄只有類別 G 顯示
const Q_COLUMNS: [(&str, &str, &str); 5] = [
    ("code-field-code", "code", ""),
    ("code-field-description", "description", ""),
    ("code-field-variance-account", "variance_account", ""),
    ("code-field-variance-account2", "variance_account2", ""),
    ("code-field-cost-nature", "cost_nature", HINT_NUMBER),
];

/// 權限檢查用的程式代號
const PROGRAM: &str = "classification-code-maintenance";

#[component]
pub fn ClassificationCodeMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, ClassificationCodePage {} }
    }
}

#[component]
fn ClassificationCodePage() -> Element {
    let perm = use_context::<Perm>();
    let plant = use_signal(String::new);
    let mut kind = use_signal(|| "D".to_owned());
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    // 查詢：qbe 為查詢列正在輸入的條件，conditions 為目前生效的條件（QBE4：預設查全部）
    let mut querying = use_signal(|| false);
    let mut qbe: Qbe = use_signal(Default::default);
    let mut conditions = use_signal(Vec::<QbeCondition>::new);
    let mut page = use_signal(|| 1u32);
    let mut codes = use_resource(move || {
        let p = plant();
        let k = kind();
        let req = QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        async move {
            if p.is_empty() {
                return Ok(ttrs_shared::Page {
                    items: Vec::new(),
                    total: 0,
                    page: 1,
                    page_size: PAGE_SIZE,
                });
            }
            api::query_codes(&p, &k, &req).await
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut copy_code = use_signal(String::new);
    let nav = use_row_nav(page, selected, move || {
        codes
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|p| p.items.iter().map(|c| c.code.clone()).collect())
            .unwrap_or_default()
    });
    let mut editor = use_signal(|| None::<Editor>);
    let mut form = use_signal(ClassificationCodeInput::default);
    let mut dialog_error = use_signal(|| None::<String>);
    // 異動記錄對話框（ADR 0012 §8）：（鍵值, 記錄）
    let mut history = use_signal(|| None::<(String, Vec<ttrs_shared::ChangeDto>)>);

    let page_data = codes.read().as_ref().and_then(|r| r.as_ref().ok()).cloned();
    let list: Vec<ClassificationCodeDto> = page_data
        .as_ref()
        .map(|p| p.items.clone())
        .unwrap_or_default();
    let total = page_data.as_ref().map_or(0, |p| p.total);
    let pages = page_data.as_ref().map_or(1, |p| {
        u32::try_from(p.total.div_ceil(u64::from(p.page_size.max(1))))
            .unwrap_or(1)
            .max(1)
    });
    let filtered = !conditions.read().is_empty();
    let cur: Option<ClassificationCodeDto> = selected()
        .and_then(|c| list.iter().find(|x| x.code == c).cloned())
        .or_else(|| list.first().cloned());
    let no_cur = cur.is_none();
    let keys: Vec<String> = list.iter().map(|c| c.code.clone()).collect();
    let cost = kind() == "G";
    let col_tab = use_signal(|| 0usize);
    let status_tab = col_tab() == 1;
    let cols = if status_tab {
        2 + status::ALL.len() + 2
    } else if cost {
        5
    } else {
        2
    };

    let mut done = move |msg: String, code: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        editor.set(None);
        dialog_error.set(None);
        selected.set(code);
        codes.restart();
    };

    // 執行查詢：先試查第 1 頁，條件有誤時留在查詢列顯示錯誤
    let run_query = move |_| {
        let conds = conditions_of(&qbe.read());
        let req = QueryRequest {
            conditions: conds.clone(),
            page: Some(1),
            page_size: Some(PAGE_SIZE),
            lang: None,
        };
        let p = plant();
        let k = kind();
        spawn(async move {
            match api::query_codes(&p, &k, &req).await {
                Ok(result) => {
                    error.set(None);
                    querying.set(false);
                    selected.set(None);
                    conditions.set(conds);
                    page.set(1);
                    message.set(Some(if result.total == 0 {
                        t("query-msg-none")
                    } else {
                        tf(
                            "query-msg-matched",
                            &[("count", result.total.to_string().as_str())],
                        )
                    }));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    // 列印、匯出 Excel（ADR 0013 SA3）：目前類別、依目前生效的查詢條件
    let report = move |format: api::ReportFormat| {
        let conds = conditions.read().clone();
        let tab = api::ReportTab::open(format);
        let p = plant();
        let part = format!("classification-codes/{}", kind());
        spawn(async move {
            if let Err(e) = api::list_report(
                &p,
                &part,
                format,
                &conds,
                crate::i18n::current().code(),
                tab,
            )
            .await
            {
                error.set(Some(e));
            }
        });
    };

    let save = move |_| {
        let p = plant();
        let k = kind();
        let target = editor();
        let req = form();
        spawn(async move {
            let result = match target {
                Some(Editor::Edit(code)) => api::update_code(&p, &k, &code, &req).await,
                _ => api::create_code(&p, &k, &req).await,
            };
            match result {
                Ok(c) => done(
                    tf("code-msg-saved", &[("code", c.code.as_str())]),
                    Some(c.code),
                ),
                Err(e) => dialog_error.set(Some(e)),
            }
        });
    };

    let cur_edit = cur.clone();
    let cur_delete = cur.clone();
    let cur_history = cur.clone();
    let cur_toggle = cur.clone();
    let cur_copy = cur.clone();
    let cur_confirm = cur.clone();
    let inactive = cur.as_ref().is_some_and(|c| !c.active);
    let confirmed = cur.as_ref().is_some_and(|c| c.confirmed);
    let v = form();
    let kind_options: Vec<(String, String)> = KINDS
        .iter()
        .map(|(c, id)| ((*c).to_owned(), format!("{c} {}", t(id))))
        .collect();

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("code-breadcrumb")} }
                h2 { {t("code-title")} }
            }
            PlantPicker { plant }
            div { class: "form-grid",
                Select {
                    id: "azf02",
                    label: t("code-field-kind"),
                    options: kind_options,
                    value: kind(),
                    onchange: move |k: String| {
                        message.set(None);
                        error.set(None);
                        selected.set(None);
                        conditions.set(Vec::new());
                        page.set(1);
                        kind.set(k);
                    },
                }
            }

            if querying() {
                div { class: "action-bar",
                    Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                    Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { error.set(None); querying.set(false); }, {t("common-action-cancel")} }
                }
                p { class: "query-hint", {t("query-hint")} }
            } else {
                div { class: "action-bar",
                    MaintenanceToolbar {
                        query: Act::new(true, move |_| { message.set(None); error.set(None); qbe.set(Default::default()); querying.set(true); }),
                        // aooi300 是單身式作業：新增、修改、刪除、停用都用 detail
                        detail_action: "detail",
                        insert: if perm.add_row() { Act::new(true, move |_| {
                            message.set(None);
                            error.set(None);
                            dialog_error.set(None);
                            form.set(ClassificationCodeInput::default());
                            editor.set(Some(Editor::New));
                        }) } else { None },
                        modify: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_edit.clone() else { return };
                            dialog_error.set(None);
                            form.set(c.to_input());
                            editor.set(Some(Editor::Edit(c.code)));
                        }),
                        // 複製（ADR 0013 SA5）：同一類別的新代號輸入在功能列後方
                        copy: Act::new(!no_cur && !copy_code().trim().is_empty(), move |_| {
                            let Some(c) = cur_copy.clone() else { return };
                            let p = plant();
                            let k = kind();
                            let new_code = copy_code();
                            spawn(async move {
                                match api::copy_code(&p, &k, &c.code, new_code.trim()).await {
                                    Ok(r) => {
                                        copy_code.set(String::new());
                                        done(tf("code-msg-copied", &[("from", c.code.as_str()), ("to", r.code.as_str())]), Some(r.code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        // 已確認不可刪除（ADR 0013 SA9-2）
                        delete: if perm.delete_row() { Act::new(!no_cur && !confirmed, move |_| {
                            let Some(c) = cur_delete.clone() else { return };
                            let p = plant();
                            let k = kind();
                            spawn(async move {
                                match api::delete_code(&p, &k, &c.code, c.version).await {
                                    Ok(()) => done(tf("code-msg-deleted", &[("code", c.code.as_str())]), None),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }) } else { None },
                        // 確認／取消確認（ADR 0013 SA9）
                        confirm: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_confirm.clone() else { return };
                            let p = plant();
                            let k = kind();
                            spawn(async move {
                                match api::confirm_code(&p, &k, &c.code, !c.confirmed, c.version).await {
                                    Ok(r) => {
                                        let id = if r.confirmed { "code-msg-confirmed" } else { "code-msg-unconfirmed" };
                                        done(tf(id, &[("code", r.code.as_str())]), Some(r.code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        confirmed,
                        invalid: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_toggle.clone() else { return };
                            let p = plant();
                            let k = kind();
                            spawn(async move {
                                match api::set_code_active(&p, &k, &c.code, !c.active, c.version).await {
                                    Ok(r) => {
                                        let id = if r.active { "code-msg-activated" } else { "code-msg-deactivated" };
                                        done(tf(id, &[("code", r.code.as_str())]), Some(r.code));
                                    }
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        invalid_action: "detail",
                        inactive,
                        invalid_label: Some(("code-action-deactivate", "code-action-activate")),
                        print: Act::new(true, move |_| report(api::ReportFormat::Pdf)),
                        export: Act::new(true, move |_| report(api::ReportFormat::Excel)),
                        history: Act::new(!no_cur, move |_| {
                            let Some(c) = cur_history.clone() else { return };
                            let p = plant();
                            let k = kind();
                            spawn(async move {
                                match api::code_history(&p, &k, &c.code).await {
                                    Ok(h) => history.set(Some((c.code.clone(), h))),
                                    Err(e) => error.set(Some(e)),
                                }
                            });
                        }),
                        if perm.can("reproduce") {
                            TextField { id: "copy-code", label: t("code-field-copy-as"), maxlength: max_len::CODE, value: copy_code(), disabled: no_cur, oninput: move |v| copy_code.set(v) }
                        }
                    }
                }
            }

            // 上下筆：移動選取列，跨頁自動換頁（ADR 0013 SA2）
            if !querying() && total > 0 {
                RecordNav {
                    total,
                    pos: nav.pos(&keys, cur.as_ref().map(|c| c.code.as_str())),
                    ongo: {
                        let keys = keys.clone();
                        move |i| { message.set(None); nav.go(i, &keys); }
                    },
                    onerror: move |e| error.set(Some(e)),
                }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            Card {
                // 資料／狀態頁籤（ADR 0012 §9）
                Tabs { tabs: vec![t(TAB_DATA), t(TAB_STATUS)], active: col_tab }
                div { class: "table-header",
                    h3 { {t("code-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead {
                            tr {
                                th { {t("code-field-code")} }
                                th { {t("code-field-description")} }
                                if status_tab {
                                    StatusHeaders {}
                                    ConfirmHeaders {}
                                } else if cost {
                                    th { {t("code-field-variance-account")} }
                                    th { {t("code-field-variance-account2")} }
                                    th { {t("code-field-cost-nature")} }
                                }
                            }
                            // 查詢列（在表格第一列輸入條件）
                            if querying() {
                                tr {
                                    for (id, field, hint) in Q_COLUMNS.into_iter().take(if cost && !status_tab { Q_COLUMNS.len() } else { 2 }) {
                                        th { key: "{field}",
                                            TextField {
                                                id: "q-{field}",
                                                label: t(id),
                                                placeholder: if hint.is_empty() { String::new() } else { t(hint) },
                                                value: qbe.read().get(field).cloned().unwrap_or_default(),
                                                oninput: move |v: String| { qbe.write().insert(field, v); },
                                            }
                                        }
                                    }
                                    if status_tab {
                                        StatusQbeCells { queryable: status::ALL, qbe }
                                        ConfirmQbeCells { qbe }
                                    }
                                }
                            }
                        }
                        tbody {
                            match &*codes.read() {
                                None => rsx! { tr { td { colspan: cols, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: cols, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: cols, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for c in list.iter().cloned() {
                                        tr {
                                            key: "{c.code}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|x| x.code == c.code) { "md-table__row--selected" },
                                            onclick: {
                                                let code = c.code.clone();
                                                move |_| { message.set(None); error.set(None); selected.set(Some(code.clone())); }
                                            },
                                            td { class: "mono", "{c.code}" }
                                            td { {c.description.clone().unwrap_or_default()} }
                                            if status_tab {
                                                StatusCells { status: c.status() }
                                                ConfirmCells { confirmed: c.confirmed, by: c.confirmed_by.clone() }
                                            } else if cost {
                                                td { class: "mono", {c.variance_account.clone().unwrap_or_default()} }
                                                td { class: "mono", {c.variance_account2.clone().unwrap_or_default()} }
                                                td { {c.cost_nature.map(|n| n.to_string()).unwrap_or_default()} }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
                ListPager { page, pages }
            }

            if let Some((key, changes)) = history() {
                HistoryDialog { record: key, changes, onclose: move |_| history.set(None) }
            }
            if let Some(ed) = editor() {
                Dialog {
                    title: match &ed {
                        Editor::New => t("code-dialog-new"),
                        Editor::Edit(code) => tf("code-dialog-edit", &[("code", code.as_str())]),
                    },
                    oncancel: move |_| { editor.set(None); dialog_error.set(None); },
                    actions: rsx! {
                        Button { icon: icons::CLOSE, tone: Tone::Neutral, variant: ButtonVariant::Text, onclick: move |_| { editor.set(None); dialog_error.set(None); }, {t("common-action-cancel")} }
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {t("common-action-ok")} }
                    },
                    if let Some(msg) = dialog_error() {
                        Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| dialog_error.set(None) }
                    }
                    div { class: "form-grid",
                        TextField { id: "azf01", label: t("code-field-code"), maxlength: max_len::CODE, value: v.code.clone(), readonly: ed != Editor::New, oninput: move |x| form.write().code = x }
                        TextField { id: "azf03", label: t("code-field-description"), maxlength: max_len::DESCRIPTION, value: v.description.clone().unwrap_or_default(), oninput: move |x| form.write().description = opt(x) }
                        if cost {
                            TextField { id: "azf05", label: t("code-field-variance-account"), maxlength: max_len::ACCOUNT, value: v.variance_account.clone().unwrap_or_default(), oninput: move |x| form.write().variance_account = opt(x) }
                            TextField { id: "azf051", label: t("code-field-variance-account2"), maxlength: max_len::ACCOUNT, value: v.variance_account2.clone().unwrap_or_default(), oninput: move |x| form.write().variance_account2 = opt(x) }
                            TextField {
                                id: "azf06",
                                label: t("code-field-cost-nature"),
                                input_type: "number",
                                value: v.cost_nature.map(|n| n.to_string()).unwrap_or_default(),
                                oninput: move |x: String| form.write().cost_nature = x.trim().parse().ok(),
                            }
                        }
                    }
                    if cost {
                        p { class: "muted", {t("code-hint-account")} }
                    }
                }
            }
        }
    }
}
