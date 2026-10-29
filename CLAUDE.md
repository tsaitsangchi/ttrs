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
2. **規格**：寫 `specs/features/<模組>/<名稱>.feature`（`# language: zh-TW`），
   檔頭註明 TipTop 程式代號、資料表、規則來源。**規格未經使用者確認前不寫實作。**
   審核中的規格標 `@draft`（BDD 執行時自動略過），並在檔頭列出「待確認」問題；
   使用者確認後移除 `@draft` 再進入 Red。
3. **Red**：在 `crates/bdd/tests/steps/<模組>_<名稱>.rs` 寫步驟，`make bdd` 必須先失敗。
4. **Green**：依序實作 domain → app → infra → api → web，直到 `make test` 全過。
5. **對照**：更新 `docs/legacy-map/<模組>.yaml`（舊表/欄位 → 新表/欄位）。
6. **檢查**：`make check`（fmt + clippy -D warnings + test）全綠才算完成。

## 規則

- 金額、數量、匯率一律 `rust_decimal::Decimal`，DB 用 `NUMERIC`；**禁止 f32/f64**（clippy 已設 deny）。
- 錯誤用 `DomainError` 變體，`code()` 是穩定 API（BDD 與前端依賴），不可隨意改名。
- 每個 domain 模組、migration 表格 COMMENT 都要註明對應的 TipTop 程式/資料表。
- 多營運中心：單一 schema + `plant_id` 欄位；所有業務表主鍵/查詢都要帶 `plant_id`。
- 新增 repository 方法時，`memory.rs` 與 `infra/pg.rs` 要同時實作且行為一致；
  `TTRS_BDD_DATABASE_URL=... make bdd` 需能對 PostgreSQL 跑同一份規格。
- Migration 只能新增，不可修改已存在的檔案。
- 不要自行猜測 TipTop 欄位意義；不確定時在 legacy-map 標 `verify: true` 並詢問使用者。

## 常用指令

```bash
make test        # 全部測試（單元 + API 契約 + BDD，記憶體後端）
make bdd         # 只跑 BDD
make bdd-pg      # 對 PostgreSQL 跑 BDD（需 TTRS_BDD_DATABASE_URL）
make api         # 啟動 API（無 DATABASE_URL 時用記憶體資料）
make web         # dx serve 前端（需先 cargo install dioxus-cli）
make check       # fmt + clippy + test
```
