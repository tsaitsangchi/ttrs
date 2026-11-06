# ADR 0012：資料異動記錄與查核

- 狀態：已採用（Key User，2026-11-06，AU1–AU16 依草稿建議確認）
- 相關：ADR 0006（權限、Actor `kind`、資料權限 `created_by`／`created_dept`）、ADR 0007（一廠一個 schema、`SET LOCAL ROLE`）、
  ADR 0011（列印與匯出寫入 `audit_log`）；TipTop `azo_file`、`lib/4gl/cl_log.4gl`

## 背景

### TipTop 的做法（萃取）

- **每張主檔的共同欄位**（例如 `tiptop/aoo/sch/gem_file.sch`）：`xxxacti` 資料有效碼、`xxxuser` 資料所有者、`xxxgrup` 資料所有部門、
  `xxxmodu` 資料修改者、`xxxdate` 最近修改日（`date`，只到日）。只保留「最後一次」的修改者與日期，改前的值不保留。
  本版 TipTop 的 .sch 沒有 `xxxoriu`／`xxxorig`（原始建立者／部門）欄位（已查 `tiptop/*/sch`，0 筆）。
- **重要資料修改記錄檔 `azo_file`**（`tiptop/aoo/sch/azo_file.sch`）：程式編號 azo01、異動人員 azo02、異動日期 azo03、
  異動時間 azo04（字串）、異動資料的 KEY azo05、異動說明 azo06（80 字）。由各程式自行 `INSERT`（tiptop 共 176 支，
  aim 21、apm 20、asf 18…），只記重要事件與一句說明，例如 `aimi100.4gl:4399-4405` 刪除料件記「R: 料號」、
  `:7154-7160` 料號變更記「Chg No:料號」；**沒有改前／改後的欄位值**，沒寫的程式就沒有記錄。
- **cl_log**（`lib/4gl/cl_log.4gl:40`）：p_ 系列系統設定程式（p_per、p_qry、p_feldname 等 8 支）把新增／修改／刪除的新值、舊值
  寫入伺服器的文字檔（`$TOP/log`），不在資料庫內，無法依資料查詢。
- **同時修改**：4GL 以 `SELECT … FOR UPDATE` 鎖住編輯中的資料列（例如 aooi030 的 `i030_cl`）；網頁沒有持續的連線，不能沿用。

### TTRS 現況

- 各表照搬 acti／user／grup／modu／date：`active`、`created_by`、`created_dept`、`updated_by`，以及 `created_on`／`updated_on`
  （`date`，例如 `migrations/plant/0003_classification_code.sql`）；少數表是 `created_at`／`updated_at`（timestamptz）。
- `audit_log`（`migrations/0003_program_registry.sql`，0023 加 `detail`）：程式、使用者、鍵值、動作、時間、說明——
  記錄了「誰做了什麼」，但沒有改前／改後的值，刪除後資料內容也不復存在。
- 沒有防止同時修改的機制：後存檔的人會覆蓋前一個人的修改（lost update）。

查核時要回答的問題：**這筆資料在某個時間點的內容是什麼？誰、透過哪支程式、在什麼時候、把哪個欄位從什麼改成什麼？刪除前的內容是什麼？**
只靠資料列上的欄位回答不了，因為每次修改都會覆蓋上一次的資訊。

## 決策

分兩層：**資料列上只放少數「現況」欄位；完整的異動軌跡放在只能新增、不能修改的異動記錄表 `change_log`。**

### 1. 資料維護表的標準欄位

| 欄位 | 型別 | 說明 | TipTop |
|---|---|---|---|
| `active` | `boolean NOT NULL DEFAULT true` | 資料有效否 | xxxacti |
| `created_by` | `text NOT NULL` | 資料所有者（資料權限依此判斷） | xxxuser |
| `created_dept` | `text NOT NULL` | 資料所有部門 | xxxgrup |
| `created_at` | `timestamptz NOT NULL DEFAULT now()` | 建立時間（到秒，帶時區） | 無（TTRS 新增） |
| `updated_by` | `text` | 最後修改者 | xxxmodu |
| `updated_at` | `timestamptz` | 最後修改時間 | xxxdate（原為 `date`） |
| `version` | `integer NOT NULL DEFAULT 1` | 版本號，每次修改加 1（樂觀鎖，§4） | 無（取代 FOR UPDATE 鎖） |

