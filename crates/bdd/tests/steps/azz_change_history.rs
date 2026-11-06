//! 資料異動記錄 — specs/features/azz/change_history.feature（ADR 0012，CH1–CH9）

use cucumber::{gherkin::Step, given, then, when};
use ttrs_app::{Actor, authorization::ActorKind};
use ttrs_shared::{ChangeDto, DepartmentInput, EmployeeInput};

use crate::{Backend, ErpWorld, steps::common::rows};

const PLANT: &str = "TW01";

/// 畫面欄位名稱 → 資料表欄位
fn column(label: &str) -> &'static str {
    match label {
        "部門編號" => "dept_code",
        "簡稱" => "name",
        "全稱" => "full_name",
        "電子郵件" => "email",
        other => panic!("未定義的欄位：{other}"),
    }
}

fn dept(code: &str, name: &str) -> DepartmentInput {
    DepartmentInput {
        dept_code: code.to_owned(),
        name: Some(name.to_owned()),
        full_name: None,
        version: None,
    }
}

fn as_user(w: &ErpWorld, user: &str) -> Actor {
    Actor {
        user_id: user.to_owned(),
        ..w.actor.clone()
    }
}

async fn dept_history(w: &mut ErpWorld, code: &str) -> Vec<ChangeDto> {
    let h = w
        .departments
        .history(&w.actor, PLANT, code, None)
        .await
        .expect("查詢異動記錄");
    w.history = Some(h.clone());
    h
}

fn last(h: &[ChangeDto]) -> &ChangeDto {
    h.last().expect("應有異動記錄")
}

fn field<'a>(c: &'a ChangeDto, label: &str) -> &'a ttrs_shared::ChangeFieldDto {
    let col = column(label);
    c.fields
        .iter()
        .find(|f| f.column == col)
        .unwrap_or_else(|| panic!("異動記錄沒有欄位 {col}：{c:?}"))
}

/// 工廠 schema 名稱（PostgreSQL 情境用）
async fn plant_schema(pool: &sqlx::PgPool) -> String {
    sqlx::query_scalar("SELECT schema_name FROM plant WHERE plant_id = $1")
        .bind(PLANT)
        .fetch_one(pool)
        .await
        .expect("工廠 schema")
}

// ----- 假設 -----

#[given(expr = "營運中心 {string} 已有部門 {string}，簡稱 {string}")]
async fn has_dept(w: &mut ErpWorld, plant: String, code: String, name: String) {
    w.departments
        .create(&w.actor, &plant, dept(&code, &name))
        .await
        .unwrap_or_else(|e| panic!("建立部門 {code} 失敗：{e}"));
}

#[given(expr = "營運中心 {string} 已有員工 {string}，電子郵件 {string}")]
async fn has_employee(w: &mut ErpWorld, plant: String, id: String, email: String) {
    let req = EmployeeInput {
        employee_id: id.clone(),
        name: Some(id.clone()),
        email: Some(email),
        ..EmployeeInput::default()
    };
    w.employees
        .create(&w.actor, &plant, req)
        .await
        .unwrap_or_else(|e| panic!("建立員工 {id} 失敗：{e}"));
}

/// 兩人讀取同一版本：版本號在「以第 N 版修改」步驟帶入，這裡只確認目前版本
#[given(expr = "使用者 {string} 與 {string} 都讀取了部門 {string} 的第 {int} 版")]
async fn both_read(w: &mut ErpWorld, _a: String, _b: String, code: String, version: i32) {
    let d = w.departments.get(&w.actor, PLANT, &code).await.unwrap();
    assert_eq!(d.version, version);
}

// ----- 當 -----

