//! 後端 API 呼叫。Web 與桌面版共用；位址可於編譯時以 TTRS_API_URL 覆寫。

use ttrs_shared::{ApiError, CreateItemRequest, ItemDto};

const API_URL: &str = match option_env!("TTRS_API_URL") {
    Some(url) => url,
    None => "http://127.0.0.1:3000",
};

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
        .get(format!("{API_URL}/api/items"))
        .query(&[("plant", plant)])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn create_item(req: &CreateItemRequest) -> Result<ItemDto, String> {
    let res = reqwest::Client::new()
        .post(format!("{API_URL}/api/items"))
        .json(req)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn confirm_item(plant: &str, no: &str) -> Result<ItemDto, String> {
    let res = reqwest::Client::new()
        .post(format!("{API_URL}/api/items/{plant}/{no}/confirm"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    into_result(res).await
}

pub async fn delete_item(plant: &str, no: &str) -> Result<(), String> {
    let res = reqwest::Client::new()
        .delete(format!("{API_URL}/api/items/{plant}/{no}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(error_message(res).await)
    }
}