- 畫面仍可只顯示日期；資料庫存到秒，同一天內的多次修改才排得出先後。
- 單身（明細）表也帶這組欄位；`created_by`／`created_dept` 沿用單頭的值，資料權限以單頭判斷。
- 交易單據另有的狀態欄位（確認碼、過帳碼、作廢）屬業務欄位，不在本 ADR；狀態變更一樣進 `change_log`（§2 `operation`）。

### 2. 異動記錄表 `change_log`

| 欄位 | 型別 | 說明 |
|---|---|---|
| `change_id` | `bigint` identity | 流水號（同一交易內依寫入順序） |
| `occurred_at` | `timestamptz NOT NULL DEFAULT now()` | 交易時間（資料庫時間；同一交易的異動時間相同） |
| `table_name` | `text NOT NULL` | 資料表 |
| `record_key` | `jsonb NOT NULL` | 主鍵（例如 `{"kind":"8","code":"A01"}`，複合鍵也適用） |
| `operation` | `text NOT NULL` | `insert`／`update`／`delete`；業務動作時為該動作代號（`activate`、`deactivate`、`confirm`、`void`…） |
| `before` | `jsonb` | 改前整筆內容（新增時為空） |
| `after` | `jsonb` | 改後整筆內容（刪除時為空） |
| `changed_fields` | `text[]` | 有變動的欄位（新增、刪除時為全部欄位） |
| `version` | `integer` | 異動後的版本號 |
| `user_id` | `text NOT NULL` | 操作者（zx01）；不經 TTRS 的直接 SQL 記為 `db:<資料庫角色>` |
| `user_dept` | `text` | 操作當時的所屬部門 |
| `actor_kind` | `text NOT NULL` | `user`／`ai`／`service`／`db`（ADR 0006） |
| `on_behalf_of` | `text` | AI 或服務帳號代表哪位使用者 |
| `program` | `text` | 程式代號（例如 `department-maintenance`） |
| `action` | `text` | 功能代號（`insert`、`modify`、`delete`…） |
| `request_id` | `uuid` | 同一個 API 請求的關聯碼；一次存檔改多張表（單頭＋單身）時可串起來 |
| `client_ip` | `inet` | 來源位址 |
| `user_agent` | `text` | 瀏覽器／程式 |
| `reason` | `text` | 修改原因（選填；AU12） |

### 3. 擷取方式：資料庫觸發器＋交易情境

- 每張資料維護表掛同一個觸發器函式 `ttrs_change_log()`（`AFTER INSERT OR UPDATE OR DELETE … FOR EACH ROW`），寫入改前／改後、
  比對變動欄位。直接 SQL、資料匯入、批次作業一樣會被記錄，不能繞過；新增作業時也不必每支程式各寫一次。
- 操作情境由 infra 在每個交易開始時設定（與現有 `set_config('role'…)`、`search_path` 同一處，`crates/infra/src/plant_schema.rs`）：
  `set_config('ttrs.user_id', …, true)`、`ttrs.user_dept`、`ttrs.actor_kind`、`ttrs.on_behalf_of`、`ttrs.program`、`ttrs.action`、
  `ttrs.request_id`、`ttrs.client_ip`、`ttrs.user_agent`、`ttrs.reason`；觸發器以 `current_setting('ttrs.user_id', true)` 讀取，
  沒有設定時記為 `actor_kind = db`、`user_id = 'db:' || current_user`。
- 業務動作（啟用、停用、確認、作廢）由 app 層設定 `ttrs.operation`，觸發器以此取代 `update`。
- 觸發器同時維護 `version`（UPDATE 時 `NEW.version = OLD.version + 1`）與 `updated_at`，app 層不必自己算。
- 記憶體後端（`memory` feature，BDD 預設）在 repository 寫入時以相同規則記錄，行為與 PostgreSQL 一致（CLAUDE.md 規則）；
  `TTRS_BDD_DATABASE_URL` 跑同一份規格驗證觸發器。

### 4. 防止同時修改（樂觀鎖）

