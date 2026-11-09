//! 選單維護與側欄樹狀選單 — specs/features/azz/menu_maintenance.feature（MN1–MN16）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::{Actor, authorization::PORTED_PROGRAMS};
use ttrs_shared::{MenuItemInput, MenuNodeDto, PermissionClassInput, ProgramInput, ProgramNameDto};

use crate::{
    ErpWorld,
    steps::{azz_authorization::set_class_permission, common::rows},
};

async fn add(w: &mut ErpWorld, menu: &str, program: &str, seq: Option<i32>) {
    let input = MenuItemInput {
        program_no: program.to_owned(),
        seq,
    };
    let r = w.menus.add(&w.actor, menu, input).await;
    w.last = Some(r.map(drop));
}

async fn item_list(w: &ErpWorld, menu: &str) -> Vec<(i32, String)> {
    w.menus
        .items(&w.actor, menu, "zh-TW")
        .await
        .expect("查詢目錄單身")
        .into_iter()
        .map(|i| (i.seq, i.program_no))
        .collect()
}

/// 以使用者的身分（權限類別取自使用者資料）取得側欄
async fn tree(w: &mut ErpWorld, user: &str, lang: &str) {
    let u = w.users.get(&w.actor, user).await.expect("使用者");
    let actor = Actor {
        user_id: u.user_id.clone(),
        dept: u.dept_code.clone().unwrap_or_default(),
        permission_class: u.permission_class.clone(),
        ..Actor::default()
    };
    w.menu_tree = Some(w.menus.tree(&actor, lang).await.expect("取得側欄"));
}

fn menu_tree(w: &ErpWorld) -> &[MenuNodeDto] {
    w.menu_tree.as_deref().expect("尚未取得側欄")
}

fn flatten(nodes: &[MenuNodeDto], level: usize, out: &mut Vec<(usize, String, bool)>) {
    for n in nodes {
        out.push((level, n.program_no.clone(), n.is_menu()));
        flatten(&n.children, level + 1, out);
    }
}

// ----- 假設 -----

#[given(expr = "已有選單程式 {string}、{string}、{string}")]
async fn menu_programs(w: &mut ErpWorld, a: String, b: String, c: String) {
    for no in [a, b, c] {
        let req = ProgramInput {
            program_no: no.clone(),
            module_code: "MENU".into(),
            ..Default::default()
        };
        w.programs
            .create(&w.actor, req)
            .await
            .unwrap_or_else(|e| panic!("登錄選單程式 {no} 失敗：{e}"));
    }
}

#[given(expr = "權限類別 {string} 擁有全部已移植作業的權限")]
async fn class_has_all(w: &mut ErpWorld, class: String) {
    for (no, ..) in PORTED_PROGRAMS {
        set_class_permission(w, &class, no, None, ["0", "0", "0"]).await;
    }
}

#[given(regex = r#"^目錄 "(\S+)" 已有 "(\S+)"（序號 (\d+)）$"#)]
async fn has_item(w: &mut ErpWorld, menu: String, program: String, seq: i32) {
    let input = MenuItemInput {
        program_no: program.clone(),
        seq: Some(seq),
    };
    w.menus
        .add(&w.actor, &menu, input)
        .await
        .unwrap_or_else(|e| panic!("目錄 {menu} 加入 {program} 失敗：{e}"));
}

#[given(expr = "權限類別 {string} 的預設選單為 {string}")]
async fn class_default_menu(w: &mut ErpWorld, class: String, menu: String) {
    let c = w.classes.get(&w.actor, &class).await.expect("權限類別");
    let req = PermissionClassInput {
        default_menu: Some(menu),
        ..c.to_input()
    };
    w.classes
        .update(&w.actor, &class, req)
        .await
        .unwrap_or_else(|e| panic!("設定預設選單失敗：{e}"));
}

#[given(expr = "權限類別 {string} 沒有作業 {string} 的權限")]
async fn class_lacks(w: &mut ErpWorld, class: String, program: String) {
    w.permissions
        .revoke(&w.actor, &class, &program)
        .await
        .unwrap_or_else(|e| panic!("移除權限失敗：{e}"));
}

#[given(expr = "程式 {string} 的英文名稱為 {string}")]
async fn english_name(w: &mut ErpWorld, program: String, name: String) {
    let dto = ProgramNameDto {
        lang: "en".into(),
        name,
        report_title: None,
    };
    w.programs
        .set_name(&w.actor, &program, dto)
        .await
        .unwrap_or_else(|e| panic!("設定英文名稱失敗：{e}"));
}

