//! 權限檢查 Authorization — specs/features/azz/authorization.feature（AQ1–AQ13）
//!
//! 「假設」步驟備妥資料時不檢查權限；只有「目前使用者執行 …」等操作步驟以真正的 Authorizer 檢查（AQ12）。

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::{
    Actor, ActorKind, AppError,
    authorization::{PORTED_PROGRAMS, my_permission},
};
use ttrs_domain::{DomainError, authorization::DenyReason};
use ttrs_shared::{
    ColumnTextDto, CreateItemRequest, PermissionClassInput, PlantInput, ProgramInput,
    ProgramNameDto, ProgramPermissionInput, QbeCondition, QueryRequest, SpecialPermissionKindDto,
    TableDictionaryInput, TableTextDto, UserInput, UserPermissionInput,
};

use super::{
    azz_program_maintenance::seed,
    common::{rows, set_query_result},
};
use crate::ErpWorld;

/// 以指定的資料所有者、所有部門建立資料的身分；權限類別空白，新增程式時不自動授權
fn owner(user: &str, dept: &str) -> Actor {
    Actor {
        user_id: user.into(),
        dept: dept.into(),
        ..Actor::default()
    }
}

fn reason(label: &str) -> DenyReason {
    match label {
        "沒有本作業權限" => DenyReason::NoProgram,
        "沒有此功能" => DenyReason::NoAction,
        "他人資料" => DenyReason::OtherUser,
        "他部門資料" => DenyReason::OtherDept,
        "單身" => DenyReason::DetailRow,
        "AI 不可執行" => DenyReason::AiNotAllowed,
        other => panic!("未定義的拒絕原因：{other}"),
    }
}

fn split_actions(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(str::to_owned)
        .collect()
}

/// 已移植作業的全部功能（PORTED_PROGRAMS）
fn program_actions(program: &str) -> Vec<String> {
    let (_, _, actions) = PORTED_PROGRAMS
        .iter()
        .find(|(no, ..)| *no == program)
        .unwrap_or_else(|| panic!("{program} 不是已移植的作業"));
    split_actions(actions)
}

/// 設定權限類別對作業的權限（先移除再新增）；`actions` 為 None 時為作業的全部功能
async fn set_class_permission(
    w: &ErpWorld,
    class: &str,
    program: &str,
    actions: Option<Vec<String>>,
    levels: [&str; 3],
) {
    let _ = w.permissions.revoke(&w.actor, class, program).await;
    let input = ProgramPermissionInput {
        program_no: program.to_owned(),
        actions,
        other_user_data: Some(levels[0].to_owned()),
        other_dept_data: Some(levels[1].to_owned()),
        detail_rows: Some(levels[2].to_owned()),
    };
    w.permissions
        .grant(&w.actor, class, input)
        .await
        .unwrap_or_else(|e| panic!("設定 {class} 對 {program} 的權限失敗：{e}"));
}

/// 使用者需要的選單程式（MENU 模組，不授權）
async fn ensure_menu(w: &ErpWorld) {
    let sql =
        "INSERT INTO module (module_code, name) VALUES ('MENU', 'MENU') ON CONFLICT DO NOTHING";
    seed(w, |s| s.add_module("MENU"), sql, &[]).await;
    let req = ProgramInput {
        program_no: "menu_main".into(),
        module_code: "MENU".into(),
        ..Default::default()
    };
    match w.programs.create(&owner("tiptop", ""), req).await {
        Ok(_) => {}
        Err(e) if e.code() == "PROGRAM_ALREADY_EXISTS" => {}
        Err(e) => panic!("建立選單 menu_main 失敗：{e}"),
    }
}

fn user_input(id: &str) -> UserInput {
    UserInput {
        user_id: id.into(),
        name: Some(id.into()),
        dept_code: Some("SAL".into()),
        permission_class: "SALES".into(),
        menu: "menu_main".into(),
        default_plant: "TW01".into(),
        ..Default::default()
    }
}

/// 執行操作前先讀出資料（不檢查權限），以便修改時沿用其餘欄位
macro_rules! unchecked {
    ($w:expr, $e:expr) => {{
        $w.authz.set_enforced(false);
        let r = $e.await;
        $w.authz.set_enforced(true);
        r
    }};
}

