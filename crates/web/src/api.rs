//! 後端 API 呼叫。Web 與桌面版共用。
//!
//! API 位址：編譯時設定 `TTRS_API_URL` 則一律使用；否則 Web 版走同源 `/api`
//! （開發由 dx proxy、正式由 nginx 轉發），桌面/行動版預設連本機 API。

use ttrs_shared::{
    ApiError, ChangePasswordInput, ClassificationCodeDto, ClassificationCodeInput, ColumnInfoDto,
    ColumnTextDto, ConstraintDto, CopyPermissionsRequest, CopyUserPermissionsRequest,
    DepartmentAccountingInput, DepartmentDto, DepartmentInput, EmployeeDto, EmployeeInput,
    GeneratePermissionsRequest, GrantRequest, IndexDto, ItemDto, LegacyTableDto,
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

// ----- Item Maintenance 料件基本資料維護（TipTop aimi100；工廠層） -----

fn item_url(plant: &str, no: &str, suffix: &str) -> String {
    plant_part_url(plant, "items", no, suffix)
}

/// QBE 查詢（item_maintenance_query.feature）；品名、規格為繁中本身，其他語系在 `texts`
pub async fn query_items(plant: &str, req: &QueryRequest) -> Result<Page<ItemDto>, String> {
    post_query(item_url(plant, "query", ""), req).await
}

/// 查詢一筆（維護用：品名、規格為繁中本身，其他語系在 `texts`）
pub async fn get_item(plant: &str, no: &str) -> Result<ItemDto, String> {
    get_json(item_url(plant, no, "")).await
}

/// 料件取號選單（item_auto_number.feature AN2、AN6）
pub async fn item_number_options(plant: &str) -> Result<ttrs_shared::ItemNumberOptionsDto, String> {
    get_json(item_url(plant, "auto-number", "")).await
}

/// 取料號（AN2–AN5）：不保留號碼
pub async fn next_item_number(
    plant: &str,
    req: &ttrs_shared::ItemNumberRequest,
) -> Result<ttrs_shared::EncodingPreviewDto, String> {
    send_json(
        reqwest::Method::POST,
        item_url(plant, "auto-number", ""),
        Some(req),
    )
    .await
}

pub async fn create_item(plant: &str, req: &ttrs_shared::ItemInput) -> Result<ItemDto, String> {
    send_json(reqwest::Method::POST, item_url(plant, "", ""), Some(req)).await
}

pub async fn update_item(
    plant: &str,
    no: &str,
    req: &ttrs_shared::ItemInput,
) -> Result<ItemDto, String> {
    send_json(reqwest::Method::PUT, item_url(plant, no, ""), Some(req)).await
}

/// confirm、unconfirm、void（作廢／恢復）、order-invalid（訂單無效切換）
pub async fn item_action(
    plant: &str,
    no: &str,
    action: &str,
    version: i32,
) -> Result<ItemDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            item_url(plant, no, &format!("/{action}"))
        ),
        None,
    )
    .await
}

pub async fn copy_item(
    plant: &str,
    no: &str,
    req: &ttrs_shared::CopyItemRequest,
) -> Result<ItemDto, String> {
    send_json(
        reqwest::Method::POST,
        item_url(plant, no, "/copy"),
        Some(req),
    )
    .await
}

pub async fn set_item_text(
    plant: &str,
    no: &str,
    req: &ttrs_shared::ItemTextDto,
) -> Result<ItemDto, String> {
    send_json(
        reqwest::Method::PUT,
        item_url(plant, no, "/texts"),
        Some(req),
    )
    .await
}

pub async fn delete_item(plant: &str, no: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{}?version={version}", item_url(plant, no, "")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn item_history(plant: &str, no: &str) -> Result<Vec<ttrs_shared::ChangeDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(format!("{}?lang={lang}", item_url(plant, no, "/history"))).await
}

