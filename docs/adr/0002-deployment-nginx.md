# ADR 0002：以 nginx 作為前端與 API 的反向代理

- 狀態：已採用
- 日期：2026-10-28

## 背景

Dioxus Web 版是靜態檔（index.html、wasm、js、css），API 由 Axum 提供。
若兩者不同源，需開放 CORS；wasm 檔數 MB，需壓縮與快取；正式環境需 HTTPS、
存取紀錄、頻率限制，並為日後多個 API instance 預留負載平衡。

## 決策

1. **正式環境**：nginx 位於最前端（設定檔 `deploy/nginx/ttrs.conf`）
   - `/` 提供 `make bundle` 產出的靜態檔，SPA 路由 fallback 到 `index.html`
   - `/api/` 轉發至 `upstream ttrs_api`（Axum，可多 instance）
   - 負責 TLS、gzip、hash 資產長期快取、頻率限制、內網 IP 白名單、存取 log
2. **開發環境**：不裝 nginx，由 `dx serve` 的 `[[web.proxy]]` 將 `/api/` 轉給本機 Axum，
   行為與正式環境同源一致。
3. **前端 API 位址**：Web 版預設使用 `window.location.origin`（同源）；
   桌面/行動版或特殊部署以編譯期 `TTRS_API_URL` 指定。
4. **CORS 預設關閉**：僅在前端與 API 不同網域時設定 `TTRS_CORS_ORIGINS` 白名單。
5. nginx 只做傳輸層的事，**不放任何業務規則**（含權限判斷）。

## 後果

- 移除 `CorsLayer::permissive()`，降低跨站請求風險。
- 部署多一個元件；小規模內網試行可暫用 `tower-http::ServeDir` 由 Axum 直接提供靜態檔。
- 長時間批次作業（月結、MRP）不應依賴放寬 proxy 逾時，後續需改為背景工作 + 查詢進度。
