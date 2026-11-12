//! 介面多語言（繁中 zh-TW、英文 en、簡中 zh-CN）— ADR 0005
//!
//! - 翻譯在 `crates/web/locales/<語系>/*.ftl`（Fluent），編譯時內嵌；畫面以訊息 ID 取用：
//!   `t("program-field-program-no")`、`tf("program-msg-saved", &[("no", "item-maintenance")])`
//! - 找不到翻譯時依序退回 zh-TW，再退回 ID 本身（方便發現漏翻）
//! - 語系決定順序：網址 `?lang=` → 瀏覽器記住的語系 → zh-TW（日後加使用者設定）
//! - 錯誤訊息依 `error-<錯誤代碼>` 翻譯，zh-TW 與未翻譯的代碼顯示伺服器訊息
//!
//! 不用 dioxus-i18n：其翻譯函式依賴元件 context，而錯誤訊息在 api.rs（元件外）翻譯。

use std::sync::atomic::{AtomicU8, Ordering};

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLang {
    ZhTw,
    En,
    ZhCn,
}

impl UiLang {
    pub const ALL: [Self; 3] = [Self::ZhTw, Self::En, Self::ZhCn];

    /// 與後端語系代碼相同（ttrs_domain::lang::LANGS）
    pub fn code(self) -> &'static str {
        match self {
            Self::ZhTw => "zh-TW",
            Self::En => "en",
            Self::ZhCn => "zh-CN",
        }
    }

    /// 切換選單上的名稱（固定以各自語言顯示）
    pub fn native_name(self) -> &'static str {
        match self {
            Self::ZhTw => "繁體中文",
            Self::En => "English",
            Self::ZhCn => "简体中文",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.code() == code)
    }

    fn index(self) -> usize {
        match self {
            Self::ZhTw => 0,
            Self::En => 1,
            Self::ZhCn => 2,
        }
    }
}

/// 各語系的翻譯檔（順序同 `UiLang::index`）
const SOURCES: [&[&str]; 3] = [
    &[
        include_str!("../locales/zh-TW/common.ftl"),
        include_str!("../locales/zh-TW/program.ftl"),
        include_str!("../locales/zh-TW/error.ftl"),
        include_str!("../locales/zh-TW/dictionary.ftl"),
        include_str!("../locales/zh-TW/item.ftl"),
        include_str!("../locales/zh-TW/user.ftl"),
        include_str!("../locales/zh-TW/permission.ftl"),
        include_str!("../locales/zh-TW/plant.ftl"),
        include_str!("../locales/zh-TW/department.ftl"),
        include_str!("../locales/zh-TW/classification.ftl"),
        include_str!("../locales/zh-TW/employee.ftl"),
        include_str!("../locales/zh-TW/program_permission.ftl"),
        include_str!("../locales/zh-TW/user_permission.ftl"),
        include_str!("../locales/zh-TW/menu.ftl"),
        include_str!("../locales/zh-TW/unit.ftl"),
        include_str!("../locales/zh-TW/basic_code.ftl"),
        include_str!("../locales/zh-TW/currency.ftl"),
        include_str!("../locales/zh-TW/exchange_rate.ftl"),
        include_str!("../locales/zh-TW/general_parameters.ftl"),
        include_str!("../locales/zh-TW/accounting_period.ftl"),
        include_str!("../locales/zh-TW/unit_conversion.ftl"),
        include_str!("../locales/zh-TW/epub.ftl"),
        include_str!("../locales/zh-TW/github.ftl"),
        include_str!("../locales/zh-TW/tax_type.ftl"),
        include_str!("../locales/zh-TW/project.ftl"),
        include_str!("../locales/zh-TW/product_class.ftl"),
        include_str!("../locales/zh-TW/item_group.ftl"),
    ],
    &[
        include_str!("../locales/en/common.ftl"),
        include_str!("../locales/en/program.ftl"),
        include_str!("../locales/en/error.ftl"),
        include_str!("../locales/en/dictionary.ftl"),
        include_str!("../locales/en/item.ftl"),
        include_str!("../locales/en/user.ftl"),
        include_str!("../locales/en/permission.ftl"),
        include_str!("../locales/en/plant.ftl"),
        include_str!("../locales/en/department.ftl"),
        include_str!("../locales/en/classification.ftl"),
        include_str!("../locales/en/employee.ftl"),
        include_str!("../locales/en/program_permission.ftl"),
        include_str!("../locales/en/user_permission.ftl"),
        include_str!("../locales/en/menu.ftl"),
        include_str!("../locales/en/unit.ftl"),
        include_str!("../locales/en/basic_code.ftl"),
        include_str!("../locales/en/currency.ftl"),
        include_str!("../locales/en/exchange_rate.ftl"),
        include_str!("../locales/en/general_parameters.ftl"),
        include_str!("../locales/en/accounting_period.ftl"),
        include_str!("../locales/en/unit_conversion.ftl"),
        include_str!("../locales/en/epub.ftl"),
        include_str!("../locales/en/github.ftl"),
        include_str!("../locales/en/tax_type.ftl"),
        include_str!("../locales/en/project.ftl"),
        include_str!("../locales/en/product_class.ftl"),
        include_str!("../locales/en/item_group.ftl"),
    ],
    &[
        include_str!("../locales/zh-CN/common.ftl"),
        include_str!("../locales/zh-CN/program.ftl"),
        include_str!("../locales/zh-CN/error.ftl"),
        include_str!("../locales/zh-CN/dictionary.ftl"),
        include_str!("../locales/zh-CN/item.ftl"),
        include_str!("../locales/zh-CN/user.ftl"),
        include_str!("../locales/zh-CN/permission.ftl"),
        include_str!("../locales/zh-CN/plant.ftl"),
        include_str!("../locales/zh-CN/department.ftl"),
        include_str!("../locales/zh-CN/classification.ftl"),
        include_str!("../locales/zh-CN/employee.ftl"),
        include_str!("../locales/zh-CN/program_permission.ftl"),
        include_str!("../locales/zh-CN/user_permission.ftl"),
        include_str!("../locales/zh-CN/menu.ftl"),
        include_str!("../locales/zh-CN/unit.ftl"),
        include_str!("../locales/zh-CN/basic_code.ftl"),
        include_str!("../locales/zh-CN/currency.ftl"),
        include_str!("../locales/zh-CN/exchange_rate.ftl"),
        include_str!("../locales/zh-CN/general_parameters.ftl"),
        include_str!("../locales/zh-CN/accounting_period.ftl"),
        include_str!("../locales/zh-CN/unit_conversion.ftl"),
        include_str!("../locales/zh-CN/epub.ftl"),
        include_str!("../locales/zh-CN/github.ftl"),
        include_str!("../locales/zh-CN/tax_type.ftl"),
        include_str!("../locales/zh-CN/project.ftl"),
        include_str!("../locales/zh-CN/product_class.ftl"),
        include_str!("../locales/zh-CN/item_group.ftl"),
    ],
];

