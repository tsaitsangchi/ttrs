//! Plant Maintenance 營運中心維護（TipTop aooi901 工廠資料建立）
//! 版面比照 tiptop/aoo/4fd/aooi901.4fd（P12）：動作列（新增 / 修改 / 刪除 / 停用・啟用 / 複製）＋ 上下筆
//!   ＋ 單筆表單（group01：代碼、名稱、時區、地址、電話、傳真；另加狀態與建立／修改資訊）＋ 營運中心清單。
//! 查詢比照 TipTop Q.查詢（i901_curs CONSTRUCT）：在同一個表單的欄位上輸入 QBE 條件（plant_maintenance_query.feature）。
//! 資料庫連線欄位（schema、DB link、MSSQL IP）與上線人數不移植（P1、P8）；規則在後端 PlantService。

use dioxus::prelude::*;
use ttrs_shared::{PlantDto, PlantInput, plant_max_len as max_len};

use crate::api;
use crate::components::material::{
    Button, ButtonVariant, Card, Chip, Notification, NotificationKind, Select, TextField, Tone,
    active_field_class, active_tone, icons,
};
use crate::components::permission::{Perm, ProgramGate};
use crate::components::qbe::{
    HINT_FLAG, ListPager, Q, RecordNav, conditions_of, use_query_records,
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
    /// 輸入 QBE 條件
    Query,
}

fn opt(v: String) -> Option<String> {
    Some(v).filter(|s| !s.trim().is_empty())
}

/// 權限檢查用的程式代號
const PROGRAM: &str = "plant-maintenance";

#[component]
pub fn PlantMaintenance() -> Element {
    rsx! {
        ProgramGate { program: PROGRAM, PlantMaintenancePage {} }
    }
}

