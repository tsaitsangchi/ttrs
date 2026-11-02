//! Plant Maintenance 營運中心維護（TipTop aooi901 工廠資料建立）
//! 版面比照 tiptop/aoo/4fd/aooi901.4fd（P12）：動作列（新增 / 修改 / 刪除 / 停用・啟用 / 複製）＋ 上下筆
//!   ＋ 單筆表單（group01：代碼、名稱、時區、地址、電話、傳真；另加狀態與建立／修改資訊）＋ 營運中心清單。
//! 資料庫連線欄位（schema、DB link、MSSQL IP）與上線人數不移植（P1、P8）；規則在後端 PlantService。

use dioxus::prelude::*;
use ttrs_shared::{PlantDto, PlantInput, plant_max_len as max_len};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Chip, IconButton, Notification, NotificationKind, Select,
    TextField, icons,
};
use crate::i18n::{t, tf};

/// 時區下拉選項（IANA 名稱）；目前值不在清單時另外加入，轉入的舊資料不會被改掉
const TIME_ZONES: [&str; 25] = [
    "Asia/Taipei",
    "Asia/Shanghai",
    "Asia/Hong_Kong",
    "Asia/Macau",
    "Asia/Singapore",
    "Asia/Kuala_Lumpur",
    "Asia/Bangkok",
    "Asia/Ho_Chi_Minh",
    "Asia/Jakarta",
    "Asia/Manila",
    "Asia/Tokyo",
    "Asia/Seoul",
    "Asia/Kolkata",
    "Asia/Dubai",
    "Europe/London",
    "Europe/Paris",
    "Europe/Berlin",
    "America/New_York",
    "America/Chicago",
    "America/Denver",
    "America/Los_Angeles",
    "America/Mexico_City",
    "America/Sao_Paulo",
    "Australia/Sydney",
    "UTC",
];