fn bundle(lang: UiLang) -> FluentBundle<FluentResource> {
    let langid = lang.code().parse().expect("語系代碼");
    let mut b = FluentBundle::new(vec![langid]);
    // 不加 Unicode 方向隔離字元（中英文都是左到右）
    b.set_use_isolating(false);
    for src in SOURCES[lang.index()] {
        let res =
            FluentResource::try_new((*src).to_owned()).expect("翻譯檔語法錯誤（測試會先擋下）");
        b.add_resource(res)
            .expect("翻譯檔有重複的訊息 ID（測試會先擋下）");
    }
    b
}

thread_local! {
    static BUNDLES: [FluentBundle<FluentResource>; 3] =
        [bundle(UiLang::ZhTw), bundle(UiLang::En), bundle(UiLang::ZhCn)];
}

static CURRENT: AtomicU8 = AtomicU8::new(0);
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const STORAGE_KEY: &str = "ttrs.lang";

pub fn current() -> UiLang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => UiLang::En,
        2 => UiLang::ZhCn,
        _ => UiLang::ZhTw,
    }
}

/// 切換語系並記在瀏覽器；同時設定 <html lang> 並標示不要由瀏覽器翻譯。
/// 瀏覽器翻譯會把文字節點換成譯文副本，之後畫面更新（例如停用→啟用的按鈕）就看不到了；
/// TTRS 自己提供三語介面（ADR 0005），由語系選單切換
pub fn set(lang: UiLang) {
    CURRENT.store(lang.index() as u8, Ordering::Relaxed);
    #[cfg(target_arch = "wasm32")]
    if let Some(w) = web_sys::window() {
        if let Some(s) = w.local_storage().ok().flatten() {
            let _ = s.set_item(STORAGE_KEY, lang.code());
        }
        if let Some(root) = w.document().and_then(|d| d.document_element()) {
            let _ = root.set_attribute("lang", lang.code());
            let _ = root.set_attribute("translate", "no");
        }
    }
}

/// 啟動時決定語系：網址 `?lang=en` 優先（方便分享連結），其次瀏覽器記住的語系，預設繁中
pub fn load() -> UiLang {
    let lang = saved().unwrap_or(UiLang::ZhTw);
    set(lang);
    lang
}

#[cfg(target_arch = "wasm32")]
fn saved() -> Option<UiLang> {
    let window = web_sys::window()?;
    let from_url = window.location().search().ok().and_then(|q| {
        q.trim_start_matches('?')
            .split('&')
            .find_map(|kv| kv.strip_prefix("lang="))
            .and_then(UiLang::from_code)
    });
    from_url.or_else(|| {
        window
            .local_storage()
            .ok()
            .flatten()
            .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
            .and_then(|c| UiLang::from_code(&c))
    })
}

