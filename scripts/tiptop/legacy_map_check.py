#!/usr/bin/env python3
"""legacy-map 完整性檢查：docs/legacy-map/*.yaml 已對應的 TipTop 表，每個 .sch 欄位都要列出。

規則（CLAUDE.md 開發流程「萃取」、「對照」）：
  - 表的 `columns`（有對應的新欄位）或 `not_ported`（不移植，附原因）必須涵蓋該表全部 .sch 欄位
  - `target` 以「(」開頭（如 "(不移植)"）是整張表的決定，不逐欄檢查，但要有 note 或 reason
  - `pending: "原因"` 表示尚未逐欄萃取（例如程式還沒正式移植），只列為提醒、不算失敗
  - 有對應新表的項目要標 `scope`：system（系統層）、group（集團層）、plant（工廠層）（docs/adr/0007 §2）
  - 欄位清單取自 legacy/catalog/tables.json（scripts/tiptop/sch_catalog.py build；客製 topcust 已覆蓋標準）

legacy/ 不進版控：沒有 legacy/catalog/tables.json 時略過檢查（exit 0）。需要 PyYAML。

用法（在專案根目錄）：
  python3 scripts/tiptop/legacy_map_check.py
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CATALOG = ROOT / "legacy" / "catalog" / "tables.json"
MAPS = ROOT / "docs" / "legacy-map"
TOKEN = re.compile(r"[a-z][a-z0-9_]*")
SCOPES = ("system", "group", "plant")


def entry_text(entry: object) -> str:
    """一列對照的 legacy 欄位文字（可能一列寫多個欄位，如 "zz02 / zz02e"）"""
    if isinstance(entry, dict):
        return str(entry.get("legacy", ""))
    return str(entry)


def main() -> int:
    if not CATALOG.exists():
        print("略過 legacy-map 檢查：沒有 legacy/catalog/tables.json（先執行 sch_catalog.py build）")
        return 0
    try:
        import yaml
    except ImportError:
        print("legacy-map 檢查需要 PyYAML：pip install pyyaml", file=sys.stderr)
        return 1

    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    failures = 0
    for path in sorted(MAPS.glob("*.yaml")):
        doc = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
        for table in doc.get("tables", []):
            if not isinstance(table, dict):
                continue
            legacy = str(table.get("legacy", ""))
            where = f"{path.name} {legacy}"
            if legacy not in catalog:
                print(f"提醒 {where}：.sch 目錄沒有此表")
                continue
            if str(table.get("target", "")).startswith("("):
                if not (table.get("note") or table.get("reason")):
                    print(f"失敗 {where}：整表的決定需要 note 或 reason")
                    failures += 1
                continue
            if table.get("scope") not in SCOPES:
                print(f"失敗 {where}：scope 須為 {'／'.join(SCOPES)}")
                failures += 1
            if table.get("pending"):
                print(f"提醒 {where}：尚未逐欄萃取（{table['pending']}）")
                continue
            columns = [c["name"] for c in catalog[legacy]["columns"]]
            known = set(columns)
            listed: set[str] = set()
            for section in ("columns", "not_ported"):
                for entry in table.get(section) or []:
                    listed.update(t for t in TOKEN.findall(entry_text(entry)) if t in known)
            missing = [c for c in columns if c not in listed]
            if missing:
                print(f"失敗 {where}：{len(missing)} 個 .sch 欄位沒有列出：{' '.join(missing)}")
                failures += 1
    if failures:
        print(f"legacy-map 檢查失敗：{failures} 張表欄位不完整", file=sys.stderr)
        return 1
    print("legacy-map 檢查通過")
    return 0


if __name__ == "__main__":
    sys.exit(main())
