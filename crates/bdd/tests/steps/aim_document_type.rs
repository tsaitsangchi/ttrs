//! 單別維護 — specs/features/aim/document_type.feature（DT1–DT14）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::report::ReportFormat;
use ttrs_domain::lang::Lang;
use ttrs_shared::{
    DocTypeAccessInput, DocTypeStorageDto, DocTypeStorageInput, DocumentTypeDto, DocumentTypeInput,
    GeneralParametersInput, QueryRequest,
};

use crate::{ErpWorld, steps::common::rows};

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

fn input(code: &str, name: &str, system: &str, kind: &str) -> DocumentTypeInput {
    DocumentTypeInput {
        doc_type_code: code.to_owned(),
        name: name.to_owned(),
        system: system.to_owned(),
        kind: kind.to_owned(),
        ..DocumentTypeInput::default()
    }
}

async fn doc(w: &ErpWorld, plant: &str, code: &str) -> DocumentTypeDto {
    w.document_types
        .get(&w.actor, plant, code)
        .await
        .unwrap_or_else(|e| panic!("查詢單別 {code} 失敗：{e}"))
}

fn value_of(d: &DocumentTypeDto, label: &str) -> String {
    match label {
        "單據名稱" => d.name.clone(),
        "系統別" => d.system.clone(),
        "單據性質" => d.kind.clone(),
        "自動編號" => yes_no(d.auto_number).into(),
        "編號方式" => d.numbering.clone(),
        "立即列印" => yes_no(d.print_now).into(),
        "立即確認" => yes_no(d.confirm_now).into(),
        "計算成本" => yes_no(d.cost_enabled).into(),
        "成會單據類別" => d.cost_class.clone(),
        "影響呆滯日期" => yes_no(d.affects_idle_date).into(),
        "倉庫設限" => d.restriction.clone(),
        "資料有效" => yes_no(d.active).into(),
        "已確認" => yes_no(d.confirmed).into(),
        other => panic!("未知欄位：{other}"),
    }
}

fn field_key(label: &str) -> &'static str {
    match label {
        "單別" => "doc_type_code",
        "單據名稱" => "name",
        "系統別" => "system",
        "單據性質" => "kind",
        "倉庫" => "warehouse",
        other => panic!("未知欄位：{other}"),
    }
}

async fn modify(
    w: &mut ErpWorld,
    plant: &str,
    code: &str,
    stale: bool,
    f: impl FnOnce(&mut DocumentTypeInput),
) {
    let mut i = doc(w, plant, code).await.to_input();
    if stale {
        i.version = w.doc_type_version;
    }
    f(&mut i);
    let r = w.document_types.update(&w.actor, plant, code, i).await;
    w.last = Some(r.map(drop));
}

async fn storages(w: &mut ErpWorld, plant: &str, code: &str, restriction: &str, step: &Step) {
    let d = doc(w, plant, code).await;
    let req = DocTypeStorageInput {
        restriction: restriction.to_owned(),
        storages: rows(step)
            .iter()
            .map(|r| DocTypeStorageDto {
                warehouse: r[0].clone(),
                location: r.get(1).cloned().unwrap_or_default(),
            })
            .collect(),
        version: Some(d.version),
    };
    let r = w
        .document_types
        .set_storages(&w.actor, plant, code, req)
        .await;
    w.last = Some(r.map(drop));
}

async fn access(w: &mut ErpWorld, plant: &str, code: &str, users: Vec<String>, depts: Vec<String>) {
    let d = doc(w, plant, code).await;
    let req = DocTypeAccessInput {
        users: Some(users),
        depts: Some(depts),
        version: Some(d.version),
    };
    let r = w
        .document_types
        .set_access(&w.actor, plant, code, req)
        .await;
    w.last = Some(r.map(drop));
}

fn must(w: &ErpWorld, what: &str) {
    if let Some(Err(e)) = &w.last {
        panic!("{what}失敗：{e}");
    }
}

// ----- 假設 -----

#[given(regex = r#"^營運中心 "([^"]*)" 已有單別 "([^"]*)"，系統別 "([^"]*)"，單據性質 "([^"]*)"$"#)]
async fn has_doc(w: &mut ErpWorld, plant: String, code: String, system: String, kind: String) {
    w.document_types
        .create(&w.actor, &plant, input(&code, &code, &system, &kind))
        .await
        .unwrap_or_else(|e| panic!("建立單別 {code} 失敗：{e}"));
}

#[given(regex = r#"^整體管理系統參數的單別位數為 (\d)$"#)]
async fn code_length(w: &mut ErpWorld, len: i16) {
    let cur = w.general.get(&w.actor).await.expect("整體參數");
    let req = GeneralParametersInput {
        accounting_periods: cur.accounting_periods,
        base_currency: cur.base_currency.clone(),
        rate_basis: cur.rate_basis.clone(),
        auto_item_number: None,
        doc_code_length: Some(len),
        version: Some(cur.version),
    };
    w.general
        .update(&w.actor, req)
        .await
        .unwrap_or_else(|e| panic!("設定單別位數失敗：{e}"));
}