/// 桌面/行動版目前沒有持久化設定，固定繁中（導入登入後改存使用者設定）
#[cfg(not(target_arch = "wasm32"))]
fn saved() -> Option<UiLang> {
    None
}

/// 在指定語系找訊息；找不到回傳 None
fn lookup(lang: UiLang, id: &str, args: Option<&FluentArgs>) -> Option<String> {
    BUNDLES.with(|bundles| {
        let b = &bundles[lang.index()];
        let pattern = b.get_message(id)?.value()?;
        let mut errors = vec![];
        Some(b.format_pattern(pattern, args, &mut errors).into_owned())
    })
}

fn translate(id: &str, args: Option<&FluentArgs>) -> String {
    lookup(current(), id, args)
        .or_else(|| lookup(UiLang::ZhTw, id, args))
        .unwrap_or_else(|| id.to_owned())
}

/// 翻譯：以訊息 ID 取用目前語系的文字
pub fn t(id: &str) -> String {
    translate(id, None)
}

/// 含參數的翻譯，如 `tf("program-msg-saved", &[("no", "item-maintenance")])`
pub fn tf(id: &str, args: &[(&str, &str)]) -> String {
    let mut fa = FluentArgs::new();
    for (k, v) in args {
        fa.set(*k, (*v).to_owned());
    }
    translate(id, Some(&fa))
}

/// 錯誤訊息：英文、簡中依 `error-<代碼>` 翻譯；繁中與未翻譯的代碼使用伺服器訊息（較具體）
pub fn error_text(code: &str, server_message: &str) -> String {
    let lang = current();
    if lang == UiLang::ZhTw {
        return server_message.to_owned();
    }
    lookup(lang, &format!("error-{code}"), None).unwrap_or_else(|| server_message.to_owned())
}

#[cfg(test)]
mod tests {
    //! 翻譯檔檢查（make check 的一部分，ADR 0005）

    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    /// 訊息 ID → 使用的變數名稱
    fn messages(lang: UiLang) -> BTreeMap<String, BTreeSet<String>> {
        let mut out = BTreeMap::new();
        for src in SOURCES[lang.index()] {
            if let Err((_, e)) = FluentResource::try_new((*src).to_owned()) {
                panic!("{} 翻譯檔語法錯誤：{e:?}", lang.code());
            }
            for line in src.lines() {
                if line.starts_with('#') || line.trim().is_empty() || line.starts_with(' ') {
                    continue;
                }
                let (id, value) = line.split_once(" = ").expect("每則訊息需為「ID = 值」單行");
                let vars = value
                    .split("{ $")
                    .skip(1)
                    .filter_map(|s| s.split_once(' ').map(|(v, _)| v.to_owned()))
                    .collect();
                assert!(
                    out.insert(id.to_owned(), vars).is_none(),
                    "{} 重複的 ID：{id}",
                    lang.code()
                );
            }
        }
        out
    }

    #[test]
    fn all_languages_have_same_ids_and_variables() {
        let base = messages(UiLang::ZhTw);
        for lang in [UiLang::En, UiLang::ZhCn] {
            let other = messages(lang);
            let missing: Vec<_> = base.keys().filter(|k| !other.contains_key(*k)).collect();
            let extra: Vec<_> = other.keys().filter(|k| !base.contains_key(*k)).collect();
            assert!(missing.is_empty(), "{} 缺少翻譯：{missing:?}", lang.code());
            assert!(
                extra.is_empty(),
                "{} 多出 zh-TW 沒有的 ID：{extra:?}",
                lang.code()
            );
            for (id, vars) in &base {
                assert_eq!(&other[id], vars, "{} 的 {id} 參數不一致", lang.code());
            }
        }
    }

    #[test]
    fn translates_with_fallback_and_arguments() {
        set(UiLang::En);
        assert_eq!(
            tf("program-msg-copied", &[("from", "a"), ("to", "b")]),
            "Copied a to b"
        );
        assert_eq!(t("list-hint-click"), " — click a row to view or edit");
        assert_eq!(t("no-such-id"), "no-such-id");
        set(UiLang::ZhTw);
        assert_eq!(t("common-action-query"), "查詢");
    }

