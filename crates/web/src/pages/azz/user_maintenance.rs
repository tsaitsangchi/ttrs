//! User Maintenance 使用者維護（TipTop p_zx 部門使用者資料維護作業）
//! 版面比照 tiptop/azz/4fd/p_zx.4fd，視覺採 Material 3（components::material）：
//!   動作列（新增 / 修改 / 刪除 / 停用・啟用 / 複製）＋ 上下筆
//!   ＋ 頁籤（Basic Data：group01 代號～選單、group02 營運中心～失效日期；Language；Status；另加可使用營運中心 U7）
//!   ＋ 使用者清單。
//! 密碼、解鎖等欄位不移植（ADR 0006）；規則在後端 UserService。

use dioxus::prelude::*;
use ttrs_shared::{UserDto, UserInput, user_max_len as max_len};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Checkbox, Chip, IconButton, Notification, NotificationKind,
    Select, Tabs, TextField, icons,
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
}

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

#[component]
pub fn UserMaintenance() -> Element {
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut mode = use_signal(|| Mode::View);
    let tab = use_signal(|| 0usize);
    let mut users = use_resource(api::list_users);
    let mut filter = use_signal(String::new);
    let mut pos = use_signal(|| None::<usize>);
    let mut form = use_signal(UserInput::default);
    let mut copy_id = use_signal(String::new);

    let all: Vec<UserDto> = users
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let f = filter().trim().to_lowercase();
    let list: Vec<UserDto> = all
        .iter()
        .filter(|u| {
            f.is_empty()
                || u.user_id.to_lowercase().contains(&f)
                || u.name.as_deref().unwrap_or("").to_lowercase().contains(&f)
        })
        .cloned()
        .collect();
    let total = list.len();
    let cur: Option<UserDto> = pos().and_then(|i| list.get(i).cloned());

    // 換筆或清單重新載入時，表單跟著目前這一筆（預設停在第一筆）
    use_effect(move || {
        let list: Vec<UserDto> = match &*users.read() {
            Some(Ok(all)) => all.clone(),
            _ => return,
        };
        if pos().is_none() && !list.is_empty() {
            pos.set(Some(0));
            return;
        }
        if *mode.peek() == Mode::View {
            form.set(
                pos()
                    .and_then(|i| list.get(i))
                    .map(UserDto::to_input)
                    .unwrap_or_default(),
            );
        }
    });

    // 存檔或其他異動後重新載入，並停在該使用者
    let mut done = move |msg: String, id: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        mode.set(Mode::View);
        filter.set(String::new());
        users.restart();
        spawn(async move {
            if let Ok(list) = api::list_users().await {
                pos.set(match id {
                    Some(id) => list.iter().position(|u| u.user_id == id),
                    None => (!list.is_empty()).then_some(0),
                });
                if let Some(u) = pos().and_then(|i| list.get(i)) {
                    form.set(u.to_input());
                }
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
                Ok(u) => done(
                    tf("user-msg-saved", &[("id", u.user_id.as_str())]),
                    Some(u.user_id),
                ),
                Err(e) => error.set(Some(e)),
            }
        });
    };

    let cur_cancel = cur.clone();
    let cur_delete = cur.clone();
    let cur_toggle = cur.clone();
    let cur_copy = cur.clone();
    let m = mode();
    let ro = m == Mode::View;
    let no_cur = cur.is_none();
    let v = form();
    let lang_options: Vec<(String, String)> = UiLang::ALL
        .iter()
        .map(|l| (l.code().to_owned(), l.native_name().to_owned()))
        .collect();

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
                    Mode::View => rsx! {
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
                                            done(tf(id, &[("id", r.user_id.as_str())]), Some(r.user_id));
                                        }
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            if cur.as_ref().is_some_and(|u| !u.active) { {t("user-action-activate")} } else { {t("user-action-deactivate")} }
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
                                            done(tf("user-msg-copied", &[("from", u.user_id.as_str()), ("to", r.user_id.as_str())]), Some(r.user_id));
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

            if m == Mode::View && total > 0 {
                div { class: "record-nav",
                    IconButton { label: t("nav-first"), path: icons::FIRST_PAGE, disabled: pos() == Some(0), onclick: move |_| pos.set(Some(0)) }
                    IconButton { label: t("nav-previous"), path: icons::CHEVRON_LEFT, disabled: pos().is_none_or(|i| i == 0), onclick: move |_| pos.set(pos().map(|i| i.saturating_sub(1))) }
                    span { class: "record-nav__count",
                        match pos() {
                            Some(i) => rsx! { {tf("nav-record-position", &[("pos", (i + 1).to_string().as_str()), ("total", total.to_string().as_str())])} },
                            None => rsx! { {tf("nav-record-count", &[("total", total.to_string().as_str())])} },
                        }
                    }
                    IconButton { label: t("nav-next"), path: icons::CHEVRON_RIGHT, disabled: pos().is_some_and(|i| i + 1 >= total), onclick: move |_| pos.set(Some(pos().map_or(0, |i| (i + 1).min(total - 1)))) }
                    IconButton { label: t("nav-last"), path: icons::LAST_PAGE, disabled: pos() == Some(total - 1), onclick: move |_| pos.set(Some(total - 1)) }
                }
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
                                    TextField { id: "zx01", label: t("user-field-user-id"), maxlength: max_len::USER_ID, value: v.user_id.clone(), readonly: m != Mode::New, oninput: move |x| form.write().user_id = x }
                                    TextField { id: "zx02", label: t("user-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().name = opt(x) }
                                    TextField { id: "zx03", label: t("user-field-dept"), maxlength: max_len::DEPT, value: v.dept_code.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().dept_code = opt(x) }
                                    TextField { id: "zx04", label: t("user-field-class"), value: v.permission_class.clone(), readonly: ro, oninput: move |x| form.write().permission_class = x }
                                    TextField { id: "zx05", label: t("user-field-menu"), value: v.menu.clone(), readonly: ro, oninput: move |x| form.write().menu = x }
                                }
                                // p_zx.4fd group02：多營運中心、預設營運中心、Email、公司帳號、失效日期
                                fieldset { class: "user-group",
                                    Checkbox { id: "zx07", label: t("user-field-multi-plant"), checked: v.multi_plant, readonly: ro, onchange: move |x| form.write().multi_plant = x }
                                    TextField { id: "zx08", label: t("user-field-default-plant"), value: v.default_plant.clone(), readonly: ro, oninput: move |x| form.write().default_plant = x }
                                    TextField { id: "zx09", label: t("user-field-email"), maxlength: max_len::EMAIL, value: v.email.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().email = opt(x) }
                                    TextField { id: "idp", label: t("user-field-idp-username"), maxlength: max_len::IDP_USERNAME, placeholder: t("user-hint-idp-username"), value: v.idp_username.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().idp_username = opt(x) }
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
                        },
                        1 => rsx! {
                            div { class: "form-grid cols-4",
                                Select { id: "zx06", label: t("user-field-lang"), options: lang_options.clone(), value: v.lang.clone().unwrap_or_else(|| UiLang::ZhTw.code().into()), readonly: ro, onchange: move |x| form.write().lang = Some(x) }
                            }
                            p { class: "muted", {t("user-hint-lang")} }
                        },
                        2 => rsx! {
                            div { class: "form-grid cols-4",
                                TextField { id: "zxacti", label: t("user-field-active"), readonly: true, value: cur.as_ref().map(|u| if u.active { t("user-status-active") } else { t("user-status-inactive") }).unwrap_or_default() }
                                TextField { id: "idp-bound", label: t("user-field-idp-bound"), readonly: true, value: cur.as_ref().map(|u| if u.idp_bound { t("user-idp-bound") } else { t("user-idp-unbound") }).unwrap_or_default() }
                                TextField { id: "zxuser", label: t("user-field-created-by"), readonly: true, value: cur.as_ref().map(|u| u.created_by.clone()).unwrap_or_default() }
                                TextField { id: "zxgrup", label: t("user-field-created-dept"), readonly: true, value: cur.as_ref().map(|u| u.created_dept.clone()).unwrap_or_default() }
                                TextField { id: "zxmodu", label: t("user-field-updated-by"), readonly: true, value: cur.as_ref().and_then(|u| u.updated_by.clone()).unwrap_or_default() }
                                TextField { id: "zxdate", label: t("user-field-updated-on"), readonly: true, value: cur.as_ref().and_then(|u| u.updated_on.map(|d| d.format("%Y/%m/%d").to_string())).unwrap_or_default() }
                            }
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
                    p { {tf("list-count-all", &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "card-body",
                    TextField { id: "user-filter", class: "legacy-search", label: t("user-field-filter"), value: filter(), oninput: move |x| { filter.set(x); pos.set(Some(0)); } }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for id in ["user-field-user-id", "user-field-name", "user-field-dept", "user-field-class", "user-field-default-plant", "user-field-active"] {
                                th { key: "{id}", {t(id)} }
                            }
                        } }
                        tbody {
                            match &*users.read() {
                                None => rsx! { tr { td { colspan: 6, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 6, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: 6, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for (i, u) in list.iter().cloned().enumerate() {
                                        tr {
                                            key: "{u.user_id}",
                                            class: "md-table__row--clickable",
                                            class: if pos() == Some(i) { "md-table__row--selected" },
                                            onclick: move |_| if mode() == Mode::View { message.set(None); error.set(None); pos.set(Some(i)); },
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
            }
            p { class: "note", {i18n::t("user-note-password")} }
        }
    }
}
