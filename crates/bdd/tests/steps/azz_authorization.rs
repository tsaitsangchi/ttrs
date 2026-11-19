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
    ColumnTextDto, PermissionClassInput, PlantInput, ProgramInput, ProgramNameDto,
    ProgramPermissionInput, QbeCondition, QueryRequest, SpecialPermissionKindDto,
    TableDictionaryInput, TableTextDto, UserInput, UserPermissionInput,
};

use super::{
    azz_program_maintenance::seed,
    azz_user_maintenance::seed_user,
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
    let (_, _, _, actions) = PORTED_PROGRAMS
        .iter()
        .find(|(no, ..)| *no == program)
        .unwrap_or_else(|| panic!("{program} 不是已移植的作業"));
    split_actions(actions)
}

/// 設定權限類別對作業的權限（先移除再新增）；`actions` 為 None 時為作業的全部功能
pub(crate) async fn set_class_permission(
    w: &ErpWorld,
    class: &str,
    program: &str,
    actions: Option<Vec<String>>,
    levels: [&str; 3],
) {
    let _ = w.permissions.revoke(&w.actor, class, program).await;
    let input = ProgramPermissionInput {
        version: None,
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
        program_type: Some("I".into()),
        program_no: "menu-main".into(),
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
        menu: "menu-main".into(),
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

/// 基本代碼的操作：「查詢區域」「新增國別」「匯出地區為」… → (動作, 種類)
fn basic_op(verb: &str) -> Option<(&'static str, ttrs_domain::basic_code::BasicCodeKind)> {
    use ttrs_domain::basic_code::BasicCodeKind as K;
    let kinds = [
        ("區域", K::Region),
        ("國別", K::Country),
        ("地區", K::Area),
        ("交運方式", K::ShippingMethod),
        ("報表抬頭", K::ReportTitle),
    ];
    let verb = verb.strip_suffix('為').unwrap_or(verb);
    let actions = [
        "取消確認",
        "查詢",
        "新增",
        "修改",
        "停用",
        "刪除",
        "複製",
        "確認",
        "列印",
        "匯出",
    ];
    for action in actions {
        if let Some(rest) = verb.strip_prefix(action)
            && let Some((_, k)) = kinds.iter().find(|(n, _)| *n == rest)
        {
            return Some((action, *k));
        }
    }
    None
}

/// 「目前使用者執行 <操作>」：操作名稱 → use case（權限檢查已啟用）
fn date(s: &str) -> chrono::NaiveDate {
    s.parse().unwrap_or_else(|_| panic!("日期格式錯誤：{s}"))
}

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
                program_type: Some("I".into()),
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
        ["刪除程式", no] => w.programs.delete(a, no, None).await,
        ["複製程式", no] => w.programs.copy(a, no, "item-lot").await.map(drop),
        ["列印程式"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.programs
                .report_in(a, Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["匯出程式為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.programs
                .report_in(a, Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["列印程式執行權限"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.permissions
                .report_in(a, Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["匯出程式執行權限為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.permissions
                .report_in(a, Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["查詢使用者特殊權限"] => {
            w.special.query(a, QueryRequest::default()).await.map(drop)
        }
        ["列印使用者特殊權限"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.special.report_in(a, Vec::new(), f, &lang).await.map(drop)
        }
        ["匯出使用者特殊權限為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.special.report_in(a, Vec::new(), f, &lang).await.map(drop)
        }
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
        ["刪除資料字典", table] => w.dictionary.delete(a, table, None).await,
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
        ["匯出資料字典", table, "為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.dictionary.report_in(a, table, f, &lang).await.map(drop)
        }
        ["列印資料字典", table] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.dictionary.report_in(a, table, f, &lang).await.map(drop)
        }
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
        ["刪除使用者", id] => w.users.delete(a, id, None).await,
        ["複製使用者", id] => w.users.copy(a, id, "u101").await.map(drop),
        ["停用使用者", id] => w.users.deactivate(a, id, None).await.map(drop),
        ["解除使用者", id, "的鎖定"] => w.users.unlock(a, id).await,
        ["啟用使用者", id] => w.users.activate(a, id, None).await.map(drop),
        ["確認使用者", id] => w.users.confirm(a, id, true, None).await.map(drop),
        ["取消確認使用者", id] => {
            let _ = unchecked!(w, w.users.confirm(a, id, true, None));
            w.users.confirm(a, id, false, None).await.map(drop)
        }
        ["列印使用者"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.users.report_in(a, Vec::new(), f, &lang).await.map(drop)
        }
        ["匯出使用者為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.users.report_in(a, Vec::new(), f, &lang).await.map(drop)
        }
        // p_zw 權限類別維護
        ["查詢權限類別"] => w.classes.query(a, QueryRequest::default()).await.map(drop),
        ["查看權限類別", code] => w.classes.get(a, code).await.map(drop),
        ["新增權限類別", code] => w.classes.create(a, class_input(code)).await.map(drop),
        ["修改權限類別", code] => w.classes.update(a, code, class_input("")).await.map(drop),
        ["停用權限類別", code] => w.classes.deactivate(a, code, None).await.map(drop),
        ["刪除權限類別", code] => w.classes.delete(a, code, None).await,
        ["更新所有使用者選單"] => w.classes.update_user_menus(a).await.map(drop),
        ["複製權限類別", code] => w.classes.copy(a, code, "COPY1").await.map(drop),
        ["確認權限類別", code] => w.classes.confirm(a, code, true, None).await.map(drop),
        ["取消確認權限類別", code] => {
            let _ = unchecked!(w, w.classes.confirm(a, code, true, None));
            w.classes.confirm(a, code, false, None).await.map(drop)
        }
        ["列印權限類別"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.classes.report_in(a, Vec::new(), f, &lang).await.map(drop)
        }
        ["匯出權限類別為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.classes.report_in(a, Vec::new(), f, &lang).await.map(drop)
        }
        // aooi901 營運中心維護
        ["查詢營運中心"] => w.plants.query(a, QueryRequest::default()).await.map(drop),
        ["查看營運中心", id] => w.plants.get(a, id).await.map(drop),
        ["新增營運中心", id] => w.plants.create(a, plant_input(id)).await.map(drop),
        ["修改營運中心", id] => w.plants.update(a, id, plant_input("")).await.map(drop),
        ["停用營運中心", id] => w.plants.deactivate(a, id, None).await.map(drop),
        ["刪除營運中心", id] => w.plants.delete(a, id, None).await,
        ["複製營運中心", id] => w.plants.copy(a, id, "TW08").await.map(drop),
        ["確認營運中心", id] => w.plants.confirm(a, id, true, None).await.map(drop),
        ["取消確認營運中心", id] => {
            let _ = unchecked!(w, w.plants.confirm(a, id, true, None));
            w.plants.confirm(a, id, false, None).await.map(drop)
        }
        ["列印營運中心"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.plants.report_in(a, Vec::new(), f, &lang).await.map(drop)
        }
        ["匯出營運中心為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.plants.report_in(a, Vec::new(), f, &lang).await.map(drop)
        }
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
            .generate(a, class, &["item-master".to_owned()])
            .await
            .map(drop),
        ["依選單產生", class, "的權限"] => w
            .permissions
            .generate_by_menu(a, class, "menu-main", false)
            .await
            .map(drop),
        ["依選單刪除", class, "的權限"] => w
            .permissions
            .delete_by_menu(a, class, "menu-main")
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
                target: "item-master".into(),
                ..Default::default()
            };
            w.special
                .update_program(a, id, "item-master", input)
                .await
                .map(drop)
        }
        ["移除", id, "的特殊權限列"] => w.special.remove(a, id, 1).await,
        ["刪除", id, "的全部特殊權限"] => w.special.remove_all(a, id).await.map(drop),
        ["複製", id, "的特殊權限"] => w.special.copy(a, id, "u002").await.map(drop),
        // aimi100 料件維護
        ["查詢料件"] => w
            .items
            .query(a, "TW01", QueryRequest::default())
            .await
            .map(drop),
        ["新增料件", no] => {
            let req = ttrs_shared::ItemInput {
                item_no: (*no).into(),
                name: Some("螺絲".into()),
                stock_unit: Some("PCS".into()),
                product_class: Some("P01".into()),
                ..Default::default()
            };
            w.items.create(a, "TW01", req).await.map(drop)
        }
        ["修改料件", no] => {
            let req = ttrs_shared::ItemInput {
                item_no: (*no).into(),
                name: Some("螺絲".into()),
                stock_unit: Some("PCS".into()),
                product_class: Some("P01".into()),
                ..Default::default()
            };
            w.items.update(a, "TW01", no, req).await.map(drop)
        }
        ["刪除料件", no] => w.items.delete(a, "TW01", no, None).await,
        ["確認料件", no] => w.items.confirm(a, "TW01", no, None).await.map(drop),
        ["取消確認料件", no] => w.items.unconfirm(a, "TW01", no, None).await.map(drop),
        ["作廢料件", no] => w.items.toggle_void(a, "TW01", no, None).await.map(drop),
        ["切換料件", no, "的訂單無效"] => w
            .items
            .toggle_order_invalid(a, "TW01", no, None)
            .await
            .map(drop),
        ["複製料件", from, "為", to] => {
            let req = ttrs_shared::CopyItemRequest {
                new_item_no: (*to).into(),
                name: None,
            };
            w.items.copy(a, "TW01", from, req).await.map(drop)
        }
        ["設定料件", no, "的英文品名"] => {
            let t = ttrs_shared::ItemTextDto {
                lang: "en".into(),
                name: Some("Screw".into()),
                spec: None,
            };
            w.items.set_text(a, "TW01", no, t).await.map(drop)
        }
        ["查詢料件", no, "的異動記錄"] => {
            w.items.history(a, "TW01", no, None).await.map(drop)
        }
        // aooi030 部門維護、aooi040 員工維護（工廠層，營運中心 TW01）
        ["查詢部門"] => w
            .departments
            .query(a, "TW01", QueryRequest::default())
            .await
            .map(drop),
        ["新增部門", code] => {
            let req = ttrs_shared::DepartmentInput {
                version: None,
                dept_code: (*code).into(),
                name: Some("部門".into()),
                full_name: None,
            };
            w.departments.create(a, "TW01", req).await.map(drop)
        }
        ["修改部門", code] => {
            let req = ttrs_shared::DepartmentInput {
                version: None,
                dept_code: (*code).into(),
                name: Some("部門".into()),
                full_name: None,
            };
            w.departments.update(a, "TW01", code, req).await.map(drop)
        }
        ["修改部門", code, "的會計資料"] => {
            let req = ttrs_shared::DepartmentAccountingInput {
                accounting_dept: true,
                control_type: "3".into(),
                ..Default::default()
            };
            w.departments
                .set_accounting(a, "TW01", code, req)
                .await
                .map(drop)
        }
        ["刪除部門", code] => w.departments.delete(a, "TW01", code, None).await,
        ["複製部門", code] => w.departments.copy(a, "TW01", code, "D99").await.map(drop),
        ["確認部門", code] => w
            .departments
            .confirm(a, "TW01", code, true, None)
            .await
            .map(drop),
        ["取消確認部門", code] => {
            let _ = unchecked!(w, w.departments.confirm(a, "TW01", code, true, None));
            w.departments
                .confirm(a, "TW01", code, false, None)
                .await
                .map(drop)
        }
        // p_zm 選單維護（menu_maintenance.feature）
        ["查詢選單目錄"] => w.menus.menus(a, "zh-TW").await.map(drop),
        ["在目錄", menu, "加入程式", program] => w
            .menus
            .add(
                a,
                menu,
                ttrs_shared::MenuItemInput {
                    program_no: (*program).into(),
                    seq: None,
                },
            )
            .await
            .map(drop),
        ["複製目錄", from, "為", to] => w.menus.copy(a, from, to).await,
        ["刪除目錄", menu] => w.menus.delete(a, menu).await,
        // 異動記錄（ADR 0012 §8，功能 history）
        ["查詢作業", program, key, "的異動記錄"] => w
            .shared_history
            .history(a, program, key, None)
            .await
            .map(drop),
        ["查詢部門", code, "的異動記錄"] => {
            w.departments.history(a, "TW01", code, None).await.map(drop)
        }
        ["查詢員工", id, "的異動記錄"] => {
            w.employees.history(a, "TW01", id, None).await.map(drop)
        }
        ["查詢分類碼", kind, code, "的異動記錄"] => w
            .classification
            .history(a, "TW01", kind, code, None)
            .await
            .map(drop),
        ["列印料件"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.items
                .report_in(a, "TW01", Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["匯出料件為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.items
                .report_in(a, "TW01", Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["確認料件分群", code] => w
            .item_groups
            .confirm(a, "TW01", code, true, None)
            .await
            .map(drop),
        ["取消確認料件分群", code] => {
            // 先以不檢查權限的方式確認，再檢查取消確認的權限
            let _ = unchecked!(w, w.item_groups.confirm(a, "TW01", code, true, None));
            w.item_groups
                .confirm(a, "TW01", code, false, None)
                .await
                .map(drop)
        }
        ["列印料件分群"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.item_groups
                .report_in(a, "TW01", Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["匯出料件分群為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.item_groups
                .report_in(a, "TW01", Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["列印部門"] => w
            .departments
            .report(a, "TW01", Vec::new(), ttrs_app::report::ReportFormat::Pdf)
            .await
            .map(drop),
        ["匯出部門為", "Excel"] => w
            .departments
            .report(a, "TW01", Vec::new(), ttrs_app::report::ReportFormat::Excel)
            .await
            .map(drop),
        ["查詢員工"] => w
            .employees
            .query(a, "TW01", QueryRequest::default())
            .await
            .map(drop),
        ["新增員工", id] => {
            let req = ttrs_shared::EmployeeInput {
                employee_id: (*id).into(),
                name: Some("員工".into()),
                ..Default::default()
            };
            w.employees.create(a, "TW01", req).await.map(drop)
        }
        ["刪除員工", id] => w.employees.delete(a, "TW01", id, None).await,
        ["複製員工", id] => w.employees.copy(a, "TW01", id, "E99").await.map(drop),
        ["確認員工", id] => w
            .employees
            .confirm(a, "TW01", id, true, None)
            .await
            .map(drop),
        ["取消確認員工", id] => {
            let _ = unchecked!(w, w.employees.confirm(a, "TW01", id, true, None));
            w.employees
                .confirm(a, "TW01", id, false, None)
                .await
                .map(drop)
        }
        ["列印員工"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.employees
                .report_in(a, "TW01", Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["匯出員工為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.employees
                .report_in(a, "TW01", Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        // aooi300 分類碼維護（工廠層，營運中心 TW01）
        ["查詢分類碼", kind] => w
            .classification
            .query(a, "TW01", kind, QueryRequest::default())
            .await
            .map(drop),
        ["複製分類碼", kind, code] => w
            .classification
            .copy(a, "TW01", kind, code, "X99")
            .await
            .map(drop),
        ["確認分類碼", kind, code] => w
            .classification
            .confirm(a, "TW01", kind, code, true, None)
            .await
            .map(drop),
        ["取消確認分類碼", kind, code] => {
            let _ = unchecked!(
                w,
                w.classification.confirm(a, "TW01", kind, code, true, None)
            );
            w.classification
                .confirm(a, "TW01", kind, code, false, None)
                .await
                .map(drop)
        }
        ["列印分類碼", kind] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.classification
                .report_in(a, "TW01", kind, Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["匯出分類碼", kind, "為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.classification
                .report_in(a, "TW01", kind, Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["新增分類碼", kind, code] => {
            let req = ttrs_shared::ClassificationCodeInput {
                code: (*code).into(),
                description: Some("分類".into()),
                ..Default::default()
            };
            w.classification
                .create(a, "TW01", kind, req)
                .await
                .map(drop)
        }
        ["修改分類碼", kind, code] => {
            let req = ttrs_shared::ClassificationCodeInput {
                code: (*code).into(),
                description: Some("分類".into()),
                ..Default::default()
            };
            w.classification
                .update(a, "TW01", kind, code, req)
                .await
                .map(drop)
        }
        ["停用分類碼", kind, code] => w
            .classification
            .set_active(a, "TW01", kind, code, false, None)
            .await
            .map(drop),
        ["刪除分類碼", kind, code] => {
            w.classification.delete(a, "TW01", kind, code, None).await
        }
        // aooi101 單位維護（共用層）
        // 編碼性質維護（aooi401；資料不存在時回 ENCODING_NATURE_NOT_FOUND，不是權限拒絕）
        ["查詢編碼性質"] => w
            .encoding_natures
            .query(a, QueryRequest::default())
            .await
            .map(drop),
        ["新增編碼性質", code] | ["修改編碼性質", code] => {
            let req = ttrs_shared::EncodingNatureInput {
                nature_code: (*code).to_owned(),
                name: Some("料號".into()),
                category: "1".into(),
                version: None,
            };
            if words[0] == "新增編碼性質" {
                w.encoding_natures.create(a, req).await.map(drop)
            } else {
                w.encoding_natures.update(a, code, req).await.map(drop)
            }
        }
        ["停用編碼性質", code] => w
            .encoding_natures
            .set_active(a, code, false, None)
            .await
            .map(drop),
        ["刪除編碼性質", code] => w.encoding_natures.delete(a, code, None).await,
        ["複製編碼性質", code] => w.encoding_natures.copy(a, code, "ZZ").await.map(drop),
        ["確認編碼性質", code] => w
            .encoding_natures
            .confirm(a, code, true, None)
            .await
            .map(drop),
        ["取消確認編碼性質", code] => w
            .encoding_natures
            .confirm(a, code, false, None)
            .await
            .map(drop),
        ["列印編碼性質"] => w
            .encoding_natures
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Pdf,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        ["匯出編碼性質為", "Excel"] => w
            .encoding_natures
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Excel,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        // 編碼類別維護（aooi402；資料不存在時回 ENCODING_CLASS_NOT_FOUND 或性質無效，不是權限拒絕）
        // 料件自動編號（item_auto_number.feature AN6）：參數未開啟時回 ITEM_AUTO_NUMBER_DISABLED，不是權限拒絕
        ["取料號", code] => w
            .encoding_classes
            .next_item_number(a, "TW01", code, &[])
            .await
            .map(drop),
        ["試算編碼類別", code] => w
            .encoding_classes
            .preview(a, "TW01", code, &[])
            .await
            .map(drop),
        ["查詢編碼類別"] => w
            .encoding_classes
            .query(a, QueryRequest::default())
            .await
            .map(drop),
        ["新增編碼類別", code] => w
            .encoding_classes
            .create(
                a,
                ttrs_shared::EncodingClassInput {
                    class_code: (*code).to_owned(),
                    name: Some("成品".into()),
                    nature_code: "A1".into(),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["修改編碼類別", code] => w
            .encoding_classes
            .update(
                a,
                code,
                ttrs_shared::EncodingClassInput {
                    class_code: (*code).to_owned(),
                    name: Some("成品".into()),
                    nature_code: "A1".into(),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["失效編碼類別", code] => w
            .encoding_classes
            .set_active(a, code, false, None)
            .await
            .map(drop),
        ["刪除編碼類別", code] => w.encoding_classes.delete(a, code, None).await,
        ["複製編碼類別", code] => w.encoding_classes.copy(a, code, "COPY").await.map(drop),
        ["維護編碼類別", code, "的各段"] => w
            .encoding_classes
            .add_segment(
                a,
                code,
                ttrs_shared::EncodingClassSegmentInput {
                    segment_type: "3".into(),
                    length: Some(4),
                    ..Default::default()
                },
            )
            .await
            .map(drop),
        ["確認編碼類別", code] => w
            .encoding_classes
            .confirm(a, code, true, None)
            .await
            .map(drop),
        ["取消確認編碼類別", code] => w
            .encoding_classes
            .confirm(a, code, false, None)
            .await
            .map(drop),
        ["列印編碼類別"] => w
            .encoding_classes
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Pdf,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        ["匯出編碼類別為", "Excel"] => w
            .encoding_classes
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Excel,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        // 編碼年對照維護（aooi404；資料不存在時回 ENCODING_YEAR_NOT_FOUND，不是權限拒絕）
        ["查詢編碼年對照"] => w
            .encoding_years
            .query(a, QueryRequest::default())
            .await
            .map(drop),
        ["新增編碼年對照", year] => w
            .encoding_years
            .create(
                a,
                ttrs_shared::EncodingYearInput {
                    year: year.parse().ok(),
                    code_value: "Q".into(),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["修改編碼年對照", year] => w
            .encoding_years
            .update(
                a,
                year.parse().unwrap_or(0),
                1,
                ttrs_shared::EncodingYearInput {
                    year: year.parse().ok(),
                    code_value: "R".into(),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["停用編碼年對照", year] => w
            .encoding_years
            .set_active(a, year.parse().unwrap_or(0), 1, false, None)
            .await
            .map(drop),
        ["刪除編碼年對照", year] => {
            w.encoding_years
                .delete(a, year.parse().unwrap_or(0), 1, None)
                .await
        }
        ["複製編碼年對照", year] => w
            .encoding_years
            .copy(a, year.parse().unwrap_or(0), 1, Some(2099))
            .await
            .map(drop),
        ["確認編碼年對照", year] => w
            .encoding_years
            .confirm(a, year.parse().unwrap_or(0), 1, true, None)
            .await
            .map(drop),
        ["取消確認編碼年對照", year] => w
            .encoding_years
            .confirm(a, year.parse().unwrap_or(0), 1, false, None)
            .await
            .map(drop),
        ["列印編碼年對照"] => w
            .encoding_years
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Pdf,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        ["匯出編碼年對照為", "Excel"] => w
            .encoding_years
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Excel,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        // 基本資料拋轉、拋轉歷史查詢（aoop603、aooq604；沒有資料中心時回 BASIC_CARRY_NOT_SOURCE，不是權限拒絕）
        ["開啟基本資料拋轉"] => w.basic_carry.info(a, "TW01").await.map(drop),
        ["執行基本資料拋轉"] => w
            .basic_carry
            .carry(
                a,
                "TW01",
                ttrs_shared::BasicCarryRequest {
                    items: vec!["department".into()],
                    targets: None,
                },
            )
            .await
            .map(drop),
        ["查詢拋轉歷史"] => w
            .basic_carry
            .history(a, QueryRequest::default())
            .await
            .map(drop),
        ["列印拋轉歷史"] => w
            .basic_carry
            .history_report(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Pdf,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        ["匯出拋轉歷史為", "Excel"] => w
            .basic_carry
            .history_report(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Excel,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        // 資料中心拋轉設定（aooi602；資料不存在時回 CARRY_* 或 GROUP_CENTER_* 錯誤，不是權限拒絕）
        ["查詢拋轉設定"] => w
            .carry_settings
            .query(a, QueryRequest::default())
            .await
            .map(drop),
        ["新增拋轉設定", c, t] => w
            .carry_settings
            .create(
                a,
                ttrs_shared::CarrySettingInput {
                    center_code: (*c).to_owned(),
                    data_type: (*t).to_owned(),
                    mode: "1".into(),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["修改拋轉設定", c, t] => w
            .carry_settings
            .update_mode(
                a,
                c,
                t,
                ttrs_shared::CarrySettingInput {
                    center_code: (*c).to_owned(),
                    data_type: (*t).to_owned(),
                    mode: "2".into(),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["刪除拋轉設定", c, t] => w.carry_settings.delete(a, c, t, None).await,
        ["複製拋轉設定", c, t] => w
            .carry_settings
            .copy(
                a,
                c,
                t,
                ttrs_shared::CarryCopyInput {
                    center_code: (*c).to_owned(),
                    data_type: "4".into(),
                },
            )
            .await
            .map(drop),
        ["新增拋轉設定", c, t, "的拋轉營運中心", plant] => w
            .carry_settings
            .add_target(
                a,
                c,
                t,
                ttrs_shared::CarryTargetInput {
                    plant: (*plant).to_owned(),
                    ..Default::default()
                },
            )
            .await
            .map(drop),
        ["刪除拋轉設定", c, t, "的拋轉營運中心", plant] => w
            .carry_settings
            .remove_target(a, c, t, plant, None)
            .await
            .map(drop),
        ["新增拋轉設定", c, t, "營運中心", plant, "的拋轉條件"] => w
            .carry_settings
            .put_condition(
                a,
                c,
                t,
                plant,
                ttrs_shared::DataControlRuleInput {
                    field: "group_code".into(),
                    condition: "6".into(),
                    values: "FG".into(),
                    version: None,
                },
                false,
            )
            .await
            .map(drop),
        ["新增拋轉設定", c, t, "營運中心", plant, "的預設值"] => w
            .carry_settings
            .put_default(
                a,
                c,
                t,
                plant,
                ttrs_shared::CarryDefaultInput {
                    field: "group_code".into(),
                    value: "RM".into(),
                    version: None,
                },
                false,
            )
            .await
            .map(drop),
        ["列印拋轉設定"] => w
            .carry_settings
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Pdf,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        ["匯出拋轉設定為", "Excel"] => w
            .carry_settings
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Excel,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        // 資料控管原則維護（aooi601；資料不存在時回 DATA_CONTROL_* 錯誤，不是權限拒絕）
        ["查詢資料控管原則"] => w
            .data_controls
            .query(a, QueryRequest::default())
            .await
            .map(drop),
        ["新增資料類別", t, "的受控營運中心", plant] => w
            .data_controls
            .add_plant(
                a,
                ttrs_shared::DataControlPlantInput {
                    data_type: (*t).to_owned(),
                    plant: (*plant).to_owned(),
                },
            )
            .await
            .map(drop),
        ["刪除資料類別", t, "的受控營運中心", plant] => w
            .data_controls
            .remove_plant(a, t, plant, None)
            .await
            .map(drop),
        ["新增資料類別", t, "營運中心", plant, "的欄位條件"] => w
            .data_controls
            .add_rule(
                a,
                t,
                plant,
                ttrs_shared::DataControlRuleInput {
                    field: "item_no".into(),
                    condition: "6".into(),
                    values: "L".into(),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["修改資料類別", t, "營運中心", plant, "的欄位條件"] => w
            .data_controls
            .update_rule(
                a,
                t,
                plant,
                ttrs_shared::DataControlRuleInput {
                    field: "item_no".into(),
                    condition: "6".into(),
                    values: "L".into(),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["刪除資料類別", t, "營運中心", plant, "的欄位條件"] => w
            .data_controls
            .remove_rule(a, t, plant, "item_no", None)
            .await
            .map(drop),
        ["複製資料類別", t, "營運中心", plant, "的欄位條件"] => w
            .data_controls
            .copy_rules(
                a,
                t,
                plant,
                ttrs_shared::DataControlCopyInput {
                    to_plant: "TW03".into(),
                },
            )
            .await
            .map(drop),
        ["列印資料控管原則"] => w
            .data_controls
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Pdf,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        ["匯出資料控管原則為", "Excel"] => w
            .data_controls
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Excel,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        // 集團中心維護（aooi600；資料不存在時回 GROUP_CENTER_NOT_FOUND，不是權限拒絕）
        ["查詢集團中心"] => w
            .group_centers
            .query(a, QueryRequest::default())
            .await
            .map(drop),
        ["新增集團中心", code] => w
            .group_centers
            .create(
                a,
                ttrs_shared::GroupCenterInput {
                    center_code: (*code).to_owned(),
                    name: None,
                    category: Some("1".into()),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["修改集團中心", code] => w
            .group_centers
            .update(
                a,
                code,
                ttrs_shared::GroupCenterInput {
                    center_code: (*code).to_owned(),
                    name: None,
                    category: Some("1".into()),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["失效集團中心", code] => w
            .group_centers
            .set_active(a, code, false, None)
            .await
            .map(drop),
        ["刪除集團中心", code] => w.group_centers.delete(a, code, None).await,
        ["複製集團中心", code] => w.group_centers.copy(a, code, "COPY").await.map(drop),
        ["設定集團中心", code, "的資料類別"] => w
            .group_centers
            .add_data_type(
                a,
                code,
                ttrs_shared::DataCenterTypeInput {
                    data_type: "1".into(),
                    plant: "TW01".into(),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["確認集團中心", code] => {
            w.group_centers.confirm(a, code, true, None).await.map(drop)
        }
        ["取消確認集團中心", code] => w
            .group_centers
            .confirm(a, code, false, None)
            .await
            .map(drop),
        ["列印集團中心"] => w
            .group_centers
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Pdf,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        ["匯出集團中心為", "Excel"] => w
            .group_centers
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Excel,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        // 編碼獨立段維護（aooi403；資料不存在時回 ENCODING_SEGMENT_NOT_FOUND，不是權限拒絕）
        ["查詢編碼獨立段"] => w
            .encoding_segments
            .query(a, QueryRequest::default())
            .await
            .map(drop),
        ["新增編碼獨立段", code] => w
            .encoding_segments
            .create(
                a,
                ttrs_shared::EncodingSegmentInput {
                    segment_code: (*code).to_owned(),
                    name: None,
                    length: Some(2),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["修改編碼獨立段", code] => w
            .encoding_segments
            .update(
                a,
                code,
                ttrs_shared::EncodingSegmentInput {
                    segment_code: (*code).to_owned(),
                    name: None,
                    length: Some(2),
                    version: None,
                },
            )
            .await
            .map(drop),
        ["失效編碼獨立段", code] => w
            .encoding_segments
            .set_active(a, code, false, None)
            .await
            .map(drop),
        ["刪除編碼獨立段", code] => w.encoding_segments.delete(a, code, None).await,
        ["複製編碼獨立段", code] => {
            w.encoding_segments.copy(a, code, "COPY").await.map(drop)
        }
        ["維護編碼獨立段", code, "的單身"] => w
            .encoding_segments
            .add_value(
                a,
                code,
                ttrs_shared::EncodingSegmentValueInput {
                    value: "01".into(),
                    ..Default::default()
                },
            )
            .await
            .map(drop),
        ["確認編碼獨立段", code] => w
            .encoding_segments
            .confirm(a, code, true, None)
            .await
            .map(drop),
        ["取消確認編碼獨立段", code] => w
            .encoding_segments
            .confirm(a, code, false, None)
            .await
            .map(drop),
        ["列印編碼獨立段"] => w
            .encoding_segments
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Pdf,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        ["匯出編碼獨立段為", "Excel"] => w
            .encoding_segments
            .report_in(
                a,
                Vec::new(),
                ttrs_app::report::ReportFormat::Excel,
                &ttrs_domain::lang::Lang::default_lang(),
            )
            .await
            .map(drop),
        ["查詢單位"] => w.units.query(a, QueryRequest::default()).await.map(drop),
        ["新增單位", code] => {
            let req = ttrs_shared::UnitInput {
                unit_code: (*code).into(),
                name: Some("單位".into()),
                ..Default::default()
            };
            w.units.create(a, req).await.map(drop)
        }
        ["修改單位", code] => {
            let req = ttrs_shared::UnitInput {
                unit_code: (*code).into(),
                name: Some("單位".into()),
                ..Default::default()
            };
            w.units.update(a, code, req).await.map(drop)
        }
        ["停用單位", code] => w.units.set_active(a, code, false, None).await.map(drop),
        ["刪除單位", code] => w.units.delete(a, code, None).await,
        ["複製單位", code] => w.units.copy(a, code, "EA").await.map(drop),
        ["確認單位", code] => w.units.confirm(a, code, true, None).await.map(drop),
        ["取消確認單位", code] => {
            let _ = unchecked!(w, w.units.confirm(a, code, true, None));
            w.units.confirm(a, code, false, None).await.map(drop)
        }
        ["列印單位"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.units.report_in(a, Vec::new(), f, &lang).await.map(drop)
        }
        ["匯出單位為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.units.report_in(a, Vec::new(), f, &lang).await.map(drop)
        }
        // axmi110 產品分類維護（工廠層，營運中心 TW01）
        ["查詢產品分類"] => w
            .product_classes
            .query(a, "TW01", QueryRequest::default())
            .await
            .map(drop),
        ["新增產品分類", code] => {
            let req = ttrs_shared::ProductClassInput {
                class_code: (*code).into(),
                name: Some("分類".into()),
                ..Default::default()
            };
            w.product_classes.create(a, "TW01", req).await.map(drop)
        }
        ["修改產品分類", code] => {
            let req = ttrs_shared::ProductClassInput {
                class_code: (*code).into(),
                name: Some("分類".into()),
                ..Default::default()
            };
            w.product_classes
                .update(a, "TW01", code, req)
                .await
                .map(drop)
        }
        ["停用產品分類", code] => w
            .product_classes
            .set_active(a, "TW01", code, false, None)
            .await
            .map(drop),
        ["刪除產品分類", code] => w.product_classes.delete(a, "TW01", code, None).await,
        ["複製產品分類", code] => w
            .product_classes
            .copy(a, "TW01", code, "P99")
            .await
            .map(drop),
        ["確認產品分類", code] => w
            .product_classes
            .confirm(a, "TW01", code, true, None)
            .await
            .map(drop),
        ["取消確認產品分類", code] => {
            let _ = unchecked!(w, w.product_classes.confirm(a, "TW01", code, true, None));
            w.product_classes
                .confirm(a, "TW01", code, false, None)
                .await
                .map(drop)
        }
        ["列印產品分類"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Pdf;
            w.product_classes
                .report_in(a, "TW01", Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["匯出產品分類為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = ttrs_app::report::ReportFormat::Excel;
            w.product_classes
                .report_in(a, "TW01", Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["查詢產品分類", code, "的異動記錄"] => w
            .product_classes
            .history(a, "TW01", code, None)
            .await
            .map(drop),
        // aimi110 料件分群維護（工廠層，營運中心 TW01）
        ["查詢料件分群"] => w
            .item_groups
            .query(a, "TW01", QueryRequest::default())
            .await
            .map(drop),
        ["新增料件分群", code] => {
            let req = ttrs_shared::ItemGroupInput {
                group_code: (*code).into(),
                ..Default::default()
            };
            w.item_groups.create(a, "TW01", req).await.map(drop)
        }
        ["修改料件分群", code] => {
            let req = ttrs_shared::ItemGroupInput {
                group_code: (*code).into(),
                ..Default::default()
            };
            w.item_groups.update(a, "TW01", code, req).await.map(drop)
        }
        ["停用料件分群", code] => w
            .item_groups
            .set_active(a, "TW01", code, false, None)
            .await
            .map(drop),
        ["複製料件分群", from, "為", to] => {
            w.item_groups.copy(a, "TW01", from, to).await.map(drop)
        }
        ["刪除料件分群", code] => w.item_groups.delete(a, "TW01", code, None).await,
        ["查詢料件分群", code, "的異動記錄"] => {
            w.item_groups.history(a, "TW01", code, None).await.map(drop)
        }
        // aooi200 專案維護（工廠層，營運中心 TW01）
        ["查詢專案"] => w
            .projects
            .query(a, "TW01", QueryRequest::default())
            .await
            .map(drop),
        ["新增專案", code] | ["修改專案", code] => {
            let req = ttrs_shared::ProjectInput {
                project_code: (*code).into(),
                name: Some("專案".into()),
                ..Default::default()
            };
            if arg(0) == "新增專案" {
                w.projects.create(a, "TW01", req).await.map(drop)
            } else {
                w.projects.update(a, "TW01", code, req).await.map(drop)
            }
        }
        ["停用專案", code] => w
            .projects
            .set_active(a, "TW01", code, false, None)
            .await
            .map(drop),
        ["刪除專案", code] => w.projects.delete(a, "TW01", code, None).await,
        ["複製專案", code] => w.projects.copy(a, "TW01", code, "P2").await.map(drop),
        ["確認專案", code] => w
            .projects
            .confirm(a, "TW01", code, true, None)
            .await
            .map(drop),
        ["取消確認專案", code] => w
            .projects
            .confirm(a, "TW01", code, false, None)
            .await
            .map(drop),
        ["查詢專案", code, "的異動記錄"] => {
            w.projects.history(a, "TW01", code, None).await.map(drop)
        }
        ["列印專案"] | ["匯出專案為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = if arg(0) == "列印專案" {
                ttrs_app::report::ReportFormat::Pdf
            } else {
                ttrs_app::report::ReportFormat::Excel
            };
            w.projects
                .report_in(a, "TW01", Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        // aooi150 稅別維護（共用層）：鍵值 T5 銷項
        ["查詢稅別"] => w.taxes.query(a, QueryRequest::default()).await.map(drop),
        ["新增稅別", code] | ["修改稅別", code] => {
            let req = ttrs_shared::TaxTypeInput {
                tax_code: (*code).into(),
                direction: "2".into(),
                name: Some("稅別".into()),
                ..Default::default()
            };
            if arg(0) == "新增稅別" {
                w.taxes.create(a, req).await.map(drop)
            } else {
                w.taxes.update(a, code, "2", req).await.map(drop)
            }
        }
        ["停用稅別", code] => w
            .taxes
            .set_active(a, code, "2", false, None)
            .await
            .map(drop),
        ["刪除稅別", code] => w.taxes.delete(a, code, "2", None).await,
        ["複製稅別", code] => w.taxes.copy(a, code, "2", "T6", "2").await.map(drop),
        ["確認稅別", code] => w.taxes.confirm(a, code, "2", true, None).await.map(drop),
        ["取消確認稅別", code] => w.taxes.confirm(a, code, "2", false, None).await.map(drop),
        ["列印稅別"] | ["匯出稅別為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = if arg(0) == "列印稅別" {
                ttrs_app::report::ReportFormat::Pdf
            } else {
                ttrs_app::report::ReportFormat::Excel
            };
            w.taxes.report_in(a, Vec::new(), f, &lang).await.map(drop)
        }
        // aooi060 每月匯率、aooi070 每日匯率、aoop010 每日匯率複製（共用層）
        ["查詢每月匯率"] => w
            .rates
            .query_monthly(a, QueryRequest::default())
            .await
            .map(drop),
        ["新增每月匯率", cur, period] => {
            let req = ttrs_shared::MonthlyRateInput {
                currency_code: (*cur).into(),
                period: (*period).into(),
                ..Default::default()
            };
            w.rates.create_monthly(a, req).await.map(drop)
        }
        ["修改每月匯率", cur, period] => {
            let req = ttrs_shared::MonthlyRateInput {
                currency_code: (*cur).into(),
                period: (*period).into(),
                ..Default::default()
            };
            w.rates.update_monthly(a, cur, period, req).await.map(drop)
        }
        ["停用每月匯率", cur, period] => w
            .rates
            .set_monthly_active(a, cur, period, false, None)
            .await
            .map(drop),
        ["刪除每月匯率", cur, period] => w.rates.delete_monthly(a, cur, period, None).await,
        ["複製每月匯率", cur, period] => w
            .rates
            .copy_monthly(a, cur, period, cur, "202612")
            .await
            .map(drop),
        ["確認每月匯率", cur, period] => w
            .rates
            .confirm_monthly(a, cur, period, true, None)
            .await
            .map(drop),
        ["取消確認每月匯率", cur, period] => {
            let _ = unchecked!(w, w.rates.confirm_monthly(a, cur, period, true, None));
            w.rates
                .confirm_monthly(a, cur, period, false, None)
                .await
                .map(drop)
        }
        ["列印每月匯率"] | ["匯出每月匯率為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = if arg(0) == "列印每月匯率" {
                ttrs_app::report::ReportFormat::Pdf
            } else {
                ttrs_app::report::ReportFormat::Excel
            };
            w.rates
                .report_monthly(a, Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["執行月底重評價", d] => w.rates.revaluate(a, date(d)).await.map(drop),
        ["查詢每日匯率"] => w
            .rates
            .query_daily(a, QueryRequest::default())
            .await
            .map(drop),
        ["新增每日匯率", cur, d] | ["修改每日匯率", cur, d] => {
            let req = ttrs_shared::DailyRateInput {
                currency_code: (*cur).into(),
                rate_date: date(d),
                rates: Default::default(),
                version: None,
            };
            if arg(0) == "新增每日匯率" {
                w.rates.create_daily(a, req).await.map(drop)
            } else {
                w.rates.update_daily(a, cur, date(d), req).await.map(drop)
            }
        }
        ["停用每日匯率", cur, d] => w
            .rates
            .set_daily_active(a, cur, date(d), false, None)
            .await
            .map(drop),
        ["刪除每日匯率", cur, d] => w.rates.delete_daily(a, cur, date(d), None).await,
        ["複製每日匯率", cur, d] => w
            .rates
            .copy_daily(a, cur, date(d), cur, date("2026-11-03"))
            .await
            .map(drop),
        ["確認每日匯率", cur, d] => w
            .rates
            .confirm_daily(a, cur, date(d), true, None)
            .await
            .map(drop),
        ["取消確認每日匯率", cur, d] => {
            let _ = unchecked!(w, w.rates.confirm_daily(a, cur, date(d), true, None));
            w.rates
                .confirm_daily(a, cur, date(d), false, None)
                .await
                .map(drop)
        }
        ["列印每日匯率"] | ["匯出每日匯率為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = if arg(0) == "列印每日匯率" {
                ttrs_app::report::ReportFormat::Pdf
            } else {
                ttrs_app::report::ReportFormat::Excel
            };
            w.rates
                .report_daily(a, Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        ["執行統一匯率", d] => w.rates.unify(a, date(d)).await.map(drop),
        ["匯入台銀匯率"] => w
            .rates
            .import_bank(
                a,
                ttrs_domain::exchange_rate::feed::BankRateType::Spot,
                true,
                None,
            )
            .await
            .map(drop),
        ["匯入海關匯率"] => w.rates.import_customs(a).await.map(drop),
        ["執行每日匯率複製", d] => w.rates.fill(a, date(d)).await.map(drop),
        ["列印每日匯率複製結果"] | ["匯出每日匯率複製結果為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = if arg(0) == "列印每日匯率複製結果" {
                ttrs_app::report::ReportFormat::Pdf
            } else {
                ttrs_app::report::ReportFormat::Excel
            };
            w.rates.report_fill(a, Vec::new(), f, &lang).await.map(drop)
        }
        // 匯率維護畫面的設定與提示（general_parameters.feature GP7）
        ["查看每月匯率設定"] => w
            .rates
            .settings(a, "monthly-exchange-rate-maintenance")
            .await
            .map(drop),
        ["查看每日匯率設定"] => w
            .rates
            .settings(a, "daily-exchange-rate-maintenance")
            .await
            .map(drop),
        // EPUB 轉 TXT（工具作業）：內容不是 EPUB，有權限時應回 EPUB_INVALID 而非權限拒絕
        ["將", "EPUB", "轉成文字"] => w.epub.convert(a, "x.epub", b"x").await.map(drop),
        // EPUB 英文翻譯：內容不是 EPUB，有權限時應回 EPUB_INVALID 而非權限拒絕
        ["提交", "EPUB", "翻譯"] => w
            .translate
            .submit(a, "x.epub", b"x", ttrs_app::Format::Txt, true)
            .await
            .map(drop),
        // GitHub EPUB 搜尋（工具作業；假來源沒有資料，有權限時回 GITHUB_NOT_FOUND 而非權限拒絕）
        ["在", "GitHub", "搜尋", "EPUB"] => w.github.search(a, "epub", false, 1).await.map(drop),
        // 多來源搜尋（沒有設定來源時搜尋結果為空；下載找不到書，都不是權限拒絕）
        ["多來源搜尋", "EPUB"] => w.epub_search.search(a, "x", true, 1).await.map(drop),
        ["下載", "搜尋結果的", "EPUB"] => w
            .epub_search
            .download_raw(a, "x", "y", None)
            .await
            .map(drop),
        // 我的 GitHub 書庫（假 GitHub 沒有這個 repo 回 GITHUB_NOT_FOUND；清單為空，都不是權限拒絕）
        ["列出", "我的", "GitHub", "書庫"] => w.epub_search.library(a).await.map(drop),
        ["加入", "我的", "GitHub", "書庫", repo] => {
            w.epub_search.add_library(a, repo).await.map(drop)
        }
        ["移除", "我的", "GitHub", "書庫", repo] => {
            w.epub_search.remove_library(a, repo, 1).await.map(drop)
        }
        // 共用 GitHub 書庫（同上：有權限時回 GITHUB_NOT_FOUND，不是權限拒絕）
        ["列出", "共用", "GitHub", "書庫"] => w.epub_search.shared_library(a).await.map(drop),
        ["加入", "共用", "GitHub", "書庫", repo] => {
            w.epub_search.add_shared_library(a, repo).await.map(drop)
        }
        ["移除", "共用", "GitHub", "書庫", repo] => w
            .epub_search
            .remove_shared_library(a, repo, 1)
            .await
            .map(drop),
        // 以網址下載（假網路沒有這個頁面，回 URL_UNREACHABLE，不是權限拒絕）
        ["讀取", "網址", "的", "EPUB"] => w
            .epub_url
            .read(a, "https://books.example.com/a.epub")
            .await
            .map(drop),
        ["下載", "網址", "的", "EPUB"] => w
            .epub_url
            .download_raw(a, "https://books.example.com/a.epub")
            .await
            .map(drop),
        ["列出", "GitHub", "repo", "的", "EPUB"] => w.github.files(a, "x/y").await.map(drop),
        ["轉換", "GitHub", "上的", "EPUB"] => {
            w.github.convert(a, "x/y", "a.epub").await.map(drop)
        }
        // aooi102 單位換算（共用層）
        ["查詢單位換算"] => w
            .conversions
            .query(a, QueryRequest::default())
            .await
            .map(drop),
        [verb, f, t] if verb.ends_with("單位換算") => {
            let c = &w.conversions;
            let input = ttrs_shared::UnitConversionInput {
                from_unit: (*f).into(),
                to_unit: (*t).into(),
                from_quantity: rust_decimal::Decimal::ONE,
                to_quantity: rust_decimal::Decimal::from(1000),
                note: None,
                version: None,
            };
            match *verb {
                "新增單位換算" => c.create(a, input).await.map(drop),
                "修改單位換算" => c.update(a, f, t, input).await.map(drop),
                "停用單位換算" => c.set_active(a, f, t, false, None).await.map(drop),
                "刪除單位換算" => c.delete(a, f, t, None).await,
                "複製單位換算" => c.copy(a, f, t, "BOX", "PCS").await.map(drop),
                "確認單位換算" => c.confirm(a, f, t, true, None).await.map(drop),
                "取消確認單位換算" => c.confirm(a, f, t, false, None).await.map(drop),
                other => panic!("未知操作：{other}"),
            }
        }
        ["列印單位換算"] | ["匯出單位換算為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = if arg(0) == "列印單位換算" {
                ttrs_app::report::ReportFormat::Pdf
            } else {
                ttrs_app::report::ReportFormat::Excel
            };
            w.conversions
                .report(a, Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        // aooi103 料件單位換算（工廠層）
        ["查詢料件", no, "的單位換算"] => {
            w.item_conversions.list(a, "TW01", no).await.map(drop)
        }
        ["取用料件", no, "的換算率"] => w
            .item_conversions
            .factor(a, "TW01", no, "BOX", "PCS")
            .await
            .map(drop),
        ["新增料件", no, "的單位換算"] => {
            let input = ttrs_shared::UnitConversionInput {
                from_unit: "BOX".into(),
                to_unit: "PCS".into(),
                from_quantity: rust_decimal::Decimal::ONE,
                to_quantity: rust_decimal::Decimal::from(12),
                note: None,
                version: None,
            };
            w.item_conversions
                .create(a, "TW01", no, input)
                .await
                .map(drop)
        }
        ["刪除料件", no, "的單位換算"] => {
            w.item_conversions
                .delete(a, "TW01", no, "BOX", "PCS", None)
                .await
        }
        ["列印料件單位換算"] | ["匯出料件單位換算為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = if arg(0) == "列印料件單位換算" {
                ttrs_app::report::ReportFormat::Pdf
            } else {
                ttrs_app::report::ReportFormat::Excel
            };
            w.item_conversions
                .report(a, "TW01", None, f, &lang)
                .await
                .map(drop)
        }
        ["查詢料件", no, "單位換算的異動記錄"] => w
            .item_conversions
            .history(a, "TW01", no, None)
            .await
            .map(drop),
        // aoos020 會計期間維護（共用層）
        ["查詢會計年度"] => w.periods.query(a, QueryRequest::default()).await.map(drop),
        ["查詢日期", d, "的會計期間"] => w
            .periods
            .locate(a, d.parse().expect("日期"))
            .await
            .map(drop),
        ["新增會計年度", y] | ["修改會計年度", y] => {
            let year: i32 = y.parse().expect("年度");
            let start = chrono::NaiveDate::from_ymd_opt(year, 1, 1).expect("日期");
            let periods = ttrs_domain::accounting_period::default_periods(start, 12)
                .iter()
                .map(|p| ttrs_shared::AccountingPeriodLine {
                    period: p.no,
                    start_date: p.start,
                    end_date: p.end,
                    quarter: p.quarter,
                })
                .collect();
            let input = ttrs_shared::AccountingYearInput {
                year,
                periods,
                version: None,
            };
            if arg(0) == "新增會計年度" {
                w.periods.create(a, input).await.map(drop)
            } else {
                w.periods.update(a, year, input).await.map(drop)
            }
        }
        ["刪除會計年度", y] => w.periods.delete(a, y.parse().expect("年度"), None).await,
        ["複製會計年度", y] => w
            .periods
            .copy(a, y.parse().expect("年度"), 2030)
            .await
            .map(drop),
        ["確認會計年度", y] | ["取消確認會計年度", y] => w
            .periods
            .confirm(a, y.parse().expect("年度"), arg(0) == "確認會計年度", None)
            .await
            .map(drop),
        ["列印會計期間"] | ["匯出會計期間為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = if arg(0) == "列印會計期間" {
                ttrs_app::report::ReportFormat::Pdf
            } else {
                ttrs_app::report::ReportFormat::Excel
            };
            w.periods.report(a, Vec::new(), f, &lang).await.map(drop)
        }
        // aoos010 整體管理系統參數（共用層，只有一筆）
        ["查看整體管理系統參數"] => w.general.get(a).await.map(drop),
        ["修改整體管理系統參數"] => {
            let input = ttrs_shared::GeneralParametersInput {
                accounting_periods: 12,
                base_currency: "TWD".into(),
                rate_basis: "daily".into(),
                auto_item_number: None,
                version: None,
            };
            w.general.update(a, input).await.map(drop)
        }
        ["列印整體管理系統參數"] | ["匯出整體管理系統參數為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = if arg(0) == "列印整體管理系統參數" {
                ttrs_app::report::ReportFormat::Pdf
            } else {
                ttrs_app::report::ReportFormat::Excel
            };
            w.general.report(a, f, &lang).await.map(drop)
        }
        // aooi050 幣別維護（共用層）
        ["查詢幣別"] => w
            .currencies
            .query(a, QueryRequest::default())
            .await
            .map(drop),
        ["新增幣別", code] | ["修改幣別", code] => {
            let req = ttrs_shared::CurrencyInput {
                currency_code: (*code).into(),
                name: Some("幣別".into()),
                ..Default::default()
            };
            if arg(0) == "新增幣別" {
                w.currencies.create(a, req).await.map(drop)
            } else {
                w.currencies.update(a, code, req).await.map(drop)
            }
        }
        ["停用幣別", code] => w
            .currencies
            .set_active(a, code, false, None)
            .await
            .map(drop),
        ["刪除幣別", code] => w.currencies.delete(a, code, None).await,
        ["複製幣別", code] => w.currencies.copy(a, code, "HKD").await.map(drop),
        ["確認幣別", code] => w.currencies.confirm(a, code, true, None).await.map(drop),
        ["取消確認幣別", code] => w.currencies.confirm(a, code, false, None).await.map(drop),
        ["列印幣別"] | ["匯出幣別為", "Excel"] => {
            let lang = ttrs_domain::lang::Lang::default_lang();
            let f = if arg(0) == "列印幣別" {
                ttrs_app::report::ReportFormat::Pdf
            } else {
                ttrs_app::report::ReportFormat::Excel
            };
            w.currencies
                .report_in(a, Vec::new(), f, &lang)
                .await
                .map(drop)
        }
        // 基本代碼（aooi100、090、110、160、210；共用層）：「<動作><種類> [代號]」
        [verb, rest @ ..] if basic_op(verb).is_some() => {
            let (action, kind) = basic_op(verb).expect("基本代碼");
            let code = rest.first().copied().unwrap_or("X01");
            let b = &w.basic_codes;
            let lang = ttrs_domain::lang::Lang::default_lang();
            match action {
                "查詢" => b.query(a, kind, QueryRequest::default()).await.map(drop),
                "新增" => {
                    let req = ttrs_shared::BasicCodeInput {
                        code: code.into(),
                        name: Some("名稱".into()),
                        ..Default::default()
                    };
                    b.create(a, kind, req).await.map(drop)
                }
                "修改" => {
                    let req = ttrs_shared::BasicCodeInput {
                        code: code.into(),
                        name: Some("名稱".into()),
                        ..Default::default()
                    };
                    b.update(a, kind, code, req).await.map(drop)
                }
                "停用" => b.set_active(a, kind, code, false, None).await.map(drop),
                "刪除" => b.delete(a, kind, code, None).await,
                "複製" => b.copy(a, kind, code, "X02").await.map(drop),
                "確認" => b.confirm(a, kind, code, true, None).await.map(drop),
                "取消確認" => b.confirm(a, kind, code, false, None).await.map(drop),
                "列印" => {
                    let f = ttrs_app::report::ReportFormat::Pdf;
                    b.report_in(a, kind, Vec::new(), f, &lang).await.map(drop)
                }
                _ => {
                    let f = ttrs_app::report::ReportFormat::Excel;
                    b.report_in(a, kind, Vec::new(), f, &lang).await.map(drop)
                }
            }
        }
        _ => panic!("未定義的操作：{op}（{}）", arg(0)),
    }
}

fn dict_input(table: &str) -> TableDictionaryInput {
    TableDictionaryInput {
        version: None,
        table_name: table.into(),
        module_code: "AIM".into(),
        table_type: None,
        upstream: None,
        downstream: None,
    }
}

pub(crate) fn class_input(code: &str) -> PermissionClassInput {
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
        version: None,
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
    // 已移植作業的模組（AXM 等）不一定在情境背景中建立，先補齊
    for (_, _, module, _) in PORTED_PROGRAMS {
        let sql = "INSERT INTO module (module_code, name) VALUES ($1, $1) ON CONFLICT DO NOTHING";
        seed(w, |s| s.add_module(module), sql, &[module]).await;
    }
    for (no, tiptop, module, actions) in PORTED_PROGRAMS {
        let req = ProgramInput {
            program_type: Some("I".into()),
            program_no: (*no).into(),
            tiptop_program_no: Some((*tiptop).into()),
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
    seed_user(w, &owner(&user, &dept), user_input(&id)).await;
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
        program_type: Some("I".into()),
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
        version: None,
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
