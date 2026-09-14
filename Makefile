# Atajos del repo. No hay package.json: esto reemplaza al `prepare` de los repos Node de la familia.

.PHONY: hooks gate gate-node

# Cablea los hooks del repo en ESTE clon (core.hooksPath es local, no viaja). Correrlo una vez por clon.
hooks:
	git config core.hooksPath .githooks
	@echo "hooks cableados: $$(git config core.hooksPath)"

# Gate completo. Mientras no exista Cargo.toml, corre solo la parte de Node y lo dice.
gate: gate-node
	@if [ -f Cargo.toml ]; then \
	  cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test; \
	else \
	  echo "⚠️  sin Cargo.toml todavía: clippy y test NO corrieron (paso 1 del plan)"; \
	fi

gate-node:
	node scripts/chasis-check.mjs
	node scripts/contexto-check.mjs
	node scripts/docs-linkcheck.mjs
