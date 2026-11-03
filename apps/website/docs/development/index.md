---
title: Development overview
description: Build and validate Volum Desktop, mobile, website, documentation, and server.
---

# Development overview

Volum is organized as a monorepo with Desktop, mobile, `volumd`, server-administrator, and website applications under `apps/`. Reusable frontend source lives under `packages/`; transport-neutral Rust services live in `crates/volum-server/`. The root `Cargo.toml` and `Cargo.lock` own the Rust workspace.

The Desktop crate is a thin Tauri adapter over the transport-neutral `volum-server` crate. The standalone `apps/volumd/` binary uses that same core without compiling a GUI or credential-store runtime.

JavaScript applications and packages keep separate `package-lock.json` files. The root `Makefile` and `package.json` dispatch into those projects rather than treating the repository as one npm workspace.

## Requirements

- Node.js 22.23.4, pinned in `.nvmrc`;
- Rust 1.99.1, pinned in `rust-toolchain.toml`;
- Tauri 2 platform prerequisites;
- platform SDKs for the native target you build;
- Docker with Compose v2 for server-image work;
- Make for the documented repository-level commands.

Confirm the core tools available on your machine:

```bash
make doctor
```

## Desktop

```bash
make setup
make desktop-dev
```

`make setup` installs the locked dependencies for every JavaScript project. Cargo downloads Rust dependencies during the first Rust build.

Browser-only UI preview:

```bash
make desktop-web-dev
```

## Other applications

| Command | Purpose |
| --- | --- |
| `make mobile-dev` | Run the mobile browser preview |
| `make mobile-ios-dev` | Run the iOS application |
| `make mobile-android-dev` | Run the Android application |
| `make server-admin-dev` | Run the `volumd` browser administrator |
| `make volumd-run` | Run the headless server |

See [Mobile development](./mobile) and [Server development](./server) for native prerequisites and isolated test-server setup.

## Website and docs

Install website dependencies only:

```bash
make website-install
```

Run the landing page:

```bash
make website-dev
```

Run documentation:

```bash
make website-docs-dev
```

Build both into `apps/website/dist/`:

```bash
make website-build
```

## Validation

```bash
make check
make compose-check
```

Use `APP` for a targeted command, such as `make dev APP=mobile`, `make build APP=website`, or `make test APP=viewer`. Run `make help` for all targets.

## Shared API contract

`packages/companion-api/openapi.json` is the committed `/api/v1` contract. Regenerate its deterministic TypeScript output after a contract change and verify that the generated file and implemented server routes agree:

```bash
npm --prefix packages/companion-api run generate
npm --prefix packages/companion-api run generate:check
```

Do not edit `packages/companion-api/generated.ts` by hand. The complete validation suite runs the drift check automatically.

## Generated data and cleanup

Rust and native mobile builds can become large:

```bash
make disk-usage
make clean
```

`make clean` removes regenerable build output. `make clean-all` also removes `node_modules`; restore them with `make setup`. Cleanup preserves local environments, signing material, Goldie output, and saved Xcode archives. Docker cache cleanup is separate because it affects every local project.

Install the repository's Gitleaks pre-commit hook before contributing. Never commit `.env` files, device tokens, signing material, pairing payloads, or real private fixtures.
