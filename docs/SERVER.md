# Volum server

Volum supports three configurations without changing the library format:

1. **Local Desktop** — the default. The UI calls the local Rust core directly.
2. **Desktop host** — enable Companion Access; Desktop serves the same authenticated API on its Tailscale address.
3. **Dedicated `volumd`** — run the core and API headlessly, normally in Docker on a NAS.

The machine running Desktop host or `volumd` is canonical. Never copy or synchronize `volum.sqlite3` between machines. Desktop mirrors contain original project files plus a private manifest, not a second database.

## Docker Compose

```sh
export VOLUM_PUBLIC_URL=http://volum-nas.your-tailnet.ts.net:43821
export VOLUM_PRINT_PAIRING=true
docker compose up -d --build
docker compose logs -f volumd
```

On first start, `volumd` creates:

- `/config/volum.sqlite3` and server authentication settings;
- `/library/Projects`, `/library/Inbox`, and `/library/Trash`;
- a five-minute pairing payload in the container log when `VOLUM_PRINT_PAIRING=true`. Outside Compose, an unset value automatically prints one while no device has been paired.

Paste that payload into **Volum Desktop → Settings → Connection**, or scan it with Volum Companion. Set `VOLUM_PRINT_PAIRING=false` after pairing; a printed pairing secret expires after five minutes and is single-use.

### Persistent storage

The included Compose file uses two named volumes. Bind mounts are also supported:

```yaml
volumes:
  - /volume1/docker/volum/config:/config
  - /volume1/3d/Volum:/library
```

Both locations must be writable by UID/GID `1000:1000`, or run the container with a NAS user that owns them. Dedicated `volumd` does not currently expose backup administration to remote clients. Stop the container and snapshot or copy both locations from the same recovery point. Do not expose or synchronize SQLite, WAL, or SHM files separately.

### Network security

The API always requires a paired bearer token except for `/identity`, `/pair`, and `/healthz`. The listener remains configurable for a private Tailnet, a trusted and firewalled LAN, or an HTTPS reverse proxy. Volum Desktop and Companion accept application-level plain HTTP only over Tailscale or localhost; use an HTTPS reverse proxy for an official client on a LAN without Tailscale.

1. Install Tailscale on the NAS or use a Tailscale sidecar.
2. Publish port `43821` only to the chosen Tailnet or trusted-LAN firewall zone.
3. Set `VOLUM_PUBLIC_URL` to the reachable private Tailscale origin, or to the reverse proxy's HTTPS origin for a LAN-only deployment.
4. Do not forward port `43821` from the public internet. If public routing is unavoidable, terminate HTTPS in a trusted reverse proxy and set `VOLUM_PUBLIC_URL` to its HTTPS origin.

## Environment

| Variable | Default | Purpose |
| --- | --- | --- |
| `VOLUM_CONFIG_DIR` | `/config` | Database and server configuration. |
| `VOLUM_LIBRARY_DIR` | `/library` | Managed canonical project files. |
| `VOLUM_BIND` | `0.0.0.0:43821` | Listener IP and port. |
| `VOLUM_PUBLIC_URL` | loopback-derived | Origin embedded in pairing payloads. Set this in containers. |
| `VOLUM_ORGANIZATION_WRITES` | `true` | Permit paired clients to edit organization and managed files. |
| `VOLUM_PRINT_PAIRING` | automatic when unset; `false` in Compose | Print a fresh pairing payload at startup. Enable it only while onboarding a device. |

## Multi-architecture image

The Dockerfile builds natively for both `linux/amd64` and `linux/arm64`. Publish a manifest with BuildKit:

```sh
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --tag ghcr.io/sabatesduran/volum-server:latest \
  --push .
```

## Managed mirror behavior

A remote Desktop can opt into a local mirror from **Settings → Connection**:

- originals download with resumable HTTP ranges;
- every completed file is verified against its BLAKE3 `ETag`;
- opening and revealing use the local copy, including while offline;
- a local edit is uploaded only if the server still has the revision originally downloaded;
- if both copies changed, the local file is moved under `Conflicts/` and the canonical server copy is downloaded;
- files deleted on the server are removed locally only when the local copy is unchanged; edited copies are preserved under `Conflicts/`.

The mirror never contains `volum.sqlite3`.
