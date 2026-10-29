//! 後端 API 呼叫。Web 與桌面版共用。
//!
//! API 位址：編譯時設定 `TTRS_API_URL` 則一律使用；否則 Web 版走同源 `/api`
//! （開發由 dx proxy、正式由 nginx 轉發），桌面/行動版預設連本機 API。

use ttrs_shared::{
    ApiError, CreateItemRequest, GrantRequest, ItemDto, NewProgramNoRequest, ProgramDto,
    ProgramInput,
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

async fn error_message(res: reqwest::Response) -> String {
    let status = res.status();
    match res.json::<ApiError>().await {
        Ok(e) => e.message,
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

// ----- p_zz 程式登錄 -----

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

pub async fn list_programs() -> Result<Vec<ProgramDto>, String> {
    let res = reqwest::Client::new()
        .get(url("/api/programs"))
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

pub async fn update_program(no: &str, req: &ProgramInput) -> Result<ProgramDto, String> {
    let res = reqwest::Client::new()
        .put(program_url(no, ""))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn delete_program(no: &str) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(program_url(no, ""))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    no_content(res).await
}

pub async fn rename_program(no: &str, new_no: &str) -> Result<ProgramDto, String> {
    let res = reqwest::Client::new()
        .post(program_url(no, "/rename"))
        .json(&NewProgramNoRequest {
            new_no: new_no.to_owned(),
        })
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
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
