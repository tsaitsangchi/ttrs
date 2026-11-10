//! 料件基本資料查詢（QBE）— specs/features/aim/item_maintenance_query.feature（IQ1–IQ10）

use cucumber::{gherkin::Step, given, when};
use ttrs_app::ports::ItemRepository;
use ttrs_domain::{item::ItemNo, plant::PlantId};
use ttrs_shared::{ItemInput, ItemTextDto, QbeCondition, QueryRequest};

use crate::{
    Backend, ErpWorld,
    steps::{
        aim_item_maintenance::{dec, existing, opt},
        common::{query_request, rows, set_query_result},
    },
};

/// 畫面欄位名稱 → 查詢欄位鍵值（與 ItemDto 欄位名稱相同）
fn field_key(label: &str) -> &'static str {
    match label {
        "料件編號" => "item_no",
        "品名" => "name",
        "規格" => "spec",
        "分群碼" => "group_code",
        "來源碼" => "source_code",
        "庫存單位" => "stock_unit",
        "產品分類" => "product_class",
        "安全庫存量" => "safety_stock",
        "檢驗否" => "inspection",
        "資料有效碼" => "active_code",
        "狀態碼" => "status_code",
        // 不可查詢（IQ10）：多屬性不移植
        "屬性群組代碼" => "attribute_group",
        other => panic!("未知欄位：{other}"),
    }
}

async fn run(w: &mut ErpWorld, plant: &str, req: QueryRequest) {
    let result = w.items.query(&w.actor, plant, req).await;
    set_query_result(w, result, |i| i.fields.item_no.clone());
}

/// 料件編號 | 品名 | 規格 | 分群碼 | 來源碼 | 庫存單位 | 產品分類 | 安全庫存量 | 檢驗否 | 資料有效碼
#[given(expr = "營運中心 {string} 已有下列料件:")]
async fn existing_items(w: &mut ErpWorld, plant: String, step: &Step) {
    for r in rows(step) {
        let req = ItemInput {
            item_no: r[0].clone(),
            name: opt(&r[1]),
            spec: opt(&r[2]),
            group_code: opt(&r[3]),
            source_code: opt(&r[4]),
            stock_unit: opt(&r[5]),
            product_class: opt(&r[6]),
            safety_stock: if r[7].is_empty() { None } else { dec(&r[7]) },
            inspection: r[8] == "是",
            ..ItemInput::default()
        };
        existing(w, &plant, &r[0], &r[9], req).await;
    }
}

#[given(expr = "料件 {string} 的語系 {string} 品名為 {string}")]
async fn item_text(w: &mut ErpWorld, no: String, lang: String, name: String) {
    let t = ItemTextDto {
        lang,
        name: Some(name),
        spec: None,
    };
    w.items
        .set_text(&w.actor, "TW01", &no, t)
        .await
        .unwrap_or_else(|e| panic!("設定料件 {no} 品名失敗：{e}"));
}

/// 資料所有者、所有部門是建立時記下的，測試直接改資料
#[given(expr = "料件 {string} 的資料所有者為 {string}，所有部門 {string}")]
async fn item_owner(w: &mut ErpWorld, no: String, user: String, dept: String) {
    match &w.backend {
        Backend::Memory(s) => {
            let plant = PlantId::parse("TW01").unwrap();
            let no = ItemNo::parse(&no).unwrap();
            let mut item = s.get_item(&plant, &no).await.unwrap().expect("料件");
            item.created_by = user;
            item.created_dept = dept;
            s.update_item(&plant, &item).await.unwrap();
        }
        Backend::Pg(pool) => {
            let mut tx = pool.begin().await.unwrap();
            sqlx::query(
                "SELECT set_config('search_path', schema_name, true) FROM plant WHERE plant_id = 'TW01'",
            )
            .execute(&mut *tx)
            .await
            .unwrap();
            sqlx::query("UPDATE item SET created_by = $2, created_dept = $3 WHERE item_no = $1")
                .bind(&no)
                .bind(&user)
                .bind(&dept)
                .execute(&mut *tx)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
    }
}

#[when(expr = "使用者在 {string} 以下列條件查詢料件:")]
async fn query(w: &mut ErpWorld, plant: String, step: &Step) {
    run(w, &plant, query_request(step, field_key, None)).await;
}

#[when(expr = "使用者在 {string} 以下列條件查詢料件，每頁 {int} 筆，取第 {int} 頁:")]
async fn query_paged(w: &mut ErpWorld, plant: String, size: u32, page: u32, step: &Step) {
    run(
        w,
        &plant,
        query_request(step, field_key, Some((size, page))),
    )
    .await;
}

/// 資料權限（IQ4）：以真正的 Authorizer 檢查
#[when(expr = "使用者在 {string} 以下列條件查詢料件（檢查權限）:")]
async fn query_checked(w: &mut ErpWorld, plant: String, step: &Step) {
    w.authz.set_enforced(true);
    run(w, &plant, query_request(step, field_key, None)).await;
    w.authz.set_enforced(false);
}

/// 由其他作業帶入料號（IQ7，g_argv1）
#[when(expr = "使用者在 {string} 以料件編號 {string} 查詢料件")]
async fn query_by_no(w: &mut ErpWorld, plant: String, no: String) {
    let req = QueryRequest {
        conditions: vec![QbeCondition {
            field: "item_no".into(),
            expr: no,
        }],
        ..QueryRequest::default()
    };
    run(w, &plant, req).await;
}