/// 「目前使用者執行 <操作>」：操作名稱 → use case（權限檢查已啟用）
async fn perform(w: &ErpWorld, op: &str) -> Result<(), AppError> {
    let a = &w.actor;
    let words: Vec<&str> = op.split(' ').collect();
    let arg = |i: usize| words.get(i).copied().unwrap_or_default();
    match words.as_slice() {
        // p_zz 程式維護
        ["查詢程式"] => w.programs.query(a, QueryRequest::default()).await.map(drop),
        ["查看程式", no] => w.programs.get(a, no).await.map(drop),
        ["新增程式", no] => {
            let req = ProgramInput {
                program_no: (*no).into(),
                module_code: "AIM".into(),
                ..Default::default()
            };
            w.programs.create(a, req).await.map(drop)
        }
        ["修改程式", no] => {
            let mut req = unchecked!(w, w.programs.get(a, no))?.to_input();
            req.remark = Some("修改".into());
            w.programs.update(a, no, req).await.map(drop)
        }
        ["設定程式", no, "的名稱"] => {
            let name = ProgramNameDto {
                lang: "zh-TW".into(),
                name: "名稱".into(),
                report_title: None,
            };
            w.programs.set_name(a, no, name).await
        }
        ["刪除程式", no] => w.programs.delete(a, no).await,
        ["複製程式", no] => w.programs.copy(a, no, "aimi201").await.map(drop),
        ["授權程式", no, "給", class] => w.programs.grant(a, no, class).await,
        // p_zta 資料字典維護（含 TipTop 對照目錄）
        ["查詢資料字典"] => w.dictionary.list(a).await.map(drop),
        ["查看資料表", table, "的欄位"] => w.dictionary.columns(a, table).await.map(drop),
        ["新增資料字典", table] => w.dictionary.create(a, dict_input(table)).await.map(drop),
        ["修改資料字典", table] => w
            .dictionary
            .update(a, table, dict_input(table))
            .await
            .map(drop),
        ["刪除資料字典", table] => w.dictionary.delete(a, table).await,
        ["設定資料表", table, "的說明"] => {
            let text = TableTextDto {
                lang: "zh-TW".into(),
                name: Some("料件".into()),
                purpose: None,
                remark: None,
            };
            w.dictionary.set_table_text(a, table, text).await
        }
        ["設定欄位", column, "的說明"] => {
            let (table, column) = column.split_once('.').expect("資料表.欄位");
            let text = ColumnTextDto {
                column_name: column.into(),
                lang: "zh-TW".into(),
                name: Some("名稱".into()),
                description: None,
                extra: None,
            };
            w.dictionary.set_column_text(a, table, text).await
        }
        ["匯出資料字典", table] => w.dictionary.export_csv(a, table, "zh-TW").await.map(drop),
        ["查詢", "TipTop", "對照目錄"] => w.legacy.search(a, "").await.map(drop),
        ["匯入", "TipTop", "對照目錄"] => w.legacy.import(a, Vec::new()).await.map(drop),
        // p_zx 使用者維護
        ["查詢使用者"] => w.users.query(a, QueryRequest::default()).await.map(drop),
        ["查看使用者", id] => w.users.get(a, id).await.map(drop),
        ["新增使用者", id] => w.users.create(a, user_input(id)).await.map(drop),
        ["新增使用者，代號空白"] => w.users.create(a, user_input("")).await.map(drop),
        ["修改使用者", id] => {
            let mut req = unchecked!(w, w.users.get(a, id))?.to_input();
            req.name = Some("修改".into());
            w.users.update(a, id, req).await.map(drop)
        }
        ["刪除使用者", id] => w.users.delete(a, id).await,
        ["複製使用者", id] => w.users.copy(a, id, "u101").await.map(drop),
        ["停用使用者", id] => w.users.deactivate(a, id).await.map(drop),
        ["啟用使用者", id] => w.users.activate(a, id).await.map(drop),
        // p_zw 權限類別維護
        ["查詢權限類別"] => w.classes.query(a, QueryRequest::default()).await.map(drop),
        ["查看權限類別", code] => w.classes.get(a, code).await.map(drop),
        ["新增權限類別", code] => w.classes.create(a, class_input(code)).await.map(drop),
        ["修改權限類別", code] => w.classes.update(a, code, class_input("")).await.map(drop),
        ["停用權限類別", code] => w.classes.deactivate(a, code).await.map(drop),
        ["刪除權限類別", code] => w.classes.delete(a, code).await,
        ["更新所有使用者選單"] => w.classes.update_user_menus(a).await.map(drop),
        // aooi901 營運中心維護
        ["查詢營運中心"] => w.plants.query(a, QueryRequest::default()).await.map(drop),
        ["查看營運中心", id] => w.plants.get(a, id).await.map(drop),
        ["新增營運中心", id] => w.plants.create(a, plant_input(id)).await.map(drop),
        ["修改營運中心", id] => w.plants.update(a, id, plant_input("")).await.map(drop),
        ["停用營運中心", id] => w.plants.deactivate(a, id).await.map(drop),
        ["刪除營運中心", id] => w.plants.delete(a, id).await,
        ["複製營運中心", id] => w.plants.copy(a, id, "TW08").await.map(drop),
        // p_zy 程式執行權限維護
        ["查詢程式執行權限"] => w
            .permissions
            .query(a, QueryRequest::default())
            .await
            .map(drop),
        ["查看權限類別", class, "的權限"] => {
            w.permissions.list(a, class, "zh-TW").await.map(drop)
        }
        ["新增", class, "的程式列", no] => w
            .permissions
            .grant(a, class, permission_input(no))
            .await
            .map(drop),
        ["修改", class, "的程式列", no] => w
            .permissions
            .update(a, class, no, permission_input(no))
            .await
            .map(drop),
        ["移除", class, "的程式列", no] => w.permissions.revoke(a, class, no).await,
        ["刪除", class, "的全部權限"] => w.permissions.revoke_all(a, class).await.map(drop),
        ["複製", class, "的權限到", to] => w.permissions.copy(a, class, to).await.map(drop),
        ["依程式產生", class, "的權限"] => w
            .permissions
            .generate(a, class, &["aimi101".to_owned()])
            .await
            .map(drop),
        ["依選單產生", class, "的權限"] => w
            .permissions
            .generate_by_menu(a, class, "menu_main", false)
            .await
            .map(drop),
        ["依選單刪除", class, "的權限"] => w
            .permissions
            .delete_by_menu(a, class, "menu_main")
            .await
            .map(drop),
        // p_zxw 使用者特殊權限維護
        ["查看使用者", id, "的特殊權限"] => {
            w.special.list(a, id, "zh-TW").await.map(drop)
        }
        ["新增", id, "的特殊權限列"] => {
            let input = UserPermissionInput {
                kind: SpecialPermissionKindDto::Class,
                target: "MGR".into(),
                ..Default::default()
            };
            w.special.add(a, id, input).await.map(drop)
        }
        ["修改", id, "的特殊權限程式列"] => {
            let input = UserPermissionInput {
                kind: SpecialPermissionKindDto::Program,
                target: "aimi101".into(),
                ..Default::default()
            };
            w.special
                .update_program(a, id, "aimi101", input)
                .await
                .map(drop)
        }
        ["移除", id, "的特殊權限列"] => w.special.remove(a, id, 1).await,
        ["刪除", id, "的全部特殊權限"] => w.special.remove_all(a, id).await.map(drop),
        ["複製", id, "的特殊權限"] => w.special.copy(a, id, "u002").await.map(drop),
        // aimi100 料件維護
        ["查詢料件"] => w.items.list(a, "TW01").await.map(drop),
        ["新增料件", no] => {
            let req = CreateItemRequest {
                plant_id: "TW01".into(),
                item_no: (*no).into(),
                name: "螺絲".into(),
                spec: None,
                stock_unit: "PCS".into(),
            };
            w.items.create(a, req).await.map(drop)
        }
        ["刪除料件", no] => w.items.delete(a, "TW01", no).await,
        ["確認料件", no] => w.items.confirm(a, "TW01", no).await.map(drop),
        ["取消確認料件", no] => w.items.unconfirm(a, "TW01", no).await.map(drop),
        _ => panic!("未定義的操作：{op}（{}）", arg(0)),
    }
}

