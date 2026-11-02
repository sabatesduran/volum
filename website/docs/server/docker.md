---
title: Install with Docker
description: Deploy volumd with Docker Compose and pair the first client.
---

# Install with Docker

The repository includes a multi-stage Dockerfile and a hardened Compose service for `linux/amd64` and `linux/arm64`.

## 1. Prepare Docker and a client route

Install Docker Engine with Compose v2 on the NAS or server. Tailscale is the recommended route: install it on the host, sign in, and note either:

- its Tailnet IPv4 address, such as `100.64.0.10`; or
- its MagicDNS name, such as `volum-nas.example.ts.net`.

Alternatively, prepare a private LAN, WireGuard, Twingate, or ZeroTier route, or a trusted HTTPS reverse proxy. Verify that a client can reach the selected origin before starting Volum. Public HTTP is not supported.

## 2. Get the deployment files

For early access, cloning the repository keeps the Dockerfile and Compose file together:

```bash
git clone https://github.com/sabatesduran/volum.git
cd volum
```

Check out the release tag you intend to run rather than an arbitrary development branch when a tagged server release is available.

## 3. Create the private environment file

Create `.env` beside `compose.yaml`:

```dotenv
VOLUM_PUBLIC_URL=http://volum-nas.example.ts.net:43821
VOLUM_ADMIN_PASSWORD=
VOLUM_PRINT_PAIRING=false
VOLUM_ORGANIZATION_WRITES=true
```

Fill the blank password value with a private value of at least 12 characters before starting Compose.

`VOLUM_PUBLIC_URL` must be the complete origin that clients can reach. Do not use `0.0.0.0`, a container-only hostname, a path, or embedded credentials.

Keep `.env` private. On first start, Volum saves only an Argon2id password hash under `/config`. You may remove `VOLUM_ADMIN_PASSWORD` from the environment after that configuration has persisted. Supplying a different value later resets the password and revokes existing administrator-browser sessions.

## 4. Start the server

Build from the checked-out source:

```bash
docker compose up -d --build
```

When a published image is available for the selected release, you can pull it instead:

```bash
docker compose pull
docker compose up -d --no-build
```

The published image name is `ghcr.io/sabatesduran/volum-server`.

## 5. Verify health

Run the health check on the server host:

```bash
curl --fail http://127.0.0.1:43821/healthz
```

The response should be:

```text
ok
```

Inspect startup logs:

```bash
docker compose logs --tail=100 volumd
```

On first startup, `volumd` creates:

- `/config/volum.sqlite3` and server authentication configuration;
- `/library/Projects`, `/library/Inbox`, and `/library/Trash`;
- the Volum web administrator at `VOLUM_PUBLIC_URL`.

## 6. Pair the first client

1. Open `VOLUM_PUBLIC_URL` in a browser.
2. Sign in with `VOLUM_ADMIN_PASSWORD`.
3. Select **Pair device**.
4. Scan the displayed QR code in Volum on mobile, or copy the pairing data into **Desktop → Settings → Connection**.
5. Confirm that the new device appears under **Paired devices**.

The QR code expires after five minutes and works once. The administrator can revoke any device immediately from the same page.

## 7. Emergency log pairing

If the web administrator is unavailable, temporarily enable the original log-based flow:

```dotenv
VOLUM_PRINT_PAIRING=true
```

Restart the container and copy the complete payload from its log. Disable the option immediately afterward:

```bash
docker compose up -d
docker compose logs --tail=100 volumd
```

```dotenv
VOLUM_PRINT_PAIRING=false
```

## 8. Confirm persistence

The default Compose file uses:

- `volum-config` for `/config`;
- `volum-library` for `/library`.

Restart the container and confirm the paired client still sees the same server identity and projects:

```bash
docker compose restart volumd
```

Continue with [Networking](./networking) and [Storage and backups](./storage-and-backups) before importing important files.
