//! User Maintenance 使用者維護（TipTop p_zx 部門使用者資料維護作業）
//! 版面比照 tiptop/azz/4fd/p_zx.4fd，視覺採 Material 3（components::material）：
//!   動作列（新增 / 修改 / 刪除 / 停用・啟用 / 複製）＋ 上下筆
//!   ＋ 頁籤（Basic Data：group01 代號～選單、group02 營運中心～失效日期；Language；Status；另加可使用營運中心 U7）
//!   ＋ 使用者清單。
//! 查詢比照 TipTop Q.查詢（p_zx_curs CONSTRUCT）：在同一個表單的欄位上輸入 QBE 條件（user_maintenance_query.feature）。
//! 密碼、解鎖等欄位不移植（ADR 0006）；規則在後端 UserService。

use dioxus::prelude::*;
use ttrs_shared::{UserDto, UserInput, user_max_len as max_len};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Chip, Notification, NotificationKind, Select, Tabs,
    TextField,
};
use crate::components::qbe::{
    HINT_DATE, HINT_FLAG, ListPager, Q, RecordNav, conditions_of, use_query_records,
};
use crate::i18n::{self, UiLang, t, tf};

const TABS: [&str; 4] = [
    "user-tab-basic",
    "user-tab-language",
    "user-tab-status",
    "user-tab-plants",
];

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    View,
    New,
    Edit,
    /// 輸入 QBE 條件
    Query,
}

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

