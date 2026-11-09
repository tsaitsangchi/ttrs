//! 選單（TipTop: zm_file 目錄檔；p_zm 系統目錄資料建立作業、cl_create_4sm 個人選單）
//! 規格：specs/features/azz/menu_maintenance.feature（MN1–MN16）
//!
//! 目錄本身是模組 MENU 的程式；單身為（序號, 子項程式）。子項本身有單身時就是下一層目錄（cl_create_4sm.4gl:218-224）。

use std::collections::{BTreeMap, BTreeSet};

use crate::error::DomainError;

/// 新增單身未指定序號時，接在最大序號之後的間隔（MN6）
pub const SEQ_STEP: i32 = 10;
/// 目錄層數上限（MN4，同程式權限依選單產生）
pub const MAX_DEPTH: usize = 20;

/// 目錄的一筆單身（zm01, zm03, zm04）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    pub menu: String,
    pub seq: i32,
    pub program: String,
}

/// 新增單身的序號：未指定時為最大序號＋10；指定時須大於 0
pub fn next_seq(items: &[MenuItem], requested: Option<i32>) -> Result<i32, DomainError> {
    match requested {
        Some(s) if s <= 0 => Err(DomainError::MenuSeqInvalid),
        Some(s) => {
            if items.iter().any(|i| i.seq == s) {
                Err(DomainError::MenuSeqDuplicate)
            } else {
                Ok(s)
            }
        }
        None => Ok(items.iter().map(|i| i.seq).max().unwrap_or(0) + SEQ_STEP),
    }
}

/// 所有單身 → 目錄 → 依序號排列的子項
pub fn children_map(all: &[MenuItem]) -> BTreeMap<&str, Vec<&MenuItem>> {
    let mut map: BTreeMap<&str, Vec<&MenuItem>> = BTreeMap::new();
    for i in all {
        map.entry(i.menu.as_str()).or_default().push(i);
    }
    for v in map.values_mut() {
        v.sort_by_key(|i| i.seq);
    }
    map
}

/// 把 `child` 加入 `menu` 會不會形成迴圈：child 就是 menu，或 menu 在 child 的下層（azz-702，p_zm.4gl:784-792、:1346-1395）
pub fn creates_cycle(all: &[MenuItem], menu: &str, child: &str) -> bool {
    if menu == child {
        return true;
    }
    let map = children_map(all);
    let mut seen = BTreeSet::new();
    let mut stack = vec![child];
    while let Some(m) = stack.pop() {
        if !seen.insert(m) {
            continue;
        }
        if let Some(items) = map.get(m) {
            for i in items {
                if i.program == menu {
                    return true;
                }
                stack.push(i.program.as_str());
            }
        }
    }
    false
}

/// 重新編號（p_zm_b_y，p_zm.4gl:931-960）：依目前順序改為 10、20、30…
pub fn renumber(items: &mut [MenuItem]) {
    items.sort_by_key(|i| i.seq);
    for (n, i) in items.iter_mut().enumerate() {
        i.seq = i32::try_from(n + 1).unwrap_or(i32::MAX / SEQ_STEP) * SEQ_STEP;
    }
}

/// 上移（`up`）或下移：與相鄰的單身交換序號（TTRS 新增，MN6）。已在最前或最後時不變
pub fn shift(items: &mut [MenuItem], program: &str, up: bool) -> Result<(), DomainError> {
    items.sort_by_key(|i| i.seq);
    let pos = items
        .iter()
        .position(|i| i.program == program)
        .ok_or(DomainError::MenuItemNotFound)?;
    let other = if up {
        pos.checked_sub(1)
    } else {
        Some(pos + 1).filter(|p| *p < items.len())
    };
    if let Some(o) = other {
        let s = items[pos].seq;
        items[pos].seq = items[o].seq;
        items[o].seq = s;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(menu: &str, seq: i32, program: &str) -> MenuItem {
        MenuItem {
            menu: menu.into(),
            seq,
            program: program.into(),
        }
    }

    #[test]
    fn next_seq_defaults_after_max() {
        let items = vec![item("m", 10, "a"), item("m", 35, "b")];
        assert_eq!(next_seq(&items, None), Ok(45));
        assert_eq!(
            next_seq(&items, Some(35)),
            Err(DomainError::MenuSeqDuplicate)
        );
        assert_eq!(next_seq(&items, Some(0)), Err(DomainError::MenuSeqInvalid));
    }

    #[test]
    fn cycle_detection() {
        let all = vec![item("main", 10, "aoo"), item("aoo", 10, "sub")];
        assert!(creates_cycle(&all, "aoo", "aoo"));
        assert!(creates_cycle(&all, "aoo", "main"));
        assert!(creates_cycle(&all, "sub", "main"));
        assert!(!creates_cycle(&all, "main", "other"));
    }

    #[test]
    fn shift_and_renumber() {
        let mut items = vec![item("m", 10, "a"), item("m", 15, "b"), item("m", 40, "c")];
        shift(&mut items, "c", true).unwrap();
        renumber(&mut items);
        let order: Vec<(&str, i32)> = items.iter().map(|i| (i.program.as_str(), i.seq)).collect();
        assert_eq!(order, [("a", 10), ("c", 20), ("b", 30)]);
    }
}
