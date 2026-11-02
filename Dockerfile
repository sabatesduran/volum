# syntax=docker/dockerfile:1.7
FROM node:22-bookworm-slim AS web
WORKDIR /src
COPY package.json package-lock.json ./
RUN npm ci
COPY index.html tsconfig.json vite.config.ts ./
COPY src ./src
COPY shared ./shared
COPY server-web ./server-web
RUN npm run build && npm run webapp:build

FROM rust:1-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    libayatana-appindicator3-dev \
    libgtk-3-dev \
    libssl-dev \
    libwebkit2gtk-4.1-dev \
    librsvg2-dev \
    patchelf \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY THIRD_PARTY_NOTICES.md ./THIRD_PARTY_NOTICES.md
COPY src/assets/volum-icon-transparent.png ./src/assets/volum-icon-transparent.png
COPY src-tauri ./src-tauri
COPY --from=web /src/dist ./dist
RUN cargo build --locked --release --manifest-path src-tauri/Cargo.toml --bin volumd

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    libayatana-appindicator3-1 \
    libgtk-3-0 \
    libssl3 \
    libwebkit2gtk-4.1-0 \
    librsvg2-2 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 1000 --shell /usr/sbin/nologin volum \
    && mkdir -p /config /library \
    && chown -R volum:volum /config /library
COPY --from=builder /src/src-tauri/target/release/volumd /usr/local/bin/volumd
COPY --from=web /src/server-web-dist /usr/share/volum/web
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
    CMD curl --fail --silent http://127.0.0.1:43821/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/volumd"]
