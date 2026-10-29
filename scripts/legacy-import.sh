#!/usr/bin/env bash
# 匯入 TipTop 原始碼到 legacy/：
#   1. rsync 原始檔到 legacy/src/{tiptop,topcust}（只取原始碼與畫面/資源檔）
#   2. 轉成 UTF-8 到 legacy/utf8/（已是 UTF-8 的直接複製）
#   3. 產生 legacy/INDEX.tsv 與 legacy/import-report.txt
# 用法見 legacy/README.md 或 --help。可重複執行（增量更新）。
set -euo pipefail

usage() {
    cat <<'EOF'
用法: scripts/legacy-import.sh [--top 來源] [--cust 來源] [選項]

  --top  PATH      $TOP 標準程式根目錄（本機路徑或 user@host:/path）
  --cust PATH      $CUST 客製程式根目錄
  --dest DIR       輸出目錄（預設：專案的 legacy/）
  --encoding ENC   非 UTF-8 檔案的來源編碼（預設：BIG5-HKSCS）
  -h, --help       顯示說明

至少需指定 --top 或 --cust 其中之一。
EOF
}

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$ROOT/legacy"
TOP_SRC=""
CUST_SRC=""
ENCODING="BIG5-HKSCS"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --top) TOP_SRC="$2"; shift 2 ;;
        --cust) CUST_SRC="$2"; shift 2 ;;
        --dest) DEST="$2"; shift 2 ;;
        --encoding) ENCODING="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "未知參數：$1" >&2; usage >&2; exit 2 ;;
    esac
done

if [[ -z "$TOP_SRC" && -z "$CUST_SRC" ]]; then
    usage >&2
    exit 2
fi

# 只匯入原始碼、畫面、Genero 資源檔與資料表定義（.sch）；42m/42f/42r 等編譯產物一律排除
EXTS=(4gl 4fd per inc global 4ad 4st 4tb 4tm sch)
RSYNC_FILTER=(--include='*/')
for e in "${EXTS[@]}"; do RSYNC_FILTER+=(--include="*.$e"); done
RSYNC_FILTER+=(--exclude='*')

sync_layer() {
    local layer="$1" src="$2"
    [[ -z "$src" ]] && return 0
    echo "==> 同步 $layer：$src"
    mkdir -p "$DEST/src/$layer"
    rsync -a --delete --prune-empty-dirs "${RSYNC_FILTER[@]}" "${src%/}/" "$DEST/src/$layer/"
}

sync_layer tiptop "$TOP_SRC"
sync_layer topcust "$CUST_SRC"

echo "==> 轉碼為 UTF-8（來源編碼 $ENCODING）"
mkdir -p "$DEST/utf8"
passthrough=0 converted=0 skipped=0
lossy=()

while IFS= read -r -d '' f; do
    rel="${f#"$DEST/src/"}"
    out="$DEST/utf8/$rel"
    # rsync 保留來源修改時間，故以「時間完全相同」判斷未變更（不能用較新，來源可能被改回舊時間）
    if [[ -f "$out" && "$(stat -c %Y "$out")" == "$(stat -c %Y "$f")" ]]; then
        skipped=$((skipped + 1))
        continue
    fi
    mkdir -p "$(dirname "$out")"
    if iconv -f UTF-8 -t UTF-8 "$f" >/dev/null 2>&1; then
        cp "$f" "$out"
        passthrough=$((passthrough + 1))
    elif iconv -f "$ENCODING" -t UTF-8 "$f" >"$out" 2>/dev/null; then
        converted=$((converted + 1))
    else
        # 含無法對應的字元：略過該字元仍輸出，並列入報告人工確認
        iconv -c -f "$ENCODING" -t UTF-8 "$f" >"$out" 2>/dev/null || true
        lossy+=("$rel")
    fi
    touch -r "$f" "$out"
done < <(find "$DEST/src" -type f -print0)

# 刪除來源已不存在的 UTF-8 檔
removed=0
while IFS= read -r -d '' u; do
    rel="${u#"$DEST/utf8/"}"
    if [[ ! -f "$DEST/src/$rel" ]]; then
        rm -f "$u"
        removed=$((removed + 1))
    fi
done < <(find "$DEST/utf8" -type f -print0)
find "$DEST/utf8" -type d -empty -delete

echo "==> 產生 INDEX.tsv"
{
    printf 'program\text\tlayer\tmodule\tpath\n'
    find "$DEST/utf8" -type f | sed "s#^$DEST/utf8/##" | sort | while IFS= read -r rel; do
        layer="${rel%%/*}"
        rest="${rel#*/}"
        module="${rest%%/*}"
        name="$(basename "$rel")"
        printf '%s\t%s\t%s\t%s\tutf8/%s\n' "${name%.*}" "${name##*.}" "$layer" "$module" "$rel"
    done
} >"$DEST/INDEX.tsv"

echo "==> 產生 import-report.txt"
{
    echo "匯入時間：$(date '+%Y-%m-%d %H:%M:%S')"
    echo "TOP 來源：${TOP_SRC:-（未指定）}"
    echo "CUST 來源：${CUST_SRC:-（未指定）}"
    echo "來源編碼：$ENCODING"
    echo
    echo "UTF-8 直接複製：$passthrough"
    echo "轉碼完成：$converted"
    echo "未變更略過：$skipped"
    echo "移除過期檔：$removed"
    echo "含無法轉換字元：${#lossy[@]}"
    for l in "${lossy[@]}"; do echo "  - $l"; done
    echo
    echo "各層檔案數（layer / ext / count）："
    tail -n +2 "$DEST/INDEX.tsv" | awk -F'\t' '{c[$3" / "$2]++} END {for (k in c) printf "  %-20s %d\n", k, c[k]}' | sort
} >"$DEST/import-report.txt"

cat "$DEST/import-report.txt"
if [[ ${#lossy[@]} -gt 0 ]]; then
    echo
    echo "注意：有 ${#lossy[@]} 個檔案含無法轉換的字元，請檢查後考慮改用 --encoding 其他編碼（例如 CP950）。" >&2
fi