fn dict_input(table: &str) -> TableDictionaryInput {
    TableDictionaryInput {
        table_name: table.into(),
        module_code: "AIM".into(),
        table_type: None,
        upstream: None,
        downstream: None,
    }
}

fn class_input(code: &str) -> PermissionClassInput {
    PermissionClassInput {
        class_code: code.into(),
        name: Some("類別".into()),
        ..Default::default()
    }
}

fn plant_input(id: &str) -> PlantInput {
    PlantInput {
        plant_id: id.into(),
        name: Some("營運中心".into()),
        ..Default::default()
    }
}

fn permission_input(no: &str) -> ProgramPermissionInput {
    ProgramPermissionInput {
        program_no: no.into(),
        actions: None,
        other_user_data: None,
        other_dept_data: None,
        detail_rows: None,
    }
}

/// 啟用權限檢查執行操作，結果記在 w.last
async fn run(w: &mut ErpWorld, op: &str) {
    w.authz.set_enforced(true);
    let result = perform(w, op).await;
    w.authz.set_enforced(false);
    w.last = Some(result);
}

// ----- 假設 -----

#[given("已登錄 TTRS 已移植的作業")]
async fn ported_programs(w: &mut ErpWorld) {
    for (no, module, actions) in PORTED_PROGRAMS {
        let req = ProgramInput {
            program_no: (*no).into(),
            module_code: (*module).into(),
            default_actions: Some((*actions).into()),
            ..Default::default()
        };
        w.programs
            .create(&owner("tiptop", ""), req)
            .await
            .unwrap_or_else(|e| panic!("登錄 {no} 失敗：{e}"));
    }
}

