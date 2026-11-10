.PHONY: test bdd bdd-pg api api-memory web desktop bundle check fmt db-create legacy-catalog

# 本機 8080 常被占用，可用 make web WEB_PORT=xxxx 覆寫
WEB_PORT ?= 8765

test:
	cargo test --workspace

bdd:
	cargo test -p ttrs-bdd --test acceptance

bdd-pg:
	@test -n "$(TTRS_BDD_DATABASE_URL)" || (echo "請設定 TTRS_BDD_DATABASE_URL"; exit 1)
	cargo test -p ttrs-bdd --test acceptance

# 有 .env 時先載入（DATABASE_URL、TTRS_API_ADDR…），連開發資料庫；沒有 .env 時用記憶體資料。
# 要強制用記憶體資料：make api-memory
api:
	@if [ -f .env ]; then echo "載入 .env"; fi
	set -a; if [ -f .env ]; then . ./.env; fi; set +a; cargo run -p ttrs-api

api-memory:
	env -u DATABASE_URL cargo run -p ttrs-api

web:
	cd crates/web && dx serve --platform web --port $(WEB_PORT)

desktop:
	cd crates/web && dx serve --platform desktop

# 正式版前端靜態檔，輸出至 target/dx/ttrs-web/release/web/public（交給 nginx）
bundle:
	cd crates/web && dx bundle --platform web --release

fmt:
	cargo fmt --all

check:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace
	python3 scripts/tiptop/legacy_map_check.py

# 建立開發與測試資料庫（需有建立 DB 權限的 PostgreSQL 角色）
db-create:
	createdb ttrs
	createdb ttrs_test

# TipTop catalog (legacy_catalog.feature): merge the .sch catalog with the naming file and import into TTRS
# (API must be running; set TTRS_API_URL for another address)
legacy-catalog:
	python3 scripts/tiptop/sch_catalog.py build
	python3 scripts/tiptop/import_catalog.py