#[when(expr = "使用者在營運中心 {string} 新增部門 {string}，簡稱 {string}")]
async fn create_dept(w: &mut ErpWorld, plant: String, code: String, name: String) {
    let r = w
        .departments
        .create(&w.actor, &plant, dept(&code, &name))
        .await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者 {string} 將部門 {string} 的簡稱改為 {string}")]
async fn user_renames(w: &mut ErpWorld, user: String, code: String, name: String) {
    let actor = as_user(w, &user);
    let r = w
        .departments
        .update(&actor, PLANT, &code, dept(&code, &name))
        .await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者 {string} 以第 {int} 版將部門 {string} 的簡稱改為 {string}")]
async fn user_renames_at(w: &mut ErpWorld, user: String, version: i32, code: String, name: String) {
    let actor = as_user(w, &user);
    let req = DepartmentInput {
        version: Some(version),
        ..dept(&code, &name)
    };
    let r = w.departments.update(&actor, PLANT, &code, req).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "AI 代表使用者 {string} 將部門 {string} 的簡稱改為 {string}")]
async fn ai_renames(w: &mut ErpWorld, user: String, code: String, name: String) {
    let actor = Actor {
        kind: ActorKind::Ai,
        ..as_user(w, &user)
    };
    let r = w
        .departments
        .update(&actor, PLANT, &code, dept(&code, &name))
        .await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者刪除部門 {string}")]
async fn delete_dept(w: &mut ErpWorld, code: String) {
    let r = w.departments.delete(&w.actor, PLANT, &code, None).await;
    w.last = Some(r);
}

#[when(expr = "使用者停用部門 {string}")]
async fn deactivate_dept(w: &mut ErpWorld, code: String) {
    let r = w.departments.deactivate(&w.actor, PLANT, &code, None).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者啟用部門 {string}")]
async fn activate_dept(w: &mut ErpWorld, code: String) {
    let r = w.departments.activate(&w.actor, PLANT, &code, None).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "使用者將員工 {string} 的電子郵件改為 {string}")]
async fn change_email(w: &mut ErpWorld, id: String, email: String) {
    let e = w.employees.get(&w.actor, PLANT, &id).await.unwrap();
    let req = EmployeeInput {
        email: Some(email),
        ..e.to_input()
    };
    let r = w.employees.update(&w.actor, PLANT, &id, req).await;
    w.last = Some(r.map(drop));
}

#[when(expr = "以應用程式的資料庫角色修改或刪除部門 {string} 的異動記錄")]
async fn tamper(w: &mut ErpWorld, _code: String) {
    let Backend::Pg(pool) = &w.backend else {
        panic!("@pg 情境只在 PostgreSQL 執行");
    };
    let schema = plant_schema(pool).await;
    let mut denied = true;
    for sql in [
        "UPDATE change_log SET user_id = 'x'",
        "DELETE FROM change_log",
    ] {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query(
            "SELECT set_config('role', ttrs_plant_role($1), true), set_config('search_path', $1, true)",
        )
        .bind(&schema)
        .execute(&mut *tx)
        .await
        .unwrap();
        let ok = sqlx::query(sql).execute(&mut *tx).await.is_ok();
        tx.rollback().await.unwrap();
        denied &= !ok;
    }
    w.db_denied = Some(denied);
}

#[when(expr = "以 SQL 直接將部門 {string} 的簡稱改為 {string}")]
async fn direct_sql(w: &mut ErpWorld, code: String, name: String) {
    let Backend::Pg(pool) = &w.backend else {
        panic!("@pg 情境只在 PostgreSQL 執行");
    };
    let schema = plant_schema(pool).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT set_config('search_path', $1, true)")
        .bind(&schema)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE department SET name = $2 WHERE dept_code = $1")
        .bind(&code)
        .bind(&name)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

// ----- 那麼 -----

#[then(expr = "部門 {string} 的異動記錄應為:")]
async fn history_is(w: &mut ErpWorld, code: String, step: &Step) {
    let h = dept_history(w, &code).await;
    let expected = rows(step);
    assert_eq!(h.len(), expected.len(), "{h:?}");
    for (c, r) in h.iter().zip(expected) {
        assert_eq!(c.version, Some(r[0].parse().unwrap()));
        assert_eq!(c.operation, r[1]);
        assert_eq!(c.user_id, r[2]);
        assert_eq!(c.program.as_deref(), Some(r[3].as_str()));
        // 「部門編號,簡稱…」：應包含這些欄位（… 表示還有其他欄位）
        for label in r[4]
            .trim_end_matches('…')
            .split(',')
            .filter(|s| !s.is_empty())
        {
            field(c, label);
        }
    }
}

#[then(expr = "部門 {string} 的版本應為 {int}")]
async fn version_is(w: &mut ErpWorld, code: String, version: i32) {
    let d = w.departments.get(&w.actor, PLANT, &code).await.unwrap();
    assert_eq!(d.version, version);
}

#[then(expr = "部門 {string} 最新一筆異動記錄應為:")]
async fn latest_is(w: &mut ErpWorld, code: String, step: &Step) {
    let h = dept_history(w, &code).await;
    let c = last(&h);
    let r = &rows(step)[0];
    assert_eq!(c.version, Some(r[0].parse().unwrap()));
    assert_eq!(c.operation, r[1]);
    assert_eq!(c.user_id, r[2]);
    let f = field(c, &r[3]);
    assert_eq!(f.before.as_deref(), Some(r[4].as_str()));
    assert_eq!(f.after.as_deref(), Some(r[5].as_str()));
    assert_eq!(c.fields.len(), 1, "只有一個欄位變動：{c:?}");
}

#[then(expr = "部門 {string} 的最後修改者應為 {string}，最後修改時間應到秒")]
async fn updated_stamp(w: &mut ErpWorld, code: String, user: String) {
    let d = w.departments.get(&w.actor, PLANT, &code).await.unwrap();
    assert_eq!(d.updated_by.as_deref(), Some(user.as_str()));
    assert!(d.updated_at.is_some(), "應有最後修改時間");
}

#[then(expr = "部門 {string} 最新一筆異動記錄的動作應為 {string}，改前的簡稱為 {string}")]
async fn deleted_before(w: &mut ErpWorld, code: String, op: String, name: String) {
    let h = dept_history(w, &code).await;
    let c = last(&h);
    assert_eq!(c.operation, op);
    assert_eq!(field(c, "簡稱").before.as_deref(), Some(name.as_str()));
    assert_eq!(field(c, "簡稱").after, None);
}

#[then(expr = "部門 {string} 異動記錄的動作依序應為 {string}")]
async fn operations(w: &mut ErpWorld, code: String, ops: String) {
    let h = dept_history(w, &code).await;
    let got: Vec<&str> = h.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(got.join(","), ops);
}

#[then(expr = "部門 {string} 的簡稱應為 {string}")]
async fn dept_name(w: &mut ErpWorld, code: String, name: String) {
    let d = w.departments.get(&w.actor, PLANT, &code).await.unwrap();
    assert_eq!(d.name, name);
}

#[then(expr = "部門 {string} 最新一筆異動記錄的操作者類型應為 {string}，代表 {string}")]
async fn actor_kind_for(w: &mut ErpWorld, code: String, kind: String, user: String) {
    let h = dept_history(w, &code).await;
    let c = last(&h);
    assert_eq!(c.actor_kind, kind);
    assert_eq!(c.on_behalf_of.as_deref(), Some(user.as_str()));
}

#[then(expr = "部門 {string} 最新一筆異動記錄的操作者類型應為 {string}")]
async fn actor_kind_is(w: &mut ErpWorld, code: String, kind: String) {
    let h = dept_history(w, &code).await;
    assert_eq!(last(&h).actor_kind, kind);
}

#[then(expr = "員工 {string} 最新一筆異動記錄的電子郵件改前改後都應為 {string}")]
async fn email_masked(w: &mut ErpWorld, id: String, masked: String) {
    let h = w
        .employees
        .history(&w.actor, PLANT, &id, None)
        .await
        .expect("查詢異動記錄");
    let c = last(&h).clone();
    w.history = Some(h);
    let f = field(&c, "電子郵件");
    assert_eq!(f.before.as_deref(), Some(masked.as_str()));
    assert_eq!(f.after.as_deref(), Some(masked.as_str()));
}

#[then(expr = "變動欄位應包含 {string}")]
fn changed_contains(w: &mut ErpWorld, label: String) {
    let h = w.history.as_ref().expect("尚未查詢異動記錄");
    field(last(h), &label);
}

#[then("資料庫應拒絕")]
fn db_denied(w: &mut ErpWorld) {
    assert_eq!(
        w.db_denied,
        Some(true),
        "應用程式的資料庫角色不可修改或刪除異動記錄"
    );
}

#[then(expr = "使用者 {string} 最近一次存檔的異動記錄應包含 {string} 與 {string}，且請求代號相同")]
async fn same_request(w: &mut ErpWorld, user: String, main: String, child: String) {
    let h = w
        .shared_history
        .history(&w.actor, "user-maintenance", &user, None)
        .await
        .expect("查詢異動記錄");
    let last = last(&h).request_id.clone().expect("應有請求代號");
    let tables: Vec<&str> = h
        .iter()
        .filter(|c| c.request_id.as_deref() == Some(last.as_str()))
        .map(|c| c.table_name.as_str())
        .collect();
    assert!(
        tables.contains(&main.as_str()) && tables.contains(&child.as_str()),
        "同一請求應同時有 {main} 與 {child}：{tables:?}"
    );
}
