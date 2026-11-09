//! 側欄樹狀選單的即時更新（menu_maintenance.feature MN15）：選單維護變更後，側欄立刻重新載入

use dioxus::prelude::*;

/// 選單版本：選單維護每次成功變更就加 1，側欄依此重新取得樹狀選單
#[derive(Clone, Copy)]
pub struct MenuVersion(pub Signal<u32>);

/// 在外框提供選單版本（App 呼叫一次）
pub fn provide_menu_version() {
    use_context_provider(|| MenuVersion(Signal::new(0)));
}

/// 通知側欄重新載入
pub fn refresh_menu(v: MenuVersion) {
    let mut s = v.0;
    *s.write() += 1;
}

/// 側欄收合的目錄（目錄代號）：記在這個瀏覽器（localStorage），讀寫失敗時就不記，畫面照常顯示（預設全部展開）
#[derive(Clone, Copy)]
pub struct Collapsed(pub Signal<std::collections::BTreeSet<String>>);

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const COLLAPSED_KEY: &str = "ttrs.menu.collapsed";

/// 讀取上次收合的目錄
pub fn load_collapsed() -> std::collections::BTreeSet<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let saved = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(COLLAPSED_KEY).ok().flatten())
            .unwrap_or_default();
        saved
            .split(',')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::collections::BTreeSet::new()
    }
}

/// 記住收合的目錄
pub fn save_collapsed(set: &std::collections::BTreeSet<String>) {
    #[cfg(target_arch = "wasm32")]
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let v: Vec<&str> = set.iter().map(String::as_str).collect();
        let _ = s.set_item(COLLAPSED_KEY, &v.join(","));
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = set;
}
