.PHONY: test bdd bdd-pg api web desktop bundle check fmt db-create

# 本機 8080 常被占用，可用 make web WEB_PORT=xxxx 覆寫
WEB_PORT ?= 8765

test:
	cargo test --workspace

bdd:
	cargo test -p ttrs-bdd --test acceptance

bdd-pg:
	@test -n "$(TTRS_BDD_DATABASE_URL)" || (echo "請設定 TTRS_BDD_DATABASE_URL"; exit 1)
	cargo test -p ttrs-bdd --test acceptance

api:
	cargo run -p ttrs-api

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

# 建立開發與測試資料庫（需有建立 DB 權限的 PostgreSQL 角色）
db-create:
	createdb ttrs_dev
	createdb ttrs_test
