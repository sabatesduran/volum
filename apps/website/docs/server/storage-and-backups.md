---
title: Storage and backups
description: Persist and protect the volumd database and managed project library.
---

# Storage and backups

A useful server backup must capture both the database and the managed project files from the same recovery point.

## Storage layout

| Container path | Contents | Requirement |
| --- | --- | --- |
| `/config` | SQLite database, migrations, and server configuration | Persistent, private, writable |
| `/library` | Managed Projects, Inbox, and Trash | Persistent, sized for originals, writable |
| `/tmp` | Temporary parsing and upload work | Ephemeral, bounded by Compose |

The container runs as UID/GID `1000:1000` by default. Both persistent locations must be writable by that identity unless you deliberately run the container as another owning user.

## Named volumes

The supplied Compose file creates `volum-config` and `volum-library`. Named volumes are simple, but verify where your Docker installation stores and backs them up.

Inspect them with:

```bash
docker volume inspect volum_volum-config
docker volume inspect volum_volum-library
```

The actual Compose project prefix may differ from `volum`.

## NAS bind mounts

Bind mounts are often easier to snapshot and inspect. Replace the service volumes with paths owned by the container user:

```yaml
services:
  volumd:
    volumes:
      - /volume1/docker/volum/config:/config
      - /volume1/3d/Volum:/library
```

Do not place `/config` inside a file-synchronization tool that copies SQLite, WAL, or SHM files independently while the service is running.

## Consistent backup procedure

Remote backup administration is not currently exposed. Use a NAS snapshot or filesystem copy:

1. Notify active clients that the server will be briefly unavailable.
2. Stop `volumd`.
3. Snapshot or copy `/config` and `/library` together.
4. Start `volumd`.
5. Confirm `/readyz` and open a known project from a client.

```bash
docker compose stop volumd
```

Create the coordinated snapshot or copy, then:

```bash
docker compose start volumd
curl --fail http://127.0.0.1:43821/readyz
```

## What not to do

- Do not copy only `volum.sqlite3` and omit the managed library.
- Do not restore `/config` and `/library` from different dates.
- Do not synchronize live SQLite, `-wal`, or `-shm` files independently.
- Do not treat a remote Desktop mirror as a server backup.
- Do not store backup copies with permissions broader than the original data.

Test recovery with a non-production copy before depending on a backup process.
