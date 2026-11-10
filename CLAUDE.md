# TTRS — TipTop ERP 移植（Rust 全端 + PostgreSQL）

將鼎新 TipTop ERP（Genero 4GL + Oracle/Informix）以 **規格驅動 / BDD** 方式重新實作。
目標是保留業務行為，不是逐行翻譯 4GL。

## 架構（依賴方向只能往下）

```
web (Dioxus) ──HTTP──▶ api (Axum) ──▶ app ──▶ domain
                         │             ▲
                         └──▶ infra ───┘   (infra 實作 app::ports)
shared：前後端共用 DTO（必須可編譯為 wasm）
bdd：驗收測試，執行 specs/features/**/*.feature
```

- `crates/domain`：純業務規則，**禁止** IO、async、sqlx、serde。
- `crates/app`：use case；只透過 `ports` trait 存取資料。`memory` feature 提供記憶體實作。
- `crates/infra`：PostgreSQL 實作；SQL 一律用字面值 + bind 參數（sqlx 0.9 會擋動態字串）。
- `crates/api`：handler 只做轉譯，錯誤碼對應 HTTP 狀態集中在 `HttpError`。
- `crates/web`：Dioxus 0.7。畫面依 TipTop 模組分資料夾 `pages/<模組>/`。
  設計系統採 Material 3（ADR 0004）：畫面只用 `components/material.rs` 的元件，版面比照該作業的 4fd，
  顏色只用 `--md-sys-color-*` token，不寫死色碼；色彩 token 由 `scripts/m3-colors.mjs` 產生，不手改。
  資料有效、資料所有者、所有部門、建立時間、最後修改者與時間、版本一律放「狀態」頁籤（`components/status.rs`，ADR 0012 §9）。
- **每支維護作業都必須有標準功能鈕**：新增、修改、刪除、查詢、列印、匯出 Excel、第一筆、上一筆、下一筆、最後一筆、
  失效（停用／啟用）、複製、異動記錄，以及「確認」（含確認與取消確認）（Key User 要求，2026-11-10；細節與例外見 ADR 0013）。
  萃取規格時即使 TipTop 該作業沒有某個功能，也要在規格列出並說明做法；不做的須列為例外經使用者確認。

## TipTop 原始碼（legacy/，唯讀，不進版控）

- 由 `scripts/legacy-import.sh` 匯入，說明見 `legacy/README.md`。
- **只讀 `legacy/utf8/`**；`legacy/src/` 是原始 Big5 檔，讀了會亂碼。兩者都禁止修改。
- 找程式先查 `legacy/INDEX.tsv`（program / ext / layer / module / path）。
- **先查 `topcust/` 再查 `tiptop/`**：客製程式可能覆寫標準邏輯，以客製為準。
- 一支作業通常要一起讀：`<模組>/4gl/<程式>.4gl`、`<模組>/4fd/<程式>.4fd`，
  以及它呼叫的 `lib/4gl/cl_*`、`sub/4gl/s_*`、`qry/4gl/q_*`。
- **表結構與欄位意義看 `<模組>/sch/<表名>.sch`**（Oracle DDL＋每欄中文說明，等同資料字典），
  例如 `tiptop/azz/sch/zz_file.sch`；萃取規則前先讀相關表的 .sch，不要只靠 4gl/4fd 推敲欄位意義。
  查詢用資料目錄：`python3 scripts/tiptop/sch_catalog.py show <表> --notes`、`search <關鍵字>`
  （先 `build`；客製 topcust 自動覆蓋標準 tiptop）。
  `legacy/schema/` 只放 .sch 沒有、另外從資料庫匯出的 DDL。錯誤訊息代碼看 `legacy/messages/`。
- 需要 TipTop 實際資料或 .sch 沒有的表時，用 `scripts/tiptop/tiptop.py`（唯讀，只連 TOPTEST），
  匯出到 `legacy/data/`、`legacy/schema/`。**TTRS 本身（api/app/infra）不可連 TipTop 資料庫。**

## 開發流程（每支 TipTop 程式都照做，不可跳步）

1. **萃取**：用資料目錄看相關資料表（`sch_catalog.py show`），再讀 `legacy/utf8/` 下的 4gl/4fd，整理業務規則，
   feature 檔頭以 `規則來源：tiptop/aim/4gl/aimi100.4gl:行號` 註明出處。
   相關表的**每個 .sch 欄位**都要在 legacy-map 列出：`columns`（對應的新欄位）或 `not_ported`（附原因），
   不確定的標 `verify: true`；不可只寫在註解或說明文字裡。
2. **規格**：寫 `specs/features/<模組>/<名稱>.feature`（`# language: zh-TW`），
   檔頭註明 TipTop 程式代號、資料表、規則來源。**規格未經使用者確認前不寫實作。**
   審核中的規格標 `@draft`（BDD 執行時自動略過），並在檔頭列出「待確認」問題；
   **不移植的欄位獨立列成一題**（表格列出欄位與原因），不要混在其他決定的說明裡。
   使用者確認後移除 `@draft` 再進入 Red。
3. **Red**：在 `crates/bdd/tests/steps/<模組>_<名稱>.rs` 寫步驟，`make bdd` 必須先失敗。
4. **Green**：依序實作 domain → app → infra → api → web，直到 `make test` 全過。
5. **對照**：更新 `docs/legacy-map/<模組>.yaml`（舊表/欄位 → 新表/欄位）。
   `scripts/tiptop/legacy_map_check.py` 檢查已對應的表每個 .sch 欄位都有列出、並標了 `scope`（沒有 legacy/ 時略過）。
