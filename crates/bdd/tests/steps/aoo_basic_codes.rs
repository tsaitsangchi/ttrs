//! 基本代碼：區域別、國別、地區（region_country_area_maintenance.feature RC1–RC7）、
//! 交運方式（shipping_method_maintenance.feature SM1–SM6）、報表抬頭（report_title_maintenance.feature RH1–RH6）。
//! 查詢、複製、確認、刪除等標準功能的步驟在 std_actions.rs。

use cucumber::{given, then, when};
use ttrs_domain::basic_code::BasicCodeKind;
use ttrs_shared::{BasicCodeDto, BasicCodeInput, BasicCodeNameDto};

use crate::{ErpWorld, steps::std_actions::basic_kind};

fn kind(entity: &str) -> BasicCodeKind {
    basic_kind(entity).unwrap_or_else(|| panic!("不是基本代碼：{entity}"))
}

async fn create(w: &mut ErpWorld, entity: &str, code: &str, name: &str, parent: Option<String>) {
    let req = BasicCodeInput {
        code: code.to_owned(),
        name: Some(name.to_owned()),
        parent,
        ..Default::default()
    };
    let r = w.basic_codes.create(&w.actor, kind(entity), req).await;
    w.last = Some(r.map(drop));
}

async fn get(w: &ErpWorld, entity: &str, code: &str) -> BasicCodeDto {
    w.basic_codes
        .get(&w.actor, kind(entity), code, None)
        .await
        .unwrap_or_else(|e| panic!("查詢{entity} {code} 失敗：{e}"))
}

fn yes_no(v: bool) -> &'static str {
    if v { "是" } else { "否" }
}

#[when(
    regex = r#"^使用者新增(區域|國別|地區|交運方式|報表抬頭) "(\S*)"，(?:名稱|說明) "([^"]*)"$"#
)]
async fn add(w: &mut ErpWorld, entity: String, code: String, name: String) {
    create(w, &entity, &code, &name, None).await;
}

#[when(regex = r#"^使用者新增(國別|地區) "(\S*)"，名稱 "([^"]*)"，隸屬(?:區域|國別) "(\S*)"$"#)]
async fn add_with_parent(
    w: &mut ErpWorld,
    entity: String,
    code: String,
    name: String,
    parent: String,
) {
    create(w, &entity, &code, &name, Some(parent)).await;
}

#[given(regex = r#"^已有(區域|國別|地區|交運方式|報表抬頭) "(\S+)"$"#)]
async fn has(w: &mut ErpWorld, entity: String, code: String) {
    create(w, &entity, &code, &code, None).await;
    if let Some(Err(e)) = &w.last {
        panic!("建立{entity} {code} 失敗：{e}");
    }
}

#[given(regex = r#"^已有(區域|國別|地區|交運方式|報表抬頭) "(\S+)"，(?:名稱|說明) "([^"]*)"$"#)]
async fn has_named(w: &mut ErpWorld, entity: String, code: String, name: String) {
    create(w, &entity, &code, &name, None).await;
    if let Some(Err(e)) = &w.last {
        panic!("建立{entity} {code} 失敗：{e}");
    }
}

#[given(regex = r#"^已有(國別|地區) "(\S+)"，隸屬(?:區域|國別) "(\S+)"$"#)]
async fn has_with_parent(w: &mut ErpWorld, entity: String, code: String, parent: String) {
    create(w, &entity, &code, &code, Some(parent)).await;
    if let Some(Err(e)) = &w.last {
        panic!("建立{entity} {code} 失敗：{e}");
    }
}

#[given(regex = r#"^(區域|國別|地區|交運方式|報表抬頭) "(\S+)" 已停用$"#)]
async fn inactive(w: &mut ErpWorld, entity: String, code: String) {
    let c = get(w, &entity, &code).await;
    w.basic_codes
        .set_active(&w.actor, kind(&entity), &code, false, Some(c.version))
        .await
        .unwrap_or_else(|e| panic!("停用{entity} {code} 失敗：{e}"));
}

