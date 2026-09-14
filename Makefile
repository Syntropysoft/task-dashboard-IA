# Atajos del repo. No hay package.json: esto reemplaza al `prepare` de los repos Node de la familia.

.PHONY: hooks gate gate-node db-up db-down db-reset

# URLs del Postgres local (docker-compose.yml). Los tests crean una base efímera por test.
export TEST_DATABASE_URL ?= postgres://td:td@localhost:55432/task_dashboard
export DATABASE_URL      ?= postgres://td:td@localhost:55432/task_dashboard

# Cablea los hooks del repo en ESTE clon (core.hooksPath es local, no viaja). Correrlo una vez por clon.
hooks:
	git config core.hooksPath .githooks
	@echo "hooks cableados: $$(git config core.hooksPath)"

# Gate completo. Levanta el Postgres local si hace falta: los tests de base no se saltean.
gate: gate-node db-up
	cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test

db-up:
	docker compose up -d --wait postgres

db-down:
	docker compose down

# Borra las bases td_test_* que dejó un test fallido.
db-reset:
	@docker compose exec -T postgres psql -U td -d task_dashboard -Atc \
	  "select datname from pg_database where datname like 'td_test_%'" \
	  | xargs -I{} docker compose exec -T postgres psql -U td -d task_dashboard -c 'drop database "{}" with (force)'

gate-node:
	node scripts/chasis-check.mjs
	node scripts/contexto-check.mjs
	node scripts/docs-linkcheck.mjs
