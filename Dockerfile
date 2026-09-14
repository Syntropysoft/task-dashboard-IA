# task-dashboard-api — imagen de deploy (Railway y cualquier runtime de contenedores).
# Objetivo: binario estático chico, sin runtime, arranque en ms. Ver docs/PLAN-PASO-1.md.

# ── build ─────────────────────────────────────────────────────────────────────────────────────
FROM rust:1-slim AS build
# El target sigue a la plataforma que construye: Railway es amd64, un Mac Apple Silicon es arm64.
# Fijarlo a x86_64 a mano falló en local con "cc: unrecognized command-line option '-m64'" (2026-09-13).
ARG TARGETARCH
RUN case "$TARGETARCH" in \
      amd64) echo x86_64-unknown-linux-musl  > /target ;; \
      arm64) echo aarch64-unknown-linux-musl > /target ;; \
      *) echo "TARGETARCH no soportado: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && apt-get update && apt-get install -y --no-install-recommends musl-tools \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add "$(cat /target)"
WORKDIR /src

# Capa de dependencias: se invalida solo cuando cambian los Cargo.*, no el código.
COPY Cargo.toml Cargo.lock ./
COPY apps/api/Cargo.toml apps/api/Cargo.toml
RUN mkdir -p apps/api/src && echo 'fn main() {}' > apps/api/src/main.rs && touch apps/api/src/lib.rs \
    && cargo build --release --target "$(cat /target)" -p task-dashboard-api \
    && rm -rf apps/api/src

COPY apps apps
COPY db db
# touch: cargo decide por mtime y el COPY puede dejar los .rs más viejos que el stub compilado.
RUN touch apps/api/src/main.rs apps/api/src/lib.rs \
    && cargo build --release --target "$(cat /target)" -p task-dashboard-api \
    && cp "target/$(cat /target)/release/task-dashboard-api" /task-dashboard-api

# ── runtime ───────────────────────────────────────────────────────────────────────────────────
# Binario musl estático: no necesita libc ni certificados propios todavía (el JWKS de syntroAuth
# llegará en 3a y traerá ca-certificates a esta capa).
FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /task-dashboard-api /task-dashboard-api
# DATABASE_URL es obligatoria (la inyecta el servicio Postgres de Railway): sin base no arranca.
ENV PORT=8080 RUST_LOG=info
EXPOSE 8080
ENTRYPOINT ["/task-dashboard-api"]