#[given(expr = "已有使用者 {string}，資料所有者 {string}，所有部門 {string}")]
async fn user_owned_by(w: &mut ErpWorld, id: String, user: String, dept: String) {
    ensure_menu(w).await;
    w.users
        .create(&owner(&user, &dept), user_input(&id))
        .await
        .unwrap_or_else(|e| panic!("建立使用者 {id} 失敗：{e}"));
}

#[given(expr = "已有程式 {string} 模組 {string}，資料所有者 {string}，所有部門 {string}")]
async fn program_owned_by(
    w: &mut ErpWorld,
    no: String,
    module: String,
    user: String,
    dept: String,
) {
    let req = ProgramInput {
        program_no: no,
        module_code: module,
        ..Default::default()
    };
    w.programs.create(&owner(&user, &dept), req).await.unwrap();
}

#[given(expr = "營運中心 {string} 已啟用，資料所有者 {string}，所有部門 {string}")]
async fn plant_owned_by(w: &mut ErpWorld, id: String, user: String, dept: String) {
    w.plants
        .create(&owner(&user, &dept), plant_input(&id))
        .await
        .unwrap();
}

#[given(expr = "權限類別 {string} 已存在，資料所有者 {string}，所有部門 {string}")]
async fn class_owned_by(w: &mut ErpWorld, code: String, user: String, dept: String) {
    w.classes
        .create(&owner(&user, &dept), class_input(&code))
        .await
        .unwrap();
}

#[given(
    expr = "權限類別 {string} 對作業 {string} 的權限為 {string}，他人資料 {string}，他部門資料 {string}，單身 {string}"
)]
async fn class_permission(
    w: &mut ErpWorld,
    class: String,
    program: String,
    actions: String,
    user: String,
    dept: String,
    detail: String,
) {
    let actions = Some(split_actions(&actions));
    set_class_permission(w, &class, &program, actions, [&user, &dept, &detail]).await;
}

#[given(
    expr = "權限類別 {string} 對作業 {string} 擁有全部功能，他人資料 {string}，他部門資料 {string}，單身 {string}"
)]
async fn class_full_permission(
    w: &mut ErpWorld,
    class: String,
    program: String,
    user: String,
    dept: String,
    detail: String,
) {
    set_class_permission(w, &class, &program, None, [&user, &dept, &detail]).await;
}

#[given(
    expr = "使用者 {string} 有特殊權限程式 {string}，允許執行功能 {string}，他人資料 {string}，他部門資料 {string}，單身 {string}"
)]
async fn special_program_permission(
    w: &mut ErpWorld,
    user: String,
    program: String,
    actions: String,
    other_user: String,
    other_dept: String,
    detail: String,
) {
    let input = UserPermissionInput {
        kind: SpecialPermissionKindDto::Program,
        target: program,
        include_own_class: false,
        actions: Some(split_actions(&actions)),
        other_user_data: Some(other_user),
        other_dept_data: Some(other_dept),
        detail_rows: Some(detail),
    };
    w.special.add(&w.actor, &user, input).await.unwrap();
}

#[given("目前使用者以 AI 代理身分操作")]
fn acting_as_ai(w: &mut ErpWorld) {
    w.actor.kind = ActorKind::Ai;
}

// ----- 當 -----

#[when(expr = "目前使用者執行 {string}")]
async fn actor_runs(w: &mut ErpWorld, op: String) {
    run(w, &op).await;
}

