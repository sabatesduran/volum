---
title: Architecture
description: Understand Volum's local, Desktop-hosted, and dedicated server architecture.
---

# Architecture

Volum uses the same Rust library core for local Desktop work, Desktop-hosted access, and the dedicated `volumd` process. React clients never receive unrestricted filesystem access.

## Deployment modes

```text
Local Desktop UI ── direct Tauri commands ── Rust core
                                              ├── SQLite
                                              └── local or linked files

Remote Desktop ─┐
                ├── authenticated /api/v1 ── Desktop host or volumd
Mobile app ─────┘                                ├── canonical SQLite
                                                └── canonical managed files
```

The hosting machine is canonical. Remote clients send authenticated commands to it and receive purpose-built network records that omit canonical filesystem paths.

## Desktop boundary

React owns presentation and transient interaction state. Rust owns:

- filesystem access and path validation;
- SQLite and migrations;
- indexing and filesystem watching;
- archive and geometry parsing;
- previews and viewer meshes;
- native open, reveal, Trash, and application actions;
- remote transport and credential retrieval.

The webview calls narrow typed commands using IDs. It has no general filesystem or shell permission.

## Local versus remote Desktop

In local mode, the Desktop UI calls the Rust core directly. Enabling Companion Access does not force the host UI through HTTP.

In remote mode, the same UI adapter sends supported operations through native Rust HTTP transport. The bearer token stays in the operating-system credential store and never enters the webview.

Unsupported remote commands fail closed. They do not mutate the client's previous local database.

## Companion server

The Axum API exposes a versioned `/api/v1` contract with separate network DTOs. Binary routes resolve assets by ID and revalidate paths against canonical roots. There is no arbitrary file-serving route.

Pairing and identity routes are outside `/api/v1`; library routes require a paired bearer token.

## Mobile boundary

The mobile React UI uses a Tauri Rust transport. That transport accepts trusted HTTPS and HTTP only for private IPs, link-local/Tailscale ranges, or hostnames that resolve entirely to private addresses. It disables redirects and returns typed JSON or bounded binary data.

After a QR scan, Tailscale and HTTPS payloads attempt identity verification and pairing automatically. If that attempt fails, the user can review or edit the advertised origin and retry; private HTTP always requires explicit confirmation that the LAN or VPN is trusted. The client verifies `/identity` before sending a one-time secret. Native Rust stores the device token in an AES-256-GCM vault whose key is protected by iOS Keychain or Android Keystore; the vault key never enters JavaScript. Sanitized snapshots and media caches are stored separately for offline use.

## Dedicated server

`volumd` starts the same migrations, managed storage, indexer, watchers, and HTTP API without a Tauri window. Container deployments separate persistent `/config` and `/library` storage from ephemeral `/tmp` work.

The shared implementation is owned by `crates/volum-server/`. It has no Tauri, keyring, or Desktop feature. `apps/desktop/src-tauri/` supplies the Tauri window, command wrappers, backups, menu bar, native events, and credential-store adapter; `apps/volumd/` supplies only the daemon entry point. The headless dependency tree contains no Tauri, Wry, WebKit, GTK, keyring, D-Bus, Zbus, or secret-service packages.

## Shared frontend contracts

- `@volum/companion-api` owns the committed OpenAPI document, deterministic generated types, normalized errors, and injected transports used by Desktop, mobile, and the browser administrator.
- `@volum/viewer` owns binary mesh parsing, materials, normalization, bounds, and camera-facing geometry helpers; each application keeps only its platform shell.
- `@volum/shared` owns QR, localization, formatting, promise-cache, and LRU policies.
- `@volum/design` owns semantic color/type tokens, dark-mode behavior, reduced-motion policy, and portable primitives.
- `@volum/photo-room` owns the shared media editor.

Production builds isolate Three.js, viewer, media, Photo Room, React, Query, state, and icon runtimes into named chunks. Manifest-derived checks budget initial raw/gzip/brotli JavaScript separately from total and lazy JavaScript.

## Managed mirror

A remote Desktop mirror stores verified originals and a private manifest. It never synchronizes SQLite. BLAKE3 ETags and revision preconditions prevent silent overwrites; divergent local files are preserved under `Conflicts/`.

## Canonical-host migration

Moving a local library to `volumd` is a staged ownership transfer, not ongoing database synchronization. Desktop creates a consistent SQLite snapshot, maps every available linked or managed original into one managed NAS layout, and uploads resumable chunks. The NAS verifies checksums, relationships, counts, and safe paths before activation.

Activation preserves `volumd` authentication and server identity outside the imported library database. After cutover, Desktop retires its former local library and communicates with the NAS through the authenticated API. See [Move a Desktop library to a NAS](../desktop/move-library-to-nas).

## Data ownership

| Data | Local Desktop | Remote Desktop or mobile |
| --- | --- | --- |
| Projects and organization | Local SQLite | Canonical server SQLite |
| Original project files | Linked or managed local roots | Canonical server library |
| Theme and local UI preferences | Local | Local to each client |
| Remote bearer token | Not applicable | Native secure credential storage |
| Offline copies | Not applicable | Explicit mobile cache or Desktop mirror |
