#!/usr/bin/env python3
"""Generate the Phase 0 inventory report page (docs/phase0/tiptop-inventory.html) from legacy/inventory/summary.json.

Usage: python3 scripts/tiptop/inventory.py && python3 scripts/tiptop/inventory_report.py
Every number on the page comes from summary.json; the effort scenarios use the per-program estimates in DAYS below (assumptions).
"""

import html
import json
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
S = json.loads((ROOT / "legacy/inventory/summary.json").read_text(encoding="utf-8"))

TYPE_ZH = {"maintenance": "維護（i）", "transaction": "交易單據（t）", "batch": "批次處理（p）",
           "report": "報表（r）", "query": "查詢（q）", "setting": "參數設定（s）", "update": "更新（u）"}
# Module names: only well-documented TipTop modules and those confirmed by the user's SOP folders; the rest wait for gao_file
MODULE_ZH = {"aap": "應付帳款", "abm": "產品結構 BOM", "abx": "保稅管理", "afa": "固定資產", "agl": "總帳",
             "aim": "庫存與料件", "amr": "物料需求規劃 MRP", "aoo": "共用基本資料", "apm": "採購", "aqc": "品質管理",
             "asf": "生產管理", "axc": "成本", "axm": "訂單與銷售", "axr": "應收帳款", "azz": "系統管理",
             "aws": "Web 服務", "aps": "APS 排程"}
# Per-program person-days (including spec confirmation, AI-assisted), from the roadmap's per-program estimates
DAYS = {"maintenance": {"S": 3, "M": 5, "L": 10, "XL": 20}, "transaction": {"S": 7, "M": 12, "L": 20, "XL": 40},
        "batch": {"S": 5, "M": 12, "L": 20, "XL": 40}, "report": {"S": 1.5, "M": 3, "L": 6, "XL": 10},
        "query": {"S": 1.5, "M": 3, "L": 6, "XL": 10}, "setting": {"S": 2, "M": 4, "L": 8, "XL": 12},
        "update": {"S": 2, "M": 4, "L": 8, "XL": 12}}


def esc(x) -> str:
    return html.escape(str(x))


def n(x) -> str:
    return f"{x:,}"


def effort(type_filter=None, factor=1.0) -> float:
    total = 0.0
    for t, sizes in S["by_type_size"].items():
        if type_filter and not type_filter(t):
            continue
        total += sum(DAYS[t][k] * v for k, v in sizes.items()) * factor
    return total


all_days = effort()
reportish = effort(lambda t: t in ("report", "query"))
core = all_days - reportish
# Scenario B: port only the 40% in use; 70% of reports/queries replaced by the report framework and NL queries
b_days = core * 0.4 + reportish * 0.4 * 0.3
c_days = b_days / 2
pm = lambda d: round(d / 21)

by_type = S["by_type"]
max_type = max(by_type.values())
type_rows = "\n".join(
    f'<div class="hbar-row"><span class="hbar-label">{esc(TYPE_ZH[t])}</span>'
    f'<span class="hbar-track"><span class="hbar" style="width:{v / max_type * 100:.1f}%"></span></span>'
    f'<span class="hbar-val">{n(v)}</span></div>'
    for t, v in by_type.items()
)

size = S["by_size"]
size_total = sum(size.values())
size_cells = "".join(
    f'<div class="fact"><b>{n(size.get(k, 0))}</b><span>{label}（{size.get(k, 0) / size_total * 100:.0f}%）</span></div>'
    for k, label in (("S", "S 小於 1,500 行"), ("M", "M 1,500–5,000 行"), ("L", "L 5,000–15,000 行"), ("XL", "XL 15,000 行以上"))
)

mods = S["modules"]
max_mod = max(m["programs"] for m in mods.values())
custom_by_base = {}
for cm, cnt in S["custom_modules"].items():
    base = "a" + cm[1:] if not cm.startswith("cg") else "g" + cm[2:]
    custom_by_base[base] = custom_by_base.get(base, 0) + cnt
