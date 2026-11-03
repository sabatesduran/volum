---
title: Server development
description: Build, run, and validate the volumd binary and container image.
---

# Server development

`volumd` is the standalone binary in `apps/volumd/`. It depends on the transport-neutral `crates/volum-server/` library for migrations, managed storage, indexing, parsing, authentication, recovery, and the Axum API. Its dependency graph intentionally excludes Tauri, Wry, WebKit, and GTK.

## Run locally

Create disposable configuration and library directories outside the repository, then run:

```bash
VOLUM_CONFIG_DIR=/path/to/test-config \
VOLUM_LIBRARY_DIR=/path/to/test-library \
VOLUM_BIND=127.0.0.1:43821 \
VOLUM_PUBLIC_URL=http://127.0.0.1:43821 \
make volumd-run
```

Use only disposable paths for development. A local Desktop client may connect to localhost. Mobile accepts Tailscale, another private IP/DNS route, or trusted HTTPS; it rejects public HTTP.

## Validate Rust

```bash
make volumd-check
make rust-check
```

The first command validates the headless feature set and asserts that no GUI runtime entered the daemon dependency tree. The second validates the complete root Cargo workspace, including Desktop and mobile adapters.

## Validate Compose

```bash
make compose-check
```

Build and start the image:

```bash
VOLUM_PUBLIC_URL=http://100.64.0.1:43821 \
VOLUM_PRINT_PAIRING=true \
docker compose up -d --build
```

Use isolated test volumes and remove them only when you are certain they contain no needed data.

## Multi-architecture publication

The server-image workflow publishes `linux/amd64` and `linux/arm64` manifests to GitHub Container Registry on release or manual dispatch. Validate both architectures and `/readyz` before describing a release as generally available. `/healthz` is only the process-liveness probe.
