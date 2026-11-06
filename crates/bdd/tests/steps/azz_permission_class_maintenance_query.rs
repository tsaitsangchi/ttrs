//! Permission Class Maintenance QBE 查詢 — specs/features/azz/permission_class_maintenance_query.feature

use cucumber::{gherkin::Step, given, when};
use ttrs_shared::{ClassIdleControlDto, IdleActionDto, PermissionClassInput, QueryRequest};

use crate::{
    ErpWorld,
    steps::{
        azz_permission_class_maintenance::by_label,
        common::{query_request, rows, set_query_result},
    },
};

/// 畫面欄位名稱 → 查詢欄位鍵值（與 PermissionClassDto 欄位名稱相同）
fn field_key(label: &str) -> &'static str {
    match label {
        "權限類別" => "class_code",
        "權限說明" => "name",
        "使用目錄編號" => "default_menu",
        "閒置設定" => "idle_control",
        "閒置處理" => "idle_action",
        "閒置時間（秒）" => "idle_seconds",
        "有效" => "active",
        "資料所有者" => "created_by",
        // 不可查詢的欄位（WQ1）
        "選單名稱" => "default_menu_name",
        other => panic!("未知欄位：{other}"),
    }
}

async fn run(w: &mut ErpWorld, req: QueryRequest) {
    let result = w.classes.query(&w.actor, req).await;
    set_query_result(w, result, |c| c.class_code.clone());
}

/// 權限類別 | 權限說明 | 使用目錄編號 | 閒置設定 | 閒置處理 | 閒置時間（秒） | 有效
#[given(expr = "已有下列權限類別:")]
async fn existing_classes(w: &mut ErpWorld, step: &Step) {
    for r in rows(step) {
        let req = PermissionClassInput {
            version: None,
            class_code: r[0].clone(),
            name: Some(r[1].clone()),
            default_menu: (!r[2].is_empty()).then(|| r[2].clone()),
            idle_control: by_label(&ClassIdleControlDto::ALL, ClassIdleControlDto::label, &r[3]),
            idle_action: by_label(&IdleActionDto::ALL, IdleActionDto::label, &r[4]),
            idle_seconds: (!r[5].is_empty()).then(|| r[5].parse().expect("閒置時間")),
        };
        w.classes.create(&w.actor, req).await.unwrap();
        if r[6] == "否" {
            w.classes.deactivate(&w.actor, &r[0], None).await.unwrap();
        }
    }
}

#[when(expr = "使用者以下列條件查詢權限類別:")]
async fn query(w: &mut ErpWorld, step: &Step) {
    run(w, query_request(step, field_key, None)).await;
}

#[when(expr = "使用者以下列條件查詢權限類別，每頁 {int} 筆，取第 {int} 頁:")]
async fn query_paged(w: &mut ErpWorld, size: u32, page: u32, step: &Step) {
    run(w, query_request(step, field_key, Some((size, page)))).await;
}
