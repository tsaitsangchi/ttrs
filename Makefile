.PHONY: test bdd bdd-pg api web desktop check fmt db-create

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
	cd crates/web && dx serve --platform web

desktop:
	cd crates/web && dx serve --platform desktop

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
