---
title: Server environment
description: Reference environment variables used by volumd.
---

# Server environment

`volumd` reads its deployment configuration from environment variables.

| Variable | Container default | Purpose |
| --- | --- | --- |
| `VOLUM_CONFIG_DIR` | `/config` | SQLite and server configuration directory |
| `VOLUM_LIBRARY_DIR` | `/library` | Canonical managed project library |
| `VOLUM_BIND` | `0.0.0.0:43821` | Listener address inside the container |
| `VOLUM_PUBLIC_URL` | Loopback-derived outside Compose | Reachable origin embedded in pairing payloads; required by the supplied Compose file |
| `VOLUM_ORGANIZATION_WRITES` | `true` | Permit paired clients to edit organization and managed files |
| `VOLUM_PHOTO_UPLOADS` | `true` | Permit paired clients to add project photos independently of organization edits |
| `VOLUM_PRINT_PAIRING` | `false` | Print a fresh five-minute pairing payload at startup only when explicitly enabled |
| `VOLUM_WEB_ENABLED` | `true` | Serve the browser administrator and enable its authentication routes |
| `VOLUM_WEB_DIR` | `/usr/share/volum/web` | Compiled browser assets inside the container |
| `VOLUM_ADMIN_PASSWORD` | Unset | Bootstrap or reset the browser administrator password; minimum 12 characters |

## `VOLUM_PUBLIC_URL`

Use a complete `http://` or `https://` origin with no path, query, fragment, credentials, or trailing application route.

Valid examples:

```dotenv
VOLUM_PUBLIC_URL=http://100.64.0.10:43821
VOLUM_PUBLIC_URL=http://volum-nas.example.ts.net:43821
VOLUM_PUBLIC_URL=https://volum.example.net
```

`0.0.0.0` is a bind address, not a client destination.

## Pairing output

Use the web administrator's **Pair device** action for normal onboarding. Set `VOLUM_PRINT_PAIRING=true` only for emergency log-based pairing. The printed payload contains a temporary secret and should be treated as sensitive until it expires or is consumed.

After pairing:

```dotenv
VOLUM_PRINT_PAIRING=false
```

## Web administrator

On the first start, set `VOLUM_ADMIN_PASSWORD` to at least 12 characters. Volum hashes it with Argon2id and stores only the hash under `/config`. Once that file is safely persisted, the clear-text environment value can be removed.

Supplying a different password later is treated as an operator-authorized reset. Existing administrator-browser tokens are revoked. Paired mobile and Desktop clients are not affected.

## Write policy

Set `VOLUM_ORGANIZATION_WRITES=false` to block organization changes and 3D-file imports. Set `VOLUM_PHOTO_UPLOADS=false` to block project-photo uploads independently. The server enforces both policies even if a client interface displays an unavailable action.