    /// 已改用翻譯的畫面不得再有直接寫死的中文字串（資料值等例外以 `i18n-allow` 註記該行）
    #[test]
    fn translated_pages_have_no_hardcoded_chinese() {
        const TRANSLATED: [(&str, &str); 36] = [
            ("main.rs", include_str!("main.rs")),
            (
                "components/session.rs",
                include_str!("components/session.rs"),
            ),
            ("components/status.rs", include_str!("components/status.rs")),
            (
                "components/history.rs",
                include_str!("components/history.rs"),
            ),
            (
                "pages/azz/menu_maintenance.rs",
                include_str!("pages/azz/menu_maintenance.rs"),
            ),
            (
                "pages/aoo/department_maintenance.rs",
                include_str!("pages/aoo/department_maintenance.rs"),
            ),
            (
                "pages/aoo/employee_maintenance.rs",
                include_str!("pages/aoo/employee_maintenance.rs"),
            ),
            (
                "pages/aoo/classification_code_maintenance.rs",
                include_str!("pages/aoo/classification_code_maintenance.rs"),
            ),
            (
                "pages/aim/item_group_maintenance.rs",
                include_str!("pages/aim/item_group_maintenance.rs"),
            ),
            (
                "pages/aoo/unit_maintenance.rs",
                include_str!("pages/aoo/unit_maintenance.rs"),
            ),
            (
                "pages/aoo/basic_code_maintenance.rs",
                include_str!("pages/aoo/basic_code_maintenance.rs"),
            ),
            (
                "pages/aoo/currency_maintenance.rs",
                include_str!("pages/aoo/currency_maintenance.rs"),
            ),
            (
                "pages/aoo/exchange_rate_maintenance.rs",
                include_str!("pages/aoo/exchange_rate_maintenance.rs"),
            ),
            (
                "pages/aoo/daily_rate_copy.rs",
                include_str!("pages/aoo/daily_rate_copy.rs"),
            ),
            (
                "pages/aoo/general_parameters.rs",
                include_str!("pages/aoo/general_parameters.rs"),
            ),
            (
                "pages/aoo/accounting_period_maintenance.rs",
                include_str!("pages/aoo/accounting_period_maintenance.rs"),
            ),
            (
                "pages/azz/epub_to_text.rs",
                include_str!("pages/azz/epub_to_text.rs"),
            ),
            (
                "pages/azz/github_epub_search.rs",
                include_str!("pages/azz/github_epub_search.rs"),
            ),
            (
                "pages/aoo/unit_conversion_maintenance.rs",
                include_str!("pages/aoo/unit_conversion_maintenance.rs"),
            ),
            (
                "pages/aoo/item_unit_conversion_maintenance.rs",
                include_str!("pages/aoo/item_unit_conversion_maintenance.rs"),
            ),
            (
                "pages/aoo/tax_type_maintenance.rs",
                include_str!("pages/aoo/tax_type_maintenance.rs"),
            ),
            (
                "pages/aoo/project_maintenance.rs",
                include_str!("pages/aoo/project_maintenance.rs"),
            ),
            (
                "pages/axm/product_classification_maintenance.rs",
                include_str!("pages/axm/product_classification_maintenance.rs"),
            ),
            ("components/plant.rs", include_str!("components/plant.rs")),
            (
                "components/multilingual.rs",
                include_str!("components/multilingual.rs"),
            ),
            (
                "components/permission.rs",
                include_str!("components/permission.rs"),
            ),
            (
                "pages/azz/program_maintenance.rs",
                include_str!("pages/azz/program_maintenance.rs"),
            ),
            (
                "pages/azz/data_dictionary.rs",
                include_str!("pages/azz/data_dictionary.rs"),
            ),
            ("pages/home.rs", include_str!("pages/home.rs")),
            (
                "pages/aim/item_maintenance.rs",
                include_str!("pages/aim/item_maintenance.rs"),
            ),
            (
                "pages/azz/legacy_catalog.rs",
                include_str!("pages/azz/legacy_catalog.rs"),
            ),
            (
                "pages/azz/user_maintenance.rs",
                include_str!("pages/azz/user_maintenance.rs"),
            ),
            (
                "pages/azz/permission_class_maintenance.rs",
                include_str!("pages/azz/permission_class_maintenance.rs"),
            ),
            (
                "pages/aoo/plant_maintenance.rs",
                include_str!("pages/aoo/plant_maintenance.rs"),
            ),
            (
                "pages/azz/program_permission_maintenance.rs",
                include_str!("pages/azz/program_permission_maintenance.rs"),
            ),
            (
                "pages/azz/user_permission_maintenance.rs",
                include_str!("pages/azz/user_permission_maintenance.rs"),
            ),
        ];
        let is_cjk = |c: char| {
            ('\u{4e00}'..='\u{9fff}').contains(&c) || ('\u{3000}'..='\u{303f}').contains(&c)
        };
        for (file, src) in TRANSLATED {
            for (n, line) in src.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                if line.contains("i18n-allow") || !code.contains('"') {
                    continue;
                }
                let in_string = code
                    .split('"')
                    .skip(1)
                    .step_by(2)
                    .any(|s| s.chars().any(is_cjk));
                assert!(
                    !in_string,
                    "{file}:{} 有未翻譯的中文字串：{}",
                    n + 1,
                    line.trim()
                );
            }
        }
    }
}
