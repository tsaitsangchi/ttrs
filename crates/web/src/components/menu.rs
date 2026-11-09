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
