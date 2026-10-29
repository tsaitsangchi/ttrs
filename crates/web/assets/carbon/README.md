# IBM Carbon 樣式（第三方，Apache-2.0）

- 來源：npm `@carbon/styles@1.116.0` 的 `css/styles.min.css`，授權見同目錄 `LICENSE`。
- 修改：移除所有 `@font-face`（原本指向 IBM CDN `1.www.s81c.com`）。內網 ERP 不依賴外部網站；
  字型改由 `main.css` 指定 IBM Plex Sans → Noto Sans TC → 系統字型。要用 IBM Plex 請自行放字型檔。
- 不要手改這個檔案。升級時重新下載、重跑上述處理，並把檔名版本號一起改。
- 主題：在根元素加 `cds--g10`（淺）或 `cds--g100`（深）；密度：容器加 `cds--layout--size-sm|md|lg`。
- Dioxus 元件包裝在 `src/components/carbon.rs`，畫面只用元件，不直接寫 `cds--` 類別。
