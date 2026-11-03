//! Program Maintenance 多語言 — specs/features/azz/program_maintenance_i18n.feature

use cucumber::{gherkin::Step, given, then, when};
use ttrs_shared::ProgramNameDto;

use crate::ErpWorld;

fn opt(v: &str) -> Option<String> {
    Some(v.to_owned()).filter(|s| !s.is_empty())
}

async fn set_name(
    w: &mut ErpWorld,
    no: &str,
    lang: &str,
    name: &str,
    report_title: Option<String>,
) -> Result<(), ttrs_app::AppError> {
    let dto = ProgramNameDto {
        lang: lang.to_owned(),
        name: name.to_owned(),
        report_title,
    };
    w.programs.set_name(&w.actor, no, dto).await
}

async fn name_in(w: &ErpWorld, no: &str, lang: &str) -> (String, String) {
    let names = w.programs.names(&w.actor, no).await.unwrap();
    names
        .into_iter()
        .find(|n| n.lang == lang)
        .map(|n| (n.name, n.report_title.unwrap_or_default()))
        .unwrap_or_default()
}

#[given(expr = "程式 {string} 在語系 {string} 名稱為 {string}")]
async fn existing_name(w: &mut ErpWorld, no: String, lang: String, name: String) {
    set_name(w, &no, &lang, &name, None).await.unwrap();
}

#[when(expr = "使用者以語系 {string} 將程式 {string} 命名為 {string}，報表抬頭 {string}")]
async fn name_program(w: &mut ErpWorld, lang: String, no: String, name: String, title: String) {
    let result = set_name(w, &no, &lang, &name, opt(&title)).await;
    w.last = Some(result);
}

#[when(expr = "使用者以語系 {string} 將程式 {string} 的 {string} 設為 {int} 個字元")]
async fn set_length(w: &mut ErpWorld, lang: String, no: String, item: String, len: usize) {
    let v = "x".repeat(len);
    let result = match item.as_str() {
        "程式名稱" => set_name(w, &no, &lang, &v, None).await,
        "報表抬頭" => set_name(w, &no, &lang, "", Some(v)).await,
        other => panic!("未知項目：{other}"),
    };
    w.last = Some(result);
}

#[when(expr = "使用者以語系 {string} 查看程式 {string}")]
async fn view_localized(w: &mut ErpWorld, lang: String, no: String) {
    w.displayed_program = Some(
        w.programs
            .get_localized(&w.actor, &no, &lang)
            .await
            .unwrap(),
    );
}

#[then(expr = "程式 {string} 的語系名稱應為:")]
async fn names_table(w: &mut ErpWorld, step: &Step, no: String) {
    let table = step.table.as_ref().expect("此步驟需要表格");
    for row in table.rows.iter().skip(1) {
        let (name, title) = name_in(w, &no, &row[0]).await;
        assert_eq!(name, row[1], "語系 {} 的名稱", row[0]);
        assert_eq!(title, row[2], "語系 {} 的報表抬頭", row[0]);
    }
}

#[then(expr = "程式 {string} 在語系 {string} 的名稱應為 {string}")]
async fn name_is(w: &mut ErpWorld, no: String, lang: String, expected: String) {
    assert_eq!(name_in(w, &no, &lang).await.0, expected);
}

#[then(expr = "顯示的程式名稱應為 {string}")]
fn displayed_name(w: &mut ErpWorld, expected: String) {
    let p = w.displayed_program.as_ref().expect("尚未查看程式");
    assert_eq!(p.name.clone().unwrap_or_default(), expected);
}
