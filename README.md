# TTRS

TipTop ERP 移植專案：Rust 全端（Axum + Dioxus）、PostgreSQL、BDD 規格驅動。
開發規則與流程見 [CLAUDE.md](CLAUDE.md)，架構決策見 [docs/adr](docs/adr)。

## 目錄

| 路徑 | 說明 |
|---|---|
| `specs/features/` | Gherkin 規格（依 TipTop 模組分資料夾） |
| `crates/domain` | 業務規則 |
| `crates/app` | use case 與 repository 介面 |
| `crates/infra` | PostgreSQL 實作 |
| `crates/api` | Axum HTTP API |
| `crates/web` | Dioxus 前端（Web / 桌面 / 行動） |
| `crates/bdd` | 驗收測試 |
| `migrations/` | SQLx migration |
| `docs/legacy-map/` | TipTop 舊表欄位對照 |
| `legacy/` | （不進版控）放 TipTop 4GL 原始碼、DDL 供 AI 萃取規格 |

## 快速開始

```bash
make test                     # 不需資料庫即可跑全部測試
make api                      # 啟動 API，http://127.0.0.1:3000
cargo install dioxus-cli      # 第一次需要
rustup target add wasm32-unknown-unknown
make web                      # 啟動前端 http://127.0.0.1:8765（/api 由 dx proxy 轉給 API）
```

### 使用 PostgreSQL

```bash
# 以 postgres 管理者建立角色與資料庫
sudo -u postgres createuser -d ttrs -P      # 密碼設為 ttrs
createdb -U ttrs -h localhost ttrs        # 開發用（DATABASE_URL）
createdb -U ttrs -h localhost ttrs_test   # BDD 用，每個場景會清空資料表，勿與 ttrs 共用
cp .env.example .env && set -a && . ./.env && set +a
make bdd-pg                   # 同一份規格對 PostgreSQL 驗證
make api                      # 啟動時自動執行 migration
```

### 桌面版

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev
make desktop
```

### 正式部署（nginx）

```bash
make bundle                                    # 產出前端靜態檔
sudo cp -r target/dx/ttrs-web/release/web/public/. /srv/ttrs/web/
sudo cp deploy/nginx/ttrs.conf /etc/nginx/conf.d/ # 先改網域、憑證路徑
sudo nginx -t && sudo systemctl reload nginx
```

架構說明見 [ADR 0002](docs/adr/0002-deployment-nginx.md)。
