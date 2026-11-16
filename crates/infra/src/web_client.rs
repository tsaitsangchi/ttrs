//! 以網址下載 EPUB 的對外連線（epub_url_download.feature UD4）：只做「解析網域」與「對已檢查過的位址發一次請求」。
//! 網址、位址檢查與轉址在 app 層（EpubUrlService）；這裡不跟隨轉址、不帶 cookie 與憑證，連線固定到檢查過的位址（防 DNS 重新綁定）。

use std::{net::IpAddr, net::SocketAddr, time::Duration};

use async_trait::async_trait;
use ttrs_app::ports::{HttpHop, WebClient};
use ttrs_domain::DomainError;

/// 每次請求的逾時（UD4）
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, Default)]
pub struct ReqwestWeb;

#[async_trait]
impl WebClient for ReqwestWeb {
    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, DomainError> {
        let found =
            tokio::time::timeout(Duration::from_secs(10), tokio::net::lookup_host((host, 0)))
                .await
                .map_err(|_| DomainError::UrlUnreachable)?
                .map_err(|_| DomainError::UrlUnreachable)?;
        let mut ips: Vec<IpAddr> = found.map(|a| a.ip()).collect();
        ips.dedup();
        Ok(ips)
    }

    async fn get(&self, url: &str, ip: IpAddr, max_bytes: u64) -> Result<HttpHop, DomainError> {
        let parsed = reqwest::Url::parse(url).map_err(|_| DomainError::UrlNotAllowed)?;
        let host = parsed
            .host_str()
            .ok_or(DomainError::UrlNotAllowed)?
            .to_owned();
        let port = parsed
            .port_or_known_default()
            .ok_or(DomainError::UrlNotAllowed)?;
        // 確保 TLS 的加密提供者已安裝（同 GitHub 連線）
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(TIMEOUT)
            .user_agent("TTRS")
            // 網域固定連到 app 層檢查過的位址，不再自己解析
            .resolve(&host, SocketAddr::new(ip, port))
            .build()
            .map_err(|_| DomainError::UrlUnreachable)?;
        let too_large = || DomainError::EpubTooLarge {
            max_mb: usize::try_from(max_bytes / 1024 / 1024).unwrap_or(0),
        };
        let mut res = client
            .get(parsed)
            .send()
            .await
            .map_err(|_| DomainError::UrlUnreachable)?;
        let header = |k: &str| {
            res.headers()
                .get(k)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let (status, location, content_type) = (
            res.status().as_u16(),
            header("location"),
            header("content-type"),
        );
        // 轉址與錯誤不需要內容
        if !(200..300).contains(&status) {
            return Ok(HttpHop {
                status,
                location,
                content_type,
                body: Vec::new(),
            });
        }
        if res.content_length().is_some_and(|n| n > max_bytes) {
            return Err(too_large());
        }
        let mut body = Vec::new();
        while let Some(chunk) = res.chunk().await.map_err(|_| DomainError::UrlUnreachable)? {
            if body.len() as u64 + chunk.len() as u64 > max_bytes {
                return Err(too_large());
            }
            body.extend_from_slice(&chunk);
        }
        Ok(HttpHop {
            status,
            location,
            content_type,
            body,
        })
    }
}
