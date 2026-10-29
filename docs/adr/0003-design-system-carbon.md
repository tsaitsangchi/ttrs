# ADR 0003：前端設計系統採用 IBM Carbon

- 狀態：**已被 [ADR 0004](0004-design-system-material3.md) 取代**（2026-10-29 改採 Material 3）
- 範圍：crates/web（Web、桌面、行動版共用同一套 HTML/CSS）

## 決策

前端排版與配色採 IBM Carbon（`@carbon/styles`，Apache-2.0）。

- 樣式：直接使用 Carbon 編譯好的 CSS（`assets/carbon/`，鎖定版本，移除外部字型），不引入 Node/Sass 建置。
- 元件：自行以 Dioxus 包裝，輸出與 `@carbon/react` 相同的 HTML 結構（`src/components/carbon.rs`）。
  畫面只用元件，不直接寫 `cds--` 類別。
- 主題：Gray 10（`cds--g10`）；密度：作業畫面用 `cds--layout--size-sm`。
- 中文字型：IBM Plex Sans → Noto Sans TC → 微軟正黑體 → 系統字型。

## 理由

比較過 Apple HIG、Material 3、IBM Carbon、SAP Fiori：

- Carbon 的資料表格、表單層次（layering）與高密度尺寸最適合 ERP。
- 外觀中性，不模仿特定作業系統，跨 Web / Windows / macOS 一致。
- 開源授權，字型（IBM Plex）與圖示可在所有平台使用。

## 後果

- 行動版的版面規範較弱，屆時參考 Material 3 的視窗級距與 List-detail 版型。
- Carbon 的品牌感（IBM 藍、直角）明顯；要換品牌色時覆寫主題 token（`--cds-interactive`、`--cds-button-primary` 等）。
- 編譯後的 CSS 只有顏色 token 是全域變數，間距與字級 token 由 `main.css` 依 Carbon 規格補上。
- 舊畫面（aimi100）尚未改用 Carbon 元件，以 `.classic` 限定舊樣式，之後逐步改寫。
