# legacy/ — TipTop 原始碼（唯讀參考）

此目錄放鼎新 TipTop 的原始碼與資料字典，**只作為萃取業務規則的參考**。
除本檔外，其餘內容都不進版控（鼎新授權限制、檔案量大）。

## 結構

```
legacy/
├── README.md        本檔（唯一進版控）
├── src/             原始檔，原封不動（通常為 Big5）
│   ├── tiptop/         $TOP 標準程式：<模組>/4gl、<模組>/4fd，以及 lib、sub、qry 共用程式
│   └── topcust/        $CUST 客製程式 — 萃取規則時優先查這裡
├── utf8/            與 src/ 同結構的 UTF-8 版本，給 AI 與人閱讀
├── schema/          資料庫 DDL、欄位說明（手動匯出）
├── messages/        錯誤訊息檔 p_ze 匯出（手動匯出）
├── INDEX.tsv        程式代號 → 檔案路徑索引（匯入腳本產生）
└── import-report.txt 最近一次匯入的摘要與轉碼失敗清單
```

## 匯入

在專案根目錄執行（來源可以是本機路徑，或 rsync 可連的 `user@host:/path`）：

```bash
scripts/legacy-import.sh --top tiptop@erp-host:/u1/topprod/tiptop --cust tiptop@erp-host:/u1/topprod/topcust
```

- 只複製原始碼與畫面/資源檔（.4gl .4fd .per .inc .global .4ad .4st .4tb .4tm），
  不複製 42m/42f 等編譯產物。
- 自動判斷編碼：已是 UTF-8 的直接複製，其餘以 BIG5-HKSCS 轉成 UTF-8；
  失敗的檔案列在 `import-report.txt`。
- 可重複執行；同一份來源會得到相同結果。

## 匯入紀錄

| 日期         | TipTop 版本 | 來源 | 匯入者 | 備註 |
| ------------ | ----------- | ---- | ------ | ---- |
| （尚未匯入） |             |      |        |      |

`schema/`、`messages/` 需從資料庫手動匯出，例如：

- `schema/<表名>.sql`：Oracle 用 `DBMS_METADATA.GET_DDL`，或 `ora2pg -t TABLE`
- `schema/columns.tsv`：表名、欄位、型別、中文說明（TipTop 資料字典）
- `messages/ze.tsv`：錯誤代碼、語系、訊息內容