/// 分群碼帶入的料件預設值（IM25）
pub async fn item_group_defaults(
    plant: &str,
    group: &str,
) -> Result<ttrs_shared::ItemInput, String> {
    get_json(plant_part_url(
        plant,
        "item-groups",
        group,
        "/item-defaults",
    ))
    .await
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

/// 確認或取消確認（ADR 0013 SA9）
pub async fn confirm_user(id: &str, confirm: bool, version: i32) -> Result<UserDto, String> {
    let suffix = if confirm { "confirm" } else { "unconfirm" };
    let res = reqwest::Client::new()
        .post(url(&format!(
            "/api/users/{}/{suffix}?version={version}",
            urlencoding(id)
        )))
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

/// 複製（ADR 0013 SA5）
pub async fn copy_class(code: &str, new_code: &str) -> Result<PermissionClassDto, String> {
    let res = reqwest::Client::new()
        .post(url(&format!(
            "/api/permission-classes/{}/copy",
            urlencoding(code)
        )))
        .json(&serde_json::json!({ "new_class_code": new_code }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

/// 確認或取消確認（ADR 0013 SA9）
pub async fn confirm_class(
    code: &str,
    confirm: bool,
    version: i32,
) -> Result<PermissionClassDto, String> {
    let suffix = if confirm { "confirm" } else { "unconfirm" };
    let res = reqwest::Client::new()
        .post(url(&format!(
            "/api/permission-classes/{}/{suffix}?version={version}",
            urlencoding(code)
        )))
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

/// 確認或取消確認（ADR 0013 SA9）
pub async fn confirm_plant(id: &str, confirm: bool, version: i32) -> Result<PlantDto, String> {
    let suffix = if confirm { "/confirm" } else { "/unconfirm" };
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
    list_report(plant, "departments", format, conditions, lang, tab).await
}

/// 工廠層作業的清單報表（ADR 0013 SA3）：`part` 為 API 路徑中的作業資料（items、item-groups…）
pub async fn list_report(
    plant: &str,
    part: &str,
    format: ReportFormat,
    conditions: &[QbeCondition],
    lang: &str,
    tab: ReportTab,
) -> Result<(), String> {
    let u = plant_part_url(plant, part, "", &format!("/report.{}", format.ext()));
    let file = format!("{plant}-{part}.{}", format.ext());
    report_at(u, &file, format, conditions, lang, tab).await
}

/// 系統層作業的清單報表（營運中心、使用者…，ADR 0013 SA3）：`path` 為 API 路徑（如 `/api/plants`）
pub async fn system_list_report(
    path: &str,
    name: &str,
    format: ReportFormat,
    conditions: &[QbeCondition],
    lang: &str,
    tab: ReportTab,
) -> Result<(), String> {
    let u = url(&format!("{path}/report.{}", format.ext()));
    let file = format!("{name}.{}", format.ext());
    report_at(u, &file, format, conditions, lang, tab).await
}

/// 取得報表檔：成功時 PDF 顯示在先開好的分頁、Excel 下載；失敗時關掉分頁並回傳錯誤訊息
async fn report_at(
    u: String,
    file: &str,
    format: ReportFormat,
    conditions: &[QbeCondition],
    lang: &str,
    tab: ReportTab,
) -> Result<(), String> {
    let q = serde_json::to_string(conditions).map_err(|e| e.to_string())?;
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
    show_report(&bytes, format, file, tab)
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

/// QBE 查詢（ADR 0013 SA1）：同一類別內
pub async fn query_codes(
    plant: &str,
    kind: &str,
    req: &QueryRequest,
) -> Result<Page<ClassificationCodeDto>, String> {
    post_query(code_url(plant, kind, "query", ""), req).await
}

/// 複製（ADR 0013 SA5）
pub async fn copy_code(
    plant: &str,
    kind: &str,
    code: &str,
    new_code: &str,
) -> Result<ClassificationCodeDto, String> {
    send_json(
        reqwest::Method::POST,
        code_url(plant, kind, code, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code })),
    )
    .await
}

/// 確認或取消確認（ADR 0013 SA9）
pub async fn confirm_code(
    plant: &str,
    kind: &str,
    code: &str,
    confirm: bool,
    version: i32,
) -> Result<ClassificationCodeDto, String> {
    let suffix = if confirm { "/confirm" } else { "/unconfirm" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!("{}?version={version}", code_url(plant, kind, code, suffix)),
        None,
    )
    .await
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

/// 工廠層作業的複製（ADR 0013 SA5）：`part` 為 API 路徑中的作業資料（departments、employees…）
async fn copy_plant_part<T: serde::de::DeserializeOwned>(
    plant: &str,
    part: &str,
    key: &str,
    new_code: &str,
) -> Result<T, String> {
    send_json(
        reqwest::Method::POST,
        plant_part_url(plant, part, key, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code })),
    )
    .await
}

/// 工廠層作業的確認或取消確認（ADR 0013 SA9）
async fn confirm_plant_part<T: serde::de::DeserializeOwned>(
    plant: &str,
    part: &str,
    key: &str,
    confirm: bool,
    version: i32,
) -> Result<T, String> {
    let suffix = if confirm { "/confirm" } else { "/unconfirm" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            plant_part_url(plant, part, key, suffix)
        ),
        None,
    )
    .await
}

/// QBE 查詢（ADR 0013 SA1）
pub async fn query_product_classes(
    plant: &str,
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::ProductClassDto>, String> {
    post_query(plant_part_url(plant, "product-classes", "query", ""), req).await
}

pub async fn copy_product_class(
    plant: &str,
    code: &str,
    new_code: &str,
) -> Result<ttrs_shared::ProductClassDto, String> {
    copy_plant_part(plant, "product-classes", code, new_code).await
}

pub async fn confirm_product_class(
    plant: &str,
    code: &str,
    confirm: bool,
    version: i32,
) -> Result<ttrs_shared::ProductClassDto, String> {
    confirm_plant_part(plant, "product-classes", code, confirm, version).await
}

/// 使用者特殊權限查詢（ADR 0013 SA1）：有特殊權限且符合條件的使用者
pub async fn query_user_permissions(
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::SpecialPermissionUserDto>, String> {
    post_query(url("/api/user-permissions/query"), req).await
}

pub async fn copy_department(
    plant: &str,
    code: &str,
    new_code: &str,
) -> Result<DepartmentDto, String> {
    copy_plant_part(plant, "departments", code, new_code).await
}

pub async fn confirm_department(
    plant: &str,
    code: &str,
    confirm: bool,
    version: i32,
) -> Result<DepartmentDto, String> {
    confirm_plant_part(plant, "departments", code, confirm, version).await
}

pub async fn copy_employee(plant: &str, id: &str, new_id: &str) -> Result<EmployeeDto, String> {
    copy_plant_part(plant, "employees", id, new_id).await
}

pub async fn confirm_employee(
    plant: &str,
    id: &str,
    confirm: bool,
    version: i32,
) -> Result<EmployeeDto, String> {
    confirm_plant_part(plant, "employees", id, confirm, version).await
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

// ----- Menu Maintenance 選單維護（p_zm）與側欄樹狀選單（menu_maintenance.feature） -----

fn menu_url(menu: &str, suffix: &str) -> String {
    url(&format!("/api/menus/{menu}{suffix}"))
}

/// 目前使用者的側欄（依介面語系）
pub async fn menu_tree() -> Result<Vec<ttrs_shared::MenuNodeDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(url(&format!("/api/menus/tree?lang={lang}"))).await
}

pub async fn list_menus() -> Result<Vec<ttrs_shared::MenuSummaryDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(url(&format!("/api/menus?lang={lang}"))).await
}

pub async fn menu_items(menu: &str) -> Result<Vec<ttrs_shared::MenuItemDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(format!("{}?lang={lang}", menu_url(menu, "/items"))).await
}

pub async fn add_menu_item(
    menu: &str,
    req: &ttrs_shared::MenuItemInput,
) -> Result<Vec<ttrs_shared::MenuItemDto>, String> {
    send_json(reqwest::Method::POST, menu_url(menu, "/items"), Some(req)).await
}

pub async fn remove_menu_item(menu: &str, program: &str) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(menu_url(menu, &format!("/items/{program}")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn shift_menu_item(
    menu: &str,
    program: &str,
    up: bool,
) -> Result<Vec<ttrs_shared::MenuItemDto>, String> {
    let dir = if up { "up" } else { "down" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        menu_url(menu, &format!("/items/{program}/{dir}")),
        None,
    )
    .await
}

pub async fn renumber_menu(menu: &str) -> Result<Vec<ttrs_shared::MenuItemDto>, String> {
    send_json::<_, ()>(reqwest::Method::POST, menu_url(menu, "/renumber"), None).await
}

pub async fn copy_menu(menu: &str, new_menu_no: &str) -> Result<(), String> {
    let res = reqwest::Client::new()
        .post(menu_url(menu, "/copy"))
        .json(&ttrs_shared::MenuCopyInput {
            new_menu_no: new_menu_no.to_owned(),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn delete_menu(menu: &str) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(menu_url(menu, ""))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

// ----- Unit 單位維護（TipTop aooi101；共用層） -----

fn unit_url(code: &str, suffix: &str) -> String {
    if code.is_empty() {
        url(&format!("/api/units{suffix}"))
    } else {
        url(&format!("/api/units/{}{suffix}", urlencoding(code)))
    }
}

/// QBE 查詢（ADR 0013 SA1）
pub async fn query_units(req: &QueryRequest) -> Result<Page<ttrs_shared::UnitDto>, String> {
    post_query(unit_url("", "/query"), req).await
}

/// 複製（ADR 0013 SA5）
pub async fn copy_unit(code: &str, new_code: &str) -> Result<ttrs_shared::UnitDto, String> {
    send_json(
        reqwest::Method::POST,
        unit_url(code, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code })),
    )
    .await
}

/// 確認或取消確認（ADR 0013 SA9）
pub async fn confirm_unit(
    code: &str,
    confirm: bool,
    version: i32,
) -> Result<ttrs_shared::UnitDto, String> {
    let suffix = if confirm { "/confirm" } else { "/unconfirm" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!("{}?version={version}", unit_url(code, suffix)),
        None,
    )
    .await
}

pub async fn create_unit(req: &ttrs_shared::UnitInput) -> Result<ttrs_shared::UnitDto, String> {
    send_json(reqwest::Method::POST, unit_url("", ""), Some(req)).await
}

pub async fn update_unit(
    code: &str,
    req: &ttrs_shared::UnitInput,
) -> Result<ttrs_shared::UnitDto, String> {
    send_json(reqwest::Method::PUT, unit_url(code, ""), Some(req)).await
}

pub async fn set_unit_active(
    code: &str,
    active: bool,
    version: i32,
) -> Result<ttrs_shared::UnitDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!("{}?version={version}", unit_url(code, suffix)),
        None,
    )
    .await
}

pub async fn delete_unit(code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{}?version={version}", unit_url(code, "")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

// ----- Encoding Nature Maintenance 編碼性質維護（TipTop aooi401；共用） -----

fn encoding_nature_url(code: &str, suffix: &str) -> String {
    if code.is_empty() {
        url(&format!("/api/encoding-natures{suffix}"))
    } else {
        url(&format!(
            "/api/encoding-natures/{}{suffix}",
            urlencoding(code)
        ))
    }
}

/// QBE 查詢（ADR 0013 SA1）
pub async fn query_encoding_natures(
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::EncodingNatureDto>, String> {
    post_query(encoding_nature_url("", "/query"), req).await
}

/// 複製（ADR 0013 SA5）
pub async fn copy_encoding_nature(
    code: &str,
    new_code: &str,
) -> Result<ttrs_shared::EncodingNatureDto, String> {
    send_json(
        reqwest::Method::POST,
        encoding_nature_url(code, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code })),
    )
    .await
}

/// 確認或取消確認（ADR 0013 SA9）
pub async fn confirm_encoding_nature(
    code: &str,
    confirm: bool,
    version: i32,
) -> Result<ttrs_shared::EncodingNatureDto, String> {
    let suffix = if confirm { "/confirm" } else { "/unconfirm" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!("{}?version={version}", encoding_nature_url(code, suffix)),
        None,
    )
    .await
}

pub async fn create_encoding_nature(
    req: &ttrs_shared::EncodingNatureInput,
) -> Result<ttrs_shared::EncodingNatureDto, String> {
    send_json(
        reqwest::Method::POST,
        encoding_nature_url("", ""),
        Some(req),
    )
    .await
}

pub async fn update_encoding_nature(
    code: &str,
    req: &ttrs_shared::EncodingNatureInput,
) -> Result<ttrs_shared::EncodingNatureDto, String> {
    send_json(
        reqwest::Method::PUT,
        encoding_nature_url(code, ""),
        Some(req),
    )
    .await
}

pub async fn set_encoding_nature_active(
    code: &str,
    active: bool,
    version: i32,
) -> Result<ttrs_shared::EncodingNatureDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!("{}?version={version}", encoding_nature_url(code, suffix)),
        None,
    )
    .await
}

pub async fn delete_encoding_nature(code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            encoding_nature_url(code, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

// ----- Encoding Class Maintenance 編碼類別維護（TipTop aooi402；共用） -----

fn encoding_class_url(code: &str, suffix: &str) -> String {
    if code.is_empty() {
        url(&format!("/api/encoding-classes{suffix}"))
    } else {
        url(&format!(
            "/api/encoding-classes/{}{suffix}",
            urlencoding(code)
        ))
    }
}

pub async fn query_encoding_classes(
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::EncodingClassDto>, String> {
    post_query(encoding_class_url("", "/query"), req).await
}

pub async fn create_encoding_class(
    req: &ttrs_shared::EncodingClassInput,
) -> Result<ttrs_shared::EncodingClassDto, String> {
    send_json(reqwest::Method::POST, encoding_class_url("", ""), Some(req)).await
}

pub async fn update_encoding_class(
    code: &str,
    req: &ttrs_shared::EncodingClassInput,
) -> Result<ttrs_shared::EncodingClassDto, String> {
    send_json(
        reqwest::Method::PUT,
        encoding_class_url(code, ""),
        Some(req),
    )
    .await
}

pub async fn copy_encoding_class(
    code: &str,
    new_code: &str,
) -> Result<ttrs_shared::EncodingClassDto, String> {
    send_json(
        reqwest::Method::POST,
        encoding_class_url(code, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code })),
    )
    .await
}

/// 停用／啟用（activate、deactivate）、確認／取消確認（confirm、unconfirm）
pub async fn set_encoding_class_state(
    code: &str,
    state: &str,
    version: i32,
) -> Result<ttrs_shared::EncodingClassDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            encoding_class_url(code, &format!("/{state}"))
        ),
        None,
    )
    .await
}

