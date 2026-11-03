# ADR 0001：整體架構與移植策略

- 狀態：已採用
- 日期：2026-10-28

## 背景

TipTop ERP 以 Genero 4GL 撰寫，資料庫為 Oracle/Informix，畫面由 GDC 桌面客戶端呈現。
程式數量龐大，且有大量客製。需移植到可長期維護、可由 AI 協助開發的技術棧。

## 決策

1. **規格驅動、逐模組取代**（Strangler Fig），不逐行翻譯 4GL。
   業務規則先萃取為 Gherkin 規格，經 Key User 確認後才實作。
2. **後端**：Rust + Axum + SQLx + PostgreSQL；分層 domain / app / infra / api。
3. **前端**：Dioxus 0.7。同一份程式碼輸出 Web、桌面（取代 GDC）、行動版（倉庫 PDA）。
   前端只透過 HTTP API 存取，不使用 Dioxus server functions，
   讓桌面/PDA/第三方系統都走同一套 API。
4. **多營運中心**：TipTop 一個營運中心一個 schema，新系統改為單一 schema + `plant_id`，
   日後可加 Row-Level Security。
   → 2026-11-03 由 ADR 0007 修訂：改為一廠一個 PostgreSQL schema（共用 schema 放系統與集團資料），
   以每廠的資料庫角色與 search_path 隔離。
5. **資料表命名**：改為可讀英文，舊表/欄位對照維護於 `docs/legacy-map/*.yaml`，
   作為 ETL 與新舊比對（Golden Master）的依據。
6. **測試**：BDD（cucumber-rs）以中文 Gherkin 撰寫；同一份規格需在記憶體與 PostgreSQL
   兩種後端通過。

## 後果

- 規格成為唯一需求來源，AI 與人都能依規格開發、審查。
- 需額外建立 ETL 與比對工具（下一階段：`crates/legacy-etl`）。
- 4GL 中隱含、未被規格捕捉的行為可能遺漏，需靠並行比對發現。
