# ADR 0011：報表與列印

- 狀態：已採用（Key User，2026-11-05，RP1–RP14 依草稿建議確認）
- 相關：ADR 0005（多語言）、ADR 0006（權限：功能代號）、ADR 0007（一廠一個 schema）；
  `docs/legacy-map/azz.yaml`（zz_file 列印欄位、zy06）

## 背景

TTRS 目前只有資料字典的 CSV 匯出，還沒有報表。TipTop 的報表機制（萃取）：

- **報表程式**：每支報表是一支 4GL（例如 `aim/4gl/aimr100.4gl`），先以 QBE 輸入條件（`CONSTRUCT … tm.wc`），
  再依排序、小計條件產生報表檔；可選擇背景執行（`g_bgjob`，aimr100.4gl:55、:237-262），以參數傳入條件後於背景產出。
- **列印方式**（`lib/4gl/cl_prt.4gl:1-15`，`cl_prt(報表檔, 列印方式, 份數, 寬度)`）：系統印表機 1–9（`lp`）、終端機印表、
  畫面顯示、依頁次切割；後續加上轉 PDF（:24）、XML（FUN-4C0039）、Crystal Reports 輸出（:23）、報表內容簡繁轉換（FUN-510030）。
  另有 `cl_export_to_excel` 匯出 Excel、`cl_mail` 寄送。
- **程式資料檔 zz_file 的列印欄位**（TTRS 已移植為 program 欄位，尚未使用）：
  zz05 列印選擇條件否（Y 時報表結束印出 QBE 條件）、zz06 報表檔附加檔名、zz07 延後列印次數、zz17 報表寬度（79/132）、
  zz21 固定列印條件、zz22 固定列印方式。
- **個人列印設定 zv_file**（程式＋使用者＋權限類別）：zv03 固定列印方式、zv06 報表樣板、zv07 印表機名稱、zv08 立即列印否、zv09 份數。
- **報表樣板 zaa_file**（程式＋語言別＋使用者＋樣板＋權限類別）：欄位內容、順序、隱藏、定位點、邊界、列印備註（簽核）否——
  使用者可自訂報表欄位與版面（`cl_prt_type_list.4gl`）。
- **權限**：列印是功能「output」（各作業 `g_action_choice="output"`，例如 p_zx.4gl:267），經 cl_chk_act_auth 檢查；
  zy06「使用者允許印表方式」在 `cl_prt.4gl:2390` 仍讀取（Crystal Reports 匯出），但 legacy-map 目前標為「FUN-5C0057 已取消」。

## 決策

### 1. 輸出格式與套件

| 格式 | 套件 | 用途 |
|---|---|---|
| **PDF** | Typst（`typst`＋`typst-pdf`，以函式庫內嵌於 infra，Apache-2.0） | 正式報表與單據；範本排版、表頭表尾、分頁、小計；內嵌 Noto Sans TC 等字型 |
| **XLSX** | `rust_xlsxwriter`（MIT／Apache-2.0） | 「匯出 Excel」（取代 cl_export_to_excel）；保留數值格式、凍結窗格 |
| **CSV** | 現有寫法 | 大量資料交換 |
| DOCX | 不做（RP3） | 需要時以 `docx-rs` 另行產生或 LibreOffice 轉檔 |

### 2. 架構

- **app 層**：每支報表一個 use case，輸入「查詢條件＋排序＋小計選項」，輸出與格式無關的報表資料（表頭、明細列、小計、總計、
  列印條件字串）；金額、數量照規則用 `Decimal`。
- **infra 層**：輸出器 `ReportRenderer`（port）——PDF（Typst 範本）、XLSX、CSV。Typst 範本放 `crates/infra/reports/<模組>/<報表>.typ`，
  進版控；版面比照 TipTop 該報表的格式，逐支移植（照萃取流程：feature 檔頭註明 4gl 行號）。
- **API**：`POST /api/reports/{程式代號}`（條件、格式）→ 小報表直接回傳檔案；超過門檻（RP6）改為背景作業。
- **前端**：報表條件畫面沿用 QBE 元件；PDF 以新分頁內嵌開啟，使用者在瀏覽器 PDF 檢視器預覽與列印；XLSX／CSV 直接下載。

### 3. 列印方式

- **一般**：伺服器產生 PDF，由瀏覽器列印（取代 TipTop 的畫面顯示／系統印表機）；字型內嵌、分頁固定。
- **畫面清單**：維護作業的查詢清單加列印樣式（`@media print`），只印清單本身，不當正式單據。
- **特殊印表機**（標籤、條碼機、中一刀連續報表紙）：伺服器經 IPP／CUPS 直接送印（RP8），取代 `lp -d`；有需求的報表才做。

### 4. 權限、稽核與次數

