---
title: Self-host Volum
description: Run an always-on canonical Volum library with volumd.
---

# Self-host Volum

`volumd` runs the Volum core, authenticated API, and a small browser administrator. It is intended for a NAS or home server that should own an always-on managed library.

::: warning Early access
The server image and remote workflows are in early access. Test with a non-critical library first, keep independent backups, and read the production-readiness notes for your release.
:::

## What the server owns

A dedicated server is canonical for:

- `volum.sqlite3` and all organization metadata;
- managed project files under `/library`;
- indexing, parsing, thumbnails, and viewer meshes;
- pairing, device tokens, and API permissions.

Clients do not copy or merge the database. Volum Desktop can keep an optional mirror of verified original files, but that mirror is not another library database.

## Recommended topology

```text
Volum Desktop ─┐
               ├── private network or HTTPS ── volumd on NAS
Volum mobile ──┘                              ├── /config
                                              └── /library
```

Install Tailscale on the NAS host, provide another private LAN/VPN route, or terminate trusted HTTPS at a reverse proxy. Publish plain port `43821` only to trusted private clients.

## Before installing

Prepare:

- Docker Engine with Compose v2;
- a Tailscale, private LAN/VPN, or HTTPS origin clients can reach;
- persistent storage for configuration and managed files;
- a backup method that snapshots both storage locations together;
- enough CPU, memory, and temporary space for geometry parsing.

## Install path

1. [Install with Docker](./docker).
2. Follow the [UGREEN NAS guide](./ugreen) when using UGOS.
3. [Restrict network access](./networking).
4. [Choose persistent storage and backups](./storage-and-backups).
5. Pair a Desktop or mobile device from the web administrator.
6. If Desktop already owns your projects, [move that library to the NAS](../desktop/move-library-to-nas).
7. Learn the [update and recovery](./update-and-recover) procedure before importing the full library.

## Client choices

Use **Volum Desktop → Settings → Connection** for the full remote interface and optional local mirror. Use Volum on mobile for workshop browsing, approved edits, uploads, and offline selections.

Host-only filesystem administration and backup recovery stay outside remote clients.