mod_rows = "\n".join(
    "<tr>"
    f'<td class="mono">{esc(code)}</td><td>{esc(MODULE_ZH.get(code, "—"))}</td>'
    f'<td class="num">{n(m["programs"])}</td>'
    f'<td><span class="mini-track"><span class="mini" style="width:{m["programs"] / max_mod * 100:.1f}%"></span></span></td>'
    f'<td class="num">{n(m["lines"])}</td><td class="num">{n(m["tables"])}</td>'
    f'<td class="num">{n(custom_by_base.get(code, 0)) if custom_by_base.get(code) else ""}</td>'
    f'<td class="types">{esc("、".join(f"{TYPE_ZH[t][:2]} {v}" for t, v in sorted(m["by_type"].items(), key=lambda x: -x[1])))}</td>'
    "</tr>"
    for code, m in mods.items()
)

largest_rows = "\n".join(
    f'<tr><td class="mono">{esc(p["program"])}</td><td class="mono">{esc(p["module"])}</td>'
    f'<td>{esc(TYPE_ZH[p["type"]])}</td><td class="num">{n(p["lines"])}</td></tr>'
    for p in S["largest"][:12]
)

shared = S["shared"]
page = f"""<title>TipTop 盤點報告</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Noto+Sans+TC:wght@400;500;700&family=Noto+Serif+TC:wght@600;700&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
/* Layout: report column up to 1100px; prose limited to ~72ch; wide tables scroll in their own containers. Same palette as the TTRS roadmap. */
:root {{
  --paper: #f5f7fa; --surface: #ffffff; --ink: #1b2330; --ink-soft: #4a5566; --line: #d8dee6;
  --rust: #b0451b; --rust-soft: #f6e3da; --steel: #1f6874; --steel-soft: #dcecee; --track: #e9edf2;
  --font-display: "Noto Serif TC", "Songti TC", "PMingLiU", serif;
  --font-body: "Noto Sans TC", "PingFang TC", "Microsoft JhengHei", system-ui, sans-serif;
  --font-mono: "IBM Plex Mono", ui-monospace, "SFMono-Regular", Consolas, monospace;
}}
@media (prefers-color-scheme: dark) {{
  :root:not([data-theme="light"]) {{
    color-scheme: dark; --paper: #11151b; --surface: #181d25; --ink: #e5e9ef; --ink-soft: #a6b0bf; --line: #2c3440;
    --rust: #e07a4b; --rust-soft: #3a2419; --steel: #5fb6c1; --steel-soft: #17323a; --track: #232a34;
  }}
}}
:root[data-theme="dark"] {{
  color-scheme: dark; --paper: #11151b; --surface: #181d25; --ink: #e5e9ef; --ink-soft: #a6b0bf; --line: #2c3440;
  --rust: #e07a4b; --rust-soft: #3a2419; --steel: #5fb6c1; --steel-soft: #17323a; --track: #232a34;
}}
* {{ box-sizing: border-box; }}
body {{ background: var(--paper); color: var(--ink); font-family: var(--font-body); line-height: 1.75;
  padding-left: max(16px, env(safe-area-inset-left, 0px)); padding-right: max(16px, env(safe-area-inset-right, 0px)); }}
.page {{ max-width: 1100px; margin: 0 auto; padding-block: 48px 96px; display: flex; flex-direction: column; gap: 52px; }}
.prose {{ max-width: 72ch; }}
h1, h2 {{ font-family: var(--font-display); text-wrap: balance; line-height: 1.3; margin: 0; }}
h1 {{ font-size: 2.125rem; }} h2 {{ font-size: 1.5rem; }}
p, ul, ol {{ margin: 0; }}
ul, ol {{ padding-left: 1.3em; display: flex; flex-direction: column; gap: 6px; }}
section {{ display: flex; flex-direction: column; gap: 18px; }}
.eyebrow {{ font-family: var(--font-mono); font-size: 0.8125rem; letter-spacing: 0.08em; text-transform: uppercase; color: var(--rust); }}
header.cover {{ display: flex; flex-direction: column; gap: 14px; border-bottom: 2px solid var(--ink); padding-bottom: 26px; }}
.lede {{ font-size: 1.1875rem; color: var(--ink-soft); max-width: 64ch; }}
.facts {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); gap: 12px; }}
.fact {{ background: var(--surface); border: 1px solid var(--line); border-radius: 8px; padding: 14px 16px; display: flex; flex-direction: column; gap: 2px; }}
.fact b {{ font-family: var(--font-mono); font-size: 1.5rem; font-weight: 500; font-variant-numeric: tabular-nums; }}
.fact span {{ color: var(--ink-soft); font-size: 0.8125rem; }}
.note {{ font-size: 0.8125rem; color: var(--ink-soft); }}
code, .mono {{ font-family: var(--font-mono); font-size: 0.875em; }}
.scroll {{ overflow-x: auto; border: 1px solid var(--line); border-radius: 8px; background: var(--surface); }}
table {{ border-collapse: collapse; width: 100%; font-size: 0.9375rem; }}
th, td {{ text-align: left; vertical-align: top; padding: 9px 14px; border-bottom: 1px solid var(--line); }}
th {{ font-weight: 500; color: var(--ink-soft); font-size: 0.8125rem; background: var(--paper); white-space: nowrap; }}
tr:last-child td {{ border-bottom: none; }}
td.num, th.num {{ text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }}
td.types {{ font-size: 0.8125rem; color: var(--ink-soft); min-width: 16rem; }}
.hbars {{ display: flex; flex-direction: column; gap: 8px; background: var(--surface); border: 1px solid var(--line); border-radius: 8px; padding: 16px 18px; }}
.hbar-row {{ display: grid; grid-template-columns: 8.5rem 1fr 4rem; gap: 12px; align-items: center; font-size: 0.9375rem; }}
.hbar-track, .mini-track {{ background: var(--track); border-radius: 3px; height: 14px; display: block; }}
.hbar {{ display: block; height: 100%; background: var(--steel); border-radius: 3px; }}
.hbar-val {{ text-align: right; font-family: var(--font-mono); font-variant-numeric: tabular-nums; }}
.mini-track {{ width: 9rem; height: 10px; }}
.mini {{ display: block; height: 100%; background: var(--steel); border-radius: 3px; }}
.callout {{ border: 1px solid var(--rust); background: var(--surface); border-radius: 8px; padding: 18px 20px; display: flex; flex-direction: column; gap: 10px; }}
.callout strong {{ color: var(--rust); }}
.scen td:first-child {{ white-space: nowrap; font-weight: 500; }}
footer {{ border-top: 1px solid var(--line); padding-top: 18px; font-size: 0.8125rem; color: var(--ink-soft); }}
:focus-visible {{ outline: 2px solid var(--rust); outline-offset: 2px; }}
@media (max-width: 560px) {{ .hbar-row {{ grid-template-columns: 6.5rem 1fr 3.2rem; }} }}
</style>

<div class="page">

<header class="cover">
  <div class="eyebrow">TTRS 移轉 · Phase 0 盤點 · {date.today().isoformat()}</div>
  <h1>TipTop 盤點報告</h1>
  <p class="lede">從 TipTop 主機（TRY-TIPTOPGP，/u1/topprod）唯讀匯入完整原始碼後的程式、客製與資料表統計，用來決定移植範圍與順序。</p>
  <div class="facts">
    <div class="fact"><b>{n(S["standard_programs"])}</b><span>標準主程式</span></div>
    <div class="fact"><b>{n(S["custom_programs"])}</b><span>客製程式（覆寫 {n(S["overrides"])}、新增 {n(S["custom_only"])}）</span></div>
    <div class="fact"><b>{n(S["tables"])}</b><span>資料表（{n(S["columns"])} 欄位）</span></div>
    <div class="fact"><b>{S["total_lines"] / 10000:.0f} 萬</b><span>標準主程式 4GL 行數</span></div>
  </div>
</header>

<section class="callout">
  <div class="eyebrow">結論</div>
  <h2>全部移植約需 {n(pm(all_days))} 人月，必須依使用率縮小範圍</h2>
  <p class="prose">依程式類型與大小估算，{n(S["standard_programs"])} 支標準主程式全部移植約 <strong>{n(round(all_days))} 人天（約 {n(pm(all_days))} 人月）</strong>。移轉藍圖假設 24 個月、4–6 人，約 100–140 人月。要守住這個時程，需要同時做到三件事：只移植實際在用的程式、報表改由報表框架與自然語言查詢涵蓋，以及生產線成熟後 AI 再把速度提升一倍。哪些程式在用，要等 DBA 回覆使用紀錄才能確定。</p>
</section>

<section>
  <div class="eyebrow">工時情境</div>
  <h2>三種範圍的估算</h2>
  <div class="scroll">
    <table class="scen">
      <thead><tr><th>情境</th><th>假設</th><th class="num">人天</th><th class="num">人月</th><th class="num">5 人需要</th></tr></thead>
      <tbody>
        <tr><td>A 全部移植</td><td>所有標準主程式逐支移植</td><td class="num">{n(round(all_days))}</td><td class="num">{n(pm(all_days))}</td><td class="num">{pm(all_days) / 5:.0f} 個月</td></tr>
        <tr><td>B 依使用率</td><td>只移植使用中的 40%；其中 70% 的報表與查詢改由報表框架、自然語言查詢涵蓋</td><td class="num">{n(round(b_days))}</td><td class="num">{n(pm(b_days))}</td><td class="num">{pm(b_days) / 5:.0f} 個月</td></tr>
        <tr><td>C 依使用率＋AI 加速</td><td>B 的範圍，生產線成熟後速度再提升一倍</td><td class="num">{n(round(c_days))}</td><td class="num">{n(pm(c_days))}</td><td class="num">{pm(c_days) / 5:.0f} 個月</td></tr>
      </tbody>
    </table>
  </div>
  <p class="note">單支人天沿用移轉藍圖：維護 3–20、交易單據 7–40、批次 5–40、報表與查詢 1.5–10（依 S/M/L/XL）。40%、70% 是暫定比例，拿到使用紀錄後以實際數字取代；客製覆寫的額外工作尚未計入。</p>
</section>

<section>
  <div class="eyebrow">程式類型</div>
  <h2>報表佔 {by_type["report"] / S["standard_programs"] * 100:.0f}%，是最大宗</h2>
  <p class="prose">類型依程式代號第 4 碼判斷（例如 aimi100 的 i 為維護作業）。報表與查詢合計 {n(by_type["report"] + by_type["query"])} 支，工時雖短但數量多，最適合改用共用報表框架與自然語言查詢；交易單據與批次處理數量較少，卻是業務規則最集中、最需要新舊比對的部分。</p>
  <div class="hbars" role="img" aria-label="各類型主程式數">
{type_rows}
  </div>
</section>

<section>
  <div class="eyebrow">程式大小</div>
  <h2>八成是小程式，少數巨型單據要特別規劃</h2>
  <div class="facts">{size_cells}</div>
  <p class="prose">行數含主程式與其輔助檔（例如 <code>s</code> 開頭的子程式、<code>_sub</code>）。最大的幾支是核心交易單據，單支就有數萬行，應拆成多份規格分批移植。</p>
  <div class="scroll">
    <table>
      <thead><tr><th>程式</th><th>模組</th><th>類型</th><th class="num">行數</th></tr></thead>
      <tbody>
{largest_rows}
      </tbody>
    </table>
  </div>
</section>

<section>
  <div class="eyebrow">模組</div>
  <h2>{len(mods)} 個標準模組</h2>
  <p class="prose">依主程式數排序。客製欄是 topcust 對應模組（如 cxm 對應 axm）的程式數，含覆寫與新增。模組中文名稱只列確定者，其餘待依模組資料（gao_file）補上。</p>
  <div class="scroll">
    <table>
      <thead><tr><th>代碼</th><th>名稱</th><th class="num">主程式</th><th></th><th class="num">行數</th><th class="num">資料表</th><th class="num">客製</th><th>類型分布</th></tr></thead>
      <tbody>
{mod_rows}
      </tbody>
    </table>
  </div>
</section>

<section>
  <div class="eyebrow">其他程式</div>
  <h2>共用程式、系統工具與客製</h2>
  <div class="scroll">
    <table>
      <thead><tr><th>類別</th><th class="num">數量</th><th>移植方式</th></tr></thead>
      <tbody>
        <tr><td>共用程式 lib（cl_*）</td><td class="num">{n(shared.get("tiptop/lib", 0))}</td><td>不單獨移植；萃取規則時一併讀，改寫為 TTRS 的共用服務與元件</td></tr>
        <tr><td>共用子程式 sub（s_*）</td><td class="num">{n(shared.get("tiptop/sub", 0))}</td><td>同上，多為共用計算與檢核，依呼叫需要移植</td></tr>
        <tr><td>開窗查詢 qry（q_*）</td><td class="num">{n(shared.get("tiptop/qry", 0))}</td><td>改為通用的查詢開窗元件，依資料表設定，不逐支移植</td></tr>
        <tr><td>系統工具 p_*</td><td class="num">{n(S["system_tools"].get("tiptop", 0))}</td><td>系統管理工具（p_zz、p_zta 已完成）；DDL、同步類工具不移植</td></tr>
        <tr><td>Web 服務 aws_*</td><td class="num">{n(sum(S["web_services"].values()))}</td><td>依實際對接的外部系統重做為 API，未使用者不移植</td></tr>
        <tr><td>客製覆寫標準程式</td><td class="num">{n(S["overrides"])}</td><td>萃取時以 topcust 版本為準；逐支評估能否改回標準流程</td></tr>
        <tr><td>客製新增程式</td><td class="num">{n(S["custom_only"])}</td><td>依使用率逐支評估</td></tr>
        <tr><td>未分類檔案</td><td class="num">{n(S["unclassified"])}</td><td>命名不符慣例的輔助檔，移植對應作業時再檢視</td></tr>
      </tbody>
    </table>
  </div>
</section>

<section>
  <div class="eyebrow">下一步</div>
  <h2>決定範圍前還需要的資料與決定</h2>
  <ol class="prose">
    <li>DBA 回覆使用紀錄（程式執行次數、異動筆數、授權人數、資料量），把情境 B 的 40% 換成實際清單。</li>
    <li>產品負責人依使用率決定不移植清單，並確認第一批：azz 系統管理核心（使用者、權限、選單）與 aoo 共用基本資料。</li>
    <li>把 TipTop 對照目錄的英文命名從 azz 的 161 張表擴大到全部 {n(S["tables"])} 張表。</li>
    <li>決定報表策略：先建報表框架與自然語言查詢，再挑使用率高的報表逐支重建。</li>
    <li>依實際範圍重排移轉藍圖的時程（v2）。</li>
  </ol>
</section>

<footer>
  來源：legacy/INDEX.tsv（scripts/legacy-import.sh）、legacy/catalog/tables.json（sch_catalog.py）、legacy/inventory/summary.json（scripts/tiptop/inventory.py）。逐支明細在 legacy/inventory/programs.tsv（不進版控）。分類依目錄與命名慣例推定。
</footer>

</div>
"""

out = ROOT / "docs/phase0/tiptop-inventory.html"
out.write_text(page, encoding="utf-8")
print(f"{out.relative_to(ROOT)}: A {pm(all_days)} / B {pm(b_days)} / C {pm(c_days)} person-months")
