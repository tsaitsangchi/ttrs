# legacy/ — TipTop 原始碼（唯讀參考）

此目錄放鼎新 TipTop 的原始碼與資料字典，**只作為萃取業務規則的參考**。
除本檔外，其餘內容都不進版控（鼎新授權限制、檔案量大）。

## 結構

```
legacy/
├── README.md        本檔（唯一進版控）
├── src/             原始檔，原封不動（通常為 Big5）
│   ├── tiptop/         $TOP 標準程式：<模組>/4gl、<模組>/4fd、<模組>/sch，以及 lib、sub、qry 共用程式
│   └── topcust/        $CUST 客製程式 — 萃取規則時優先查這裡
├── utf8/            與 src/ 同結構的 UTF-8 版本，給 AI 與人閱讀
├── schema/          （選用）從資料庫另外匯出的 DDL；資料表定義以 src/<層>/<模組>/sch/*.sch 為主
├── data/            從 TipTop TOPTEST 匯出的樣本資料（CSV，scripts/tiptop/tiptop.py export）
├── catalog/         由 .sch 解析的資料目錄（scripts/tiptop/sch_catalog.py build，匯入時自動重建）
├── messages/        錯誤訊息檔 p_ze 匯出（手動匯出）
├── INDEX.tsv        程式代號 → 檔案路徑索引（匯入腳本產生）
└── import-report.txt 最近一次匯入的摘要與轉碼失敗清單
```

## 匯入

在專案根目錄執行（來源可以是本機路徑，或 rsync 可連的 `user@host:/path`）：

```bash
scripts/legacy-import.sh --top tiptop@erp-host:/u1/topprod/tiptop --cust tiptop@erp-host:/u1/topprod/topcust
```

- 只複製原始碼、畫面/資源檔與資料表定義（.4gl .4fd .per .inc .global .4ad .4st .4tb .4tm .sch），
  不複製 42m/42f 等編譯產物。
- 自動判斷編碼：已是 UTF-8 的直接複製，其餘以 BIG5-HKSCS 轉成 UTF-8；
  失敗的檔案列在 `import-report.txt`。
- 可重複執行；同一份來源會得到相同結果。

## 匯入紀錄

| 日期         | TipTop 版本 | 來源 | 匯入者 | 備註 |
| ------------ | ----------- | ---- | ------ | ---- |
| （尚未匯入） |             |      |        |      |

## 資料表定義（.sch）

TipTop 每個模組的 `sch/<表名>.sch` 是該模組資料表的 Oracle DDL，
每個欄位後面有中文說明（即資料字典），例如 `tiptop/azz/sch/zz_file.sch`。
解析成資料目錄後查詢最方便（topcust 自動覆蓋 tiptop）：

```bash
python3 scripts/tiptop/sch_catalog.py build            # 產生 legacy/catalog/
python3 scripts/tiptop/sch_catalog.py show zz_file --notes
python3 scripts/tiptop/sch_catalog.py search 程式代號
```

`legacy/catalog/columns.tsv` 一欄一列，也可以直接 grep。

`schema/`、`messages/` 需從資料庫手動匯出，例如：

- `schema/<表名>.sql`：.sch 沒有的表，用 `scripts/tiptop/tiptop.py ddl <表名>` 匯出（DBMS_METADATA）

## 直接查 TipTop 資料庫（唯讀）

`scripts/tiptop/tiptop.py`：python-oracledb thin 模式（不需 Oracle Client），只能執行單一 SELECT，
連線後設為唯讀交易、從不 commit。只連 TOPTEST，請用唯讀帳號。

```bash
python3 -m venv .venv-tiptop && .venv-tiptop/bin/pip install -r scripts/tiptop/requirements.txt
# .env 設定 TIPTOP_ORACLE_DSN / USER / PASSWORD（見 .env.example）
.venv-tiptop/bin/python scripts/tiptop/tiptop.py ping
.venv-tiptop/bin/python scripts/tiptop/tiptop.py describe zz_file
.venv-tiptop/bin/python scripts/tiptop/tiptop.py export zz_file --where "zz011 = 'AIM'" --limit 200
```
- `messages/ze.tsv`：錯誤代碼、語系、訊息內容
