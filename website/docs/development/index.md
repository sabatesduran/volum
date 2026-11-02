---
title: Development overview
description: Build and validate Volum Desktop, mobile, website, documentation, and server.
---

# Development overview

Volum contains a Desktop Tauri application, a mobile Tauri application, the `volumd` binary, and the static product and documentation website.

## Requirements

- Node.js 20 or newer;
- stable Rust;
- Tauri 2 platform prerequisites;
- platform SDKs for the native target you build;
- Docker with Compose v2 for server-image work.

## Desktop

```bash
npm install
npm run tauri dev
```

Browser-only UI preview:

```bash
npm run dev
```

## Website and docs

Install website dependencies:

```bash
npm --prefix website install
```

Run the landing page:

```bash
npm run website:dev
```

Run documentation:

```bash
npm run website:docs:dev
```

Build both into `website/dist/`:

```bash
npm run website:build
```

## Validation

```bash
npm run build
npm test -- --run
npm --prefix mobile run build
npm --prefix mobile test
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run website:build
```

Install the repository's Gitleaks pre-commit hook before contributing. Never commit `.env` files, device tokens, signing material, pairing payloads, or real private fixtures.