6. **檢查**：`make check`（fmt + clippy -D warnings + test + legacy-map 檢查）全綠才算完成。

## 規則

- 金額、數量、匯率一律 `rust_decimal::Decimal`，DB 用 `NUMERIC`；**禁止 f32/f64**（clippy 已設 deny）。
- 錯誤用 `DomainError` 變體，`code()` 是穩定 API（BDD 與前端依賴），不可隨意改名。
- 每個 domain 模組、migration 表格 COMMENT 都要註明對應的 TipTop 程式/資料表。
- 多營運中心（ADR 0007，實作中）：一廠一個 PostgreSQL schema（`ds01`、`ds02`…，名稱小寫，即營運中心的資料庫代碼）；
  共用 schema `public` 放系統資料、營運中心與法人、拋轉設定；主檔與交易是工廠層，放在工廠 schema、不帶 `plant_id`；
  共用 schema 中參照營運中心的資料才帶 `plant_id`。每張表在 legacy-map 標 `scope`（system／group／plant）。
- 新增 repository 方法時，`memory.rs` 與 `infra/pg.rs` 要同時實作且行為一致；
  `TTRS_BDD_DATABASE_URL=... make bdd` 需能對 PostgreSQL 跑同一份規格。
- Migration 只能新增，不可修改已存在的檔案。
- 資料維護表一律帶 `version`／`created_at`／`updated_at` 並掛 `ttrs_bump_version`、`ttrs_change_log` 觸發器（ADR 0012）；
  寫入的 use case 以 `change_log::run(actor, 程式, 功能, …)` 包住，修改／刪除／啟用停用比對版本，記憶體後端同步記錄異動。
- 不要自行猜測 TipTop 欄位意義；不確定時在 legacy-map 標 `verify: true` 並詢問使用者。

## 多語言（ADR 0005）

- 語系：`zh-TW`（預設、翻譯來源）、`en`、`zh-CN`；只用 `ttrs_domain::lang::Lang`，不自行定義語系清單。
- **畫面文字一律經翻譯函式**，不可直接寫中文字串；翻譯放 `crates/web/locales/<語系>/*.ftl`，
  ID 規則 `<範圍>-<類別>-<名稱>`（如 `program-field-program-no`、`common-action-query`），
  三個語系的 ID 必須一致（`make check` 會檢查）。使用者輸入的資料值（如 QBE 條件值）不翻譯，
  該行加 `// i18n-allow` 註記。畫面完成翻譯後，加入 `i18n.rs` 的 `translated_pages_have_no_hardcoded_chinese` 清單。
- 錯誤訊息翻譯 ID 為 `error-<錯誤代碼>`，代碼與 `DomainError::code()` 一致；新增錯誤代碼時同時補三語翻譯。
- **需多語言的業務資料**：每種資料一張 `<資料>_text` 表（鍵值＋`lang`，外鍵 CASCADE），
  不建通用翻譯表；顯示時用共用退回規則（指定語系 → zh-TW → 空白），
  API 以 `?lang=` 決定顯示語系並回傳所有語系清單；畫面用共用的 `MultilingualField` 元件。
- 萃取 TipTop 規格時，遇到語言別欄位（如 gaz02、gat02、gaq02）照此規則對應，語言別 `0/1/2` → `zh-TW/en/zh-CN`。

## 登入與權限（ADR 0006）

- 登入一律走公司 SSO（OIDC），TTRS 不存密碼；TipTop 密碼相關欄位（zx10、zx16、zx17、zx19、zx20、gbt_file）不移植。
- 使用者代號沿用 TipTop `zx01`（各表 `created_by` 存它）；公司帳號以 `idp_subject` 綁定，不自動建帳號。
- 權限沿用 TipTop 模型與資料（zw/zy/zxw/zyw/gbl），動作代號用 TipTop action id（insert、modify、delete、query、confirm…）。
- **每個 use case 都要宣告（程式代號, 動作）並經 app 層 `Authorizer` 檢查**；修改／刪除再依資料擁有者 `created_by`、
  擁有部門 `created_dept` 套用資料權限。畫面隱藏按鈕只是方便，伺服器一定要檢查。拒絕回 `PERMISSION_DENIED`。
- 新增 use case 時，同時加入 `specs/features/azz/authorization.feature` 的對照表；新作業另加入
  `ttrs_app::authorization::PORTED_PROGRAMS` 與起始資料 migration（ADMIN 全部功能）。BDD 預設不檢查權限，由該規格驗證。
- 工廠資料以每廠的資料庫角色與 search_path 隔離（ADR 0007 §3）；每個交易先 `SET LOCAL ROLE ttrs_<schema>`、
  `SET LOCAL search_path = <schema>, public`。
- AI 代表使用者操作（Actor `kind = ai`），預設不得 confirm／post／approve／delete；背景作業用服務帳號。

## 常用指令

```bash
make test        # 全部測試（單元 + API 契約 + BDD，記憶體後端）
make bdd         # 只跑 BDD
make bdd-pg      # 對 PostgreSQL 跑 BDD（需 TTRS_BDD_DATABASE_URL）
make api         # 啟動 API（無 DATABASE_URL 時用記憶體資料）
make web         # dx serve 前端（需先 cargo install dioxus-cli）
make check       # fmt + clippy + test
```