#[given(regex = r#"^使用者已停用營運中心 "([^"]*)" 的單別 "([^"]*)"$"#)]
async fn deactivated(w: &mut ErpWorld, plant: String, code: String) {
    let d = doc(w, &plant, &code).await;
    w.document_types
        .set_active(&w.actor, &plant, &code, false, Some(d.version))
        .await
        .unwrap_or_else(|e| panic!("停用單別失敗：{e}"));
}

#[given(regex = r#"^使用者已確認營運中心 "([^"]*)" 的單別 "([^"]*)"$"#)]
async fn confirmed(w: &mut ErpWorld, plant: String, code: String) {
    let d = doc(w, &plant, &code).await;
    w.document_types
        .confirm(&w.actor, &plant, &code, true, Some(d.version))
        .await
        .unwrap_or_else(|e| panic!("確認單別失敗：{e}"));
}

#[given(regex = r#"^另一位使用者已修改營運中心 "([^"]*)" 的單別 "([^"]*)"$"#)]
async fn other_user_modified(w: &mut ErpWorld, plant: String, code: String) {
    let d = doc(w, &plant, &code).await;
    w.doc_type_version = Some(d.version);
    let mut i = d.to_input();
    i.memo = Some("他人修改".into());
    w.document_types
        .update(&w.actor, &plant, &code, i)
        .await
        .unwrap_or_else(|e| panic!("修改單別失敗：{e}"));
}

#[given(
    regex = r#"^使用者在營運中心 "([^"]*)" 設定單別 "([^"]*)" 的倉庫設限為 "([^"]*)"，倉儲如下:$"#
)]
async fn given_storages(w: &mut ErpWorld, plant: String, code: String, r: String, step: &Step) {
    storages(w, &plant, &code, &r, step).await;
    must(w, "設定倉庫設限");
}

#[given(
    regex = r#"^使用者在營運中心 "([^"]*)" 設定單別 "([^"]*)" 可使用的使用者 "([^"]*)"、部門 "([^"]*)"$"#
)]
async fn given_access(w: &mut ErpWorld, plant: String, code: String, user: String, dept: String) {
    access(w, &plant, &code, vec![user], vec![dept]).await;
    must(w, "設定使用者設限");
}

// ----- 當 -----

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 新增單別 "([^"]*)"，名稱 "([^"]*)"，系統別 "([^"]*)"，單據性質 "([^"]*)"$"#
)]
async fn create(
    w: &mut ErpWorld,
    plant: String,
    code: String,
    name: String,
    system: String,
    kind: String,
) {
    let r = w
        .document_types
        .create(&w.actor, &plant, input(&code, &name, &system, &kind))
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 新增單別 "([^"]*)"，名稱 "([^"]*)"，系統別 "([^"]*)"，單據性質 "([^"]*)"，自動編號 "([^"]*)"，編號方式 "([^"]*)"$"#
)]
#[allow(clippy::too_many_arguments)]
async fn create_numbering(
    w: &mut ErpWorld,
    plant: String,
    code: String,
    name: String,
    system: String,
    kind: String,
    auto: String,
    numbering: String,
) {
    let mut i = input(&code, &name, &system, &kind);
    i.auto_number = Some(auto == "是");
    i.numbering = Some(numbering);
    let r = w.document_types.create(&w.actor, &plant, i).await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 把單別 "([^"]*)" 改為計算成本 "([^"]*)"，成會單據類別 "([^"]*)"$"#
)]
async fn set_cost(w: &mut ErpWorld, plant: String, code: String, cost: String, class: String) {
    modify(w, &plant, &code, false, |i| {
        i.cost_enabled = Some(cost == "是");
        i.cost_class = Some(class);
    })
    .await;
}

#[when(regex = r#"^使用者(以舊版本)?在營運中心 "([^"]*)" 把單別 "([^"]*)" 的名稱改為 "([^"]*)"$"#)]
async fn rename(w: &mut ErpWorld, stale: String, plant: String, code: String, name: String) {
    modify(w, &plant, &code, !stale.is_empty(), |i| i.name = name).await;
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 設定單別 "([^"]*)" 的倉庫設限為 "([^"]*)"，倉儲如下:$"#
)]
async fn when_storages(w: &mut ErpWorld, plant: String, code: String, r: String, step: &Step) {
    storages(w, &plant, &code, &r, step).await;
}