// ----- 當 -----

#[when(expr = "使用者在目錄 {string} 加入程式:")]
async fn add_rows(w: &mut ErpWorld, menu: String, step: &Step) {
    for r in rows(step) {
        add(w, &menu, &r[1], Some(r[0].parse().expect("序號"))).await;
        if let Some(Err(e)) = &w.last {
            panic!("加入 {} 失敗：{e}", r[1]);
        }
    }
}

#[when(expr = "使用者在目錄 {string} 加入程式 {string}")]
async fn add_one(w: &mut ErpWorld, menu: String, program: String) {
    add(w, &menu, &program, None).await;
}

#[when(expr = "使用者在目錄 {string} 以序號 {int} 加入程式 {string}")]
async fn add_with_seq(w: &mut ErpWorld, menu: String, seq: i32, program: String) {
    add(w, &menu, &program, Some(seq)).await;
}

#[when(expr = "使用者將目錄 {string} 的 {string} 上移")]
async fn move_up(w: &mut ErpWorld, menu: String, program: String) {
    let r = w.menus.shift(&w.actor, &menu, &program, true).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者將目錄 {string} 重新編號")]
async fn renumber(w: &mut ErpWorld, menu: String) {
    let r = w.menus.renumber(&w.actor, &menu).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者將目錄 {string} 複製為 {string}")]
async fn copy(w: &mut ErpWorld, from: String, to: String) {
    let r = w.menus.copy(&w.actor, &from, &to).await;
    w.last = Some(r);
}

#[when(expr = "使用者刪除目錄 {string}")]
async fn delete(w: &mut ErpWorld, menu: String) {
    let r = w.menus.delete(&w.actor, &menu).await;
    w.last = Some(r);
}

#[when(expr = "使用者 {string} 取得側欄選單")]
async fn get_tree(w: &mut ErpWorld, user: String) {
    tree(w, &user, "zh-TW").await;
}

#[when(expr = "使用者 {string} 以語系 {string} 取得側欄選單")]
async fn get_tree_in(w: &mut ErpWorld, user: String, lang: String) {
    tree(w, &user, &lang).await;
}

// ----- 那麼 -----

#[then(expr = "目錄 {string} 的單身應為 {string}")]
async fn items_are(w: &mut ErpWorld, menu: String, expected: String) {
    let got: Vec<String> = item_list(w, &menu)
        .await
        .into_iter()
        .map(|(_, p)| p)
        .collect();
    assert_eq!(got.join(","), expected);
}

#[then(expr = "目錄 {string} 中 {string} 的序號應為 {int}")]
async fn seq_of(w: &mut ErpWorld, menu: String, program: String, seq: i32) {
    let got = item_list(w, &menu)
        .await
        .into_iter()
        .find(|(_, p)| *p == program)
        .map(|(s, _)| s);
    assert_eq!(got, Some(seq));
}

#[then(expr = "目錄 {string} 的序號應為 {string}")]
async fn seqs_are(w: &mut ErpWorld, menu: String, expected: String) {
    let got: Vec<String> = item_list(w, &menu)
        .await
        .into_iter()
        .map(|(s, _)| s.to_string())
        .collect();
    assert_eq!(got.join(","), expected);
}

#[then("側欄選單應為:")]
fn tree_is(w: &mut ErpWorld, step: &Step) {
    let mut got = Vec::new();
    flatten(menu_tree(w), 1, &mut got);
    let expected: Vec<(usize, String, bool)> = rows(step)
        .into_iter()
        .map(|r| (r[0].parse().expect("層級"), r[1].clone(), r[2] == "目錄"))
        .collect();
    assert_eq!(got, expected);
}

#[then(expr = "側欄選單的第一層應為 {string}")]
fn first_level(w: &mut ErpWorld, expected: String) {
    let got: Vec<&str> = menu_tree(w).iter().map(|n| n.program_no.as_str()).collect();
    assert_eq!(got.join(","), expected);
}

#[then("側欄選單應為空")]
fn tree_empty(w: &mut ErpWorld) {
    assert!(menu_tree(w).is_empty(), "側欄應為空：{:?}", menu_tree(w));
}

#[then(expr = "側欄選單的第一層名稱應為 {string}")]
fn first_name(w: &mut ErpWorld, expected: String) {
    let got: Vec<&str> = menu_tree(w).iter().map(|n| n.name.as_str()).collect();
    assert_eq!(got.join(","), expected);
}
