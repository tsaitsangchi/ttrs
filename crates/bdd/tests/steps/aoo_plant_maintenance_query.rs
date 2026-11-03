//! Plant Maintenance QBE 查詢 — specs/features/aoo/plant_maintenance_query.feature

use cucumber::{gherkin::Step, given, when};
use ttrs_shared::{PlantInput, QueryRequest};

use crate::{
    ErpWorld,
    steps::common::{query_request, rows, set_query_result},
};

/// 畫面欄位名稱 → 查詢欄位鍵值（與 PlantDto 欄位名稱相同）
fn field_key(label: &str) -> &'static str {
    match label {
        "營運中心代碼" => "plant_id",
        "營運中心名稱" => "name",
        "時區" => "time_zone",
        "地址" => "address",
        "營運中心電話" => "phone",
        "營運中心傳真" => "fax",
        "資料有效" => "active",
        "資料庫代碼" => "schema_name",
        // azp04 不移植，用來驗證不可查的欄位
        "伺服器編號" => "server_no",
        other => panic!("未知欄位：{other}"),
    }
}

fn text(v: &str) -> Option<String> {
    (!v.is_empty()).then(|| v.to_owned())
}

async fn run(w: &mut ErpWorld, req: QueryRequest) {
    let result = w.plants.query(&w.actor, req).await;
    set_query_result(w, result, |p| p.plant_id.clone());
}

/// 營運中心代碼 | 營運中心名稱 | 時區 | 地址 | 營運中心電話 | 營運中心傳真 | 資料有效
#[given(expr = "已有下列營運中心:")]
async fn existing_plants(w: &mut ErpWorld, step: &Step) {
    for r in rows(step) {
        let req = PlantInput {
            plant_id: r[0].clone(),
            name: text(&r[1]),
            time_zone: text(&r[2]),
            address: text(&r[3]),
            phone: text(&r[4]),
            fax: text(&r[5]),
            schema_name: None,
        };
        w.plants.create(&w.actor, req).await.unwrap();
        if r[6] == "否" {
            w.plants.deactivate(&w.actor, &r[0]).await.unwrap();
        }
    }
}

#[when(expr = "使用者以下列條件查詢營運中心:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    run(w, query_request(step, field_key, None)).await;
}

#[when(expr = "使用者以下列條件查詢營運中心，每頁 {int} 筆，取第 {int} 頁:")]
async fn query_paged(w: &mut ErpWorld, size: u32, page: u32, step: &Step) {
    run(w, query_request(step, field_key, Some((size, page)))).await;
}