pub async fn delete_encoding_class(code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            encoding_class_url(code, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 單身編輯的方式
#[derive(Clone, Copy, PartialEq)]
pub enum SegmentEdit {
    /// 新增在最後（段次空白時為最大＋1）
    Add,
    /// 在指定段次插入，之後的段次加 1
    Insert(i32),
    /// 修改指定段次
    Update(i32),
}

pub async fn save_encoding_class_segment(
    code: &str,
    edit: SegmentEdit,
    req: &ttrs_shared::EncodingClassSegmentInput,
) -> Result<ttrs_shared::EncodingClassDto, String> {
    let (method, suffix) = match edit {
        SegmentEdit::Add => (reqwest::Method::POST, "/segments".to_owned()),
        SegmentEdit::Insert(n) => (reqwest::Method::POST, format!("/segments/{n}/insert")),
        SegmentEdit::Update(n) => (reqwest::Method::PUT, format!("/segments/{n}")),
    };
    send_json(method, encoding_class_url(code, &suffix), Some(req)).await
}

pub async fn remove_encoding_class_segment(
    code: &str,
    no: i32,
    version: i32,
) -> Result<ttrs_shared::EncodingClassDto, String> {
    send_json::<_, ()>(
        reqwest::Method::DELETE,
        format!(
            "{}?version={version}",
            encoding_class_url(code, &format!("/segments/{no}"))
        ),
        None,
    )
    .await
}

/// 編碼試算（encoding_rule.feature ER4）
pub async fn preview_encoding_class(
    code: &str,
    plant: &str,
    selections: Vec<String>,
) -> Result<ttrs_shared::EncodingPreviewDto, String> {
    send_json(
        reqwest::Method::POST,
        encoding_class_url(code, "/preview"),
        Some(&ttrs_shared::EncodingPreviewRequest {
            plant: plant.to_owned(),
            selections,
        }),
    )
    .await
}

pub async fn renumber_encoding_class(
    code: &str,
    version: i32,
) -> Result<ttrs_shared::EncodingClassDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            encoding_class_url(code, "/segments/renumber")
        ),
        None,
    )
    .await
}

// ----- Encoding Year Maintenance 編碼年對照維護（TipTop aooi404；共用） -----
// 鍵值 key 為「年/長度」（EncodingYearDto::key），直接當路徑使用

fn encoding_year_url(key: &str, suffix: &str) -> String {
    if key.is_empty() {
        url(&format!("/api/encoding-years{suffix}"))
    } else {
        url(&format!("/api/encoding-years/{key}{suffix}"))
    }
}

pub async fn query_encoding_years(
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::EncodingYearDto>, String> {
    post_query(encoding_year_url("", "/query"), req).await
}

pub async fn create_encoding_year(
    req: &ttrs_shared::EncodingYearInput,
) -> Result<ttrs_shared::EncodingYearDto, String> {
    send_json(reqwest::Method::POST, encoding_year_url("", ""), Some(req)).await
}

pub async fn update_encoding_year(
    key: &str,
    req: &ttrs_shared::EncodingYearInput,
) -> Result<ttrs_shared::EncodingYearDto, String> {
    send_json(reqwest::Method::PUT, encoding_year_url(key, ""), Some(req)).await
}

pub async fn copy_encoding_year(
    key: &str,
    new_year: &str,
) -> Result<ttrs_shared::EncodingYearDto, String> {
    send_json(
        reqwest::Method::POST,
        encoding_year_url(key, "/copy"),
        Some(&ttrs_shared::EncodingYearCopyRequest {
            new_year: new_year.trim().parse().ok(),
        }),
    )
    .await
}

/// 停用／啟用（activate、deactivate）、確認／取消確認（confirm、unconfirm）
pub async fn set_encoding_year_state(
    key: &str,
    state: &str,
    version: i32,
) -> Result<ttrs_shared::EncodingYearDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            encoding_year_url(key, &format!("/{state}"))
        ),
        None,
    )
    .await
}

pub async fn delete_encoding_year(key: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{}?version={version}", encoding_year_url(key, "")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

// ----- 料件拋轉（aimi100 carry；item_carry.feature） -----

pub async fn item_carry_info(plant: &str) -> Result<ttrs_shared::ItemCarryInfoDto, String> {
    get_json(url(&format!(
        "/api/plants/{}/items-carry",
        urlencoding(plant)
    )))
    .await
}

pub async fn run_item_carry(
    plant: &str,
    no: &str,
    req: &ttrs_shared::ItemCarryRequest,
) -> Result<ttrs_shared::ItemCarryResultDto, String> {
    send_json(
        reqwest::Method::POST,
        url(&format!(
            "/api/plants/{}/items-carry/{}",
            urlencoding(plant),
            urlencoding(no)
        )),
        Some(req),
    )
    .await
}

pub async fn item_carry_history(no: &str) -> Result<Vec<ttrs_shared::CarryLogDto>, String> {
    get_json(url(&format!(
        "/api/items-carry-history/{}",
        urlencoding(no)
    )))
    .await
}

// ----- Basic Data Carry 基本資料拋轉（aoop603）、Carry History 拋轉歷史查詢（aooq604） -----

pub async fn basic_carry_info(plant: &str) -> Result<ttrs_shared::BasicCarryInfoDto, String> {
    get_json(url(&format!(
        "/api/plants/{}/basic-data-carry",
        urlencoding(plant)
    )))
    .await
}

pub async fn run_basic_carry(
    plant: &str,
    req: &ttrs_shared::BasicCarryRequest,
) -> Result<ttrs_shared::BasicCarryResultDto, String> {
    send_json(
        reqwest::Method::POST,
        url(&format!(
            "/api/plants/{}/basic-data-carry",
            urlencoding(plant)
        )),
        Some(req),
    )
    .await
}

pub async fn query_carry_history(
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::CarryLogDto>, String> {
    post_query(url("/api/carry-history/query"), req).await
}

// ----- Carry Setting Maintenance 資料中心拋轉設定（TipTop aooi602；共用） -----

fn carry_url(center: &str, data_type: &str, suffix: &str) -> String {
    if center.is_empty() {
        url(&format!("/api/carry-settings{suffix}"))
    } else {
        url(&format!(
            "/api/carry-settings/{}/{}{suffix}",
            urlencoding(center),
            urlencoding(data_type)
        ))
    }
}

pub async fn query_carry_settings(
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::CarrySettingDto>, String> {
    post_query(carry_url("", "", "/query"), req).await
}

pub async fn create_carry_setting(
    req: &ttrs_shared::CarrySettingInput,
) -> Result<ttrs_shared::CarrySettingDto, String> {
    send_json(reqwest::Method::POST, carry_url("", "", ""), Some(req)).await
}

pub async fn update_carry_setting(
    center: &str,
    data_type: &str,
    req: &ttrs_shared::CarrySettingInput,
) -> Result<ttrs_shared::CarrySettingDto, String> {
    send_json(
        reqwest::Method::PUT,
        carry_url(center, data_type, ""),
        Some(req),
    )
    .await
}

pub async fn delete_carry_setting(
    center: &str,
    data_type: &str,
    version: i32,
) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            carry_url(center, data_type, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn copy_carry_setting(
    center: &str,
    data_type: &str,
    req: &ttrs_shared::CarryCopyInput,
) -> Result<ttrs_shared::CarrySettingDto, String> {
    send_json(
        reqwest::Method::POST,
        carry_url(center, data_type, "/copy"),
        Some(req),
    )
    .await
}

/// 新增（edit＝false）或修改拋轉營運中心（detail）
pub async fn save_carry_target(
    center: &str,
    data_type: &str,
    edit: bool,
    req: &ttrs_shared::CarryTargetInput,
) -> Result<ttrs_shared::CarrySettingDto, String> {
    let method = if edit {
        reqwest::Method::PUT
    } else {
        reqwest::Method::POST
    };
    send_json(method, carry_url(center, data_type, "/targets"), Some(req)).await
}

/// 刪除拋轉營運中心、拋轉條件或預設值；`suffix` 如 /targets/TW02、/targets/TW02/conditions/group_code
pub async fn remove_carry_part(
    center: &str,
    data_type: &str,
    suffix: &str,
    version: i32,
) -> Result<ttrs_shared::CarrySettingDto, String> {
    send_json::<_, ()>(
        reqwest::Method::DELETE,
        format!("{}?version={version}", carry_url(center, data_type, suffix)),
        None,
    )
    .await
}

/// 新增或修改拋轉條件（detail）
pub async fn save_carry_condition(
    center: &str,
    data_type: &str,
    plant: &str,
    edit: bool,
    req: &ttrs_shared::DataControlRuleInput,
) -> Result<ttrs_shared::CarrySettingDto, String> {
    let method = if edit {
        reqwest::Method::PUT
    } else {
        reqwest::Method::POST
    };
    send_json(
        method,
        carry_url(
            center,
            data_type,
            &format!("/targets/{}/conditions", urlencoding(plant)),
        ),
        Some(req),
    )
    .await
}

/// 新增或修改預設值（set_default_value）
pub async fn save_carry_default(
    center: &str,
    data_type: &str,
    plant: &str,
    edit: bool,
    req: &ttrs_shared::CarryDefaultInput,
) -> Result<ttrs_shared::CarrySettingDto, String> {
    let method = if edit {
        reqwest::Method::PUT
    } else {
        reqwest::Method::POST
    };
    send_json(
        method,
        carry_url(
            center,
            data_type,
            &format!("/targets/{}/defaults", urlencoding(plant)),
        ),
        Some(req),
    )
    .await
}

// ----- Data Control Maintenance 資料控管原則維護（TipTop aooi601；共用） -----

fn data_control_url(suffix: &str) -> String {
    url(&format!("/api/data-controls{suffix}"))
}

fn data_control_plant_url(data_type: &str, plant: &str, suffix: &str) -> String {
    data_control_url(&format!(
        "/{}/plants/{}{suffix}",
        urlencoding(data_type),
        urlencoding(plant)
    ))
}

pub async fn query_data_controls(
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::DataControlDto>, String> {
    post_query(data_control_url("/query"), req).await
}

/// 新增受控營運中心（detail）
pub async fn add_data_control_plant(
    req: &ttrs_shared::DataControlPlantInput,
) -> Result<ttrs_shared::DataControlDto, String> {
    send_json(
        reqwest::Method::POST,
        data_control_url("/plants"),
        Some(req),
    )
    .await
}

/// 刪除受控營運中心連同欄位條件（detail）
pub async fn remove_data_control_plant(
    data_type: &str,
    plant: &str,
    version: i32,
) -> Result<ttrs_shared::DataControlDto, String> {
    send_json::<_, ()>(
        reqwest::Method::DELETE,
        format!(
            "{}?version={version}",
            data_control_plant_url(data_type, plant, "")
        ),
        None,
    )
    .await
}

/// 把欄位條件複製到另一個營運中心（reproduce）
pub async fn copy_data_control_rules(
    data_type: &str,
    plant: &str,
    to_plant: &str,
) -> Result<ttrs_shared::DataControlDto, String> {
    send_json(
        reqwest::Method::POST,
        data_control_plant_url(data_type, plant, "/copy"),
        Some(&ttrs_shared::DataControlCopyInput {
            to_plant: to_plant.to_owned(),
        }),
    )
    .await
}

/// 新增（edit＝false）或修改欄位條件（condition_detail）
pub async fn save_data_control_rule(
    data_type: &str,
    plant: &str,
    edit: bool,
    req: &ttrs_shared::DataControlRuleInput,
) -> Result<ttrs_shared::DataControlDto, String> {
    let method = if edit {
        reqwest::Method::PUT
    } else {
        reqwest::Method::POST
    };
    send_json(
        method,
        data_control_plant_url(data_type, plant, "/rules"),
        Some(req),
    )
    .await
}

/// 刪除欄位條件（condition_detail）
pub async fn remove_data_control_rule(
    data_type: &str,
    plant: &str,
    field: &str,
    version: i32,
) -> Result<ttrs_shared::DataControlDto, String> {
    send_json::<_, ()>(
        reqwest::Method::DELETE,
        format!(
            "{}?version={version}",
            data_control_plant_url(data_type, plant, &format!("/rules/{}", urlencoding(field)))
        ),
        None,
    )
    .await
}

// ----- Group Center Maintenance 集團中心維護（TipTop aooi600；共用） -----

fn group_center_url(code: &str, suffix: &str) -> String {
    if code.is_empty() {
        url(&format!("/api/group-centers{suffix}"))
    } else {
        url(&format!("/api/group-centers/{}{suffix}", urlencoding(code)))
    }
}

pub async fn query_group_centers(
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::GroupCenterDto>, String> {
    post_query(group_center_url("", "/query"), req).await
}

pub async fn create_group_center(
    req: &ttrs_shared::GroupCenterInput,
) -> Result<ttrs_shared::GroupCenterDto, String> {
    send_json(reqwest::Method::POST, group_center_url("", ""), Some(req)).await
}

pub async fn update_group_center(
    code: &str,
    req: &ttrs_shared::GroupCenterInput,
) -> Result<ttrs_shared::GroupCenterDto, String> {
    send_json(reqwest::Method::PUT, group_center_url(code, ""), Some(req)).await
}

pub async fn copy_group_center(
    code: &str,
    new_code: &str,
) -> Result<ttrs_shared::GroupCenterDto, String> {
    send_json(
        reqwest::Method::POST,
        group_center_url(code, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code })),
    )
    .await
}

/// 停用／啟用（activate、deactivate）、確認／取消確認（confirm、unconfirm）
pub async fn set_group_center_state(
    code: &str,
    state: &str,
    version: i32,
) -> Result<ttrs_shared::GroupCenterDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            group_center_url(code, &format!("/{state}"))
        ),
        None,
    )
    .await
}

pub async fn delete_group_center(code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{}?version={version}", group_center_url(code, "")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 資料類別設定：新增一列（data_type_setting）
pub async fn add_group_center_data_type(
    code: &str,
    req: &ttrs_shared::DataCenterTypeInput,
) -> Result<ttrs_shared::GroupCenterDto, String> {
    send_json(
        reqwest::Method::POST,
        group_center_url(code, "/data-types"),
        Some(req),
    )
    .await
}

/// 資料類別設定：刪除一列（data_type_setting）
pub async fn remove_group_center_data_type(
    code: &str,
    data_type: &str,
    plant: &str,
    version: i32,
) -> Result<ttrs_shared::GroupCenterDto, String> {
    send_json::<_, ()>(
        reqwest::Method::DELETE,
        format!(
            "{}?version={version}",
            group_center_url(
                code,
                &format!(
                    "/data-types/{}/{}",
                    urlencoding(data_type),
                    urlencoding(plant)
                )
            )
        ),
        None,
    )
    .await
}

// ----- Encoding Segment Maintenance 編碼獨立段維護（TipTop aooi403；共用） -----

fn encoding_segment_url(code: &str, suffix: &str) -> String {
    if code.is_empty() {
        url(&format!("/api/encoding-segments{suffix}"))
    } else {
        url(&format!(
            "/api/encoding-segments/{}{suffix}",
            urlencoding(code)
        ))
    }
}

pub async fn query_encoding_segments(
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::EncodingSegmentDto>, String> {
    post_query(encoding_segment_url("", "/query"), req).await
}

pub async fn create_encoding_segment(
    req: &ttrs_shared::EncodingSegmentInput,
) -> Result<ttrs_shared::EncodingSegmentDto, String> {
    send_json(
        reqwest::Method::POST,
        encoding_segment_url("", ""),
        Some(req),
    )
    .await
}

pub async fn update_encoding_segment(
    code: &str,
    req: &ttrs_shared::EncodingSegmentInput,
) -> Result<ttrs_shared::EncodingSegmentDto, String> {
    send_json(
        reqwest::Method::PUT,
        encoding_segment_url(code, ""),
        Some(req),
    )
    .await
}

pub async fn copy_encoding_segment(
    code: &str,
    new_code: &str,
) -> Result<ttrs_shared::EncodingSegmentDto, String> {
    send_json(
        reqwest::Method::POST,
        encoding_segment_url(code, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code })),
    )
    .await
}

/// 停用／啟用（activate、deactivate）、確認／取消確認（confirm、unconfirm）
pub async fn set_encoding_segment_state(
    code: &str,
    state: &str,
    version: i32,
) -> Result<ttrs_shared::EncodingSegmentDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            encoding_segment_url(code, &format!("/{state}"))
        ),
        None,
    )
    .await
}

