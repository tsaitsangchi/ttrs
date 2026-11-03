//! User Maintenance QBE 查詢 — specs/features/azz/user_maintenance_query.feature

use chrono::NaiveDate;
use cucumber::{gherkin::Step, given, when};
use ttrs_shared::{QueryRequest, UserInput};

use crate::{
    ErpWorld,
    steps::common::{query_request, rows, set_query_result},
};

/// 畫面欄位名稱 → 查詢欄位鍵值（與 UserDto 欄位名稱相同）
fn field_key(label: &str) -> &'static str {
    match label {
        "使用者代號" => "user_id",
        "使用者名稱" => "name",
        "部門代號" => "dept_code",
        "權限類別" => "permission_class",
        "使用者選單" => "menu",
        "多營運中心使用者" => "multi_plant",
        "預設營運中心" => "default_plant",
        "電子郵件" => "email",
        "公司帳號" => "idp_username",
        "失效日期" => "valid_until",
        "偏好語言" => "lang",
        "資料有效" => "active",
        "資料所有者" => "created_by",
        "資料所有部門" => "created_dept",
        "最後修改者" => "updated_by",
        "最後修改日期" => "updated_on",
        "可使用營運中心" => "allowed_plants",
        other => panic!("未知欄位：{other}"),
    }
}

fn text(v: &str) -> Option<String> {
    (!v.is_empty()).then(|| v.to_owned())
}

async fn run(w: &mut ErpWorld, req: QueryRequest) {
    let result = w.users.query(&w.actor, req).await;
    set_query_result(w, result, |u| u.user_id.clone());
}

/// 使用者代號 | 使用者名稱 | 部門代號 | 權限類別 | 預設營運中心 | 電子郵件 | 公司帳號 | 偏好語言 | 失效日期 | 資料有效
#[given(expr = "已有下列使用者:")]
async fn existing_users(w: &mut ErpWorld, step: &Step) {
    for r in rows(step) {
        let req = UserInput {
            user_id: r[0].clone(),
            name: text(&r[1]),
            dept_code: text(&r[2]),
            permission_class: r[3].clone(),
            menu: "menu_main".into(),
            default_plant: r[4].clone(),
            email: text(&r[5]),
            idp_username: text(&r[6]),
            lang: text(&r[7]),
            valid_until: text(&r[8])
                .map(|d| NaiveDate::parse_from_str(&d, "%Y/%m/%d").expect("失效日期")),
            ..Default::default()
        };
        w.users.create(&w.actor, req).await.unwrap();
        if r[9] == "否" {
            w.users.deactivate(&w.actor, &r[0]).await.unwrap();
        }
    }
}

#[when(expr = "使用者以下列條件查詢使用者:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    run(w, query_request(step, field_key, None)).await;
}

#[when(expr = "使用者以下列條件查詢使用者，每頁 {int} 筆，取第 {int} 頁:")]
async fn query_paged(w: &mut ErpWorld, size: u32, page: u32, step: &Step) {
    run(w, query_request(step, field_key, Some((size, page)))).await;
}