const DEFAULT_TIME_ZONE: &str = "Asia/Taipei";

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
pub fn PlantMaintenance() -> Element {
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut mode = use_signal(|| Mode::View);
    let mut plants = use_resource(api::list_plants);
    let mut filter = use_signal(String::new);
    let mut pos = use_signal(|| None::<usize>);
    let mut form = use_signal(PlantInput::default);
    let mut copy_id = use_signal(String::new);

    let all: Vec<PlantDto> = plants
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .cloned()
        .unwrap_or_default();
    let f = filter().trim().to_lowercase();
    let list: Vec<PlantDto> = all
        .iter()
        .filter(|p| {
            f.is_empty()
                || p.plant_id.to_lowercase().contains(&f)
                || p.name.to_lowercase().contains(&f)
        })
        .cloned()
        .collect();
    let total = list.len();
    let cur: Option<PlantDto> = pos().and_then(|i| list.get(i).cloned());

    // 換筆或清單重新載入時，表單跟著目前這一筆（預設停在第一筆）
    use_effect(move || {
        let list: Vec<PlantDto> = match &*plants.read() {
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
                    .map(PlantDto::to_input)
                    .unwrap_or_default(),
            );
        }
    });

    // 存檔或其他異動後重新載入，並停在該營運中心
    let mut done = move |msg: String, id: Option<String>| {
        error.set(None);
        message.set(Some(msg));
        mode.set(Mode::View);
        filter.set(String::new());
        plants.restart();
        spawn(async move {
            if let Ok(list) = api::list_plants().await {
                pos.set(match id {
                    Some(id) => list.iter().position(|p| p.plant_id == id),
                    None => (!list.is_empty()).then_some(0),
                });
                if let Some(p) = pos().and_then(|i| list.get(i)) {
                    form.set(p.to_input());
                }
            }
        });
    };

    let cur_id = cur.as_ref().map(|p| p.plant_id.clone()).unwrap_or_default();
    let save = move |_| {
        let req = form();
        let editing = mode() == Mode::Edit;
        let original = cur_id.clone();
        spawn(async move {
            let result = if editing {
                api::update_plant(&original, &req).await
            } else {
                api::create_plant(&req).await
            };
            match result {
                Ok(p) => done(
                    tf("plant-msg-saved", &[("id", p.plant_id.as_str())]),
                    Some(p.plant_id),
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
    let tz = v
        .time_zone
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_TIME_ZONE.to_owned());
    let mut tz_options: Vec<(String, String)> = TIME_ZONES
        .iter()
        .map(|z| ((*z).to_owned(), (*z).to_owned()))
        .collect();
    if !TIME_ZONES.contains(&tz.as_str()) {
        tz_options.insert(0, (tz.clone(), tz.clone()));
    }

    rsx! {
        div { class: "page",
            div { class: "page-header",
                div { class: "program-id", {t("plant-breadcrumb")} }
                h2 { {t("plant-title")} }
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
                                form.set(cur_cancel.as_ref().map(PlantDto::to_input).unwrap_or_default());
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
                                form.set(PlantInput { time_zone: Some(DEFAULT_TIME_ZONE.into()), ..Default::default() });
                                mode.set(Mode::New);
                            },
                            {t("common-action-new")}
                        }
                        Button { variant: ButtonVariant::Tonal, disabled: no_cur, onclick: move |_| mode.set(Mode::Edit), {t("common-action-edit")} }
                        Button {
                            variant: ButtonVariant::Error,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(p) = cur_delete.clone() else { return };
                                spawn(async move {
                                    match api::delete_plant(&p.plant_id).await {
                                        Ok(()) => done(tf("plant-msg-deleted", &[("id", p.plant_id.as_str())]), None),
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            {t("common-action-delete")}
                        }
                        // 有效 → 停用、停用 → 啟用（P9）
                        Button {
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(p) = cur_toggle.clone() else { return };
                                spawn(async move {
                                    match api::set_plant_active(&p.plant_id, !p.active).await {
                                        Ok(r) => {
                                            let id = if r.active { "plant-msg-activated" } else { "plant-msg-deactivated" };
                                            done(tf(id, &[("id", r.plant_id.as_str())]), Some(r.plant_id));
                                        }
                                        Err(e) => error.set(Some(e)),
                                    }
                                });
                            },
                            if cur.as_ref().is_some_and(|p| !p.active) { {t("plant-action-activate")} } else { {t("plant-action-deactivate")} }
                        }
                        div { class: "spacer" }
                        TextField { id: "copy-id", label: t("plant-field-copy-as"), maxlength: max_len::ID, value: copy_id(), disabled: no_cur, oninput: move |v| copy_id.set(v) }
                        Button {
                            variant: ButtonVariant::Outlined,
                            disabled: no_cur,
                            onclick: move |_| {
                                let Some(p) = cur_copy.clone() else { return };
                                let new_id = copy_id();
                                spawn(async move {
                                    match api::copy_plant(&p.plant_id, &new_id).await {
                                        Ok(r) => {
                                            copy_id.set(String::new());
                                            done(tf("plant-msg-copied", &[("from", p.plant_id.as_str()), ("to", r.plant_id.as_str())]), Some(r.plant_id));
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

            // ----- 單筆表單（aooi901.4fd group01） -----
            Card {
                div { class: "card-body",
                    div { class: "form-grid",
                        TextField { id: "azp01", label: t("plant-field-id"), maxlength: max_len::ID, value: v.plant_id.clone(), readonly: m != Mode::New, oninput: move |x| form.write().plant_id = x }
                        TextField { id: "azp02", label: t("plant-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().name = opt(x) }
                        Select { id: "azp052", label: t("plant-field-time-zone"), options: tz_options, value: tz, readonly: ro, onchange: move |x| form.write().time_zone = Some(x) }
                        TextField { id: "azp06", label: t("plant-field-phone"), maxlength: max_len::PHONE, value: v.phone.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().phone = opt(x) }
                        TextField { id: "azp051", class: "span-2", label: t("plant-field-address"), maxlength: max_len::ADDRESS, value: v.address.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().address = opt(x) }
                        TextField { id: "azp07", label: t("plant-field-fax"), maxlength: max_len::FAX, value: v.fax.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().fax = opt(x) }
                        TextField { id: "active", label: t("plant-field-active"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).map(|p| if p.active { t("plant-status-active") } else { t("plant-status-inactive") }).unwrap_or_default() }
                    }
                    p { class: "muted", {t("plant-hint-time-zone")} }
                    div { class: "form-grid cols-4",
                        TextField { id: "created-by", label: t("plant-field-created-by"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).map(|p| p.created_by.clone()).unwrap_or_default() }
                        TextField { id: "created-on", label: t("plant-field-created-on"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).map(|p| p.created_on.format("%Y/%m/%d").to_string()).unwrap_or_default() }
                        TextField { id: "updated-by", label: t("plant-field-updated-by"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).and_then(|p| p.updated_by.clone()).unwrap_or_default() }
                        TextField { id: "updated-on", label: t("plant-field-updated-on"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).and_then(|p| p.updated_on.map(|d| d.format("%Y/%m/%d").to_string())).unwrap_or_default() }
                    }
                }
            }

            // ----- 營運中心清單 -----
            Card {
                div { class: "table-header",
                    h3 { {t("plant-list-title")} }
                    p { {tf("list-count-all", &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "card-body",
                    TextField { id: "plant-filter", class: "legacy-search", label: t("plant-field-filter"), value: filter(), oninput: move |x| { filter.set(x); pos.set(Some(0)); } }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for id in ["plant-field-id", "plant-field-name", "plant-field-time-zone", "plant-field-phone", "plant-field-active"] {
                                th { key: "{id}", {t(id)} }
                            }
                        } }
                        tbody {
                            match &*plants.read() {
                                None => rsx! { tr { td { colspan: 5, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 5, "{e}" } } },
                                Some(Ok(_)) if list.is_empty() => rsx! { tr { td { colspan: 5, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(_)) => rsx! {
                                    for (i, p) in list.iter().cloned().enumerate() {
                                        tr {
                                            key: "{p.plant_id}",
                                            class: "md-table__row--clickable",
                                            class: if pos() == Some(i) { "md-table__row--selected" },
                                            onclick: move |_| if mode() == Mode::View { message.set(None); error.set(None); pos.set(Some(i)); },
                                            td { class: "mono", "{p.plant_id}" }
                                            td { "{p.name}" }
                                            td { class: "mono", "{p.time_zone}" }
                                            td { {p.phone.clone().unwrap_or_default()} }
                                            td {
                                                if p.active { Chip { label: t("plant-status-active") } } else { Chip { label: t("plant-status-inactive"), warning: true } }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }
            p { class: "note", {t("plant-note-schema")} }
        }
    }
}