pub async fn delete_encoding_segment(code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            encoding_segment_url(code, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 單身：新增一列（line 為 None）或修改一列
pub async fn save_encoding_segment_value(
    code: &str,
    line: Option<i32>,
    req: &ttrs_shared::EncodingSegmentValueInput,
) -> Result<ttrs_shared::EncodingSegmentDto, String> {
    match line {
        None => {
            send_json(
                reqwest::Method::POST,
                encoding_segment_url(code, "/values"),
                Some(req),
            )
            .await
        }
        Some(l) => {
            send_json(
                reqwest::Method::PUT,
                encoding_segment_url(code, &format!("/values/{l}")),
                Some(req),
            )
            .await
        }
    }
}

pub async fn remove_encoding_segment_value(
    code: &str,
    line: i32,
    version: i32,
) -> Result<ttrs_shared::EncodingSegmentDto, String> {
    send_json::<_, ()>(
        reqwest::Method::DELETE,
        format!(
            "{}?version={version}",
            encoding_segment_url(code, &format!("/values/{line}"))
        ),
        None,
    )
    .await
}

// ----- Product Classification 產品分類維護（TipTop axmi110；工廠層） -----

pub async fn create_product_class(
    plant: &str,
    req: &ttrs_shared::ProductClassInput,
) -> Result<ttrs_shared::ProductClassDto, String> {
    send_json(
        reqwest::Method::POST,
        plant_part_url(plant, "product-classes", "", ""),
        Some(req),
    )
    .await
}

pub async fn update_product_class(
    plant: &str,
    code: &str,
    req: &ttrs_shared::ProductClassInput,
) -> Result<ttrs_shared::ProductClassDto, String> {
    send_json(
        reqwest::Method::PUT,
        plant_part_url(plant, "product-classes", code, ""),
        Some(req),
    )
    .await
}

pub async fn set_product_class_active(
    plant: &str,
    code: &str,
    active: bool,
    version: i32,
) -> Result<ttrs_shared::ProductClassDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            plant_part_url(plant, "product-classes", code, suffix)
        ),
        None,
    )
    .await
}

pub async fn delete_product_class(plant: &str, code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            plant_part_url(plant, "product-classes", code, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn product_class_history(
    plant: &str,
    code: &str,
) -> Result<Vec<ttrs_shared::ChangeDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(format!(
        "{}?lang={lang}",
        plant_part_url(plant, "product-classes", code, "/history")
    ))
    .await
}

// ----- Warehouse Maintenance 倉庫儲位維護（TipTop aimi201、aimi200；工廠層） -----

pub async fn query_warehouses(
    plant: &str,
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::WarehouseDto>, String> {
    post_query(plant_part_url(plant, "warehouses", "", "/query"), req).await
}

pub async fn create_warehouse(
    plant: &str,
    req: &ttrs_shared::WarehouseInput,
) -> Result<ttrs_shared::WarehouseDto, String> {
    send_json(
        reqwest::Method::POST,
        plant_part_url(plant, "warehouses", "", ""),
        Some(req),
    )
    .await
}

pub async fn update_warehouse(
    plant: &str,
    code: &str,
    req: &ttrs_shared::WarehouseInput,
) -> Result<ttrs_shared::WarehouseDto, String> {
    send_json(
        reqwest::Method::PUT,
        plant_part_url(plant, "warehouses", code, ""),
        Some(req),
    )
    .await
}

pub async fn copy_warehouse(
    plant: &str,
    code: &str,
    new_code: &str,
) -> Result<ttrs_shared::WarehouseDto, String> {
    send_json(
        reqwest::Method::POST,
        plant_part_url(plant, "warehouses", code, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code })),
    )
    .await
}