#[component]
fn PlantMaintenancePage() -> Element {
    let perm = use_context::<Perm>();
    let mut error = use_signal(|| None::<String>);
    let mut message = use_signal(|| None::<String>);
    let mut mode = use_signal(|| Mode::View);
    let mut rec = use_query_records(|req| async move { api::query_plants(&req).await });
    let mut form = use_signal(PlantInput::default);
    let mut copy_id = use_signal(String::new);

    let cur: Option<PlantDto> = rec.current.read().clone();
    let (total, pages) = rec.totals();
    let filtered = !rec.conditions.read().is_empty();
    let offset = rec.offset();

    // 換筆時表單跟著目前這一筆
    use_effect(move || {
        let c = rec.current.read().clone();
        if *mode.peek() == Mode::View {
            form.set(c.as_ref().map(PlantDto::to_input).unwrap_or_default());
        }
    });

    // 存檔或其他異動後：顯示該營運中心並重新載入清單
    let mut done = move |msg: String, p: Option<PlantDto>| {
        error.set(None);
        message.set(Some(msg));
        mode.set(Mode::View);
        form.set(p.as_ref().map(PlantDto::to_input).unwrap_or_default());
        if p.is_none() {
            rec.pos.set(None);
        }
        rec.show(p);
    };

    // 執行查詢：先試查第 1 頁，條件有誤時留在查詢模式顯示錯誤
    let run_query = move |_| {
        let conds = conditions_of(&rec.qbe.read());
        let req = rec.request(conds.clone(), 1);
        spawn(async move {
            match api::query_plants(&req).await {
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
                    Some(p),
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
    let q = m == Mode::Query;
    let ro = matches!(m, Mode::View | Mode::Query);
    let qbe = rec.qbe;
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
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: save, {t("common-action-ok")} }
                        Button {
                            icon: icons::CLOSE, tone: Tone::Neutral,
                            variant: ButtonVariant::Text,
                            onclick: move |_| {
                                error.set(None);
                                form.set(cur_cancel.as_ref().map(PlantDto::to_input).unwrap_or_default());
                                mode.set(Mode::View);
                            },
                            {t("common-action-cancel")}
                        }
                    },
                    Mode::Query => rsx! {
                        Button { icon: icons::CHECK, tone: Tone::Success, onclick: run_query, {t("common-action-ok")} }
                        Button {
                            icon: icons::CLOSE, tone: Tone::Neutral,
                            variant: ButtonVariant::Text,
                            onclick: move |_| {
                                error.set(None);
                                form.set(rec.current.peek().as_ref().map(PlantDto::to_input).unwrap_or_default());
                                mode.set(Mode::View);
                            },
                            {t("common-action-cancel")}
                        }
                    },
                    Mode::View => rsx! {
                        if perm.can("query") {
                            Button {
                                icon: icons::SEARCH, tone: Tone::Primary,
                                variant: ButtonVariant::Tonal,
                                onclick: move |_| {
                                    message.set(None);
                                    error.set(None);
                                    rec.qbe.set(Default::default());
                                    mode.set(Mode::Query);
                                },
                                {t("common-action-query")}
                            }
                        }
                        if perm.can("insert") {
                            Button {
                                icon: icons::ADD, tone: Tone::Success,
                                onclick: move |_| {
                                    message.set(None);
                                    error.set(None);
                                    form.set(PlantInput { time_zone: Some(DEFAULT_TIME_ZONE.into()), ..Default::default() });
                                    mode.set(Mode::New);
                                },
                                {t("common-action-new")}
                            }
                        }
                        if perm.can("modify") {
                            Button { icon: icons::EDIT, tone: Tone::Tertiary, variant: ButtonVariant::Tonal, disabled: no_cur, onclick: move |_| mode.set(Mode::Edit), {t("common-action-edit")} }
                        }
                        if perm.can("delete") {
                            Button {
                                icon: icons::DELETE, tone: Tone::Error,
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
                        }
                        // 有效 → 停用、停用 → 啟用（P9）
                        if perm.can("modify") {
                            Button {
                                icon: if cur.as_ref().is_some_and(|p| !p.active) { icons::CHECK_CIRCLE } else { icons::BLOCK }, tone: if cur.as_ref().is_some_and(|p| !p.active) { Tone::Success } else { Tone::Error },
                                variant: ButtonVariant::Outlined,
                                disabled: no_cur,
                                onclick: move |_| {
                                    let Some(p) = cur_toggle.clone() else { return };
                                    spawn(async move {
                                        match api::set_plant_active(&p.plant_id, !p.active).await {
                                            Ok(r) => {
                                                let id = if r.active { "plant-msg-activated" } else { "plant-msg-deactivated" };
                                                done(tf(id, &[("id", r.plant_id.as_str())]), Some(r));
                                            }
                                            Err(e) => error.set(Some(e)),
                                        }
                                    });
                                },
                                if cur.as_ref().is_some_and(|p| !p.active) { {t("plant-action-activate")} } else { {t("plant-action-deactivate")} }
                            }
                        }
                        div { class: "spacer" }
                        if perm.can("reproduce") {
                            TextField { id: "copy-id", label: t("plant-field-copy-as"), maxlength: max_len::ID, value: copy_id(), disabled: no_cur, oninput: move |v| copy_id.set(v) }
                            Button {
                                icon: icons::COPY, tone: Tone::Tertiary,
                                variant: ButtonVariant::Outlined,
                                disabled: no_cur,
                                onclick: move |_| {
                                    let Some(p) = cur_copy.clone() else { return };
                                    let new_id = copy_id();
                                    spawn(async move {
                                        match api::copy_plant(&p.plant_id, &new_id).await {
                                            Ok(r) => {
                                                copy_id.set(String::new());
                                                done(tf("plant-msg-copied", &[("from", p.plant_id.as_str()), ("to", r.plant_id.as_str())]), Some(r));
                                            }
                                            Err(e) => error.set(Some(e)),
                                        }
                                    });
                                },
                                {t("common-action-copy")}
                            }
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

            // ----- 單筆表單（aooi901.4fd group01） -----
            Card {
                div { class: "card-body",
                    div { class: "form-grid",
                        Q { field: "plant_id", label: t("plant-field-id"), query: q, qbe,
                            TextField { id: "azp01", label: t("plant-field-id"), maxlength: max_len::ID, value: v.plant_id.clone(), readonly: m != Mode::New, oninput: move |x| form.write().plant_id = x }
                        }
                        Q { field: "name", label: t("plant-field-name"), query: q, qbe,
                            TextField { id: "azp02", label: t("plant-field-name"), maxlength: max_len::NAME, value: v.name.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().name = opt(x) }
                        }
                        // 資料庫代碼＝工廠 schema（P13）：新增時可輸入，空白自動給號；建立後唯讀
                        Q { field: "schema_name", label: t("plant-field-schema-name"), hint_text: "ds*", query: q, qbe,
                            TextField { id: "azp03", label: t("plant-field-schema-name"), maxlength: 4, placeholder: if m == Mode::New { t("plant-hint-schema-name") } else { String::new() }, value: v.schema_name.clone().unwrap_or_default(), readonly: m != Mode::New, oninput: move |x| form.write().schema_name = opt(x) }
                        }
                        Q { field: "time_zone", label: t("plant-field-time-zone"), hint_text: "Asia/*", query: q, qbe,
                            Select { id: "azp052", label: t("plant-field-time-zone"), options: tz_options, value: tz, readonly: ro, onchange: move |x| form.write().time_zone = Some(x) }
                        }
                        Q { field: "phone", label: t("plant-field-phone"), query: q, qbe,
                            TextField { id: "azp06", label: t("plant-field-phone"), maxlength: max_len::PHONE, value: v.phone.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().phone = opt(x) }
                        }
                        Q { field: "address", label: t("plant-field-address"), class: "span-2", query: q, qbe,
                            TextField { id: "azp051", class: "span-2", label: t("plant-field-address"), maxlength: max_len::ADDRESS, value: v.address.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().address = opt(x) }
                        }
                        Q { field: "fax", label: t("plant-field-fax"), query: q, qbe,
                            TextField { id: "azp07", label: t("plant-field-fax"), maxlength: max_len::FAX, value: v.fax.clone().unwrap_or_default(), readonly: ro, oninput: move |x| form.write().fax = opt(x) }
                        }
                        Q { field: "active", label: t("plant-field-active"), hint: HINT_FLAG, query: q, qbe,
                            TextField { id: "active", label: t("plant-field-active"), readonly: true, class: active_field_class(cur.as_ref().filter(|_| m != Mode::New).map(|p| p.active)), value: cur.as_ref().filter(|_| m != Mode::New).map(|p| if p.active { t("plant-status-active") } else { t("plant-status-inactive") }).unwrap_or_default() }
                        }
                    }
                    p { class: "muted", {t("plant-hint-time-zone")} }
                    // 建立／修改資訊不在 aooi901 查詢欄位內（PQ1）
                    if !q {
                        div { class: "form-grid cols-4",
                            TextField { id: "created-by", label: t("plant-field-created-by"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).map(|p| p.created_by.clone()).unwrap_or_default() }
                            TextField { id: "created-on", label: t("plant-field-created-on"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).map(|p| p.created_on.format("%Y/%m/%d").to_string()).unwrap_or_default() }
                            TextField { id: "updated-by", label: t("plant-field-updated-by"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).and_then(|p| p.updated_by.clone()).unwrap_or_default() }
                            TextField { id: "updated-on", label: t("plant-field-updated-on"), readonly: true, value: cur.as_ref().filter(|_| m != Mode::New).and_then(|p| p.updated_on.map(|d| d.format("%Y/%m/%d").to_string())).unwrap_or_default() }
                        }
                    }
                }
            }

            // ----- 營運中心清單 -----
            Card {
                div { class: "table-header",
                    h3 { {t("plant-list-title")} }
                    p { {tf(if filtered { "list-count-filtered" } else { "list-count-all" }, &[("total", total.to_string().as_str())])} {t("list-hint-click")} }
                }
                div { class: "table-scroll",
                    table { class: "md-table",
                        thead { tr {
                            for id in ["plant-field-id", "plant-field-name", "plant-field-time-zone", "plant-field-phone", "plant-field-active"] {
                                th { key: "{id}", {t(id)} }
                            }
                        } }
                        tbody {
                            match &*rec.list.read() {
                                None => rsx! { tr { td { colspan: 5, {t("common-msg-loading")} } } },
                                Some(Err(e)) => rsx! { tr { td { colspan: 5, "{e}" } } },
                                Some(Ok(page)) if page.items.is_empty() => rsx! { tr { td { colspan: 5, class: "muted", {t("common-msg-no-data")} } } },
                                Some(Ok(page)) => rsx! {
                                    for (i, p) in page.items.iter().cloned().enumerate() {
                                        tr {
                                            key: "{p.plant_id}",
                                            class: "md-table__row--clickable",
                                            class: if cur.as_ref().is_some_and(|c| c.plant_id == p.plant_id) { "md-table__row--selected" },
                                            onclick: move |_| if mode() == Mode::View { message.set(None); error.set(None); rec.go(offset + i as u64); },
                                            td { class: "mono", "{p.plant_id}" }
                                            td { "{p.name}" }
                                            td { class: "mono", "{p.time_zone}" }
                                            td { {p.phone.clone().unwrap_or_default()} }
                                            td {
                                                Chip { label: if p.active { t("plant-status-active") } else { t("plant-status-inactive") }, tone: active_tone(p.active) }
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
            p { class: "note", {t("plant-note-schema")} }
        }
    }
}
