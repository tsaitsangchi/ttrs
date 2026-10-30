#!/usr/bin/env python3
"""Import the TipTop catalog into TTRS (make legacy-catalog).

Spec: specs/features/azz/legacy_catalog.feature (C2, C3)
- Reads the .sch catalog (legacy/catalog/tables.json; build it first with sch_catalog.py build)
  and the naming files (docs/legacy-map/tiptop-naming/*.yaml).
- Merges them and POSTs to the TTRS API /api/legacy-catalog/import (a full replace each time).
- TTRS itself never connects to the TipTop database; this script only reads local files and calls the TTRS API.

Usage:
  python3 scripts/tiptop/import_catalog.py            # API address from TTRS_API_URL, default http://127.0.0.1:3000
  python3 scripts/tiptop/import_catalog.py --dry-run  # Merge and check only; print counts, do not import
"""

import argparse
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]


def load_naming() -> dict:
    naming: dict = {}
    for f in sorted((ROOT / "docs/legacy-map/tiptop-naming").glob("*.yaml")):
        naming.update(yaml.safe_load(f.read_text(encoding="utf-8")) or {})
    return naming


def build_payload() -> list[dict]:
    catalog_file = ROOT / "legacy/catalog/tables.json"
    if not catalog_file.exists():
        sys.exit("legacy/catalog/tables.json not found; run python3 scripts/tiptop/sch_catalog.py build first")
    catalog = json.loads(catalog_file.read_text(encoding="utf-8"))
    naming = load_naming()
    missing = sorted(set(catalog) - set(naming))
    if missing:
        sys.exit(f"Tables missing from the naming file: {', '.join(missing)} (add them to docs/legacy-map/tiptop-naming/)")
    payload = []
    for legacy, meta in sorted(catalog.items()):
        n = naming[legacy]
        cols = n.get("columns") or {}
        verify = set(n.get("verify") or [])
        pk = set(meta.get("primary_key") or [])
        columns = []
        for c in meta["columns"]:
            if c["name"] not in cols:
                sys.exit(f"{legacy}.{c['name']} has no English name in the naming file")
            columns.append({
                "legacy_name": c["name"],
                "name": cols[c["name"]],
                "data_type": c["type"],
                "primary_key": c["name"] in pk,
                "description": c["description"],
                "notes": " / ".join(c["notes"]) or None,
                "unused": bool(c["no_use"]) or cols[c["name"]].startswith("unused_"),
                "verify": c["name"] in verify,
            })
        payload.append({
            "legacy_name": legacy,
            "name": n["name"],
            "title": meta["title"],
            "purpose": meta.get("purpose") or None,
            "module": meta["module"],
            "columns": columns,
        })
    return payload


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    payload = build_payload()
    columns = sum(len(t["columns"]) for t in payload)
    if args.dry_run:
        print(f"Merge OK: {len(payload)} tables, {columns} columns (not imported)")
        return 0
    base = os.environ.get("TTRS_API_URL", "http://127.0.0.1:3000").rstrip("/")
    req = urllib.request.Request(
        f"{base}/api/legacy-catalog/import",
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers={"content-type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as res:
            result = json.loads(res.read())
    except urllib.error.HTTPError as e:
        print(f"Import failed (HTTP {e.code}): {e.read().decode('utf-8', 'replace')}", file=sys.stderr)
        return 1
    except urllib.error.URLError as e:
        print(f"Cannot reach the TTRS API at {base}: {e.reason} (start it with make api first)", file=sys.stderr)
        return 1
    print(f"Import done: {result['tables']} tables, {result['columns']} columns → {base}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