- 查詢回傳 `version`；修改、刪除、啟用／停用的請求必須帶回 `version`，`UPDATE … WHERE <主鍵> AND version = $n`，
  影響 0 筆時回 `RECORD_CHANGED`（HTTP 409，「資料已被他人修改，請重新查詢」）。
- 取代 TipTop 的 `FOR UPDATE` 鎖：不佔用連線，也不會因使用者離開畫面而一直鎖住資料。

### 5. 防止竄改

- 應用程式使用的資料庫角色（`ttrs_<schema>` 及 API 連線角色）對 `change_log` 只有 `INSERT`、`SELECT`；`REVOKE UPDATE, DELETE, TRUNCATE`。
  觸發器函式以 `SECURITY DEFINER` 寫入，擁有者為不給應用程式登入的角色。
- 進階（AU7）：每筆記錄帶前一筆的雜湊值（`prev_hash`、`row_hash`），串成雜湊鏈，可驗證記錄是否被刪改。

### 6. 放置位置與保存

- 工廠資料的異動記錄放各工廠 schema 的 `change_log`；系統資料（`public`）放 `public.change_log`，與資料權限的隔離一致（ADR 0007）。
- 依月份分割（`PARTITION BY RANGE (occurred_at)`），過期的整個月份可直接歸檔或卸除，不必逐筆刪除。
- `change_log`（資料異動）與 `audit_log`（查詢、列印、匯出、登入等存取與安全事件）分開：用途、筆數、保存期限都不同。

### 7. 敏感欄位

- 密碼雜湊、驗證碼、權杖類欄位不寫入 `change_log`（觸發器參數 `exclude`）。
- 個人資料（電話、身分證號、住址等）只記「有修改」，內容以 `***` 取代（觸發器參數 `mask`）。
- 每張表要排除或遮蔽的欄位寫在建立觸發器的 migration，並在 legacy-map 該表註明。

### 8. 畫面：「異動記錄」

- 每支資料維護作業加功能「異動記錄」（功能代號 AU11），列出該筆資料的每次異動：時間、操作者、程式、動作，
  以及改前、改後的欄位對照（欄位名稱用畫面欄位名稱，依語系顯示）。
- 經 Authorizer 檢查，並套用與查詢相同的資料權限；加入 `authorization.feature` 對照表、`PORTED_PROGRAMS` 與 ADMIN 起始資料。

### 9. 畫面：「狀態」頁籤

資料有效、資料所有者、資料所有部門、建立時間、最後修改者、最後修改時間（到秒並帶時區，以瀏覽器時區顯示；舊資料沒有時間時顯示日期）、版本統一放在各維護作業的
**「狀態」頁籤**，比照 TipTop 4fd 的 info／Status 頁（`tiptop/azz/4fd/p_zx.4fd`、`tiptop/aim/4fd/aimi100.4fd` 等 580 支畫面：
xxxuser、xxxmodu、xxxgrup、xxxdate、xxxacti）。共用元件 `crates/web/src/components/status.rs`（Key User，2026-11-06）：

- **單筆表單**（使用者、程式資料、營運中心維護）：上方頁籤加「狀態」，內容用 `StatusForm`；查詢模式只顯示後端可查詢的欄位。
- **表格作業**（部門、員工、權限類別、分類碼；程式權限與使用者特殊權限的單身）：表格上方「資料／狀態」頁籤切換欄位，
  狀態頁保留識別欄位（代號、名稱）＋狀態欄位；資料有效不再放在資料頁。
- 資料表沒有的欄位不顯示（例如 zz_file 無資料有效碼與建立日期、zy_file／zxw_file 無資料有效碼）。
- **查詢**（Key User，2026-11-06）：每個作業的查詢都可用狀態頁籤的所有欄位下條件（資料有效、資料所有者、資料所有部門、
  建立時間、最後修改者、最後修改時間、版本；時間以日期比對 `created_on`／`updated_on`，程式資料的建立時間取 `created_at` 的 UTC 日期）。
  白名單在 `ttrs_app::query`（各作業）與 `program_query`，PostgreSQL 欄位對應在各 `pg_*.rs`。
- 資料字典（p_zta）已有「狀態」頁，欄位為產生者 zta06、產生日期 zta05，維持原樣。
- 新作業一律照此做；清單式的瀏覽表（單筆表單下方的清單）可保留資料有效欄方便挑選。

