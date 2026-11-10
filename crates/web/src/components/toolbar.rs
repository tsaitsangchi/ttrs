//! 維護作業的標準功能列（docs/adr/0013-standard-maintenance-actions.md §1）：固定順序
//! 查詢、新增、修改、複製、刪除、確認／取消確認、失效／啟用、列印、匯出 Excel、異動記錄，之後是作業專屬的按鈕（`children`）。
//! 上下筆在功能列下方（[`crate::components::qbe::RecordNav`]）。
//! 沒有該功能權限時隱藏；有權限但目前不可用（例如沒有資料）時顯示為停用（SA6）。

use dioxus::prelude::*;

use crate::components::material::{Button, ButtonVariant, Tone, icons};
use crate::components::permission::Perm;
use crate::i18n::t;

/// 一個功能：按下時的處理與目前是否可用
#[derive(Clone, PartialEq)]
pub struct Act {
    pub on: EventHandler<()>,
    pub enabled: bool,
}

impl Act {
    pub fn new(enabled: bool, on: impl FnMut(()) + 'static) -> Option<Self> {
        Some(Self {
            on: EventHandler::new(on),
            enabled,
        })
    }
}

#[component]
pub fn MaintenanceToolbar(
    #[props(default)] query: Option<Act>,
    #[props(default)] insert: Option<Act>,
    #[props(default)] modify: Option<Act>,
    #[props(default)] copy: Option<Act>,
    #[props(default)] delete: Option<Act>,
    /// 確認／取消確認（功能 confirm、notconfirm；ADR 0013 SA9）：一個按鈕依 `confirmed` 切換
    #[props(default)]
    confirm: Option<Act>,
    /// 目前這筆已確認：按鈕顯示為「取消確認」並檢查 notconfirm
    #[props(default)]
    confirmed: bool,
    /// 失效／啟用（功能 invalid）
    #[props(default)]
    invalid: Option<Act>,
    /// 目前這筆已失效：按鈕顯示為「啟用」
    #[props(default)]
    inactive: bool,
    /// 失效／啟用檢查的功能代號；未給時為 invalid（例如營運中心沿用 modify，AQ3）
    #[props(default)]
    invalid_action: Option<&'static str>,
    /// 新增、修改、刪除檢查的功能代號；未給時為 insert、modify、delete
    /// （TipTop 單身式作業如 p_zw、aooi101 一律用 detail）
    #[props(default)]
    detail_action: Option<&'static str>,
    /// 失效、啟用的按鈕文字（翻譯 ID），未給時用通用的「失效」「啟用」
    #[props(default)]
    invalid_label: Option<(&'static str, &'static str)>,
    #[props(default)] print: Option<Act>,
    #[props(default)] export: Option<Act>,
    #[props(default)] history: Option<Act>,
    /// 作業專屬的按鈕（確認、取消確認…）
    #[props(default)]
    children: Element,
) -> Element {
    let perm = use_context::<Perm>();
    let show = |a: &Option<Act>, action: &str| a.is_some() && perm.can(action);
    let (deactivate, activate) =
        invalid_label.unwrap_or(("common-action-deactivate", "common-action-activate"));
    let btn = |a: &Option<Act>| {
        let a = a.clone().expect("已檢查");
        (a.enabled, a.on)
    };
    rsx! {
        if show(&query, "query") {
            {
                let (enabled, on) = btn(&query);
                rsx! { Button { icon: icons::SEARCH, tone: Tone::Primary, variant: ButtonVariant::Tonal, disabled: !enabled, onclick: move |_| on.call(()), {t("common-action-query")} } }
            }
        }
        if show(&insert, detail_action.unwrap_or("insert")) {
            {
                let (enabled, on) = btn(&insert);
                rsx! { Button { icon: icons::ADD, tone: Tone::Success, disabled: !enabled, onclick: move |_| on.call(()), {t("common-action-new")} } }
            }
        }
        if show(&modify, detail_action.unwrap_or("modify")) {
            {
                let (enabled, on) = btn(&modify);
                rsx! { Button { icon: icons::EDIT, tone: Tone::Tertiary, variant: ButtonVariant::Tonal, disabled: !enabled, onclick: move |_| on.call(()), {t("common-action-edit")} } }
            }
        }
        if show(&copy, "reproduce") {
            {
                let (enabled, on) = btn(&copy);
                rsx! { Button { icon: icons::COPY, tone: Tone::Primary, variant: ButtonVariant::Outlined, disabled: !enabled, onclick: move |_| on.call(()), {t("common-action-copy")} } }
            }
        }
        if show(&delete, detail_action.unwrap_or("delete")) {
            {
                let (enabled, on) = btn(&delete);
                rsx! { Button { icon: icons::DELETE, tone: Tone::Error, variant: ButtonVariant::Error, disabled: !enabled, onclick: move |_| on.call(()), {t("common-action-delete")} } }
            }
        }
        if show(&confirm, if confirmed { "notconfirm" } else { "confirm" }) {
            {
                let (enabled, on) = btn(&confirm);
                rsx! {
                    Button {
                        icon: if confirmed { icons::UNDO } else { icons::CHECK_CIRCLE },
                        tone: if confirmed { Tone::Tertiary } else { Tone::Success },
                        variant: ButtonVariant::Outlined,
                        disabled: !enabled,
                        onclick: move |_| on.call(()),
                        if confirmed { {t("common-action-unconfirm")} } else { {t("common-action-confirm")} }
                    }
                }
            }
        }
        if show(&invalid, invalid_action.unwrap_or("invalid")) {
            {
                let (enabled, on) = btn(&invalid);
                rsx! {
                    Button {
                        icon: if inactive { icons::CHECK_CIRCLE } else { icons::BLOCK },
                        tone: if inactive { Tone::Success } else { Tone::Error },
                        variant: ButtonVariant::Outlined,
                        disabled: !enabled,
                        onclick: move |_| on.call(()),
                        if inactive { {t(activate)} } else { {t(deactivate)} }
                    }
                }
            }
        }
        if show(&print, "output") {
            {
                let (enabled, on) = btn(&print);
                rsx! { Button { icon: icons::PRINT, tone: Tone::Primary, variant: ButtonVariant::Outlined, disabled: !enabled, onclick: move |_| on.call(()), {t("common-action-print")} } }
            }
        }
        if show(&export, "exporttoexcel") {
            {
                let (enabled, on) = btn(&export);
                rsx! { Button { icon: icons::DOWNLOAD, tone: Tone::Primary, variant: ButtonVariant::Outlined, disabled: !enabled, onclick: move |_| on.call(()), {t("common-action-export-excel")} } }
            }
        }
        if show(&history, "history") {
            {
                let (enabled, on) = btn(&history);
                rsx! { Button { icon: icons::HISTORY, tone: Tone::Primary, variant: ButtonVariant::Outlined, disabled: !enabled, onclick: move |_| on.call(()), {t("common-action-history")} } }
            }
        }
        {children}
    }
}
