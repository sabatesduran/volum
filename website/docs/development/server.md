---
title: Server development
description: Build, run, and validate the volumd binary and container image.
---

# Server development

`volumd` is a binary target in the Desktop Rust crate. It reuses migrations, managed storage, indexing, parsing, authentication, and the Axum API.

## Run locally

Create disposable configuration and library directories outside the repository, then run:

```bash
VOLUM_CONFIG_DIR=/path/to/test-config \
VOLUM_LIBRARY_DIR=/path/to/test-library \
VOLUM_BIND=127.0.0.1:43821 \
VOLUM_PUBLIC_URL=http://127.0.0.1:43821 \
cargo run --manifest-path src-tauri/Cargo.toml --bin volumd
```

Use only disposable paths for development. A local Desktop client may connect to localhost. Mobile accepts Tailscale, another private IP/DNS route, or trusted HTTPS; it rejects public HTTP.

## Validate Rust

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## Validate Compose

```bash
VOLUM_PUBLIC_URL=http://100.64.0.1:43821 docker compose config -q
```

Build and start the image:

```bash
VOLUM_PUBLIC_URL=http://100.64.0.1:43821 \
VOLUM_PRINT_PAIRING=true \
docker compose up -d --build
```

Use isolated test volumes and remove them only when you are certain they contain no needed data.

## Multi-architecture publication

The server-image workflow publishes `linux/amd64` and `linux/arm64` manifests to GitHub Container Registry on release or manual dispatch. Validate both architectures and `/healthz` before describing a release as generally available.
