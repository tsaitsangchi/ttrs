#!/usr/bin/env python3
"""TipTop 資料目錄：解析 legacy/src/<層>/<模組>/sch/*.sch（Oracle DDL＋每欄中文說明），
產生機器可讀的目錄，供萃取規格、legacy-map 草稿、DDL 建議與資料轉換使用。

規則：
  - topcust/ 的 .sch 覆蓋 tiptop/ 同名表（客製優先），目錄記錄被覆蓋的來源
  - 只解析不修改 legacy/；輸出在 legacy/catalog/（不進版控）

輸出：
  legacy/catalog/tables.json   完整目錄（表、欄位、型別、可否空值、預設值、說明、主鍵、索引、來源）
  legacy/catalog/columns.tsv   一欄一列，方便 grep
  legacy/catalog/report.txt    統計與解析警告

用法（在專案根目錄，只需標準函式庫）：
  python3 scripts/tiptop/sch_catalog.py build
  python3 scripts/tiptop/sch_catalog.py show zz_file
  python3 scripts/tiptop/sch_catalog.py search 程式代號
  python3 scripts/tiptop/sch_catalog.py search zz0 --columns
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "legacy" / "src"
OUT = ROOT / "legacy" / "catalog"
# 後者覆蓋前者（客製優先）
LAYERS = ("tiptop", "topcust")

HEADER = re.compile(r"^(檔案代號|檔案名稱|檔案目的|上游檔案|下游檔案)\s*[:：]\s*(.*)$")
CREATE = re.compile(r"^\s*create\s+table\s+(\w+)", re.I)
COLUMN = re.compile(
    r"""^(?P<name>[a-z][a-z0-9_]*)\s+
        (?P<type>[a-z0-9]+)(?:\((?P<args>[0-9,\s]+)\))?
        (?P<rest>[^/]*?)\s*,?\s*
        (?:/\*(?P<comment>.*?)\*/)?\s*$""",
    re.I | re.X,
)
CONT_COMMENT = re.compile(r"^\s*/\*(.*?)\*/\s*$")
PK = re.compile(r"alter\s+table\s+(\w+)\s+add\s+constraint\s+(\w+)\s+primary\s+key\s*\(([^)]*)\)", re.I)
INDEX = re.compile(r"create\s+(unique\s+)?index\s+(\w+)\s+on\s+(\w+)\s*\(([^)]*)\)", re.I)
DEFAULT = re.compile(r"default\s+('(?:[^']|'')*'|[^\s,]+)", re.I)


@dataclass
class Column:
    name: str
    type: str  # Oracle 型別原文，如 varchar2(20)、number(5)、date
    base_type: str  # varchar2 / number / date / blob …
    length: int | None = None  # varchar2(n)
    precision: int | None = None  # number(p,s)
    scale: int | None = None
    nullable: bool = True
    default: str | None = None
    description: str = ""  # 第一行說明
    notes: list[str] = field(default_factory=list)  # 後續說明行（代碼清單、範例…）
    no_use: bool = False  # 說明為 No Use


@dataclass
class Index:
    name: str
    unique: bool
    columns: list[str]


@dataclass
class Table:
    name: str
    title: str = ""  # 檔案名稱
    purpose: str = ""
    upstream: str = ""
    downstream: str = ""
    module: str = ""
    layer: str = ""
    path: str = ""
    overrides: list[str] = field(default_factory=list)  # 被本表覆蓋的來源（tiptop 層）
    columns: list[Column] = field(default_factory=list)
    primary_key: list[str] = field(default_factory=list)
    indexes: list[Index] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)


def clean_comment(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def parse_sch(path: Path) -> Table:
    text = path.read_text(encoding="utf-8", errors="replace")
    rel = path.relative_to(SRC)
    table = Table(name=path.stem.lower(), layer=rel.parts[0], module=rel.parts[1], path=str(rel))
    in_create = False
    last: Column | None = None

    for raw in text.splitlines():
        line = raw.strip()
        if not line:
            continue
        if m := HEADER.match(line):
            key, value = m.group(1), m.group(2).strip()
            if key == "檔案代號" and value and value.lower() != table.name:
                table.warnings.append(f"檔頭代號 {value} 與檔名不同")
            elif key == "檔案名稱":
                table.title = value
            elif key == "檔案目的":
                table.purpose = value
            elif key == "上游檔案":
                table.upstream = value
            elif key == "下游檔案":
                table.downstream = value
            continue
        if m := CREATE.match(line):
            in_create = True
            if m.group(1).lower() != table.name:
                table.warnings.append(f"create table {m.group(1)} 與檔名不同")
            continue
        if in_create:
            if line.startswith(");"):
                in_create = False
                last = None
                continue
            if line == "(":
                continue
            if m := CONT_COMMENT.match(line):
                note = clean_comment(m.group(1))
                if last and note:
                    last.notes.append(note)
                continue
            if m := COLUMN.match(line):
                last = make_column(m)
                table.columns.append(last)
                continue
            table.warnings.append(f"無法解析的欄位行：{line[:80]}")
            continue
        if m := PK.search(line):
            table.primary_key = [c.strip().lower() for c in m.group(3).split(",") if c.strip()]
            continue
        if m := INDEX.search(line):
            cols = [c.strip().lower() for c in m.group(4).split(",") if c.strip()]
            if cols:  # sys_ 開頭、欄位清單空白的系統索引略過
                table.indexes.append(Index(m.group(2).lower(), bool(m.group(1)), cols))
            continue

    if not table.columns:
        table.warnings.append("沒有解析到任何欄位")
    names = {c.name for c in table.columns}
    for c in table.primary_key:
        if c not in names:
            table.warnings.append(f"主鍵欄位 {c} 不在欄位清單")
    return table


def make_column(m: re.Match) -> Column:
    base = m.group("type").lower()
    args = [a.strip() for a in (m.group("args") or "").split(",") if a.strip()]
    rest = m.group("rest") or ""
    col = Column(
        name=m.group("name").lower(),
        type=base + (f"({','.join(args)})" if args else ""),
        base_type=base,
        nullable="not null" not in rest.lower(),
        description=clean_comment(m.group("comment") or ""),
    )
    if base in ("varchar2", "varchar", "char", "nvarchar2") and args:
        col.length = int(args[0])
    elif base == "number" and args:
        col.precision = int(args[0])
        col.scale = int(args[1]) if len(args) > 1 else 0
    if d := DEFAULT.search(rest):
        col.default = d.group(1)
    col.no_use = col.description.lower().replace(" ", "") == "nouse"
    return col


def build() -> dict[str, Table]:
    tables: dict[str, Table] = {}
    for layer in LAYERS:
        for path in sorted((SRC / layer).glob("*/sch/*.sch")):
            t = parse_sch(path)
            if prev := tables.get(t.name):
                t.overrides = [prev.path, *prev.overrides]
            tables[t.name] = t
    return tables


def write(tables: dict[str, Table]) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    data = {name: asdict(t) for name, t in sorted(tables.items())}
    (OUT / "tables.json").write_text(json.dumps(data, ensure_ascii=False, indent=1), encoding="utf-8")

    with (OUT / "columns.tsv").open("w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, delimiter="\t", lineterminator="\n")
        w.writerow(["table", "table_title", "module", "layer", "column", "type", "nullable", "pk", "description", "notes"])
        for t in sorted(tables.values(), key=lambda t: t.name):
            for c in t.columns:
                w.writerow([t.name, t.title, t.module, t.layer, c.name, c.type,
                            "Y" if c.nullable else "N", "Y" if c.name in t.primary_key else "",
                            c.description, " / ".join(c.notes)])

    cols = sum(len(t.columns) for t in tables.values())
    no_pk = sorted(t.name for t in tables.values() if not t.primary_key)
    overridden = sorted(t.name for t in tables.values() if t.overrides)
    warned = [t for t in tables.values() if t.warnings]
    by_module: dict[str, int] = {}
    for t in tables.values():
        by_module[f"{t.layer}/{t.module}"] = by_module.get(f"{t.layer}/{t.module}", 0) + 1
    types: dict[str, int] = {}
    for t in tables.values():
        for c in t.columns:
            types[c.base_type] = types.get(c.base_type, 0) + 1
    lines = [
        f"資料表：{len(tables)}，欄位：{cols}",
        "各層/模組：" + "、".join(f"{k} {v}" for k, v in sorted(by_module.items())),
        "型別：" + "、".join(f"{k} {v}" for k, v in sorted(types.items(), key=lambda x: -x[1])),
        f"No Use 欄位：{sum(c.no_use for t in tables.values() for c in t.columns)}",
        f"無主鍵的表（{len(no_pk)}）：{', '.join(no_pk) or '無'}",
        f"客製覆蓋標準的表（{len(overridden)}）：{', '.join(overridden) or '無'}",
        f"有解析警告的表：{len(warned)}",
    ]
    for t in warned:
        for w_ in t.warnings:
            lines.append(f"  {t.path}: {w_}")
    report = "\n".join(lines) + "\n"
    (OUT / "report.txt").write_text(report, encoding="utf-8")
    print(report, end="")
    print(f"已寫入 {OUT.relative_to(ROOT)}/tables.json、columns.tsv、report.txt")


def load() -> dict:
    path = OUT / "tables.json"
    if not path.exists():
        sys.exit("尚未建立目錄，請先執行：python3 scripts/tiptop/sch_catalog.py build")
    return json.loads(path.read_text(encoding="utf-8"))


def disp_len(s: str) -> int:
    return sum(2 if ord(ch) > 0x2E7F else 1 for ch in s)


def pad(s: str, width: int) -> str:
    return s + " " * max(0, width - disp_len(s))


def cmd_show(args) -> None:
    data = load()
    name = args.table.lower()
    t = data.get(name) or data.get(f"{name}_file")
    if not t:
        sys.exit(f"找不到 {args.table}；可用 search 查詢")
    print(f"{t['name']}  {t['title']}")
    print(f"  來源：{t['path']}" + (f"（覆蓋 {', '.join(t['overrides'])}）" if t["overrides"] else ""))
    for label, key in (("目的", "purpose"), ("上游", "upstream"), ("下游", "downstream")):
        if t[key]:
            print(f"  {label}：{t[key]}")
    print(f"  主鍵：({', '.join(t['primary_key'])})" if t["primary_key"] else "  主鍵：無")
    for i in t["indexes"]:
        print(f"  {'唯一' if i['unique'] else ''}索引 {i['name']}：({', '.join(i['columns'])})")
    print()
    rows = []
    for c in t["columns"]:
        flags = ("PK " if c["name"] in t["primary_key"] else "") + ("NOT NULL" if not c["nullable"] else "")
        if c["default"]:
            flags += f" DEFAULT {c['default']}"
        rows.append((c["name"], c["type"], flags.strip(), c["description"], c["notes"]))
    w0 = max(disp_len(r[0]) for r in rows)
    w1 = max(disp_len(r[1]) for r in rows)
    w2 = max(disp_len(r[2]) for r in rows)
    for name_, typ, flags, desc, notes in rows:
        print(f"  {pad(name_, w0)}  {pad(typ, w1)}  {pad(flags, w2)}  {desc}")
        if args.notes:
            for n in notes:
                print(f"  {pad('', w0)}  {pad('', w1)}  {pad('', w2)}    {n}")
    if t["warnings"]:
        print("\n  解析警告：" + "；".join(t["warnings"]))


def cmd_search(args) -> None:
    data = load()
    kw = args.keyword.lower()
    hits = 0
    for t in data.values():
        if not args.columns and (kw in t["name"] or kw in t["title"].lower()):
            print(f"{t['name']:<14} {t['title']}  [{t['layer']}/{t['module']}]")
            hits += 1
        for c in t["columns"]:
            text = " ".join([c["name"], c["description"], *c["notes"]]).lower()
            if kw in text:
                print(f"  {t['name']}.{c['name']:<12} {c['type']:<14} {c['description']}")
                hits += 1
    print(f"（{hits} 筆）")


def main() -> None:
    p = argparse.ArgumentParser(description="TipTop .sch 資料目錄（詳見檔頭說明）")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("build", help="解析所有 .sch，產生 legacy/catalog/")
    s = sub.add_parser("show", help="顯示一張表（可省略 _file）")
    s.add_argument("table")
    s.add_argument("--notes", action="store_true", help="一併顯示欄位的後續說明行")
    s = sub.add_parser("search", help="依表名、表說明、欄位名或欄位說明搜尋")
    s.add_argument("keyword")
    s.add_argument("--columns", action="store_true", help="只搜尋欄位")
    args = p.parse_args()

    if args.cmd == "build":
        write(build())
    elif args.cmd == "show":
        cmd_show(args)
    else:
        cmd_search(args)


if __name__ == "__main__":
    main()
