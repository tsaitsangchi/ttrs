#!/usr/bin/env python3
"""TipTop Oracle 唯讀工具：查詢與匯出 TipTop 資料表定義、樣本資料，供萃取規格與資料轉換參考。

只讀不寫：
  1. 只接受單一 SELECT / WITH 敘述（拒絕 DML、DDL、PL/SQL 與多敘述）
  2. 連線後立即 SET TRANSACTION READ ONLY，資料庫層級拒絕任何寫入
  3. 從不 commit，結束時 rollback

連線設定（環境變數，或專案根目錄的 .env；密碼不會被印出）：
  TIPTOP_ORACLE_DSN       主機:埠/服務名，例 10.10.130.188:1521/toptest（tnsnames 的 GSMC_TOPTEST）
  TIPTOP_ORACLE_USER      帳號（建議 DBA 提供的唯讀帳號）
  TIPTOP_ORACLE_PASSWORD  密碼
  TIPTOP_ORACLE_SCHEMA    （選用）要查的 schema，預設同帳號；TipTop 各營運中心各一個 schema，如 ds、ds63

用法（在專案根目錄）：
  .venv-tiptop/bin/python scripts/tiptop/tiptop.py ping
  .venv-tiptop/bin/python scripts/tiptop/tiptop.py tables 'ZZ%'
  .venv-tiptop/bin/python scripts/tiptop/tiptop.py describe zz_file
  .venv-tiptop/bin/python scripts/tiptop/tiptop.py ddl azo_file            # 存到 legacy/schema/<schema>/azo_file.sql
  .venv-tiptop/bin/python scripts/tiptop/tiptop.py export zz_file --where "zz011 = 'AIM'" --limit 200
  .venv-tiptop/bin/python scripts/tiptop/tiptop.py query "SELECT zz01, zz03 FROM zz_file WHERE zz01 LIKE 'aim%'"

匯出檔放在 legacy/ 底下（不進版控）。
"""

from __future__ import annotations

import argparse
import csv
import os
import re
import sys
from pathlib import Path

import oracledb

ROOT = Path(__file__).resolve().parents[2]
IDENT = re.compile(r"^[A-Za-z][A-Za-z0-9_$#]{0,127}$")


class ReadOnlyViolation(Exception):
    pass


# ---------------------------------------------------------------------------
# 設定
# ---------------------------------------------------------------------------

def load_env() -> None:
    """讀取專案根目錄 .env（已設定的環境變數優先）。只處理 KEY=VALUE，不展開變數。"""
    path = ROOT / ".env"
    if not path.exists():
        return
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        os.environ.setdefault(key.strip(), value.strip().strip('"').strip("'"))


def setting(name: str, required: bool = True) -> str | None:
    value = os.environ.get(name)
    if required and not value:
        sys.exit(f"缺少設定 {name}（請在 .env 或環境變數設定，見 scripts/tiptop/tiptop.py 檔頭說明）")
    return value


def ident(name: str) -> str:
    if not IDENT.match(name):
        sys.exit(f"不合法的名稱：{name!r}")
    return name.upper()


# ---------------------------------------------------------------------------
# 唯讀連線
# ---------------------------------------------------------------------------

def check_select(sql: str) -> str:
    """只允許單一 SELECT / WITH 敘述。"""
    text = re.sub(r"/\*.*?\*/", " ", sql, flags=re.S)
    text = re.sub(r"--[^\n]*", " ", text).strip().rstrip(";").strip()
    if ";" in text:
        raise ReadOnlyViolation("只允許單一敘述")
    first = text.split(None, 1)[0].upper() if text else ""
    if first not in ("SELECT", "WITH"):
        raise ReadOnlyViolation(f"只允許 SELECT / WITH，收到：{first or '(空白)'}")
    return text


