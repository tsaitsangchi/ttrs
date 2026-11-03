# ADR 0008：程式代號改用英文

- 狀態：已採用（Key User，2026-11-03，E1–E7 依草稿確認）
- 範圍：TTRS 的程式代號（program.program_no）與所有引用它的資料：程式執行權限、使用者特殊權限、選單、
  權限檢查（Authorizer）、起始資料、規格與畫面
- 相關：ADR 0006 §4（權限沿用 TipTop 模型與資料）、ADR 0007、`specs/features/azz/program_maintenance*.feature`、
  `specs/features/azz/authorization.feature`

## 背景

Key User 要求把 aooi030 這類 TipTop 程式代號改成對應的英文程式代號，而且**資料的鍵值也改成英文**（不只畫面顯示）。

目前程式代號沿用 TipTop zz01（例如 `p_zx`、`aooi030`），是下列資料的鍵值：
`program`、`program_name`、`program_permission`（zy02）、`user_permission`（zxw04 程式列）、`menu_item`（zm04），
以及 Authorizer 每個 use case 宣告的程式代號、migrations 0015／0017 的起始資料、畫面的 `ProgramGate`。
引用 `program` 的外鍵都已設 `ON UPDATE CASCADE`，改代號時子表會跟著改。

ADR 0006 §4 原本希望 TipTop 的權限資料（zy_file、zxw_file）能直接匯入；改成英文鍵值後，匯入時要經過對照轉換。

## 決策

1. **英文程式代號是 TTRS 的程式鍵值**：`program.program_no` 存英文代號；TipTop 代號另存 `program.tiptop_program_no`
   （zz01，唯一、可空白），只用於轉檔與對照。
2. **格式（E1）**：小寫英文、數字與連字號（kebab-case），以英文字母開頭，最多 40 字，例如 `department-maintenance`。
   與畫面網址的命名一致（目前已是 `/aoo/department-maintenance`）。不加模組前綴，模組另有欄位。
3. **已移植作業的對照（E2）**：

   | TipTop | 英文程式代號 |
   |---|---|
   | p_zz | `program-maintenance` |
   | p_zta | `data-dictionary` |
   | p_zx | `user-maintenance` |
   | p_zxw | `user-permission-maintenance` |
   | p_zw | `permission-class-maintenance` |
   | p_zy | `program-permission-maintenance` |
   | aooi901 | `plant-maintenance` |
   | aooi030 | `department-maintenance` |
   | aooi040 | `employee-maintenance` |
   | aimi100 | `item-maintenance` |

   對照記在 `docs/legacy-map/programs.yaml`（新增），移植新作業時一併決定英文代號並在規格檔頭註明。
4. **程式維護（p_zz）跟著調整（E3）**：TipTop 依代號推導的規則不再適用——
   程式類別由第 4 碼推導、標準命名自動產生執行指令 `$FGLRUN $AIMi/aimi100`（program_maintenance.feature 的對應情境）。
   改為：程式類別必填、執行指令照輸入保存不自動產生；報表類別必須有報表類型的規則照舊（依程式類別判斷）。
   另增「TipTop 程式代號」欄位（選填、不可重複），可查詢。
5. **TipTop 權限資料轉檔（E4）**：zz_file 只轉入已移植（有英文代號）的程式；zy_file、zxw_file、zm_file 以
   `tiptop_program_no` 對照成英文代號，對照不到的列不轉入並列出清單。選單程式（MENU）也要有英文代號（例如 `menu-main`）。
6. **既有資料（E5）**：新增 migration 依對照表更新現有程式代號（外鍵 `ON UPDATE CASCADE` 帶動子表），
   並填入 `tiptop_program_no`；起始資料 0015／0017 不修改（migration 只能新增），由新 migration 改名。
7. **規格與程式（E6）**：Authorizer 的程式代號、`PORTED_PROGRAMS`、畫面 `ProgramGate`、權限規格對照表、
   各規格中當作資料的程式代號（例如程式維護規格的 `aimi100`、`p_query`）全部改用英文代號；
   規格檔頭的「TipTop 對應」仍寫 TipTop 代號，作為出處。

## 待確認

| # | 問題 | 建議 |
|---|---|---|
| E1 | 格式用 kebab-case（`department-maintenance`），還是 snake_case（`department_maintenance`）或其他？上限 40 字可以嗎？ | kebab-case、40 字；和網址一致 |
| E2 | 上表 10 支作業的英文代號是否合適？ | 照上表 |
| E3 | 程式維護不再依代號推導程式類別與執行指令（改為必填／照輸入），可以嗎？ | 可以；這兩條規則只對 TipTop 代號有意義 |
| E4 | TipTop 未移植的程式（zz_file 數千支）不轉入 TTRS 程式資料，其權限資料也跳過，可以嗎？ | 可以；TTRS 只登錄自己的程式，移植時再登錄 |
| E5 | 選單程式（TipTop menu_*）的英文代號規則？ | `menu-` 加原名轉成 kebab-case，例如 `menu-main` |
| E6 | 程式維護規格（program_maintenance*.feature）中以 TipTop 代號為例的情境（約 100 處），改用英文代號重寫，可以嗎？ | 可以；改寫後一併確認 |
| E7 | ADR 0006 §4「TipTop 權限資料可直接匯入」改為「經程式代號對照後匯入」，可以嗎？ | 可以 |

## 實作順序（確認後）

1. legacy-map `programs.yaml` 對照表；程式維護規格改寫（E3、E6）並確認。
2. migration：`program.tiptop_program_no`、依對照改名；程式代號檢核改為 E1 格式。
3. Authorizer、`PORTED_PROGRAMS`、畫面 `ProgramGate`、權限規格對照表與所有規格資料改用英文代號。
4. `make check`、PostgreSQL 版 BDD。