#[when(regex = r#"^使用者(停用|啟用)(區域|國別|地區|交運方式|報表抬頭) "(\S+)"$"#)]
async fn toggle(w: &mut ErpWorld, op: String, entity: String, code: String) {
    let r = w
        .basic_codes
        .set_active(&w.actor, kind(&entity), &code, op == "啟用", None)
        .await;
    w.last = Some(r.map(drop));
}

#[when(regex = r#"^使用者將(國別|地區) "(\S+)" 的隸屬(?:區域|國別)改為 "(\S*)"$"#)]
async fn change_parent(w: &mut ErpWorld, entity: String, code: String, parent: String) {
    let mut input = get(w, &entity, &code).await.to_input();
    input.parent = Some(parent);
    let r = w
        .basic_codes
        .update(&w.actor, kind(&entity), &code, input)
        .await;
    w.last = Some(r.map(drop));
}

#[when(
    regex = r#"^使用者將(區域|國別|地區|交運方式|報表抬頭) "(\S+)" 的(?:名稱|說明)改為 "([^"]*)"$"#
)]
async fn rename(w: &mut ErpWorld, entity: String, code: String, name: String) {
    let mut input = get(w, &entity, &code).await.to_input();
    input.name = Some(name);
    let r = w
        .basic_codes
        .update(&w.actor, kind(&entity), &code, input)
        .await;
    w.last = Some(r.map(drop));
}

/// 多語言名稱（RH4）
#[when(expr = "使用者將報表抬頭 {string} 的 {string} 名稱設為 {string}")]
async fn set_lang_name(w: &mut ErpWorld, code: String, lang: String, name: String) {
    let mut input = get(w, "報表抬頭", &code).await.to_input();
    input.names.retain(|n| n.lang != lang);
    input.names.push(BasicCodeNameDto { lang, name });
    let r = w
        .basic_codes
        .update(&w.actor, BasicCodeKind::ReportTitle, &code, input)
        .await;
    w.last = Some(r.map(drop));
}

#[then(expr = "以語系 {string} 顯示報表抬頭 {string} 的名稱應為 {string}")]
async fn name_in(w: &mut ErpWorld, lang: String, code: String, expected: String) {
    let c = w
        .basic_codes
        .get(&w.actor, BasicCodeKind::ReportTitle, &code, Some(&lang))
        .await
        .expect("報表抬頭");
    assert_eq!(c.name.unwrap_or_default(), expected);
}

#[then(regex = r#"^(區域|國別|地區|交運方式|報表抬頭) "(\S+)" 應為有效$"#)]
async fn is_active(w: &mut ErpWorld, entity: String, code: String) {
    assert!(get(w, &entity, &code).await.active, "應為有效");
}

#[then(regex = r#"^(區域|國別|地區|交運方式|報表抬頭) "(\S+)" 的資料有效應為 "(是|否)"$"#)]
async fn active_is(w: &mut ErpWorld, entity: String, code: String, expected: String) {
    assert_eq!(yes_no(get(w, &entity, &code).await.active), expected);
}

#[then(regex = r#"^(國別|地區) "(\S+)" 的隸屬(?:區域|國別)應為 "(\S*)"$"#)]
async fn parent_is(w: &mut ErpWorld, entity: String, code: String, expected: String) {
    assert_eq!(
        get(w, &entity, &code).await.parent.unwrap_or_default(),
        expected
    );
}

#[then(regex = r#"^(區域|國別|地區|交運方式|報表抬頭) "(\S+)" 的(?:名稱|說明)應為 "([^"]*)"$"#)]
async fn name_is(w: &mut ErpWorld, entity: String, code: String, expected: String) {
    assert_eq!(
        get(w, &entity, &code).await.name.unwrap_or_default(),
        expected
    );
}

#[then(regex = r#"^(區域|國別|地區|交運方式|報表抬頭) "(\S+)" 應不存在$"#)]
async fn absent(w: &mut ErpWorld, entity: String, code: String) {
    let r = w
        .basic_codes
        .get(&w.actor, kind(&entity), &code, None)
        .await;
    assert!(r.is_err(), "{entity} {code} 應不存在");
}
