//! 料件分群查詢（QBE）— specs/features/aim/item_group_query.feature（GQ1–GQ3）

use cucumber::{gherkin::Step, given, when};
use ttrs_shared::{ItemGroupInput, QueryRequest};

use crate::{
    ErpWorld,
    steps::common::{query_request, rows, set_query_result},
};

fn field_key(label: &str) -> &'static str {
    match label {
        "分群碼" => "group_code",
        "說明" => "description",
        "來源碼" => "source_code",
        "庫存單位" => "stock_unit",
        // 不可查詢（GQ3）：固定單一單位
        "單位使用方式" => "unit_usage",
        other => panic!("未知欄位：{other}"),
    }
}

fn opt(v: &str) -> Option<String> {
    Some(v.to_owned()).filter(|s| !s.is_empty())
}

async fn run(w: &mut ErpWorld, plant: &str, req: QueryRequest) {
    let result = w.item_groups.query(&w.actor, plant, req).await;
    set_query_result(w, result, |g| g.fields.group_code.clone());
}

/// 分群碼 | 說明 | 來源碼 | 庫存單位 | 資料有效
#[given(expr = "營運中心 {string} 已有下列料件分群:")]
async fn existing_groups(w: &mut ErpWorld, plant: String, step: &Step) {
    for r in rows(step) {
        let req = ItemGroupInput {
            group_code: r[0].clone(),
            description: opt(&r[1]),
            source_code: opt(&r[2]),
            stock_unit: opt(&r[3]),
            ..ItemGroupInput::default()
        };
        w.item_groups
            .create(&w.actor, &plant, req)
            .await
            .unwrap_or_else(|e| panic!("建立料件分群 {} 失敗：{e}", r[0]));
        if r[4] == "否" {
            w.item_groups
                .set_active(&w.actor, &plant, &r[0], false, None)
                .await
                .unwrap();
        }
    }
}

#[when(expr = "使用者在 {string} 以下列條件查詢料件分群:")]
async fn query(w: &mut ErpWorld, plant: String, step: &Step) {
    run(w, &plant, query_request(step, field_key, None)).await;
}