#[when(
    regex = r#"^使用者在營運中心 "([^"]*)" 設定單別 "([^"]*)" 可使用的使用者 "([^"]*)"、部門 "([^"]*)"$"#
)]
async fn when_access(w: &mut ErpWorld, plant: String, code: String, user: String, dept: String) {
    access(w, &plant, &code, vec![user], vec![dept]).await;
    must(w, "設定使用者設限");
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 設定單別 "([^"]*)" 可使用的部門 "([^"]*)"$"#)]
async fn when_dept(w: &mut ErpWorld, plant: String, code: String, dept: String) {
    access(w, &plant, &code, Vec::new(), vec![dept]).await;
    must(w, "設定部門設限");
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 刪除單別 "([^"]*)"$"#)]
async fn delete(w: &mut ErpWorld, plant: String, code: String) {
    let d = doc(w, &plant, &code).await;
    let r = w
        .document_types
        .delete(&w.actor, &plant, &code, Some(d.version))
        .await;
    w.last = Some(r);
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 把單別 "([^"]*)" 複製為 "([^"]*)"$"#)]
async fn copy(w: &mut ErpWorld, plant: String, code: String, new_code: String) {
    let r = w
        .document_types
        .copy(&w.actor, &plant, &code, &new_code)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 查詢單別$"#)]
async fn query_all(w: &mut ErpWorld, plant: String) {
    let r = w
        .document_types
        .query(&w.actor, &plant, QueryRequest::default())
        .await;
    crate::steps::common::set_query_result(w, r, |d| d.doc_type_code.clone());
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 以下列條件查詢單別:$"#)]
async fn query_by(w: &mut ErpWorld, plant: String, step: &Step) {
    let req = crate::steps::common::query_request(step, field_key, None);
    let r = w.document_types.query(&w.actor, &plant, req).await;
    crate::steps::common::set_query_result(w, r, |d| d.doc_type_code.clone());
}

#[when(regex = r#"^使用者在營運中心 "([^"]*)" 列印單別$"#)]
async fn report(w: &mut ErpWorld, plant: String) {
    let r = w
        .document_types
        .report_in(
            &w.actor,
            &plant,
            Vec::new(),
            ReportFormat::Pdf,
            &Lang::default_lang(),
        )
        .await;
    w.last = Some(r.map(|r| {
        w.report = Some(r);
    }));
}

// ----- 那麼 -----

#[then(regex = r#"^營運中心 "([^"]*)" 的單別 "([^"]*)" 應為:$"#)]
async fn is(w: &mut ErpWorld, plant: String, code: String, step: &Step) {
    let d = doc(w, &plant, &code).await;
    for r in rows(step) {
        assert_eq!(value_of(&d, &r[0]), r[1], "欄位 {}", r[0]);
    }
}

#[then(regex = r#"^營運中心 "([^"]*)" 的單別 "([^"]*)" 可用倉儲應為:$"#)]
async fn storage_check(w: &mut ErpWorld, plant: String, code: String, step: &Step) {
    must(w, "設定倉庫設限");
    for r in rows(step) {
        let loc = Some(r[1].as_str()).filter(|s| !s.is_empty());
        let ok = w
            .document_types
            .check_storage(&plant, &code, &r[0], loc)
            .await
            .is_ok();
        assert_eq!(yes_no(ok), r[2], "倉儲 {} {}", r[0], r[1]);
    }
}

#[then(regex = r#"^單別使用檢查應為:$"#)]
async fn use_check(w: &mut ErpWorld, step: &Step) {
    for r in rows(step) {
        let got = match w
            .document_types
            .check_use("TW01", &r[0], &r[2], &r[1])
            .await
        {
            Ok(_) => "可用".to_owned(),
            Err(e) => e.code().to_owned(),
        };
        assert_eq!(got, r[3], "單別 {} 使用者 {} 性質 {}", r[0], r[1], r[2]);
    }
}

#[then(regex = r#"^營運中心 "([^"]*)" 的單別 "([^"]*)" 應不存在$"#)]
async fn gone(w: &mut ErpWorld, plant: String, code: String) {
    must(w, "刪除單別");
    assert!(w.document_types.get(&w.actor, &plant, &code).await.is_err());
}

#[then(
    regex = r#"^營運中心 "([^"]*)" 的單別 "([^"]*)" 應有 (\d+) 筆倉庫設限、(\d+) 位設限使用者、(\d+) 個設限部門$"#
)]
async fn children(w: &mut ErpWorld, plant: String, code: String, s: usize, u: usize, d: usize) {
    let x = doc(w, &plant, &code).await;
    assert_eq!((x.storages.len(), x.users.len(), x.depts.len()), (s, u, d));
}

#[then(regex = r#"^單別清單順序應為 "([^"]*)"$"#)]
async fn order_is(w: &mut ErpWorld, expected: String) {
    if let Some(Err(e)) = &w.last {
        panic!("查詢失敗：{e}");
    }
    let (keys, _) = w.query_result.clone().expect("尚未查詢");
    assert_eq!(keys.join("、"), expected);
}

#[then(regex = r#"^營運中心 "([^"]*)" 單別 "([^"]*)" 的異動記錄應有 "([^"]+)"$"#)]
async fn history(w: &mut ErpWorld, plant: String, code: String, expected: String) {
    let h = w
        .document_types
        .history(&w.actor, &plant, &code, None)
        .await
        .expect("異動記錄");
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join("、"), expected);
}
