//! QBE 查詢的共用畫面元件（TipTop 各維護作業的 Q.查詢）：條件輸入框、上下筆、清單換頁，
//! 以及「查詢結果的第幾筆」的狀態（[`use_query_records`]）。條件語法見 ttrs_domain::qbe。

use std::{collections::BTreeMap, future::Future};

use dioxus::prelude::*;
use ttrs_shared::{Page, QbeCondition, QueryRequest};

use crate::components::material::{Button, ButtonVariant, IconButton, TextField, icons};
use crate::i18n::{t, tf};

/// 清單每頁筆數（QBE7）
pub const PAGE_SIZE: u32 = 50;

pub const HINT_FLAG: &str = "qbe-hint-flag";
pub const HINT_DATE: &str = "qbe-hint-date";
pub const HINT_NUMBER: &str = "qbe-hint-number";

/// 查詢模式中正在輸入的條件（欄位鍵值 → 條件）
pub type Qbe = Signal<BTreeMap<&'static str, String>>;

/// 送出的條件：空白的欄位不送（QBE4）
pub fn conditions_of(qbe: &BTreeMap<&'static str, String>) -> Vec<QbeCondition> {
    qbe.iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| QbeCondition {
            field: (*k).to_owned(),
            expr: v.clone(),
        })
        .collect()
}

/// 查詢模式下把欄位換成 QBE 條件輸入框（文字），其餘模式照常顯示原本的欄位
#[component]
pub fn Q(
    field: &'static str,
    label: String,
    query: bool,
    qbe: Qbe,
    /// 提示文字的翻譯 ID
    #[props(default)]
    hint: &'static str,
    /// 已翻譯或資料值的提示（優先於 `hint`）
    #[props(default)]
    hint_text: String,
    #[props(default)] class: &'static str,
    children: Element,
) -> Element {
    if !query {
        return children;
    }
    let placeholder = if hint_text.is_empty() {
        t(hint)
    } else {
        hint_text
    };
    rsx! {
        TextField {
            id: "q-{field}",
            label,
            class,
            placeholder,
            value: qbe.read().get(field).cloned().unwrap_or_default(),
            oninput: move |v: String| {
                qbe.write().insert(field, v);
            },
        }
    }
}

/// 筆數與上下筆（4fd 底部 cnt；第一筆／上一筆／下一筆／最後一筆／跳到第 N 筆）
#[component]
pub fn RecordNav(
    total: u64,
    pos: Option<u64>,
    ongo: EventHandler<u64>,
    onerror: EventHandler<String>,
) -> Element {
    let mut jump = use_signal(String::new);
    rsx! {
        div { class: "record-nav",
            IconButton {
                label: t("nav-first"),
                path: icons::FIRST_PAGE,
                disabled: pos == Some(0),
                onclick: move |_| ongo.call(0),
            }
            IconButton {
                label: t("nav-previous"),
                path: icons::CHEVRON_LEFT,
                disabled: pos.is_none_or(|i| i == 0),
                onclick: move |_| {
                    if let Some(i) = pos.filter(|i| *i > 0) {
                        ongo.call(i - 1);
                    }
                },
            }
            span { class: "record-nav__count",
                match pos {
                    Some(i) => rsx! { {tf("nav-record-position", &[("pos", (i + 1).to_string().as_str()), ("total", total.to_string().as_str())])} },
                    None => rsx! { {tf("nav-record-count", &[("total", total.to_string().as_str())])} },
                }
            }
            IconButton {
                label: t("nav-next"),
                path: icons::CHEVRON_RIGHT,
                disabled: pos.is_some_and(|i| i + 1 >= total),
                onclick: move |_| ongo.call(pos.map_or(0, |i| i + 1)),
            }
            IconButton {
                label: t("nav-last"),
                path: icons::LAST_PAGE,
                disabled: pos == Some(total.saturating_sub(1)),
                onclick: move |_| ongo.call(total.saturating_sub(1)),
            }
            TextField {
                id: "jump",
                class: "record-nav__jump",
                label: t("nav-jump-to"),
                input_type: "number",
                value: jump(),
                oninput: move |v| jump.set(v),
            }
            Button {
                variant: ButtonVariant::Outlined,
                onclick: move |_| {
                    match jump().trim().parse::<u64>() {
                        Ok(n) if (1..=total).contains(&n) => {
                            jump.set(String::new());
                            ongo.call(n - 1);
                        }
                        _ => onerror.call(tf("nav-jump-invalid", &[("total", total.to_string().as_str())])),
                    }
                },
                {t("nav-jump")}
            }
        }
    }
}

/// 清單換頁（第 N / M 頁、上一頁、下一頁）
#[component]
pub fn ListPager(page: Signal<u32>, pages: u32) -> Element {
    rsx! {
        div { class: "table-footer",
            span { {tf("list-page-position", &[("page", page().to_string().as_str()), ("pages", pages.to_string().as_str())])} }
            IconButton {
                label: t("list-page-previous"),
                path: icons::CHEVRON_LEFT,
                disabled: page() <= 1,
                onclick: move |_| page -= 1,
            }
            IconButton {
                label: t("list-page-next"),
                path: icons::CHEVRON_RIGHT,
                disabled: page() >= pages,
                onclick: move |_| page += 1,
            }
        }
    }
}

