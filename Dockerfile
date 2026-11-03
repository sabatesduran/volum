# syntax=docker/dockerfile:1.7
FROM node:22.23.4-bookworm-slim@sha256:4b150b48b4116e2a22f2c39cc27025fbf4f153e2078fd3b14d159fdb5c7279da AS web
WORKDIR /src
COPY packages/companion-api ./packages/companion-api
COPY packages/design ./packages/design
COPY apps/server-admin/package.json apps/server-admin/package-lock.json ./apps/server-admin/
RUN npm ci --prefix apps/server-admin
COPY apps/server-admin/index.html apps/server-admin/tsconfig.json apps/server-admin/vite.config.ts ./apps/server-admin/
COPY apps/server-admin/src ./apps/server-admin/src
COPY scripts/check-bundle-budgets.mjs ./scripts/check-bundle-budgets.mjs
RUN npm --prefix apps/server-admin run build \
    && npm --prefix apps/server-admin run budget

FROM rust:1.99.1-bookworm@sha256:9181b56c2a21ab1d5e73cffe04fc762426d30aa7fac3c91141dd033f49a7fc8b AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/build.rs ./apps/desktop/src-tauri/
COPY apps/desktop/src-tauri/src ./apps/desktop/src-tauri/src
COPY apps/mobile/src-tauri/Cargo.toml apps/mobile/src-tauri/build.rs ./apps/mobile/src-tauri/
COPY apps/mobile/src-tauri/src ./apps/mobile/src-tauri/src
COPY apps/volumd ./apps/volumd
COPY crates/tauri-plugin-app-locale ./crates/tauri-plugin-app-locale
COPY crates/volum-server ./crates/volum-server
RUN cargo build --locked --release -p volumd

FROM debian:bookworm-slim@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3 AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 --shell /usr/sbin/nologin volum \
    && mkdir -p /config /library \
    && chown -R volum:volum /config /library
COPY --from=builder /src/target/release/volumd /usr/local/bin/volumd
COPY --from=web /src/apps/server-admin/dist /usr/share/volum/web
USER volum
ENV VOLUM_CONFIG_DIR=/config \
    VOLUM_LIBRARY_DIR=/library \
    VOLUM_BIND=0.0.0.0:43821 \
    VOLUM_WEB_ENABLED=true \
    VOLUM_WEB_DIR=/usr/share/volum/web \
    VOLUM_ORGANIZATION_WRITES=true
VOLUME ["/config", "/library"]
EXPOSE 43821
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:43821/readyz || exit 1
ENTRYPOINT ["/usr/local/bin/volumd"]