pub async fn delete_warehouse(plant: &str, code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            plant_part_url(plant, "warehouses", code, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 停用／啟用（activate、deactivate）、確認／取消確認（confirm、unconfirm）
pub async fn set_warehouse_state(
    plant: &str,
    code: &str,
    state: &str,
    version: i32,
) -> Result<ttrs_shared::WarehouseDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            plant_part_url(plant, "warehouses", code, &format!("/{state}"))
        ),
        None,
    )
    .await
}

pub async fn warehouse_history(
    plant: &str,
    code: &str,
) -> Result<Vec<ttrs_shared::ChangeDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(format!(
        "{}?lang={lang}",
        plant_part_url(plant, "warehouses", code, "/history")
    ))
    .await
}

/// 新增（edit＝false）或修改儲位
pub async fn save_warehouse_location(
    plant: &str,
    code: &str,
    edit: bool,
    req: &ttrs_shared::WarehouseLocationInput,
) -> Result<ttrs_shared::WarehouseDto, String> {
    let method = if edit {
        reqwest::Method::PUT
    } else {
        reqwest::Method::POST
    };
    send_json(
        method,
        plant_part_url(plant, "warehouses", code, "/locations"),
        Some(req),
    )
    .await
}

pub async fn remove_warehouse_location(
    plant: &str,
    code: &str,
    location: &str,
    version: i32,
) -> Result<ttrs_shared::WarehouseDto, String> {
    send_json::<_, ()>(
        reqwest::Method::DELETE,
        format!(
            "{}?version={version}",
            plant_part_url(
                plant,
                "warehouses",
                code,
                &format!("/locations/{}", urlencoding(location))
            )
        ),
        None,
    )
    .await
}

// ----- Item Group 料件分群維護（TipTop aimi110；工廠層） -----

/// QBE 查詢（item_group_query.feature）
pub async fn query_item_groups(
    plant: &str,
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::ItemGroupDto>, String> {
    post_query(plant_part_url(plant, "item-groups", "query", ""), req).await
}

pub async fn create_item_group(
    plant: &str,
    req: &ttrs_shared::ItemGroupInput,
) -> Result<ttrs_shared::ItemGroupDto, String> {
    send_json(
        reqwest::Method::POST,
        plant_part_url(plant, "item-groups", "", ""),
        Some(req),
    )
    .await
}

pub async fn update_item_group(
    plant: &str,
    code: &str,
    req: &ttrs_shared::ItemGroupInput,
) -> Result<ttrs_shared::ItemGroupDto, String> {
    send_json(
        reqwest::Method::PUT,
        plant_part_url(plant, "item-groups", code, ""),
        Some(req),
    )
    .await
}

pub async fn copy_item_group(
    plant: &str,
    code: &str,
    new_code: &str,
) -> Result<ttrs_shared::ItemGroupDto, String> {
    send_json(
        reqwest::Method::POST,
        plant_part_url(plant, "item-groups", code, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code })),
    )
    .await
}

pub async fn set_item_group_active(
    plant: &str,
    code: &str,
    active: bool,
    version: i32,
) -> Result<ttrs_shared::ItemGroupDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            plant_part_url(plant, "item-groups", code, suffix)
        ),
        None,
    )
    .await
}

/// 確認／取消確認（ADR 0013 SA9）
pub async fn confirm_item_group(
    plant: &str,
    code: &str,
    confirm: bool,
    version: i32,
) -> Result<ttrs_shared::ItemGroupDto, String> {
    let suffix = if confirm { "/confirm" } else { "/unconfirm" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            plant_part_url(plant, "item-groups", code, suffix)
        ),
        None,
    )
    .await
}

pub async fn delete_item_group(plant: &str, code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            plant_part_url(plant, "item-groups", code, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn item_group_history(
    plant: &str,
    code: &str,
) -> Result<Vec<ttrs_shared::ChangeDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(format!(
        "{}?lang={lang}",
        plant_part_url(plant, "item-groups", code, "/history")
    ))
    .await
}

// ----- 基本代碼：區域別、國別、地區、交運方式、報表抬頭（/api/basic-codes/{kind}） -----

use ttrs_shared::{BasicCodeDto, BasicCodeInput, BasicCodeKindDto};

/// 基本代碼的 API 路徑前綴（列印、匯出也用它）
pub fn basic_code_path(kind: BasicCodeKindDto) -> String {
    format!("/api/basic-codes/{}", kind.slug())
}

fn basic_code_url(kind: BasicCodeKindDto, code: &str, suffix: &str) -> String {
    let base = basic_code_path(kind);
    if code.is_empty() {
        url(&format!("{base}{suffix}"))
    } else {
        url(&format!("{base}/{}{suffix}", urlencoding(code)))
    }
}

/// QBE 查詢（ADR 0013 SA1）；名稱依 req.lang 顯示
pub async fn query_basic_codes(
    kind: BasicCodeKindDto,
    req: &QueryRequest,
) -> Result<Page<BasicCodeDto>, String> {
    post_query(basic_code_url(kind, "", "/query"), req).await
}

/// 全部代碼（隸屬欄位的下拉選單）
pub async fn list_basic_codes(
    kind: BasicCodeKindDto,
    lang: &str,
) -> Result<Vec<BasicCodeDto>, String> {
    get_json(format!("{}?lang={lang}", basic_code_url(kind, "", ""))).await
}

pub async fn create_basic_code(
    kind: BasicCodeKindDto,
    req: &BasicCodeInput,
) -> Result<BasicCodeDto, String> {
    send_json(
        reqwest::Method::POST,
        basic_code_url(kind, "", ""),
        Some(req),
    )
    .await
}

pub async fn update_basic_code(
    kind: BasicCodeKindDto,
    code: &str,
    req: &BasicCodeInput,
) -> Result<BasicCodeDto, String> {
    send_json(
        reqwest::Method::PUT,
        basic_code_url(kind, code, ""),
        Some(req),
    )
    .await
}

/// 停用 deactivate、啟用 activate、確認 confirm、取消確認 unconfirm
pub async fn set_basic_code_state(
    kind: BasicCodeKindDto,
    code: &str,
    state: &str,
    version: i32,
) -> Result<BasicCodeDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            basic_code_url(kind, code, &format!("/{state}"))
        ),
        None,
    )
    .await
}

/// 複製（ADR 0013 SA5）
pub async fn copy_basic_code(
    kind: BasicCodeKindDto,
    code: &str,
    new_code: &str,
) -> Result<BasicCodeDto, String> {
    send_json(
        reqwest::Method::POST,
        basic_code_url(kind, code, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code })),
    )
    .await
}

pub async fn delete_basic_code(
    kind: BasicCodeKindDto,
    code: &str,
    version: i32,
) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            basic_code_url(kind, code, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

// ----- 幣別維護（aooi050；/api/currencies） -----

fn currency_url(code: &str, suffix: &str) -> String {
    if code.is_empty() {
        url(&format!("/api/currencies{suffix}"))
    } else {
        url(&format!("/api/currencies/{}{suffix}", urlencoding(code)))
    }
}

/// QBE 查詢（ADR 0013 SA1）
pub async fn query_currencies(
    req: &QueryRequest,
) -> Result<Page<ttrs_shared::CurrencyDto>, String> {
    post_query(currency_url("", "/query"), req).await
}

/// 複製（ADR 0013 SA5）
pub async fn copy_currency(code: &str, new_code: &str) -> Result<ttrs_shared::CurrencyDto, String> {
    send_json(
        reqwest::Method::POST,
        currency_url(code, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code })),
    )
    .await
}

/// 確認或取消確認（ADR 0013 SA9）
pub async fn confirm_currency(
    code: &str,
    confirm: bool,
    version: i32,
) -> Result<ttrs_shared::CurrencyDto, String> {
    let suffix = if confirm { "/confirm" } else { "/unconfirm" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!("{}?version={version}", currency_url(code, suffix)),
        None,
    )
    .await
}

pub async fn create_currency(
    req: &ttrs_shared::CurrencyInput,
) -> Result<ttrs_shared::CurrencyDto, String> {
    send_json(reqwest::Method::POST, currency_url("", ""), Some(req)).await
}

pub async fn update_currency(
    code: &str,
    req: &ttrs_shared::CurrencyInput,
) -> Result<ttrs_shared::CurrencyDto, String> {
    send_json(reqwest::Method::PUT, currency_url(code, ""), Some(req)).await
}

pub async fn set_currency_active(
    code: &str,
    active: bool,
    version: i32,
) -> Result<ttrs_shared::CurrencyDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!("{}?version={version}", currency_url(code, suffix)),
        None,
    )
    .await
}