/// 單筆維護作業的查詢狀態：生效的條件、清單頁次與內容、目前是第幾筆與顯示中的資料
pub struct QueryRecords<T: 'static> {
    /// 查詢模式中正在輸入的條件
    pub qbe: Qbe,
    /// 目前生效的條件（QBE4：預設查全部）
    pub conditions: Signal<Vec<QbeCondition>>,
    /// 清單頁次（1 起算）
    pub page: Signal<u32>,
    pub list: Resource<Result<Page<T>, String>>,
    /// 目前顯示的是查詢結果的第幾筆（0 起算）
    pub pos: Signal<Option<u64>>,
    /// 目前顯示的資料
    pub current: Signal<Option<T>>,
    /// 換頁後要在該頁載入完成時顯示 `pos` 那一筆
    sync: Signal<bool>,
}

impl<T> Clone for QueryRecords<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for QueryRecords<T> {}

impl<T: Clone + 'static> QueryRecords<T> {
    /// 查詢請求（`page` 為頁次）
    pub fn request(&self, conditions: Vec<QbeCondition>, page: u32) -> QueryRequest {
        QueryRequest {
            conditions,
            page: Some(page),
            page_size: Some(PAGE_SIZE),
            lang: None,
        }
    }

    /// 移到查詢結果的第 i 筆（0 起算），必要時換頁
    pub fn go(mut self, i: u64) {
        self.pos.set(Some(i));
        self.page
            .set(u32::try_from(i / u64::from(PAGE_SIZE)).unwrap_or(0) + 1);
        self.sync.set(true);
    }

    /// 查詢成功後套用新條件：有資料時停在第一筆
    pub fn apply(mut self, conditions: Vec<QbeCondition>, total: u64) {
        // 清掉舊條件的結果，避免新結果載入前誤取舊清單的第一筆
        self.list.clear();
        self.conditions.set(conditions);
        self.page.set(1);
        if total == 0 {
            self.pos.set(None);
            self.current.set(None);
        } else {
            self.go(0);
        }
    }

    /// 存檔等異動後：顯示指定資料並重新載入清單
    pub fn show(mut self, item: Option<T>) {
        self.current.set(item);
        self.list.restart();
    }

    /// （總筆數, 總頁數）
    pub fn totals(&self) -> (u64, u32) {
        self.list
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map_or((0, 1), |p| {
                (
                    p.total,
                    u32::try_from(p.total.div_ceil(u64::from(p.page_size)))
                        .unwrap_or(1)
                        .max(1),
                )
            })
    }

    /// 清單目前這一頁第一筆的序號（0 起算）
    pub fn offset(&self) -> u64 {
        u64::from(self.page.peek().saturating_sub(1)) * u64::from(PAGE_SIZE)
    }
}

/// 建立查詢狀態：`fetch` 以查詢請求取一頁資料；首次載入有資料時停在第一筆
pub fn use_query_records<T, F>(
    fetch: impl Fn(QueryRequest) -> F + Copy + 'static,
) -> QueryRecords<T>
where
    T: Clone + PartialEq + 'static,
    F: Future<Output = Result<Page<T>, String>> + 'static,
{
    let qbe: Qbe = use_signal(BTreeMap::new);
    let conditions = use_signal(Vec::<QbeCondition>::new);
    let page = use_signal(|| 1u32);
    let list = use_resource(move || {
        fetch(QueryRequest {
            conditions: conditions(),
            page: Some(page()),
            page_size: Some(PAGE_SIZE),
            lang: None,
        })
    });
    let mut pos = use_signal(|| None::<u64>);
    let mut current = use_signal(|| None::<T>);
    let mut sync = use_signal(|| false);

    use_effect(move || {
        let loaded = list.read();
        let Some(Ok(p)) = &*loaded else { return };
        // 首次載入（或刪除後沒有顯示中的資料）停在第一筆
        if pos.peek().is_none() && current.peek().is_none() && !sync() {
            if let Some(first) = p.items.first() {
                pos.set(Some(u64::from(p.page - 1) * u64::from(p.page_size)));
                current.set(Some(first.clone()));
            }
            return;
        }
        // 換頁或跳筆後，等該頁載入完成再顯示目標那一筆
        if !sync() || p.page != page() {
            return;
        }
        let Some(i) = pos() else { return };
        let offset = u64::from(p.page - 1) * u64::from(p.page_size);
        let picked = i
            .checked_sub(offset)
            .and_then(|k| p.items.get(usize::try_from(k).ok()?))
            .cloned();
        sync.set(false);
        current.set(picked);
    });

    QueryRecords {
        qbe,
        conditions,
        page,
        list,
        pos,
        current,
        sync,
    }
}