class TipTop:
    def __init__(self) -> None:
        load_env()
        self.conn = oracledb.connect(
            user=setting("TIPTOP_ORACLE_USER"),
            password=setting("TIPTOP_ORACLE_PASSWORD"),
            dsn=setting("TIPTOP_ORACLE_DSN"),
        )
        self.conn.autocommit = False
        with self.conn.cursor() as cur:
            cur.execute("SET TRANSACTION READ ONLY")
        self.schema = (setting("TIPTOP_ORACLE_SCHEMA", required=False) or self.conn.username).upper()

    def select(self, sql: str, params: dict | None = None) -> tuple[list[str], list[tuple]]:
        sql = check_select(sql)
        with self.conn.cursor() as cur:
            cur.execute(sql, params or {})
            cols = [d[0].lower() for d in cur.description]
            rows = [tuple(read_lob(v) for v in row) for row in cur.fetchall()]
        return cols, rows

    def close(self) -> None:
        self.conn.rollback()
        self.conn.close()


def read_lob(v):
    return v.read() if isinstance(v, oracledb.LOB) else v


# ---------------------------------------------------------------------------
# 輸出
# ---------------------------------------------------------------------------

def print_table(cols: list[str], rows: list[tuple]) -> None:
    text_rows = [["" if v is None else str(v) for v in r] for r in rows]
    widths = [max([disp_len(c)] + [disp_len(r[i]) for r in text_rows]) for i, c in enumerate(cols)]
    widths = [min(w, 60) for w in widths]

    def fmt(cells):
        return "  ".join(pad(cells[i][:60], widths[i]) for i in range(len(cols)))

    print(fmt(cols))
    print("  ".join("-" * w for w in widths))
    for r in text_rows:
        print(fmt(r))
    print(f"（{len(rows)} 筆）")


def disp_len(s: str) -> int:
    """中文字以 2 格寬計算，讓欄位對齊"""
    return sum(2 if ord(ch) > 0x2E7F else 1 for ch in s)


def pad(s: str, width: int) -> str:
    return s + " " * max(0, width - disp_len(s))