## 實作狀態

- **第一階段（2026-11-06）**：工廠層的 department、employee、classification_code。
  - `migrations/0024_change_log.sql`：觸發器函式 `ttrs_bump_version`、`ttrs_change_log`，月份分割 `ttrs_change_log_partitions`
    （由 `ttrs_ensure_plant_schema` 在啟動時呼叫）、功能 history。
  - `migrations/plant/0004_change_log.sql`：各工廠 schema 的 `change_log`（月份分割＋預設分割表，應用程式角色只能新增與查詢）、
    三張表加 `version`／`created_at`／`updated_at` 與觸發器（employee.email 遮蔽）。
  - 操作情境：`ttrs_app::change_log::run`（use case）、`with_request`（API 中介層：X-Request-Id、X-Forwarded-For、User-Agent）；
    `plant_tx` 寫入 `ttrs.*`。記憶體後端以相同規則記錄。
  - API：修改、刪除、啟用／停用須帶版本（body `version` 或 `?version=`），沒帶 400 `VERSION_REQUIRED`、不符 409 `RECORD_CHANGED`；
    `GET …/{key}/history?lang=`。畫面：「異動記錄」按鈕與對話框（`components/history.rs`）。
  - AU2 先**新增** `created_at`／`updated_at`（timestamptz），原 `created_on`／`updated_on`（date）暫時保留；
    domain 改用時間戳後於第二階段移除。
- **第二階段（2026-11-06）**：共用層的表。
  - `migrations/0025_change_log_shared.sql`：`public.change_log`（月份分割；共用 schema 由應用程式登入角色擁有，
    改以觸發器 `ttrs_change_log_guard` 擋下修改與刪除，歸檔或測試清除時於交易內設定 `ttrs.allow_purge`）；
    user_account、permission_class、plant、program、program_permission、user_permission、table_dictionary、account_policy
    加版本與時間欄位與觸發器；user_plant、program_name、table_dictionary_text、column_dictionary_text 只記異動。user_account.email 遮蔽。
  - 操作情境改在**取出資料庫連線時**設定（sqlx `before_acquire`／`after_connect`，連線層級的 `ttrs.*`），
    共用層與工廠層的寫入都適用，不必逐一改寫 repository。
  - 可使用營運中心改為只刪除移除的、新增增加的（不再整批刪除重建），`all_plants` 併入使用者的同一個 UPDATE。
  - 查詢：`GET /api/history/{程式}/{鍵值}`（`ChangeHistoryService`），主檔與子表一併列出（CH2）；各作業加「異動記錄」按鈕。
  - 版本必填：修改（body `version`）、刪除與停用／啟用（`?version=`）。例外：程式權限、使用者特殊權限的單列移除與整批作業
    （依選單產生、刪除全部…）不帶版本。
  - 記憶體後端記錄主檔與可使用營運中心；程式名稱、資料字典說明與連帶刪除的子表只在 PostgreSQL 記錄。
- **尚未完成**：AU2 移除 `created_on`／`updated_on`（date）——各 domain 改用時間戳後再處理。
## 確認事項（皆採建議）