#[when(expr = "權限類別 {string} 在作業 {string} 沒有功能 {string} 時，目前使用者執行 {string}")]
async fn runs_without_action(
    w: &mut ErpWorld,
    class: String,
    program: String,
    action: String,
    op: String,
) {
    let actions = program_actions(&program)
        .into_iter()
        .filter(|a| *a != action)
        .collect();
    set_class_permission(w, &class, &program, Some(actions), ["0", "0", "0"]).await;
    run(w, &op).await;
}

#[when(expr = "權限類別 {string} 在作業 {string} 擁有全部功能時，目前使用者執行 {string}")]
async fn runs_with_all_actions(w: &mut ErpWorld, class: String, program: String, op: String) {
    set_class_permission(w, &class, &program, None, ["0", "0", "0"]).await;
    run(w, &op).await;
}

#[when(expr = "目前使用者以條件 {string} 查詢使用者")]
async fn actor_queries_users(w: &mut ErpWorld, condition: String) {
    let (label, expr) = condition.split_once(' ').expect("欄位 條件");
    let field = match label {
        "使用者代號" => "user_id",
        other => panic!("未定義的查詢欄位：{other}"),
    };
    let req = QueryRequest {
        conditions: vec![QbeCondition {
            field: field.into(),
            expr: expr.into(),
        }],
        ..Default::default()
    };
    w.authz.set_enforced(true);
    let result = w.users.query(&w.actor, req).await;
    w.authz.set_enforced(false);
    set_query_result(w, result, |u| u.user_id.clone());
}

#[when(expr = "目前使用者取得 {string} 的全部清單")]
async fn actor_lists(w: &mut ErpWorld, list: String) {
    w.authz.set_enforced(true);
    let result = match list.as_str() {
        "程式" => w.programs.list().await.map(|_| ()),
        "權限類別" => w.classes.list().await.map(|_| ()),
        "使用者" => w.users.list().await.map(|_| ()),
        "營運中心" => w.plants.list().await.map(|_| ()),
        other => panic!("未定義的清單：{other}"),
    };
    w.authz.set_enforced(false);
    w.last = Some(result);
}

#[when(expr = "查看目前使用者在作業 {string} 的有效權限")]
async fn view_effective(w: &mut ErpWorld, program: String) {
    let p = my_permission(w.authz.as_ref(), &w.actor, &program)
        .await
        .unwrap();
    w.my_permission = Some(p);
}

// ----- 那麼 -----

#[then(expr = "應被拒絕，作業 {string}、功能 {string}、原因 {string}")]
fn denied(w: &mut ErpWorld, program: String, action: String, label: String) {
    match &w.last {
        Some(Err(AppError::Domain(DomainError::PermissionDenied {
            program: p,
            action: a,
            reason: r,
        }))) => assert_eq!(
            (p.as_str(), a.as_str(), *r),
            (program.as_str(), action.as_str(), reason(&label))
        ),
        other => panic!("預期被拒絕（{program} {action} {label}），實際：{other:?}"),
    }
}

#[then("不應被權限拒絕")]
fn not_denied(w: &mut ErpWorld) {
    match &w.last {
        Some(Err(e)) => assert_ne!(e.code(), "PERMISSION_DENIED", "{e}"),
        Some(Ok(())) => {}
        None => panic!("尚未執行操作"),
    }
}

fn effective(w: &ErpWorld) -> &ttrs_shared::MyPermissionDto {
    let p = w.my_permission.as_ref().expect("尚未查看有效權限");
    assert!(p.allowed, "沒有本作業的權限");
    p
}

#[then("有效權限應為:")]
fn effective_is(w: &mut ErpWorld, step: &Step) {
    let p = effective(w);
    for r in rows(step) {
        let actual = match r[0].as_str() {
            "允許執行功能" => p.actions.join(", "),
            "他人資料" => p.other_user_data.clone(),
            "他部門資料" => p.other_dept_data.clone(),
            "單身" => p.detail_rows.clone(),
            other => panic!("未定義的欄位：{other}"),
        };
        assert_eq!(actual, r[1], "{}", r[0]);
    }
}

#[then(regex = r#"^有效權限的(他人資料|他部門資料|單身)應為 "(\d)"$"#)]
fn effective_level_is(w: &mut ErpWorld, field: String, expected: String) {
    let p = effective(w);
    let actual = match field.as_str() {
        "他人資料" => &p.other_user_data,
        "他部門資料" => &p.other_dept_data,
        _ => &p.detail_rows,
    };
    assert_eq!(actual, &expected, "{field}");
}
