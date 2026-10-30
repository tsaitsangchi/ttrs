#!/usr/bin/env python3
"""TipTop program inventory (TTRS migration Phase 0).

Reads legacy/INDEX.tsv (from legacy-import.sh) and legacy/catalog/tables.json (from sch_catalog.py build),
and produces:
  legacy/inventory/programs.tsv   one row per main program (type, size, screens, customization override)
  legacy/inventory/summary.json   aggregate counts (consumed by the Phase 0 report)

Classification rules (inferred from file layout and naming, not by guessing what a program does):
  - Only <layer>/<module>/4gl/*.4gl counts. 42m (compile-time copies), demo, work, ora, rest, bin are excluded
  - Main program: [module 3][type 1][number 3](optional 1 char), e.g. aimi100, axmt410
      type: i maintenance  t transaction  p batch  r report  q query  s setting  u update
  - Supporting file: s<main program> or <main program>_<suffix>, belongs to that main program
  - Shared: lib (cl_*), sub (s_*), qry (q_*) and topcust clib/csub/cqry
  - System tools p_*, web services aws_*
  - Customization: a topcust program with the same name as a tiptop main program = override; a topcust-only name = new custom program
Size buckets (total lines of main program + supporting files): S < 1,500, M < 5,000, L < 15,000, XL >= 15,000

Usage: python3 scripts/tiptop/inventory.py
"""

import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LEGACY = ROOT / "legacy"
MAIN_RE = re.compile(r"^([a-z]{3})([irptqsu])(\d{3})([a-z0-9]?)$")
EXCLUDED_MODULES = {"demo", "work", "ora", "rest", "bin", "ds4gl2", "config"}
SHARED_MODULES = {"lib", "sub", "qry", "clib", "csub", "cqry"}
TYPE_NAMES = {"i": "maintenance", "t": "transaction", "p": "batch", "r": "report",
              "q": "query", "s": "setting", "u": "update"}


def size_bucket(lines: int) -> str:
    if lines < 1500:
        return "S"
    if lines < 5000:
        return "M"
    if lines < 15000:
        return "L"
    return "XL"


def count_lines(rel: str) -> int:
    try:
        with open(LEGACY / rel, "rb") as f:
            return sum(1 for _ in f)
    except OSError:
        return 0


def owner_of(name: str) -> str | None:
    """Main program a supporting file belongs to; None if it can't be determined."""
    base = name.split("_", 1)[0]
    if MAIN_RE.match(base):
        return base
    if base.startswith("s") and MAIN_RE.match(base[1:]):
        return base[1:]
    return None