| # | 問題 | 建議 |
|---|---|---|
| AU1 | 範圍：所有資料維護表（系統資料、主檔、交易單頭與單身）都套用 §1 欄位與 §2 異動記錄，可以嗎？ | 可以；log 類、暫存類、`change_log`／`audit_log` 本身除外 |
| AU2 | `created_on`／`updated_on`（`date`）改為 `created_at`／`updated_at`（`timestamptz`）：以新 migration 新增欄位、搬資料（日期補 00:00）、移除舊欄位，可以嗎？ | 可以（不修改既有 migration） |
| AU3 | 加 `version` 樂觀鎖，版本不符回新錯誤代碼 `RECORD_CHANGED`（409），可以嗎？ | 可以 |
| AU4 | 資料所有者：TipTop 本版只有 xxxuser／xxxgrup（無原始建立者欄位）。`created_by`／`created_dept` 維持「資料所有者」意義；真正的建立者可從 `change_log` 的 insert 記錄查到。暫不做「資料所有權移轉」，可以嗎？ | 可以；需要移轉時再另加 `owner_*` 欄位 |
| AU5 | 用資料庫觸發器擷取改前／改後（§3），而非各 use case 自行寫入，可以嗎？ | 可以 |
| AU6 | `before`／`after` 存整筆內容（而非只存變動欄位），可以嗎？ | 可以：重建任一時間點的內容最簡單；空間以月份分割與壓縮處理 |
| AU7 | 雜湊鏈（§5 進階）：寫入需依序計算，大量並行寫入時會互相等待。要第一階段就做嗎？ | 先不做；權限限制只能新增＋資料庫備份；移植會計模組（agl、aap、aar）前再做 |
| AU8 | 不經 TTRS 的直接 SQL 修改也記錄（`actor_kind = db`），可以嗎？ | 可以 |
| AU9 | 異動記錄保存期限？ | 至少 10 年（參考商業會計法第 38 條：會計憑證 5 年、帳簿與報表 10 年；請會計確認），線上保留期限另定 |
| AU10 | `client_ip`、`user_agent` 屬個人資料，要記錄嗎？ | 記錄；異動記錄畫面只給有「異動記錄」功能權限的人看 |
| AU11 | 「異動記錄」功能代號：TipTop 沒有對應的標準功能，用 `history`（TTRS 新增），可以嗎？ | 可以；預設給 ADMIN |
| AU12 | 修改原因 `reason`：要強制填寫嗎？ | 先選填；之後可依作業（例如會計科目、價格）設定為必填 |
| AU13 | 遮蔽與排除（§7）：個資只記「有修改」、不記內容，可以嗎？哪些欄位算個資？ | 可以；先列：員工電話、身分證號、住址、電子郵件、銀行帳號 |
| AU14 | 既有資料沒有歷史；上線時不補「起始快照」，歷史從啟用此機制開始，可以嗎？ | 可以 |
| AU15 | 單頭＋單身一次存檔：每一列各一筆記錄，以 `request_id` 串起來（而非合併成一筆），可以嗎？ | 可以 |
| AU16 | 不移植的項目（下表），可以嗎？ | 照列 |

### AU16 不移植的項目

| 項目 | 原因 |
|---|---|
| `azo_file` 重要資料修改記錄檔（azo01–azo06） | 由 `change_log` 取代：所有異動自動記錄、含改前改後的值；legacy-map 對應到 `change_log` |
| `cl_log` 文字記錄檔（`$TOP/log`） | 由 `change_log` 取代；記錄放在資料庫才能依資料查詢與控管權限 |
| `SELECT … FOR UPDATE` 編輯鎖 | 網頁無持續連線，改為 `version` 樂觀鎖（§4） |
| xxxdate 只到日的修改日期 | 改為 `updated_at`（timestamptz），畫面可只顯示日期 |

## 影響

- 新 migration（public 與 `migrations/plant/`）：
  - 建立 `change_log`（月份分割）、`ttrs_change_log()` 觸發器函式、權限（只能新增）。
  - 已移植的表：補 `version`、`date` 欄位改 `timestamptz`（AU2）、掛觸發器、設定排除與遮蔽欄位。
  - 新功能 `history` 加入各已移植作業的程式資料與 ADMIN 權限（AU11）。
- infra：交易開始時設定 `ttrs.*` 情境（`plant_schema.rs`）；`pg.rs` 的修改、刪除改為帶 `version` 條件。
- app：`ports` 增加異動記錄查詢；`memory.rs` 實作同樣的記錄規則；新錯誤 `RecordChanged`（`RECORD_CHANGED`），三語翻譯。
- shared／api／web：DTO 帶 `version`；修改、刪除請求帶回；新 API `GET …/{key}/history`；共用的「異動記錄」對話框元件。
- 規格：`specs/features/azz/change_history.feature`（修改後查得到改前改後、刪除後仍查得到內容、版本衝突、記錄不可修改或刪除、
  敏感欄位遮蔽、直接 SQL 也記錄）；`authorization.feature` 加 `history`。
- legacy-map：`azo_file` 對應 `change_log`；各表 xxxdate 改對應 `updated_at`。
- 之後每支新作業照 §1 建表、掛觸發器，作為移植檢查項目。