def write_csv(path: Path, cols: list[str], rows: list[tuple]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as f:
        w = csv.writer(f)
        w.writerow(cols)
        w.writerows(rows)
    print(f"已寫入 {path.relative_to(ROOT)}（{len(rows)} 筆）")


# ---------------------------------------------------------------------------
# 指令
# ---------------------------------------------------------------------------

def cmd_ping(tt: TipTop, _args) -> None:
    cols, rows = tt.select(
        """SELECT sys_context('USERENV', 'DB_NAME') AS db_name,
                  sys_context('USERENV', 'SESSION_USER') AS session_user,
                  (SELECT value FROM nls_database_parameters WHERE parameter = 'NLS_CHARACTERSET') AS charset
           FROM dual"""
    )
    print(f"Oracle {tt.conn.version}（thin 模式，唯讀交易）")
    for c, v in zip(cols, rows[0]):
        print(f"  {c}: {v}")
    print(f"  查詢 schema: {tt.schema}")


def cmd_tables(tt: TipTop, args) -> None:
    cols, rows = tt.select(
        """SELECT t.table_name, t.num_rows, c.comments
           FROM all_tables t
           LEFT JOIN all_tab_comments c ON c.owner = t.owner AND c.table_name = t.table_name
           WHERE t.owner = :owner AND t.table_name LIKE :pattern
           ORDER BY t.table_name""",
        {"owner": tt.schema, "pattern": args.pattern.upper()},
    )
    print_table(cols, rows)


def cmd_describe(tt: TipTop, args) -> None:
    table = ident(args.table)
    cols, rows = tt.select(
        """SELECT c.column_name,
                  c.data_type || CASE
                      WHEN c.data_type IN ('VARCHAR2', 'CHAR', 'NVARCHAR2') THEN '(' || c.char_length || ')'
                      WHEN c.data_type = 'NUMBER' AND c.data_precision IS NOT NULL
                          THEN '(' || c.data_precision || NVL2(NULLIF(c.data_scale, 0), ',' || c.data_scale, '') || ')'
                  END AS type,
                  DECODE(c.nullable, 'N', 'NOT NULL', '') AS nullable,
                  m.comments
           FROM all_tab_columns c
           LEFT JOIN all_col_comments m
             ON m.owner = c.owner AND m.table_name = c.table_name AND m.column_name = c.column_name
           WHERE c.owner = :owner AND c.table_name = :t
           ORDER BY c.column_id""",
        {"owner": tt.schema, "t": table},
    )
    if not rows:
        sys.exit(f"{tt.schema}.{table} 不存在或沒有權限")
    print_table(cols, rows)
    _, pk = tt.select(
        """SELECT LISTAGG(cc.column_name, ', ') WITHIN GROUP (ORDER BY cc.position)
           FROM all_constraints k
           JOIN all_cons_columns cc ON cc.owner = k.owner AND cc.constraint_name = k.constraint_name
           WHERE k.owner = :owner AND k.table_name = :t AND k.constraint_type = 'P'""",
        {"owner": tt.schema, "t": table},
    )
    if pk and pk[0][0]:
        print(f"主鍵：({pk[0][0]})")


def cmd_ddl(tt: TipTop, args) -> None:
    table = ident(args.table)
    _, rows = tt.select(
        "SELECT dbms_metadata.get_ddl('TABLE', :t, :owner) FROM dual",
        {"t": table, "owner": tt.schema},
    )
    out = ROOT / "legacy" / "schema" / tt.schema.lower() / f"{table.lower()}.sql"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(rows[0][0].strip() + "\n", encoding="utf-8")
    print(f"已寫入 {out.relative_to(ROOT)}")


def cmd_export(tt: TipTop, args) -> None:
    table = ident(args.table)
    sql = f"SELECT * FROM {tt.schema}.{table}"
    if args.where:
        sql += f" WHERE {args.where}"
    if args.order:
        sql += f" ORDER BY {args.order}"
    if args.limit:
        sql += f" FETCH FIRST {int(args.limit)} ROWS ONLY"
    cols, rows = tt.select(sql)
    out = Path(args.out) if args.out else ROOT / "legacy" / "data" / tt.schema.lower() / f"{table.lower()}.csv"
    write_csv(out, cols, rows)


def cmd_query(tt: TipTop, args) -> None:
    cols, rows = tt.select(args.sql)
    if args.csv:
        write_csv(Path(args.csv), cols, rows)
    else:
        print_table(cols, rows)


def main() -> None:
    p = argparse.ArgumentParser(description="TipTop Oracle 唯讀工具（詳見檔頭說明）")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("ping", help="測試連線並顯示資料庫資訊")
    s = sub.add_parser("tables", help="列出資料表與說明")
    s.add_argument("pattern", nargs="?", default="%", help="表名樣式（LIKE），例 'ZZ%%'")
    s = sub.add_parser("describe", help="欄位、型別、說明與主鍵")
    s.add_argument("table")
    s = sub.add_parser("ddl", help="以 DBMS_METADATA 匯出建表 DDL 到 legacy/schema/")
    s.add_argument("table")
    s = sub.add_parser("export", help="匯出資料為 CSV（預設 legacy/data/<schema>/<表>.csv）")
    s.add_argument("table")
    s.add_argument("--where", help="WHERE 條件（不含 WHERE 字）")
    s.add_argument("--order", help="ORDER BY 欄位")
    s.add_argument("--limit", type=int, default=1000, help="最多筆數，預設 1000；0 表示不限")
    s.add_argument("--out", help="輸出路徑")
    s = sub.add_parser("query", help="執行單一 SELECT")
    s.add_argument("sql")
    s.add_argument("--csv", help="輸出為 CSV 檔")
    args = p.parse_args()

    try:
        tt = TipTop()
    except oracledb.Error as e:
        sys.exit(f"連線失敗：{e}")
    try:
        {
            "ping": cmd_ping,
            "tables": cmd_tables,
            "describe": cmd_describe,
            "ddl": cmd_ddl,
            "export": cmd_export,
            "query": cmd_query,
        }[args.cmd](tt, args)
    except ReadOnlyViolation as e:
        sys.exit(f"拒絕執行：{e}")
    except oracledb.Error as e:
        sys.exit(f"Oracle 錯誤：{e}")
    finally:
        tt.close()


if __name__ == "__main__":
    main()