def main() -> int:
    index = LEGACY / "INDEX.tsv"
    if not index.exists():
        sys.exit("legacy/INDEX.tsv not found; run scripts/legacy-import.sh first")
    rows = [line.rstrip("\n").split("\t") for line in index.open(encoding="utf-8", errors="surrogateescape")][1:]

    four_gl: list[tuple[str, str, str, str]] = []  # (layer, module, name, path)
    forms: dict[tuple[str, str], list[str]] = defaultdict(list)
    for name, ext, layer, module, path in rows:
        parts = path.split("/")
        if len(parts) != 5 or module in EXCLUDED_MODULES:
            continue
        if ext == "4gl" and parts[3] == "4gl":
            four_gl.append((layer, module, name, path))
        elif ext == "4fd" and parts[3] == "4fd":
            forms[(layer, module)].append(name)

    programs: dict[tuple[str, str], dict] = {}  # (layer, name) -> info
    shared = Counter()
    tools = Counter()
    services = Counter()
    other: list[str] = []
    aux: list[tuple[str, str, str, str]] = []
    for layer, module, name, path in four_gl:
        if module in SHARED_MODULES:
            shared[f"{layer}/{module}"] += 1
            continue
        if name.startswith("p_"):
            tools[layer] += 1
            continue
        if name.startswith("aws_"):
            services[layer] += 1
            continue
        m = MAIN_RE.match(name)
        if m:
            programs[(layer, name)] = {
                "layer": layer, "module": module, "program": name, "type": m.group(2),
                "lines": count_lines(path), "aux_files": 0, "forms": 0,
            }
        else:
            aux.append((layer, module, name, path))
    for layer, module, name, path in aux:
        owner = owner_of(name)
        info = programs.get((layer, owner)) if owner else None
        if info is None:
            other.append(f"{layer}/{module}/{name}")
            continue
        info["aux_files"] += 1
        info["lines"] += count_lines(path)
    for (layer, module), names in forms.items():
        for f in names:
            owner = owner_of(f) or (f if MAIN_RE.match(f) else None)
            if owner and (layer, owner) in programs:
                programs[(layer, owner)]["forms"] += 1

    standard = {n for (l, n) in programs if l == "tiptop"}
    custom = {n for (l, n) in programs if l == "topcust"}
    overrides = sorted(standard & custom)
    custom_only = sorted(custom - standard)

    out_dir = LEGACY / "inventory"
    out_dir.mkdir(exist_ok=True)
    with open(out_dir / "programs.tsv", "w", encoding="utf-8") as f:
        f.write("program\tlayer\tmodule\ttype\tlines\tsize\taux_files\tforms\tcustom\n")
        for info in sorted(programs.values(), key=lambda x: (x["layer"], x["module"], x["program"])):
            flag = ""
            if info["layer"] == "tiptop" and info["program"] in custom:
                flag = "overridden"
            elif info["layer"] == "topcust":
                flag = "override" if info["program"] in standard else "custom_only"
            f.write("\t".join(str(x) for x in (
                info["program"], info["layer"], info["module"], TYPE_NAMES[info["type"]],
                info["lines"], size_bucket(info["lines"]), info["aux_files"], info["forms"], flag,
            )) + "\n")

    std_programs = [p for p in programs.values() if p["layer"] == "tiptop"]
    by_module: dict[str, Counter] = defaultdict(Counter)
    lines_by_module = Counter()
    for p in std_programs:
        by_module[p["module"]][p["type"]] += 1
        lines_by_module[p["module"]] += p["lines"]
    catalog = json.loads((LEGACY / "catalog/tables.json").read_text(encoding="utf-8"))
    tables_by_module = Counter(t["module"] for t in catalog.values())
    columns_by_module = Counter()
    for t in catalog.values():
        columns_by_module[t["module"]] += len(t["columns"])

    summary = {
        "standard_programs": len(std_programs),
        "custom_programs": len(custom),
        "overrides": len(overrides),
        "custom_only": len(custom_only),
        "by_type": {TYPE_NAMES[k]: v for k, v in Counter(p["type"] for p in std_programs).most_common()},
        "by_size": dict(Counter(size_bucket(p["lines"]) for p in std_programs)),
        "by_type_size": {
            TYPE_NAMES[t]: dict(Counter(size_bucket(p["lines"]) for p in std_programs if p["type"] == t))
            for t in TYPE_NAMES
        },
        "total_lines": sum(p["lines"] for p in std_programs),
        "shared": dict(shared),
        "system_tools": dict(tools),
        "web_services": dict(services),
        "unclassified": len(other),
        "modules": {
            m: {
                "programs": sum(c.values()),
                "by_type": {TYPE_NAMES[k]: v for k, v in c.items()},
                "lines": lines_by_module[m],
                "tables": tables_by_module.get(m, 0),
                "columns": columns_by_module.get(m, 0),
            }
            for m, c in sorted(by_module.items(), key=lambda x: -sum(x[1].values()))
        },
        "custom_modules": dict(Counter(p["module"] for p in programs.values() if p["layer"] == "topcust")),
        "tables": len(catalog),
        "columns": sum(columns_by_module.values()),
        "largest": sorted(
            ({"program": p["program"], "module": p["module"], "type": TYPE_NAMES[p["type"]], "lines": p["lines"]}
             for p in std_programs), key=lambda x: -x["lines"])[:15],
        "overrides_list": overrides,
    }
    (out_dir / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"Standard main programs {summary['standard_programs']}, custom {summary['custom_programs']} "
          f"(overrides {summary['overrides']}, new {summary['custom_only']}), shared {sum(shared.values())}, "
          f"system tools {sum(tools.values())}, web services {sum(services.values())}, unclassified {len(other)}")
    print(f"Written to {out_dir.relative_to(ROOT)}/programs.tsv, summary.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
