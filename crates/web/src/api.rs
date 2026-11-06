//! 後端 API 呼叫。Web 與桌面版共用。
//!
//! API 位址：編譯時設定 `TTRS_API_URL` 則一律使用；否則 Web 版走同源 `/api`
//! （開發由 dx proxy、正式由 nginx 轉發），桌面/行動版預設連本機 API。

use ttrs_shared::{
    ApiError, ChangePasswordInput, ClassificationCodeDto, ClassificationCodeInput, ColumnInfoDto,
    ColumnTextDto, ConstraintDto, CopyPermissionsRequest, CopyUserPermissionsRequest,
    CreateItemRequest, DepartmentAccountingInput, DepartmentDto, DepartmentInput, EmployeeDto,
    EmployeeInput, GeneratePermissionsRequest, GrantRequest, IndexDto, ItemDto, LegacyTableDto,
    LegacyTableSummaryDto, MenuPermissionsRequest, MigrationDto, MyPermissionDto,
    NewPlantIdRequest, NewProgramNoRequest, NewUserIdRequest, Page, PasswordResetInput,
    PasswordResetRequestInput, PasswordResetStatusDto, PermissionBatchResultDto,
    PermissionClassDto, PermissionClassInput, PlantDto, PlantInput, ProgramDto, ProgramInput,
    ProgramPermissionDto, ProgramPermissionInput, QbeCondition, QueryRequest, SessionDto,
    SignInInput, SignInPlantsDto, SwitchPlantInput, TableDictionaryDto, TableDictionaryInput,
    TableTextDto, UpdatedCountDto, UserDto, UserInput, UserPermissionDto, UserPermissionInput,
};

fn base_url() -> String {
    if let Some(url) = option_env!("TTRS_API_URL") {
        return url.trim_end_matches('/').to_owned();
    }
    #[cfg(target_arch = "wasm32")]
    if let Some(origin) = web_sys::window().and_then(|w| w.location().origin().ok()) {
        return origin;
    }
    "http://127.0.0.1:3000".to_owned()
}

fn url(path: &str) -> String {
    format!("{}{path}", base_url())
}

async fn into_result<T: serde::de::DeserializeOwned>(res: reqwest::Response) -> Result<T, String> {
    if res.status().is_success() {
        res.json::<T>().await.map_err(|e| e.to_string())
    } else {
        Err(error_message(res).await)
    }
}

/// 帶錯誤代碼的失敗結果：畫面要依代碼決定下一步時使用（如須改密碼）
#[derive(Debug, Clone, PartialEq)]
pub struct ApiFailure {
    pub code: String,
    pub message: String,
}

async fn failure(res: reqwest::Response) -> ApiFailure {
    let status = res.status();
    match res.json::<ApiError>().await {
        Ok(e) => ApiFailure {
            message: crate::i18n::error_text(&e.code, &e.message),
            code: e.code,
        },
        Err(_) => ApiFailure {
            code: String::new(),
            message: format!("HTTP {status}"),
        },
    }
}

fn network(e: reqwest::Error) -> ApiFailure {
    ApiFailure {
        code: String::new(),
        message: e.to_string(),
    }
}

async fn error_message(res: reqwest::Response) -> String {
    let status = res.status();
    match res.json::<ApiError>().await {
        // 依錯誤代碼翻譯成目前介面語系（L4）
        Ok(e) => crate::i18n::error_text(&e.code, &e.message),
        Err(_) => format!("HTTP {status}"),
    }
}