#[component]
pub fn UserMaintenance() -> Element {
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut mode = use_signal(|| Mode::View);
    let mut tab = use_signal(|| 0usize);
    let mut rec = use_query_records(|req| async move { api::query_users(&req).await });
    let mut form = use_signal(UserInput::default);
    let mut copy_id = use_signal(String::new);

    let cur: Option<UserDto> = rec.current.read().clone();
    let (total, pages) = rec.totals();
    let filtered = !rec.conditions.read().is_empty();
    let offset = rec.offset();

    // 換筆時表單跟著目前這一筆
    use_effect(move || {
        let c = rec.current.read().clone();
        if *mode.peek() == Mode::View {
            form.set(c.as_ref().map(UserDto::to_input).unwrap_or_default());
        }
    });

    // 存檔或其他異動後：顯示該使用者並重新載入清單
    let mut done = move |msg: String, u: Option<UserDto>| {
        error.set(None);
        message.set(Some(msg));
        mode.set(Mode::View);
        form.set(u.as_ref().map(UserDto::to_input).unwrap_or_default());
        if u.is_none() {
            rec.pos.set(None);
        }
        rec.show(u);
    };

    // 執行查詢：先試查第 1 頁，條件有誤時留在查詢模式顯示錯誤
    let run_query = move |_| {
        let conds = conditions_of(&rec.qbe.read());
        let req = rec.request(conds.clone(), 1);
        spawn(async move {
            match api::query_users(&req).await {
                Ok(page) => {
                    error.set(None);
                    mode.set(Mode::View);
                    rec.apply(conds, page.total);
                    message.set(Some(if page.total == 0 {
                        t("query-msg-none")
                    } else {
                        tf(
                            "query-msg-matched",
                            &[("count", page.total.to_string().as_str())],
                        )
                    }));
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let cur_id = cur.as_ref().map(|u| u.user_id.clone()).unwrap_or_default();
    let save = move |_| {
        let req = form();
        let editing = mode() == Mode::Edit;
        let original = cur_id.clone();
        spawn(async move {
            let result = if editing {
                api::update_user(&original, &req).await
            } else {
                api::create_user(&req).await
            };
            match result {
                Ok(u) => done(tf("user-msg-saved", &[("id", u.user_id.as_str())]), Some(u)),
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let cur_cancel = cur.clone();
    let cur_delete = cur.clone();
    let cur_toggle = cur.clone();
    let cur_copy = cur.clone();
    let m = mode();
    let q = m == Mode::Query;
    let ro = matches!(m, Mode::View | Mode::Query);
    let no_cur = cur.is_none();
    let v = form();
    let lang_options: Vec<(String, String)> = UiLang::ALL
        .iter()
        .map(|l| (l.code().to_owned(), l.native_name().to_owned()))
        .collect();
    let lang_hint = UiLang::ALL
        .iter()
        .map(|l| l.code())
        .collect::<Vec<_>>()
        .join("|");
    let qbe = rec.qbe;

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("user-breadcrumb")} }
                h2 { {t("user-title")} }
            }

            // ----- 動作列 -----
            div { class: "action-bar",
                match m {
                    Mode::New | Mode::Edit => rsx! {
                        Button { onclick: save, {t("common-action-ok")} }
                        Button {
                            variant: ButtonVariant::Text,
                            onclick: move |_| {
                                error.set(None);
                                form.set(cur_cancel.as_ref().map(UserDto::to_input).unwrap_or_default());
                                mode.set(Mode::View);
                            },
                            {t("common-action-cancel")}
                        }
                    },
                    Mode::Query => rsx! {
                        Button { onclick: run_query, {t("common-action-ok")} }
                        Button {
                            variant: ButtonVariant::Text,
                            onclick: move |_| {
                                error.set(None);
                                form.set(rec.current.peek().as_ref().map(UserDto::to_input).unwrap_or_default());
                                mode.set(Mode::View);
                            },
                            {t("common-action-cancel")}
                        }
                    },
                    Mode::View => rsx! {
                        Button {
                            variant: ButtonVariant::Tonal,
                            onclick: move |_| {
                                message.set(None);
                                error.set(None);
                                rec.qbe.set(Default::default());
                                mode.set(Mode::Query);
                                tab.set(0);
                            },
                            {t("common-action-query")}
                        }
                        Button {
                            onclick: move |_| {
                                message.set(None);
                                error.set(None);
                                form.set(UserInput { lang: Some(UiLang::ZhTw.code().into()), ..Default::default() });
                                mode.set(Mode::New);
                            },
                            {t("common-action-new")}
                        }
                        Button { variant: ButtonVariant::Tonal, disabled: no_cur, onclick: move |_| mode.set(Mode::Edit), {t("common-action-edit")} }
                        Button {
                            variant: ButtonVariant::Error,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(u) = cur_delete.clone() else { return };
                                spawn(async move {
                                    match api::delete_user(&u.user_id).await {
                                        Ok(()) => done(tf("user-msg-deleted", &[("id", u.user_id.as_str())]), None),
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            {t("common-action-delete")}
                        }
                        // TipTop 無效切換：有效 → 停用、停用 → 啟用
                        Button {
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(u) = cur_toggle.clone() else { return };
                                spawn(async move {
                                    match api::set_user_active(&u.user_id, !u.active).await {
                                        Ok(r) => {
                                            let id = if r.active { "user-msg-activated" } else { "user-msg-deactivated" };
                                            done(tf(id, &[("id", r.user_id.as_str())]), Some(r));
                                        }
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            if cur.as_ref().is_some_and(|u| !u.active) { {t("user-action-activate")} } else { {t("user-action-deactivate")} }
                        }
                        // 串接 p_zxw「維護使用者特殊權限」（p_zx action maintain_zxw）
                        if let Some(u) = cur.as_ref() {
                            Link {
                                class: "md-button md-state md-button--outlined",
                                to: crate::Route::UserPermissionsFor { user: u.user_id.clone() },
                                {t("user-action-special")}
                            }
                        }
                        div { class: "spacer" }
                        TextField { id: "copy-id", label: t("user-field-copy-as"), maxlength: max_len::USER_ID, value: copy_id(), disabled: no_cur, oninput: move |v| copy_id.set(v) }
                        Button {
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(u) = cur_copy.clone() else { return };
                                let new_id = copy_id();
                                spawn(async move {
                                    match api::copy_user(&u.user_id, &new_id).await {
                                        Ok(r) => {
                                            copy_id.set(String::new());
                                            done(tf("user-msg-copied", &[("from", u.user_id.as_str()), ("to", r.user_id.as_str())]), Some(r));
                                        }
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            {t("common-action-copy")}
                        }
                    },
                }
            }

            if q {
                p { class: "query-hint", {t("query-hint")} }
            }

            if m == Mode::View && total > 0 {
                RecordNav { total, pos: (rec.pos)(), ongo: move |i| rec.go(i), onerror: move |e| error.set(Some(e)) }
            }

            if let Some(msg) = error() {
                Notification { kind: NotificationKind::Error, title: msg, onclose: move |_| error.set(None) }
            }
            if let Some(msg) = message() {
                Notification { kind: NotificationKind::Success, title: msg, onclose: move |_| message.set(None) }
            }

            Card {
                Tabs { tabs: TABS.iter().map(|s| t(s)).collect::<Vec<_>>(), active: tab }
                div { class: "card-body",
                    match tab() {
                        0 => rsx! {
                            div { class: "user-groups",
                                // p_zx.4fd group01：代號、名稱、部門、權限類別、選單
                                fieldset { class: "user-group",
                                    Q { field: "user_id", label: t("user-field-user-id"), query: q, qbe,
                                        TextField { id: "zx01", label: t("user-field-user-id"), maxlength: max_len::USER_ID, value: v.user_id.clone(), readonly: m != Mode::New, oninput: move |x| form.write().user_id = x }
                                    }
                                    Q { field: "name", label: t("user-field-name"), query: q, qbe,
                                        TextField { id: "zx02", label: t("user-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().name = opt(x) }
                                    }
                                    Q { field: "dept_code", label: t("user-field-dept"), query: q, qbe,
                                        TextField { id: "zx03", label: t("user-field-dept"), maxlength: max_len::DEPT, value: v.dept_code.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().dept_code = opt(x) }
                                    }
                                    Q { field: "permission_class", label: t("user-field-class"), query: q, qbe,
                                        TextField { id: "zx04", label: t("user-field-class"), value: v.permission_class.clone(), readonly: ro, oninput: move |x| form.write().permission_class = x }
                                    }
                                    Q { field: "menu", label: t("user-field-menu"), query: q, qbe,
                                        TextField { id: "zx05", label: t("user-field-menu"), value: v.menu.clone(), readonly: ro, oninput: move |x| form.write().menu = x }
                                    }
                                }
                                // p_zx.4fd group02：多營運中心、預設營運中心、Email、公司帳號、失效日期
                                fieldset { class: "user-group",
                                    Q { field: "multi_plant", label: t("user-field-multi-plant"), hint: HINT_FLAG, query: q, qbe,
                                        Checkbox { id: "zx07", label: t("user-field-multi-plant"), checked: v.multi_plant, readonly: ro, onchange: move |x| form.write().multi_plant = x }
                                    }
                                    Q { field: "default_plant", label: t("user-field-default-plant"), query: q, qbe,
                                        TextField { id: "zx08", label: t("user-field-default-plant"), value: v.default_plant.clone(), readonly: ro, oninput: move |x| form.write().default_plant = x }
                                    }
                                    Q { field: "email", label: t("user-field-email"), query: q, qbe,
                                        TextField { id: "zx09", label: t("user-field-email"), maxlength: max_len::EMAIL, value: v.email.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().email = opt(x) }
                                    }
                                    Q { field: "idp_username", label: t("user-field-idp-username"), query: q, qbe,
                                        TextField { id: "idp", label: t("user-field-idp-username"), maxlength: max_len::IDP_USERNAME, placeholder: t("user-hint-idp-username"), value: v.idp_username.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().idp_username = opt(x) }
                                    }
                                    Q { field: "valid_until", label: t("user-field-valid-until"), hint: HINT_DATE, query: q, qbe,
                                        TextField {
                                            id: "zx18",
                                            label: t("user-field-valid-until"),
                                            input_type: "date",
                                            value: v.valid_until.map(|d| d.to_string()).unwrap_or_default(),
                                            readonly: ro,
                                            oninput: move |x: String| form.write().valid_until = ttrs_shared::NaiveDate::parse_from_str(&x, "%Y-%m-%d").ok(),
                                        }
                                    }
                                }
                            }
                        },
                        1 => rsx! {
                            div { class: "form-grid cols-4",
                                Q { field: "lang", label: t("user-field-lang"), hint_text: lang_hint.clone(), query: q, qbe,
                                    Select { id: "zx06", label: t("user-field-lang"), options: lang_options.clone(), value: v.lang.clone().unwrap_or_else(|| UiLang::ZhTw.code().into()), readonly: ro, onchange: move |x| form.write().lang = Some(x) }
                                }
                            }
                            p { class: "muted", {t("user-hint-lang")} }
                        },
                        2 => rsx! {
                            div { class: "form-grid cols-4",
                                Q { field: "active", label: t("user-field-active"), hint: HINT_FLAG, query: q, qbe,
                                    TextField { id: "zxacti", label: t("user-field-active"), readonly: true, value: cur.as_ref().map(|u| if u.active { t("user-status-active") } else { t("user-status-inactive") }).unwrap_or_default() }
                                }
                                if !q {
                                    TextField { id: "idp-bound", label: t("user-field-idp-bound"), readonly: true, value: cur.as_ref().map(|u| if u.idp_bound { t("user-idp-bound") } else { t("user-idp-unbound") }).unwrap_or_default() }
                                }
                                Q { field: "created_by", label: t("user-field-created-by"), query: q, qbe,
                                    TextField { id: "zxuser", label: t("user-field-created-by"), readonly: true, value: cur.as_ref().map(|u| u.created_by.clone()).unwrap_or_default() }
                                }
                                Q { field: "created_dept", label: t("user-field-created-dept"), query: q, qbe,
                                    TextField { id: "zxgrup", label: t("user-field-created-dept"), readonly: true, value: cur.as_ref().map(|u| u.created_dept.clone()).unwrap_or_default() }
                                }
                                Q { field: "updated_by", label: t("user-field-updated-by"), query: q, qbe,
                                    TextField { id: "zxmodu", label: t("user-field-updated-by"), readonly: true, value: cur.as_ref().and_then(|u| u.updated_by.clone()).unwrap_or_default() }
                                }
                                Q { field: "updated_on", label: t("user-field-updated-on"), hint: HINT_DATE, query: q, qbe,
                                    TextField { id: "zxdate", label: t("user-field-updated-on"), readonly: true, value: cur.as_ref().and_then(|u| u.updated_on.map(|d| d.format("%Y/%m/%d").to_string())).unwrap_or_default() }
                                }
                            }
                        },
                        _ if q => rsx! {
                            p { class: "muted", {t("query-msg-tab-not-queryable")} }
                        },
                        _ => rsx! {
                            p { class: "muted", {t("user-hint-plants")} }
                            table { class: "md-table",
                                thead { tr { th { {t("user-col-plant")} } th { {t("user-col-default")} } } }
                                tbody {
                                    for p in cur.as_ref().map(|u| u.allowed_plants.clone()).unwrap_or_default() {
                                        tr { key: "{p}",
                                            td { class: "mono", "{p}" }
                                            td { if cur.as_ref().is_some_and(|u| u.default_plant == p) { "✓" } }
                                        }
                                    }
                                }
                            }
                        },
                    }
                }
            }

            // ----- 使用者清單 -----
            Card {
                div { class: "table-header",
                    h3 { {t("user-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for id in ["user-field-user-id", "user-field-name", "user-field-dept", "user-field-class", "user-field-default-plant", "user-field-active"] {
                                th { key: "{id}", {t(id)} }
                            }
                        } }
                        tbody {
                            match &*rec.list.read() {
                                None => rsx! { tr { td { colspan: 6, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 6, "{e}" } } },
                                Some(Ok(page)) if page.items.is_empty() => rsx! { tr { td { colspan: 6, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(page)) => rsx! {
                                    for (i, u) in page.items.iter().cloned().enumerate() {
                                        tr {
                                            key: "{u.user_id}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|c| c.user_id == u.user_id) { "md-table__row--selected" },
                                            onclick: move |_| if mode() == Mode::View { message.set(None); error.set(None); rec.go(offset + i as u64); },
                                            td { class: "mono", "{u.user_id}" }
                                            td { {u.name.clone().unwrap_or_default()} }
                                            td { {u.dept_code.clone().unwrap_or_default()} }
                                            td { "{u.permission_class}" }
                                            td { class: "mono", "{u.default_plant}" }
                                            td {
                                                if u.active { Chip { label: t("user-status-active") } } else { Chip { label: t("user-status-inactive"), warning: true } }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
                ListPager { page: rec.page, pages }
            }
            p { class: "note", {i18n::t("user-note-password")} }
        }
    }
}