- 列印、匯出經 Authorizer 檢查功能 **output**（沿用 TipTop action id）；資料權限與查詢相同，查不到的資料也印不出來。
- 每次產生報表寫入稽核紀錄：程式、使用者、營運中心、條件、格式、筆數。
- zz05＝Y 時報表最後印出查詢條件；每頁印程式代號、列印人、列印時間、第幾頁／共幾頁。
- 單據重印（RP10）：記錄列印次數，第二次起加「副本」浮水印。

### 5. 背景作業

- 筆數或執行時間超過門檻的報表排入背景佇列（取代 TipTop `g_bgjob`），完成後通知並保留檔案一段期間供下載（RP6、RP7）。
- 佇列先用資料庫表＋API 程序內的工作者；不另裝訊息佇列。

## 確認事項（皆採建議）

| # | 問題 | 建議 |
|---|---|---|
| RP1 | PDF 用 Typst（內嵌函式庫）產生，可以嗎？ | 可以 |
| RP2 | Excel 用 rust_xlsxwriter 產生 .xlsx（不產生舊版 .xls），可以嗎？ | 可以 |
| RP3 | DOCX 先不支援，有實際需求再加，可以嗎？ | 可以 |
| RP4 | 報表版面：比照 TipTop 該報表的格式逐支移植（欄位、順序、小計），不另行設計，可以嗎？ | 可以 |
| RP5 | 第一支試做報表選哪一支？ | `aimr100` 料件基本資料列印表（料件已移植、條件單純；無 topcust 客製） |
|  | （2026-11-05 改為部門維護的列印 aooi030，department_report.feature DR1；aimr100 等料件移植完成後再做） |  |
| RP6 | 背景作業門檻：超過幾筆或幾秒改為背景？ | 5,000 筆或預估超過 30 秒 |
| RP7 | 背景產出的檔案保留多久？ | 7 天 |
| RP8 | 有沒有需要伺服器直接送印的報表（標籤、條碼、中一刀連續報表紙）？哪些？ | 請提供清單；沒有就先不做 |
| RP9 | 列印權限：用功能代號 output 檢查；匯出 Excel 也算 output，還是另設 exporttoexcel（資料字典已有此功能代號）？ | 匯出另設 exporttoexcel，與列印分開控管 |
| RP10 | 哪些單據重印要加「副本」浮水印並記錄次數？ | 先不做，等第一支單據類報表移植時再定 |
| RP11 | zy06「使用者允許印表方式」：legacy-map 標為已取消，但 cl_prt.4gl:2390 仍讀取。要移植嗎？ | 不移植；以 output／exporttoexcel 功能代號控管（請確認 TOPPROD 是否有使用） |
| RP12 | 報表語言：依登入者的介面語系產生（標題、欄名），資料值不翻譯，可以嗎？ | 可以（ADR 0005） |
| RP13 | 紙張：預設 A4 直式；寬報表（zz17＝132）用 A4 橫式，可以嗎？ | 可以 |
| RP14 | 不移植的 TipTop 列印功能（下表），可以嗎？ | 照列 |

### RP14 不移植的項目

| 項目 | 原因 |
|---|---|
| zv_file 個人列印設定（固定列印方式、印表機名稱、立即列印、份數） | 改由瀏覽器列印對話框選印表機與份數；特殊印表機另依 RP8 設定 |
| zaa_file 使用者自訂報表樣板（欄位隱藏、順序、定位點、邊界） | 報表版面改由 Typst 範本統一維護；使用者要不同欄位時改用匯出 Excel |
| 終端機印表、依頁次切割、狀態顯示（lpstat） | 文字終端機時代的功能，瀏覽器 PDF 檢視器已涵蓋 |
| XML 報表輸出、Crystal Reports 輸出 | 由 PDF／XLSX 取代 |
| 報表內容簡繁轉換（FUN-510030） | 改依語系產生（RP12） |
| zz06 報表檔附加檔名、zz07 延後列印次數 | Genero 報表檔命名用；TTRS 不在伺服器留報表檔名 |

## 影響

- 新增套件：`typst-as-lib`、`typst`、`typst-pdf`、`typst-layout`、`rust_xlsxwriter`（infra）；字型 Noto Sans TC、SC 子集（SIL OFL，約 14 MB）
  放 `crates/infra/reports/fonts`，編譯時內嵌（DR9）。
- 新 port：`ReportRenderer`；背景作業表與工作者（RP6 確認後）。
- 新增功能代號 output（及 RP9 的 exporttoexcel）到各報表程式的權限設定；`PORTED_PROGRAMS` 與起始資料 migration 照規則加入。
- legacy-map：zv_file、zaa_file 列為不移植（RP14）；zy06 依 RP11 更新。