pub async fn delete_currency(code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{}?version={version}", currency_url(code, "")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

// ----- 稅別維護（aooi150；/api/tax-types/{稅別碼}/{進銷項}） -----

use ttrs_shared::{TaxTypeDto, TaxTypeInput};

fn tax_type_url(code: &str, direction: &str, suffix: &str) -> String {
    if code.is_empty() {
        url(&format!("/api/tax-types{suffix}"))
    } else {
        url(&format!(
            "/api/tax-types/{}/{}{suffix}",
            urlencoding(code),
            urlencoding(direction)
        ))
    }
}

/// QBE 查詢（ADR 0013 SA1）
pub async fn query_tax_types(req: &QueryRequest) -> Result<Page<TaxTypeDto>, String> {
    post_query(tax_type_url("", "", "/query"), req).await
}

pub async fn create_tax_type(req: &TaxTypeInput) -> Result<TaxTypeDto, String> {
    send_json(reqwest::Method::POST, tax_type_url("", "", ""), Some(req)).await
}

/// 修改；`key` 為畫面選取的「稅別碼 進銷項」（TaxTypeDto::key）
pub async fn update_tax_type(key: &str, req: &TaxTypeInput) -> Result<TaxTypeDto, String> {
    let (code, direction) = key.split_once(' ').unwrap_or((key, ""));
    send_json(
        reqwest::Method::PUT,
        tax_type_url(code, direction, ""),
        Some(req),
    )
    .await
}

/// 複製（ADR 0013 SA5）：新稅別碼與進銷項
pub async fn copy_tax_type(
    code: &str,
    direction: &str,
    new_code: &str,
    new_direction: &str,
) -> Result<TaxTypeDto, String> {
    send_json(
        reqwest::Method::POST,
        tax_type_url(code, direction, "/copy"),
        Some(&serde_json::json!({ "new_code": new_code, "new_direction": new_direction })),
    )
    .await
}

async fn tax_type_state(
    code: &str,
    direction: &str,
    state: &str,
    version: i32,
) -> Result<TaxTypeDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            tax_type_url(code, direction, &format!("/{state}"))
        ),
        None,
    )
    .await
}

/// 確認或取消確認（ADR 0013 SA9）
pub async fn confirm_tax_type(
    code: &str,
    direction: &str,
    confirm: bool,
    version: i32,
) -> Result<TaxTypeDto, String> {
    let state = if confirm { "confirm" } else { "unconfirm" };
    tax_type_state(code, direction, state, version).await
}

pub async fn set_tax_type_active(
    code: &str,
    direction: &str,
    active: bool,
    version: i32,
) -> Result<TaxTypeDto, String> {
    let state = if active { "activate" } else { "deactivate" };
    tax_type_state(code, direction, state, version).await
}

pub async fn delete_tax_type(code: &str, direction: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            tax_type_url(code, direction, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

// ----- 專案維護（aooi200；工廠層 /api/plants/{plant}/projects） -----

use ttrs_shared::{ProjectDto, ProjectInput};

/// QBE 查詢（ADR 0013 SA1）
pub async fn query_projects(plant: &str, req: &QueryRequest) -> Result<Page<ProjectDto>, String> {
    post_query(plant_part_url(plant, "projects", "query", ""), req).await
}

pub async fn create_project(plant: &str, req: &ProjectInput) -> Result<ProjectDto, String> {
    send_json(
        reqwest::Method::POST,
        plant_part_url(plant, "projects", "", ""),
        Some(req),
    )
    .await
}

pub async fn update_project(
    plant: &str,
    code: &str,
    req: &ProjectInput,
) -> Result<ProjectDto, String> {
    send_json(
        reqwest::Method::PUT,
        plant_part_url(plant, "projects", code, ""),
        Some(req),
    )
    .await
}

/// 複製 reproduce
pub async fn copy_project(plant: &str, code: &str, new_code: &str) -> Result<ProjectDto, String> {
    copy_plant_part(plant, "projects", code, new_code).await
}

/// 確認或取消確認（ADR 0013 SA9）
pub async fn confirm_project(
    plant: &str,
    code: &str,
    confirm: bool,
    version: i32,
) -> Result<ProjectDto, String> {
    confirm_plant_part(plant, "projects", code, confirm, version).await
}

/// 失效切換 invalid
pub async fn set_project_active(
    plant: &str,
    code: &str,
    active: bool,
    version: i32,
) -> Result<ProjectDto, String> {
    let suffix = if active { "/activate" } else { "/deactivate" };
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            plant_part_url(plant, "projects", code, suffix)
        ),
        None,
    )
    .await
}

pub async fn delete_project(plant: &str, code: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            plant_part_url(plant, "projects", code, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 異動記錄（ADR 0012 §8）
pub async fn project_history(
    plant: &str,
    code: &str,
) -> Result<Vec<ttrs_shared::ChangeDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(format!(
        "{}?lang={lang}",
        plant_part_url(plant, "projects", code, "/history")
    ))
    .await
}

// ----- 匯率（aooi060 /api/monthly-rates、aooi070 /api/daily-rates、aoop010 /api/daily-rate-fill） -----

use ttrs_shared::{
    DailyRateDto, DailyRateFillLine, DailyRateInput, GeneralParametersDto, GeneralParametersInput,
    MonthlyRateDto, MonthlyRateInput, RateBatchResult, RateImportRequest, RateImportResult,
    RateSettingsDto, RatesDto,
};

fn rate_url(part: &str, currency: &str, key: &str, suffix: &str) -> String {
    if currency.is_empty() {
        url(&format!("/api/{part}{suffix}"))
    } else {
        url(&format!(
            "/api/{part}/{}/{}{suffix}",
            urlencoding(currency),
            urlencoding(key)
        ))
    }
}

pub async fn query_monthly_rates(req: &QueryRequest) -> Result<Page<MonthlyRateDto>, String> {
    post_query(rate_url("monthly-rates", "", "", "/query"), req).await
}

pub async fn create_monthly_rate(req: &MonthlyRateInput) -> Result<MonthlyRateDto, String> {
    send_json(
        reqwest::Method::POST,
        rate_url("monthly-rates", "", "", ""),
        Some(req),
    )
    .await
}

pub async fn update_monthly_rate(
    currency: &str,
    period: &str,
    req: &MonthlyRateInput,
) -> Result<MonthlyRateDto, String> {
    send_json(
        reqwest::Method::PUT,
        rate_url("monthly-rates", currency, period, ""),
        Some(req),
    )
    .await
}

/// 停用／啟用（activate、deactivate）、確認／取消確認（confirm、unconfirm）
pub async fn set_monthly_rate_state(
    currency: &str,
    period: &str,
    state: &str,
    version: i32,
) -> Result<MonthlyRateDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            rate_url("monthly-rates", currency, period, &format!("/{state}"))
        ),
        None,
    )
    .await
}

pub async fn copy_monthly_rate(
    currency: &str,
    period: &str,
    new_currency: &str,
    new_period: &str,
) -> Result<MonthlyRateDto, String> {
    send_json(
        reqwest::Method::POST,
        rate_url("monthly-rates", currency, period, "/copy"),
        Some(&serde_json::json!({ "currency_code": new_currency, "period": new_period })),
    )
    .await
}

pub async fn delete_monthly_rate(currency: &str, period: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            rate_url("monthly-rates", currency, period, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 月底重評價（MR7）
pub async fn revaluate_rates(date: &str) -> Result<RateBatchResult, String> {
    send_json(
        reqwest::Method::POST,
        rate_url("monthly-rates", "", "", "/revaluate"),
        Some(&serde_json::json!({ "date": date })),
    )
    .await
}

pub async fn query_daily_rates(req: &QueryRequest) -> Result<Page<DailyRateDto>, String> {
    post_query(rate_url("daily-rates", "", "", "/query"), req).await
}

pub async fn create_daily_rate(req: &DailyRateInput) -> Result<DailyRateDto, String> {
    send_json(
        reqwest::Method::POST,
        rate_url("daily-rates", "", "", ""),
        Some(req),
    )
    .await
}

pub async fn update_daily_rate(
    currency: &str,
    date: &str,
    req: &DailyRateInput,
) -> Result<DailyRateDto, String> {
    send_json(
        reqwest::Method::PUT,
        rate_url("daily-rates", currency, date, ""),
        Some(req),
    )
    .await
}

pub async fn set_daily_rate_state(
    currency: &str,
    date: &str,
    state: &str,
    version: i32,
) -> Result<DailyRateDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            rate_url("daily-rates", currency, date, &format!("/{state}"))
        ),
        None,
    )
    .await
}

pub async fn copy_daily_rate(
    currency: &str,
    date: &str,
    new_currency: &str,
    new_date: &str,
) -> Result<DailyRateDto, String> {
    send_json(
        reqwest::Method::POST,
        rate_url("daily-rates", currency, date, "/copy"),
        Some(&serde_json::json!({ "currency_code": new_currency, "rate_date": new_date })),
    )
    .await
}

