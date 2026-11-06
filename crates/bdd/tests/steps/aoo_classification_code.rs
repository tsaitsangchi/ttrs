//! aooi300 分類碼維護 — specs/features/aoo/classification_code.feature（CC1–CC9）

use cucumber::{given, then, when};
use ttrs_shared::{ClassificationCodeDto, ClassificationCodeInput};

use crate::ErpWorld;

/// 背景啟用的營運中心
const PLANT: &str = "TW01";

fn input(code: &str, description: &str) -> ClassificationCodeInput {
    ClassificationCodeInput {
        code: code.into(),
        description: Some(description.into()).filter(|d: &String| !d.is_empty()),
        ..Default::default()
    }
}

async fn find(w: &ErpWorld, kind: &str, code: &str) -> Option<ClassificationCodeDto> {
    w.classification
        .list(&w.actor, PLANT, kind)
        .await
        .unwrap_or_else(|e| panic!("讀取類別 {kind} 的分類碼失敗：{e}"))
        .into_iter()
        .find(|c| c.code == code)
}

// ----- 假設 -----

#[given(expr = "類別 {string} 已有分類碼 {string}")]
async fn existing(w: &mut ErpWorld, kind: String, code: String) {
    w.classification
        .create(&w.actor, PLANT, &kind, input(&code, &code))
        .await
        .unwrap_or_else(|e| panic!("建立分類碼 {kind}/{code} 失敗：{e}"));
}

// ----- 當 -----

#[when(expr = "使用者新增類別 {string} 的分類碼 {string}，說明 {string}")]
async fn create(w: &mut ErpWorld, kind: String, code: String, description: String) {
    let result = w
        .classification
        .create(&w.actor, PLANT, &kind, input(&code, &description))
        .await;
    w.last = Some(result.map(drop));
}

#[when(expr = "使用者刪除類別 {string} 的分類碼 {string}")]
async fn delete(w: &mut ErpWorld, kind: String, code: String) {
    w.last = Some(
        w.classification
            .delete(&w.actor, PLANT, &kind, &code, None)
            .await,
    );
}

#[when(expr = "使用者停用類別 {string} 的分類碼 {string}")]
async fn deactivate(w: &mut ErpWorld, kind: String, code: String) {
    let result = w
        .classification
        .set_active(&w.actor, PLANT, &kind, &code, false, None)
        .await;
    w.last = Some(result.map(drop));
}

// ----- 那麼 -----

#[then(expr = "類別 {string} 的分類碼 {string} 應為有效")]
async fn is_active(w: &mut ErpWorld, kind: String, code: String) {
    let c = find(w, &kind, &code).await.expect("分類碼不存在");
    assert!(c.active, "應為有效");
}

#[then(expr = "類別 {string} 的分類碼 {string} 應為無效")]
async fn is_inactive(w: &mut ErpWorld, kind: String, code: String) {
    let c = find(w, &kind, &code).await.expect("分類碼不存在");
    assert!(!c.active, "應為無效");
}

#[then(expr = "類別 {string} 的分類碼 {string} 應存在")]
async fn exists(w: &mut ErpWorld, kind: String, code: String) {
    assert!(
        find(w, &kind, &code).await.is_some(),
        "{kind}/{code} 應存在"
    );
}