pub async fn list_items(plant: &str) -> Result<Vec<ItemDto>, String> {
    let res = reqwest::Client::new()
        .get(url("/api/items"))
        .query(&[("plant", plant)])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn create_item(req: &CreateItemRequest) -> Result<ItemDto, String> {
    let res = reqwest::Client::new()
        .post(url("/api/items"))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn confirm_item(plant: &str, no: &str) -> Result<ItemDto, String> {
    let res = reqwest::Client::new()
        .post(url(&format!("/api/items/{plant}/{no}/confirm")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn unconfirm_item(plant: &str, no: &str) -> Result<ItemDto, String> {
    let res = reqwest::Client::new()
        .post(url(&format!("/api/items/{plant}/{no}/unconfirm")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn delete_item(plant: &str, no: &str) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(url(&format!("/api/items/{plant}/{no}")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(error_message(res).await)
    }
}

// ----- Program Maintenance 程式維護 -----

fn program_url(no: &str, suffix: &str) -> String {
    url(&format!("/api/programs/{}{suffix}", urlencoding(no)))
}

/// 程式代號可能含 `_` 以外的符號，路徑參數一律編碼
fn urlencoding(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn no_content(res: reqwest::Response) -> Result<(), String> {
    if res.status().is_success() {
        Ok(())
    } else {
        Err(error_message(res).await)
    }
}

/// 目前的連線（sign_in.feature）：登入者、目前營運中心、可切換的營運中心；未登入為 None
pub async fn session() -> Result<Option<SessionDto>, String> {
    let res = reqwest::Client::new()
        .get(url("/api/session"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Ok(None);
    }
    into_result(res).await.map(Some)
}

/// 登入（ADR 0009）：成功時伺服器設定連線 cookie
/// 登入；`plant` 為登入畫面選的營運中心，None 時由伺服器自動選取（sign_in.feature SQ12）
pub async fn sign_in(
    user_id: &str,
    password: &str,
    plant: Option<&str>,
) -> Result<SessionDto, ApiFailure> {
    let res = reqwest::Client::new()
        .post(url("/api/session"))
        .json(&SignInInput {
            user_id: user_id.to_owned(),
            password: password.to_owned(),
            plant_id: plant.map(str::to_owned),
        })
        .send()
        .await
        .map_err(network)?;
    if res.status().is_success() {
        res.json().await.map_err(network)
    } else {
        Err(failure(res).await)
    }
}

pub async fn sign_out() -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(url("/api/session"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 變更密碼（webpasswd）
pub async fn change_password(input: &ChangePasswordInput) -> Result<(), String> {
    let res = reqwest::Client::new()
        .post(url("/api/password"))
        .json(input)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 登入畫面輸入使用者代號後的營運中心選項（SL1–SL8）；無法連線時空白
pub async fn sign_in_plants(user_id: &str) -> SignInPlantsDto {
    let Ok(res) = reqwest::Client::new()
        .get(url("/api/sign-in/plants"))
        .query(&[("user_id", user_id)])
        .send()
        .await
    else {
        return SignInPlantsDto::default();
    };
    into_result(res).await.unwrap_or_default()
}

/// 忘記密碼是否開放（ADR 0010 R9）；無法連線時視為不開放
pub async fn password_reset_available() -> bool {
    let Ok(res) = reqwest::Client::new()
        .get(url("/api/password-reset"))
        .send()
        .await
    else {
        return false;
    };
    into_result::<PasswordResetStatusDto>(res)
        .await
        .is_ok_and(|s| s.available)
}

/// 申請重設密碼：帳號不存在也回成功（R2）
pub async fn request_password_reset(user_id: &str) -> Result<(), String> {
    let res = reqwest::Client::new()
        .post(url("/api/password-reset"))
        .json(&PasswordResetRequestInput {
            user_id: user_id.to_owned(),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 以重設連結設定新密碼
pub async fn confirm_password_reset(input: &PasswordResetInput) -> Result<(), String> {
    let res = reqwest::Client::new()
        .post(url("/api/password-reset/confirm"))
        .json(input)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 切換營運中心（aoos901）
pub async fn switch_plant(plant: &str) -> Result<SessionDto, String> {
    let res = reqwest::Client::new()
        .put(url("/api/session/plant"))
        .json(&SwitchPlantInput {
            plant_id: plant.to_owned(),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

/// 目前使用者在此作業的有效權限（authorization.feature AQ13）
pub async fn my_permission(program: &str) -> Result<MyPermissionDto, String> {
    get_json(url(&format!("/api/me/permissions/{program}"))).await
}

/// QBE 查詢（條件語法見 ttrs_domain::qbe）
pub async fn query_programs(req: &QueryRequest) -> Result<Page<ProgramDto>, String> {
    post_query(url("/api/programs/query"), req).await
}

/// 各作業的 QBE 查詢共用：POST 條件，回傳結果
async fn post_query<T: serde::de::DeserializeOwned>(
    url: String,
    req: &QueryRequest,
) -> Result<T, String> {
    let res = reqwest::Client::new()
        .post(url)
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn create_program(req: &ProgramInput) -> Result<ProgramDto, String> {
    let res = reqwest::Client::new()
        .post(url("/api/programs"))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

/// 修改；`req.program_no` 與 `no` 不同時一併改代號
pub async fn update_program(no: &str, req: &ProgramInput) -> Result<ProgramDto, String> {
    let res = reqwest::Client::new()
        .put(program_url(no, ""))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn delete_program(no: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{}?version={version}", program_url(no, "")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn copy_program(no: &str, new_no: &str) -> Result<ProgramDto, String> {
    let res = reqwest::Client::new()
        .post(program_url(no, "/copy"))
        .json(&NewProgramNoRequest {
            new_no: new_no.to_owned(),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn program_permissions(no: &str) -> Result<Vec<String>, String> {
    let res = reqwest::Client::new()
        .get(program_url(no, "/permissions"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn grant_permission(no: &str, class: &str) -> Result<(), String> {
    let res = reqwest::Client::new()
        .post(program_url(no, "/permissions"))
        .json(&GrantRequest {
            permission_class: class.to_owned(),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

// ----- Data Dictionary Maintenance 資料字典維護 -----

fn dict_url(table: &str, suffix: &str) -> String {
    url(&format!(
        "/api/dictionary/tables/{}{suffix}",
        urlencoding(table)
    ))
}

async fn get_json<T: serde::de::DeserializeOwned>(u: String) -> Result<T, String> {
    let res = reqwest::Client::new()
        .get(u)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

async fn put_json<B: serde::Serialize>(u: String, body: &B) -> Result<(), String> {
    let res = reqwest::Client::new()
        .put(u)
        .json(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn schema_tables() -> Result<Vec<String>, String> {
    get_json(url("/api/dictionary/schema-tables")).await
}

/// 已建字典的資料表；表名稱、目的以目前介面語系顯示（無則退回繁中）
pub async fn list_dictionaries() -> Result<Vec<TableDictionaryDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(url(&format!("/api/dictionary/tables?lang={lang}"))).await
}

pub async fn create_dictionary(req: &TableDictionaryInput) -> Result<TableDictionaryDto, String> {
    let res = reqwest::Client::new()
        .post(url("/api/dictionary/tables"))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn update_dictionary(
    table: &str,
    req: &TableDictionaryInput,
) -> Result<TableDictionaryDto, String> {
    let res = reqwest::Client::new()
        .put(dict_url(table, ""))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn delete_dictionary(table: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{}?version={version}", dict_url(table, "")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn set_table_text(table: &str, text: &TableTextDto) -> Result<(), String> {
    put_json(dict_url(table, "/texts"), text).await
}

pub async fn table_columns(table: &str) -> Result<Vec<ColumnInfoDto>, String> {
    get_json(dict_url(table, "/columns")).await
}

pub async fn column_texts(table: &str) -> Result<Vec<ColumnTextDto>, String> {
    get_json(dict_url(table, "/column-texts")).await
}

pub async fn set_column_text(table: &str, text: &ColumnTextDto) -> Result<(), String> {
    put_json(dict_url(table, "/column-texts"), text).await
}

pub async fn table_indexes(table: &str) -> Result<Vec<IndexDto>, String> {
    get_json(dict_url(table, "/indexes")).await
}

pub async fn table_constraints(table: &str) -> Result<Vec<ConstraintDto>, String> {
    get_json(dict_url(table, "/constraints")).await
}

pub async fn table_history(table: &str) -> Result<Vec<MigrationDto>, String> {
    get_json(dict_url(table, "/history")).await
}

/// 已移植資料表對應的 TipTop 舊表；沒有對照時為 None
pub async fn table_legacy(table: &str) -> Result<Option<LegacyTableDto>, String> {
    get_json(dict_url(table, "/legacy")).await
}

// ----- TipTop 對照目錄 -----

pub async fn search_legacy(query: &str) -> Result<Vec<LegacyTableSummaryDto>, String> {
    get_json(url(&format!(
        "/api/legacy-catalog?q={}",
        urlencoding(query)
    )))
    .await
}

pub async fn get_legacy(legacy: &str) -> Result<LegacyTableDto, String> {
    get_json(url(&format!("/api/legacy-catalog/{}", urlencoding(legacy)))).await
}

/// 匯出 CSV 的下載網址（由瀏覽器直接下載）
pub fn export_url(table: &str, lang: &str) -> String {
    format!("{}?lang={lang}", dict_url(table, "/export"))
}

// ----- User Maintenance 使用者維護（TipTop p_zx） -----

fn user_url(id: &str, suffix: &str) -> String {
    url(&format!("/api/users/{}{suffix}", urlencoding(id)))
}

pub async fn list_users() -> Result<Vec<UserDto>, String> {
    get_json(url("/api/users")).await
}

pub async fn query_users(req: &QueryRequest) -> Result<Page<UserDto>, String> {
    post_query(url("/api/users/query"), req).await
}

pub async fn create_user(req: &UserInput) -> Result<UserDto, String> {
    let res = reqwest::Client::new()
        .post(url("/api/users"))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn update_user(id: &str, req: &UserInput) -> Result<UserDto, String> {
    let res = reqwest::Client::new()
        .put(user_url(id, ""))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn delete_user(id: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{}?version={version}", user_url(id, "")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn copy_user(id: &str, new_id: &str) -> Result<UserDto, String> {
    let res = reqwest::Client::new()
        .post(user_url(id, "/copy"))
        .json(&NewUserIdRequest {
            new_user_id: new_id.to_owned(),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

/// 停用或啟用（TipTop p_zx 無效切換）
pub async fn set_user_active(id: &str, active: bool, version: i32) -> Result<UserDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    let res = reqwest::Client::new()
        .post(format!("{}?version={version}", user_url(id, suffix)))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

/// 解除鎖定（p_zx user_unlock）
pub async fn unlock_user(id: &str) -> Result<(), String> {
    let res = reqwest::Client::new()
        .post(user_url(id, "/unlock"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

// ----- Permission Class Maintenance 權限類別維護（TipTop p_zw） -----

fn class_url(code: &str, suffix: &str) -> String {
    url(&format!(
        "/api/permission-classes/{}{suffix}",
        urlencoding(code)
    ))
}

/// 全部營運中心（下拉選單用的參考清單，authorization.feature AQ4）
pub async fn list_plants() -> Result<Vec<PlantDto>, String> {
    get_json(url("/api/plants")).await
}

pub async fn list_classes() -> Result<Vec<PermissionClassDto>, String> {
    get_json(url("/api/permission-classes")).await
}

pub async fn query_classes(req: &QueryRequest) -> Result<Page<PermissionClassDto>, String> {
    post_query(url("/api/permission-classes/query"), req).await
}

pub async fn create_class(req: &PermissionClassInput) -> Result<PermissionClassDto, String> {
    let res = reqwest::Client::new()
        .post(url("/api/permission-classes"))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn update_class(
    code: &str,
    req: &PermissionClassInput,
) -> Result<PermissionClassDto, String> {
    let res = reqwest::Client::new()
        .put(class_url(code, ""))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn delete_class(code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{}?version={version}", class_url(code, "")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 停用或啟用（TipTop p_zw 有效欄位）
pub async fn set_class_active(
    code: &str,
    active: bool,
    version: i32,
) -> Result<PermissionClassDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    let res = reqwest::Client::new()
        .post(format!("{}?version={version}", class_url(code, suffix)))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

/// 更新所有使用者目錄編號，回傳更新的使用者數
pub async fn update_user_menus() -> Result<u64, String> {
    let res = reqwest::Client::new()
        .post(url("/api/permission-classes/update-user-menus"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result::<UpdatedCountDto>(res).await.map(|r| r.updated)
}

// ----- Plant Maintenance 營運中心維護（TipTop aooi901） -----

fn plant_url(id: &str, suffix: &str) -> String {
    url(&format!("/api/plants/{}{suffix}", urlencoding(id)))
}

pub async fn query_plants(req: &QueryRequest) -> Result<Page<PlantDto>, String> {
    post_query(url("/api/plants/query"), req).await
}

pub async fn create_plant(req: &PlantInput) -> Result<PlantDto, String> {
    let res = reqwest::Client::new()
        .post(url("/api/plants"))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn update_plant(id: &str, req: &PlantInput) -> Result<PlantDto, String> {
    let res = reqwest::Client::new()
        .put(plant_url(id, ""))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn delete_plant(id: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{}?version={version}", plant_url(id, "")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn copy_plant(id: &str, new_id: &str) -> Result<PlantDto, String> {
    let res = reqwest::Client::new()
        .post(plant_url(id, "/copy"))
        .json(&NewPlantIdRequest {
            new_plant_id: new_id.to_owned(),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

/// 停用或啟用（P9）
pub async fn set_plant_active(id: &str, active: bool, version: i32) -> Result<PlantDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    let res = reqwest::Client::new()
        .post(format!("{}?version={version}", plant_url(id, suffix)))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

// ----- Program Permission Maintenance 程式執行權限維護（TipTop p_zy） -----

fn class_perm_url(class: &str, suffix: &str) -> String {
    url(&format!(
        "/api/permission-classes/{}{suffix}",
        urlencoding(class)
    ))
}

async fn post_batch<B: serde::Serialize>(
    target: String,
    body: &B,
) -> Result<PermissionBatchResultDto, String> {
    let res = reqwest::Client::new()
        .post(target)
        .json(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

/// 程式名稱依目前介面語系顯示
/// p_zy 查詢：符合條件的權限類別
pub async fn query_program_permissions(
    req: &QueryRequest,
) -> Result<Page<PermissionClassDto>, String> {
    post_query(url("/api/program-permissions/query"), req).await
}

/// 權限類別符合同一組查詢條件的程式（單身）
pub async fn query_class_permissions(
    class: &str,
    req: &QueryRequest,
) -> Result<Vec<ProgramPermissionDto>, String> {
    post_query(class_perm_url(class, "/permission-query"), req).await
}

pub async fn list_class_permissions(class: &str) -> Result<Vec<ProgramPermissionDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(format!(
        "{}?lang={lang}",
        class_perm_url(class, "/permissions")
    ))
    .await
}

pub async fn grant_class_permission(
    class: &str,
    req: &ProgramPermissionInput,
) -> Result<ProgramPermissionDto, String> {
    let res = reqwest::Client::new()
        .post(class_perm_url(class, "/permissions"))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn update_class_permission(
    class: &str,
    program: &str,
    req: &ProgramPermissionInput,
) -> Result<ProgramPermissionDto, String> {
    let res = reqwest::Client::new()
        .put(class_perm_url(
            class,
            &format!("/permissions/{}", urlencoding(program)),
        ))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn revoke_class_permission(class: &str, program: &str) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(class_perm_url(
            class,
            &format!("/permissions/{}", urlencoding(program)),
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn revoke_all_permissions(class: &str) -> Result<PermissionBatchResultDto, String> {
    let res = reqwest::Client::new()
        .delete(class_perm_url(class, "/permissions"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn copy_permissions(
    class: &str,
    target: &str,
) -> Result<PermissionBatchResultDto, String> {
    let body = CopyPermissionsRequest {
        target_class: target.to_owned(),
    };
    post_batch(class_perm_url(class, "/copy-permissions"), &body).await
}

pub async fn generate_permissions(
    class: &str,
    program_nos: &[String],
) -> Result<PermissionBatchResultDto, String> {
    let body = GeneratePermissionsRequest {
        program_nos: program_nos.to_vec(),
    };
    post_batch(class_perm_url(class, "/generate-permissions"), &body).await
}

pub async fn generate_permissions_by_menu(
    class: &str,
    menu: &str,
    reset_existing: bool,
) -> Result<PermissionBatchResultDto, String> {
    let body = MenuPermissionsRequest {
        menu: menu.to_owned(),
        reset_existing,
    };
    post_batch(
        class_perm_url(class, "/generate-permissions-by-menu"),
        &body,
    )
    .await
}

pub async fn delete_permissions_by_menu(
    class: &str,
    menu: &str,
) -> Result<PermissionBatchResultDto, String> {
    let body = MenuPermissionsRequest {
        menu: menu.to_owned(),
        reset_existing: false,
    };
    post_batch(class_perm_url(class, "/delete-permissions-by-menu"), &body).await
}

// ----- User Special Permission Maintenance 使用者特殊權限維護（TipTop p_zxw） -----

/// 程式名稱依目前介面語系顯示
pub async fn list_user_permissions(user: &str) -> Result<Vec<UserPermissionDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(format!("{}?lang={lang}", user_url(user, "/permissions"))).await
}

/// 回傳新增後的全部特殊權限
pub async fn add_user_permission(
    user: &str,
    req: &UserPermissionInput,
) -> Result<Vec<UserPermissionDto>, String> {
    let res = reqwest::Client::new()
        .post(user_url(user, "/permissions"))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn update_user_program_permission(
    user: &str,
    program: &str,
    req: &UserPermissionInput,
) -> Result<UserPermissionDto, String> {
    let res = reqwest::Client::new()
        .put(user_url(
            user,
            &format!("/permissions/programs/{}", urlencoding(program)),
        ))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn remove_user_permission(user: &str, line_no: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(user_url(user, &format!("/permissions/{line_no}")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn remove_all_user_permissions(user: &str) -> Result<PermissionBatchResultDto, String> {
    let res = reqwest::Client::new()
        .delete(user_url(user, "/permissions"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn copy_user_permissions(
    user: &str,
    target: &str,
) -> Result<PermissionBatchResultDto, String> {
    let body = CopyUserPermissionsRequest {
        target_user: target.to_owned(),
    };
    post_batch(user_url(user, "/copy-permissions"), &body).await
}

// ----- 部門維護（aooi030）、員工維護（aooi040）：工廠層，路徑帶營運中心 -----

fn plant_part_url(plant: &str, part: &str, key: &str, suffix: &str) -> String {
    let base = format!("/api/plants/{}/{part}", urlencoding(plant));
    if key.is_empty() {
        url(&format!("{base}{suffix}"))
    } else {
        url(&format!("{base}/{}{suffix}", urlencoding(key)))
    }
}

async fn send_json<T: serde::de::DeserializeOwned, B: serde::Serialize>(
    method: reqwest::Method,
    u: String,
    body: Option<&B>,
) -> Result<T, String> {
    let mut req = reqwest::Client::new().request(method, u);
    if let Some(b) = body {
        req = req.json(b);
    }
    let res = req.send().await.map_err(|e| e.to_string())?;
    into_result(res).await
}

// ----- 報表（ADR 0011）：PDF 在新分頁預覽列印，Excel 下載 -----

/// 報表格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Pdf,
    Excel,
}

impl ReportFormat {
    fn ext(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Excel => "xlsx",
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn mime(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Excel => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        }
    }
}

/// 列印時先開好的分頁（在按下按鈕的當下開，避免被瀏覽器的彈出視窗封鎖擋下）
pub struct ReportTab(#[cfg(target_arch = "wasm32")] Option<web_sys::Window>);

impl ReportTab {
    /// 列印才需要先開分頁；匯出 Excel 直接下載
    pub fn open(format: ReportFormat) -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            let win = (format == ReportFormat::Pdf)
                .then(|| {
                    web_sys::window()?
                        .open_with_url_and_target("", "_blank")
                        .ok()?
                })
                .flatten();
            Self(win)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = format;
            Self()
        }
    }
}

/// 取得部門報表；成功時 PDF 顯示在先開好的分頁、Excel 下載；失敗時關掉分頁並回傳錯誤訊息
pub async fn department_report(
    plant: &str,
    format: ReportFormat,
    conditions: &[QbeCondition],
    lang: &str,
    tab: ReportTab,
) -> Result<(), String> {
    let q = serde_json::to_string(conditions).map_err(|e| e.to_string())?;
    let u = plant_part_url(
        plant,
        "departments",
        "",
        &format!("/report.{}", format.ext()),
    );
    let res = reqwest::Client::new()
        .get(u)
        .query(&[("q", q.as_str()), ("lang", lang)])
        .send()
        .await
        .map_err(|e| e.to_string());
    let res = match res {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            close_tab(&tab);
            return Err(error_message(r).await);
        }
        Err(e) => {
            close_tab(&tab);
            return Err(e);
        }
    };
    let bytes = res.bytes().await.map_err(|e| e.to_string())?;
    show_report(
        &bytes,
        format,
        &format!("{plant}-departments.{}", format.ext()),
        tab,
    )
}

#[cfg(target_arch = "wasm32")]
fn close_tab(tab: &ReportTab) {
    if let Some(w) = &tab.0 {
        let _ = w.close();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn close_tab(_tab: &ReportTab) {}

#[cfg(target_arch = "wasm32")]
fn show_report(
    bytes: &[u8],
    format: ReportFormat,
    file: &str,
    tab: ReportTab,
) -> Result<(), String> {
    use wasm_bindgen::JsCast;
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(format.mime());
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    match (format, tab.0) {
        (ReportFormat::Pdf, Some(w)) => w.location().set_href(&url).map_err(err)?,
        _ => {
            let doc = web_sys::window()
                .and_then(|w| w.document())
                .ok_or("沒有 document")?;
            let a: web_sys::HtmlAnchorElement = doc
                .create_element("a")
                .map_err(err)?
                .dyn_into()
                .map_err(|_| "建立連結失敗".to_owned())?;
            a.set_href(&url);
            a.set_download(file);
            a.click();
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn show_report(
    _bytes: &[u8],
    _format: ReportFormat,
    _file: &str,
    _tab: ReportTab,
) -> Result<(), String> {
    Ok(())
}

// ----- Classification Code 分類碼維護（TipTop aooi300） -----

fn code_url(plant: &str, kind: &str, code: &str, suffix: &str) -> String {
    plant_part_url(
        plant,
        &format!("classification-codes/{}", urlencoding(kind)),
        code,
        suffix,
    )
}

pub async fn list_codes(plant: &str, kind: &str) -> Result<Vec<ClassificationCodeDto>, String> {
    get_json(code_url(plant, kind, "", "")).await
}

pub async fn create_code(
    plant: &str,
    kind: &str,
    req: &ClassificationCodeInput,
) -> Result<ClassificationCodeDto, String> {
    send_json(
        reqwest::Method::POST,
        code_url(plant, kind, "", ""),
        Some(req),
    )
    .await
}

pub async fn update_code(
    plant: &str,
    kind: &str,
    code: &str,
    req: &ClassificationCodeInput,
) -> Result<ClassificationCodeDto, String> {
    send_json(
        reqwest::Method::PUT,
        code_url(plant, kind, code, ""),
        Some(req),
    )
    .await
}

pub async fn set_code_active(
    plant: &str,
    kind: &str,
    code: &str,
    active: bool,
    version: i32,
) -> Result<ClassificationCodeDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!("{}?version={version}", code_url(plant, kind, code, suffix)),
        None,
    )
    .await
}

pub async fn delete_code(plant: &str, kind: &str, code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            code_url(plant, kind, code, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn query_departments(
    plant: &str,
    req: &QueryRequest,
) -> Result<Page<DepartmentDto>, String> {
    post_query(plant_part_url(plant, "departments", "", "/query"), req).await
}

pub async fn create_department(
    plant: &str,
    req: &DepartmentInput,
) -> Result<DepartmentDto, String> {
    send_json(
        reqwest::Method::POST,
        plant_part_url(plant, "departments", "", ""),
        Some(req),
    )
    .await
}

pub async fn update_department(
    plant: &str,
    code: &str,
    req: &DepartmentInput,
) -> Result<DepartmentDto, String> {
    send_json(
        reqwest::Method::PUT,
        plant_part_url(plant, "departments", code, ""),
        Some(req),
    )
    .await
}

pub async fn set_department_accounting(
    plant: &str,
    code: &str,
    req: &DepartmentAccountingInput,
) -> Result<DepartmentDto, String> {
    send_json(
        reqwest::Method::PUT,
        plant_part_url(plant, "departments", code, "/accounting"),
        Some(req),
    )
    .await
}

pub async fn set_department_active(
    plant: &str,
    code: &str,
    active: bool,
    version: i32,
) -> Result<DepartmentDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            plant_part_url(plant, "departments", code, suffix)
        ),
        None,
    )
    .await
}

pub async fn delete_department(plant: &str, code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            plant_part_url(plant, "departments", code, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn query_employees(plant: &str, req: &QueryRequest) -> Result<Page<EmployeeDto>, String> {
    post_query(plant_part_url(plant, "employees", "", "/query"), req).await
}

pub async fn create_employee(plant: &str, req: &EmployeeInput) -> Result<EmployeeDto, String> {
    send_json(
        reqwest::Method::POST,
        plant_part_url(plant, "employees", "", ""),
        Some(req),
    )
    .await
}

pub async fn update_employee(
    plant: &str,
    id: &str,
    req: &EmployeeInput,
) -> Result<EmployeeDto, String> {
    send_json(
        reqwest::Method::PUT,
        plant_part_url(plant, "employees", id, ""),
        Some(req),
    )
    .await
}

pub async fn set_employee_active(
    plant: &str,
    id: &str,
    active: bool,
    version: i32,
) -> Result<EmployeeDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            plant_part_url(plant, "employees", id, suffix)
        ),
        None,
    )
    .await
}

pub async fn delete_employee(plant: &str, id: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            plant_part_url(plant, "employees", id, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

// ----- 資料異動記錄（ADR 0012 §8；功能 history） -----

/// 部門、員工的異動記錄；欄位名稱依目前介面語系
pub async fn history(
    plant: &str,
    part: &str,
    key: &str,
) -> Result<Vec<ttrs_shared::ChangeDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(format!(
        "{}?lang={lang}",
        plant_part_url(plant, part, key, "/history")
    ))
    .await
}

/// 共用層作業的異動記錄（程式代號＋鍵值，例如 user-maintenance／u001）
pub async fn shared_history(
    program: &str,
    key: &str,
) -> Result<Vec<ttrs_shared::ChangeDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(url(&format!("/api/history/{program}/{key}?lang={lang}"))).await
}

/// 分類碼的異動記錄
pub async fn code_history(
    plant: &str,
    kind: &str,
    code: &str,
) -> Result<Vec<ttrs_shared::ChangeDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(format!(
        "{}?lang={lang}",
        code_url(plant, kind, code, "/history")
    ))
    .await
}