pub async fn delete_daily_rate(currency: &str, date: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            rate_url("daily-rates", currency, date, "")
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 新增時的預設匯率（DR4）
pub async fn daily_rate_defaults(currency: &str) -> Result<RatesDto, String> {
    get_json(url(&format!(
        "/api/daily-rates/defaults/{}",
        urlencoding(currency)
    )))
    .await
}

/// 統一匯率（DR7）
pub async fn unify_rates(date: &str) -> Result<RateBatchResult, String> {
    send_json(
        reqwest::Method::POST,
        rate_url("daily-rates", "", "", "/unify"),
        Some(&serde_json::json!({ "date": date })),
    )
    .await
}

/// 匯率自動匯入（aooi070 ★ auto_import）：台銀牌告，`kind` 為 cash 或 spot；`csv` 有值時用上傳的檔案（IM1 備援）。
/// 今天已有每日匯率而未同意覆寫時，錯誤代碼為 EXCHANGE_RATE_IMPORT_OVERWRITE_REQUIRED
pub async fn import_bank_rates(
    kind: &str,
    overwrite: bool,
    csv: Option<String>,
) -> Result<RateImportResult, ApiFailure> {
    post_with_code(
        url("/api/daily-rates/import/bank"),
        &RateImportRequest {
            kind: Some(kind.to_owned()),
            overwrite,
            csv,
        },
    )
    .await
}

/// 匯率自動匯入：關務署每旬報關匯率（IM4）
pub async fn import_customs_rates() -> Result<RateImportResult, ApiFailure> {
    post_with_code(
        url("/api/daily-rates/import/customs"),
        &serde_json::json!({}),
    )
    .await
}

async fn post_with_code<T: serde::de::DeserializeOwned, B: serde::Serialize + ?Sized>(
    u: String,
    body: &B,
) -> Result<T, ApiFailure> {
    let res = reqwest::Client::new()
        .post(u)
        .json(body)
        .send()
        .await
        .map_err(network)?;
    if res.status().is_success() {
        res.json().await.map_err(network)
    } else {
        Err(failure(res).await)
    }
}

/// 匯率維護畫面的設定與提示（general_parameters.feature GP7）
pub async fn rate_settings(monthly: bool) -> Result<RateSettingsDto, String> {
    get_json(url(if monthly {
        "/api/monthly-rates/settings"
    } else {
        "/api/daily-rates/settings"
    }))
    .await
}

// ----- 整體管理系統參數（aoos010 /api/general-parameters） -----

pub async fn get_general_parameters() -> Result<GeneralParametersDto, String> {
    get_json(url("/api/general-parameters")).await
}

pub async fn update_general_parameters(
    input: &GeneralParametersInput,
) -> Result<GeneralParametersDto, String> {
    send_json(
        reqwest::Method::PUT,
        url("/api/general-parameters"),
        Some(input),
    )
    .await
}

/// 每日匯率複製（aoop010，CP1）
pub async fn fill_daily_rates(until: &str) -> Result<Vec<DailyRateFillLine>, String> {
    send_json(
        reqwest::Method::POST,
        url("/api/daily-rate-fill"),
        Some(&serde_json::json!({ "until": until })),
    )
    .await
}

/// 每日匯率複製結果的列印、匯出 Excel：結果由畫面帶回
pub async fn fill_report(
    lines: &[DailyRateFillLine],
    format: ReportFormat,
    lang: &str,
    tab: ReportTab,
) -> Result<(), String> {
    let u = url(&format!("/api/daily-rate-fill/report.{}", format.ext()));
    let file = format!("daily-rate-fill.{}", format.ext());
    let res = reqwest::Client::new()
        .post(u)
        .query(&[("lang", lang)])
        .json(lines)
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
    show_report(&bytes, format, &file, tab)
}

// ----- 會計期間（aoos020 /api/accounting-years、/api/accounting-days） -----

use ttrs_shared::{AccountingDayDto, AccountingPeriodLine, AccountingYearDto, AccountingYearInput};

pub async fn query_accounting_years(req: &QueryRequest) -> Result<Page<AccountingYearDto>, String> {
    post_query(url("/api/accounting-years/query"), req).await
}

/// 新增時的預設各期（PD5）
pub async fn accounting_year_defaults(
    year: i32,
    start: &str,
) -> Result<Vec<AccountingPeriodLine>, String> {
    get_json(url(&format!(
        "/api/accounting-years/defaults?year={year}&start={}",
        urlencoding(start)
    )))
    .await
}

pub async fn create_accounting_year(
    input: &AccountingYearInput,
) -> Result<AccountingYearDto, String> {
    send_json(
        reqwest::Method::POST,
        url("/api/accounting-years"),
        Some(input),
    )
    .await
}

pub async fn update_accounting_year(
    year: i32,
    input: &AccountingYearInput,
) -> Result<AccountingYearDto, String> {
    send_json(
        reqwest::Method::PUT,
        url(&format!("/api/accounting-years/{year}")),
        Some(input),
    )
    .await
}

pub async fn delete_accounting_year(year: i32, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(url(&format!(
            "/api/accounting-years/{year}?version={version}"
        )))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 複製為另一個年度（PD9）
pub async fn copy_accounting_year(year: i32, to: i32) -> Result<AccountingYearDto, String> {
    send_json(
        reqwest::Method::POST,
        url(&format!("/api/accounting-years/{year}/copy")),
        Some(&serde_json::json!({ "to": to })),
    )
    .await
}

/// 確認 confirm／取消確認 unconfirm
pub async fn set_accounting_year_state(
    year: i32,
    state: &str,
    version: i32,
) -> Result<AccountingYearDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        url(&format!(
            "/api/accounting-years/{year}/{state}?version={version}"
        )),
        None,
    )
    .await
}

/// 週期期間查詢（aooq010，PD3）
pub async fn accounting_days(from: &str, to: &str) -> Result<Vec<AccountingDayDto>, String> {
    get_json(url(&format!(
        "/api/accounting-days?from={}&to={}",
        urlencoding(from),
        urlencoding(to)
    )))
    .await
}

// ----- 單位換算（aooi102 /api/unit-conversions、aooi103 /api/plants/{plant}/items/{no}/unit-conversions） -----

use ttrs_shared::{ItemUnitConversionDto, UnitConversionDto, UnitConversionInput};

fn conv_url(from: &str, to: &str, suffix: &str) -> String {
    url(&format!(
        "/api/unit-conversions/{}/{}{suffix}",
        urlencoding(from),
        urlencoding(to)
    ))
}

pub async fn query_unit_conversions(req: &QueryRequest) -> Result<Page<UnitConversionDto>, String> {
    post_query(url("/api/unit-conversions/query"), req).await
}

pub async fn create_unit_conversion(
    input: &UnitConversionInput,
) -> Result<UnitConversionDto, String> {
    send_json(
        reqwest::Method::POST,
        url("/api/unit-conversions"),
        Some(input),
    )
    .await
}

pub async fn update_unit_conversion(
    from: &str,
    to: &str,
    input: &UnitConversionInput,
) -> Result<UnitConversionDto, String> {
    send_json(reqwest::Method::PUT, conv_url(from, to, ""), Some(input)).await
}

pub async fn delete_unit_conversion(from: &str, to: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{}?version={version}", conv_url(from, to, "")))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 停用 deactivate／啟用 activate／確認 confirm／取消確認 unconfirm
pub async fn set_unit_conversion_state(
    from: &str,
    to: &str,
    state: &str,
    version: i32,
) -> Result<UnitConversionDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            conv_url(from, to, &format!("/{state}"))
        ),
        None,
    )
    .await
}

pub async fn copy_unit_conversion(
    from: &str,
    to: &str,
    new_from: &str,
    new_to: &str,
) -> Result<UnitConversionDto, String> {
    send_json(
        reqwest::Method::POST,
        conv_url(from, to, "/copy"),
        Some(&serde_json::json!({ "from_unit": new_from, "to_unit": new_to })),
    )
    .await
}

fn item_conv_url(plant: &str, item: &str, suffix: &str) -> String {
    url(&format!(
        "/api/plants/{}/items/{}/unit-conversions{suffix}",
        urlencoding(plant),
        urlencoding(item)
    ))
}

pub async fn list_item_unit_conversions(
    plant: &str,
    item: &str,
) -> Result<Vec<ItemUnitConversionDto>, String> {
    get_json(item_conv_url(plant, item, "")).await
}

pub async fn create_item_unit_conversion(
    plant: &str,
    item: &str,
    input: &UnitConversionInput,
) -> Result<ItemUnitConversionDto, String> {
    send_json(
        reqwest::Method::POST,
        item_conv_url(plant, item, ""),
        Some(input),
    )
    .await
}

pub async fn update_item_unit_conversion(
    plant: &str,
    item: &str,
    from: &str,
    to: &str,
    input: &UnitConversionInput,
) -> Result<ItemUnitConversionDto, String> {
    send_json(
        reqwest::Method::PUT,
        item_conv_url(
            plant,
            item,
            &format!("/{}/{}", urlencoding(from), urlencoding(to)),
        ),
        Some(input),
    )
    .await
}

