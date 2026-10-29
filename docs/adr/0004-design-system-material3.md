# ADR 0004：前端設計系統改採 Material 3

- 狀態：已採用（2026-10-29），取代 [ADR 0003](0003-design-system-carbon.md)
- 範圍：crates/web（Web、桌面、行動版共用同一套 HTML/CSS）

## 決策

前端排版與配色採 Google Material 3（M3）。

- 色彩：以種子色 `#1E5AA8` 經官方演算法（`@material/material-color-utilities` 0.4.0，SchemeTonalSpot）
  產生淺色、深色兩套 `--md-sys-color-*` token，另加與種子色協調的 success / warning。
  產生器在 `scripts/m3-colors.mjs`，輸出 `assets/material/m3-colors.css`（不手改）。換品牌色只要換種子色重跑。
- 元件：M3 官方 Web 元件（`@material/web`）已進入維護模式，且是 Lit 自訂元素，不採用。
  改由 `assets/material/m3.css`（樣式）＋ `src/components/material.rs`（Dioxus 元件）依 M3 規格自行實作。
- 主題：`md-theme-light`（深色 `md-theme-dark` 已產生，尚未提供切換）。
- 字型：Roboto → Noto Sans TC → 微軟正黑體 → 系統字型（字型檔未內附）。

## ERP 調整（偏離 M3 預設之處）

- 密度：欄位 40px、按鈕 36px、清單列 40px（約 M3 density -4），以容納 4fd 的欄位量。
- Outlined text field 的標籤固定浮在框線上（不做 placeholder 到標籤的動畫），欄位多時較易掃讀。
- 資料表格：M3 規格沒有 data table，依 M3 色彩、狀態層自訂（`.md-table`）。
- 提示訊息：M3 沒有 inline notification，以 error / success container 色做 banner。

## 理由

- 色彩系統最完整，可由單一種子色產生通過對比要求的淺/深色主題，換品牌色成本最低。
- 字型、圖示授權寬鬆，跨平台可用；Web 使用者熟悉此風格。
- 不需引入第三方 CSS 檔（Carbon 約 0.9 MB），樣式只含實際用到的元件。

## 後果

- 元件需自行維護，新增元件時要對照 m3.material.io 規格。
- 舊畫面（aimi100、首頁）以 `.classic` 限定舊樣式，色彩已改用 M3 token，之後逐步改用 M3 元件。