pub async fn delete_item_unit_conversion(
    plant: &str,
    item: &str,
    from: &str,
    to: &str,
    version: i32,
) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!(
            "{}?version={version}",
            item_conv_url(
                plant,
                item,
                &format!("/{}/{}", urlencoding(from), urlencoding(to))
            )
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 停用 deactivate／啟用 activate
pub async fn set_item_unit_conversion_state(
    plant: &str,
    item: &str,
    from: &str,
    to: &str,
    state: &str,
    version: i32,
) -> Result<ItemUnitConversionDto, String> {
    send_json::<_, ()>(
        reqwest::Method::POST,
        format!(
            "{}?version={version}",
            item_conv_url(
                plant,
                item,
                &format!("/{}/{}/{state}", urlencoding(from), urlencoding(to))
            )
        ),
        None,
    )
    .await
}

pub async fn item_unit_conversion_history(
    plant: &str,
    item: &str,
) -> Result<Vec<ttrs_shared::ChangeDto>, String> {
    let lang = crate::i18n::current().code();
    get_json(format!(
        "{}?lang={lang}",
        item_conv_url(plant, item, "/history")
    ))
    .await
}

// ----- EPUB 轉 TXT／PDF（/api/epub-to-text；epub_to_text.feature、epub_to_pdf.feature） -----

/// 轉換完成的檔案：檔名、書名、作者、章節數與內容位元組
pub struct EpubFile {
    pub file_name: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub chapters: u32,
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

/// 百分比解碼（回應標頭的書名、作者）
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = if b[i] == b'%' && i + 2 < b.len() {
            std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        } else {
            None
        };
        if let Some(v) = hex {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 把轉換回應（txt 為 JSON、pdf 為二進位＋標頭）轉成 [`EpubFile`]
async fn epub_file(res: reqwest::Response, format: &str) -> Result<EpubFile, String> {
    if !res.status().is_success() {
        return Err(error_message(res).await);
    }
    if format == "pdf" {
        let header = |k: &str| {
            res.headers()
                .get(k)
                .and_then(|v| v.to_str().ok())
                .map(percent_decode)
        };
        let file_name = header("x-epub-file-name").unwrap_or_else(|| "book.pdf".to_owned());
        let title = header("x-epub-title").filter(|t| !t.is_empty());
        let author = header("x-epub-author").filter(|t| !t.is_empty());
        let chapters = header("x-epub-chapters")
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let bytes = res.bytes().await.map_err(|e| e.to_string())?.to_vec();
        return Ok(EpubFile {
            file_name,
            title,
            author,
            chapters,
            bytes,
            mime: "application/pdf",
        });
    }
    let r: ttrs_shared::EpubTextDto = res.json().await.map_err(|e| e.to_string())?;
    Ok(EpubFile {
        file_name: r.file_name,
        title: r.title,
        author: r.author,
        chapters: r.chapters,
        bytes: r.text.into_bytes(),
        mime: "text/plain;charset=utf-8",
    })
}

/// 上傳 EPUB 位元組，轉成 txt 或 pdf（可選簡體轉繁體）
pub async fn epub_convert(
    name: &str,
    bytes: Vec<u8>,
    format: &str,
    s2t: bool,
) -> Result<EpubFile, String> {
    let res = reqwest::Client::new()
        .post(url(&format!(
            "/api/epub-to-text?name={}&format={format}&s2t={s2t}",
            urlencoding(name)
        )))
        .header("content-type", "application/epub+zip")
        .body(bytes)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    epub_file(res, format).await
}

/// 讓瀏覽器下載檔案（文字為 UTF-8 不含 BOM，ET6）
#[cfg(target_arch = "wasm32")]
pub fn download_file(file: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(err)?;
    let href = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    let doc = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("no document")?;
    let a: web_sys::HtmlAnchorElement = doc
        .create_element("a")
        .map_err(err)?
        .dyn_into()
        .map_err(|_| "anchor".to_owned())?;
    a.set_href(&href);
    a.set_download(file);
    a.click();
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn download_file(_file: &str, _bytes: &[u8], _mime: &str) -> Result<(), String> {
    Ok(())
}

// ----- EPUB 英文翻譯（/api/epub-translate；epub_translate.feature） -----

/// 等待一段時間（輪詢翻譯進度用；非瀏覽器環境不等待）
#[cfg(target_arch = "wasm32")]
pub async fn sleep_ms(ms: u32) {
    gloo_timers::future::TimeoutFuture::new(ms).await;
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn sleep_ms(_ms: u32) {}

/// 提交翻譯工作（已勾選同意才會送出）：回工作編號
pub async fn epub_translate_submit(
    name: &str,
    bytes: Vec<u8>,
    format: &str,
    consent: bool,
) -> Result<String, String> {
    let res = reqwest::Client::new()
        .post(url(&format!(
            "/api/epub-translate?name={}&format={format}&consent={consent}",
            urlencoding(name)
        )))
        .header("content-type", "application/epub+zip")
        .body(bytes)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let r: ttrs_shared::TranslateSubmitDto = into_result(res).await?;
    Ok(r.job_id)
}

pub async fn epub_translate_status(id: &str) -> Result<ttrs_shared::TranslateJobDto, String> {
    get_json(url(&format!("/api/epub-translate/{}", urlencoding(id)))).await
}

/// 下載翻譯結果（txt 或 pdf）
pub async fn epub_translate_download(id: &str, format: &str) -> Result<EpubFile, String> {
    let res = reqwest::Client::new()
        .get(url(&format!(
            "/api/epub-translate/{}/download",
            urlencoding(id)
        )))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    epub_file(res, format).await
}

// ----- EPUB 多來源搜尋（/api/epub-search；epub_web_search.feature） -----

pub async fn epub_search(
    q: &str,
    open_only: bool,
    page: u32,
) -> Result<ttrs_shared::EpubSearchPage, String> {
    get_json(url(&format!(
        "/api/epub-search?q={}&open={open_only}&page={page}",
        urlencoding(q)
    )))
    .await
}

// ----- 我的 GitHub 書庫（/api/epub-search/library；epub_github_library.feature） -----

pub async fn epub_library() -> Result<Vec<ttrs_shared::EpubLibraryRepoDto>, String> {
    get_json(url("/api/epub-search/library")).await
}

pub async fn epub_library_add(repo: &str) -> Result<ttrs_shared::EpubLibraryRepoDto, String> {
    let res = reqwest::Client::new()
        .post(url("/api/epub-search/library"))
        .json(&ttrs_shared::EpubLibraryAddRequest {
            repo: repo.to_owned(),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn epub_library_remove(repo: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(url(&format!(
            "/api/epub-search/library?repo={}&version={version}",
            urlencoding(repo)
        )))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

// ----- 共用 GitHub 書庫（/api/epub-search/shared-library；epub_github_shared_library.feature） -----

pub async fn epub_shared_library() -> Result<Vec<ttrs_shared::EpubSharedRepoDto>, String> {
    get_json(url("/api/epub-search/shared-library")).await
}

pub async fn epub_shared_library_add(repo: &str) -> Result<ttrs_shared::EpubSharedRepoDto, String> {
    let res = reqwest::Client::new()
        .post(url("/api/epub-search/shared-library"))
        .json(&ttrs_shared::EpubLibraryAddRequest {
            repo: repo.to_owned(),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn epub_shared_library_remove(repo: &str, version: i32) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(url(&format!(
            "/api/epub-search/shared-library?repo={}&version={version}",
            urlencoding(repo)
        )))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

/// 下載原始 EPUB（不轉換）
pub async fn epub_search_download(source: &str, id: &str, title: &str) -> Result<EpubFile, String> {
    let res = reqwest::Client::new()
        .get(url(&format!(
            "/api/epub-search/download?source={}&id={}&title={}",
            urlencoding(source),
            urlencoding(id),
            urlencoding(title)
        )))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(error_message(res).await);
    }
    let name = res
        .headers()
        .get("x-epub-file-name")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .unwrap_or_else(|| "book.epub".to_owned());
    let bytes = res.bytes().await.map_err(|e| e.to_string())?.to_vec();
    Ok(EpubFile {
        file_name: name,
        title: None,
        author: None,
        chapters: 0,
        bytes,
        mime: "application/epub+zip",
    })
}

/// 轉換結果：直接取得檔案，或（英文翻譯）取得翻譯工作編號
pub enum Converted {
    File(EpubFile),
    Job(String),
}

/// 轉換搜尋結果中的一本；`translate` 時伺服器改為提交翻譯工作
pub async fn epub_search_convert(
    book: &ttrs_shared::EpubBookDto,
    format: &str,
    s2t: bool,
    translate: bool,
    consent: bool,
) -> Result<Converted, String> {
    let res = reqwest::Client::new()
        .post(url("/api/epub-search/convert"))
        .json(&ttrs_shared::EpubBookConvertRequest {
            source: book.source.clone(),
            id: book.id.clone(),
            title: Some(book.title.clone()),
            format: Some(format.to_owned()),
            s2t: Some(s2t),
            translate: Some(translate),
            consent: Some(consent),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status().as_u16() == 202 {
        let r: ttrs_shared::TranslateSubmitDto = into_result(res).await?;
        return Ok(Converted::Job(r.job_id));
    }
    epub_file(res, format).await.map(Converted::File)
}

// ----- 以網址下載 EPUB（/api/epub-url；epub_url_download.feature） -----

pub async fn epub_url_read(link: &str) -> Result<ttrs_shared::EpubUrlDto, String> {
    get_json(url(&format!("/api/epub-url?url={}", urlencoding(link)))).await
}

/// 下載網址的原始 EPUB（不轉換）
pub async fn epub_url_download(link: &str) -> Result<EpubFile, String> {
    let res = reqwest::Client::new()
        .get(url(&format!(
            "/api/epub-url/download?url={}",
            urlencoding(link)
        )))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(error_message(res).await);
    }
    let name = res
        .headers()
        .get("x-epub-file-name")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .unwrap_or_else(|| "book.epub".to_owned());
    let bytes = res.bytes().await.map_err(|e| e.to_string())?.to_vec();
    Ok(EpubFile {
        file_name: name,
        title: None,
        author: None,
        chapters: 0,
        bytes,
        mime: "application/epub+zip",
    })
}

/// 轉換網址的 EPUB；`translate` 時伺服器改為提交翻譯工作
pub async fn epub_url_convert(
    link: &str,
    format: &str,
    s2t: bool,
    translate: bool,
    consent: bool,
) -> Result<Converted, String> {
    let res = reqwest::Client::new()
        .post(url("/api/epub-url/convert"))
        .json(&ttrs_shared::EpubBookConvertRequest {
            source: "url".to_owned(),
            id: link.to_owned(),
            title: None,
            format: Some(format.to_owned()),
            s2t: Some(s2t),
            translate: Some(translate),
            consent: Some(consent),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status().as_u16() == 202 {
        let r: ttrs_shared::TranslateSubmitDto = into_result(res).await?;
        return Ok(Converted::Job(r.job_id));
    }
    epub_file(res, format).await.map(Converted::File)
}

// ----- GitHub EPUB 搜尋（/api/github-epub；github_epub_search.feature） -----

pub async fn github_epub_files(repo: &str) -> Result<ttrs_shared::GithubEpubListDto, String> {
    get_json(url(&format!(
        "/api/github-epub/files?repo={}",
        urlencoding(repo)
    )))
    .await
}

pub async fn github_epub_convert(
    repo: &str,
    path: &str,
    format: &str,
    s2t: bool,
) -> Result<EpubFile, String> {
    let res = reqwest::Client::new()
        .post(url("/api/github-epub/convert"))
        .json(&ttrs_shared::GithubEpubConvertRequest {
            repo: repo.to_owned(),
            path: path.to_owned(),
            format: Some(format.to_owned()),
            s2t: Some(s2t),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    epub_file(res, format).await
}
