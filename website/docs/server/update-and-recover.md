---
title: Update and recover
description: Update a volumd container and restore a coordinated server backup.
---

# Update and recover

Treat server updates as storage migrations: read the release notes, take a coordinated backup, and verify clients after the new container starts.

## Update a published image

1. Back up `/config` and `/library` together.
2. Pull the image for the intended release.
3. Recreate the service.
4. Review logs and health.

```bash
docker compose pull
docker compose up -d --no-build
docker compose logs --tail=100 volumd
curl --fail http://127.0.0.1:43821/healthz
```

Prefer a versioned release tag for long-lived deployments once versioned images are published. Use `latest` only when you intentionally want the newest released server.

## Update a source build

Check out the intended release tag, then rebuild:

```bash
git fetch --tags
git checkout <release-tag>
docker compose up -d --build
```

Do not build an unreviewed branch over an important library without a tested rollback path.

## Restore a coordinated backup

1. Stop `volumd`.
2. Move the current `/config` and `/library` aside; do not delete them yet.
3. Restore both locations from the same recovery point.
4. Confirm ownership and permissions.
5. Start `volumd`.
6. Check health, logs, server identity, project counts, and several original files.

If restoring the same server configuration, previously paired devices may remain trusted because server authentication state is part of `/config`. Revoke devices that should no longer have access after recovery.

## Failed migration or startup

Keep the container stopped and preserve the failed data directories. Record:

- the previous and attempted server versions;
- the complete startup error without pairing payloads or tokens;
- storage ownership and free space;
- whether `/config` and `/library` came from the same recovery point.

Restore the coordinated pre-update backup rather than manually editing SQLite.

For a failed Desktop-to-NAS cutover, also preserve:

- `/config/library-migrations/` and `/config/library-migration-activation.json`;
- `/config/migration-rollbacks/`;
- `/library/.volum-migrations/` and `/library/.volum-rollbacks/`;
- the retired Desktop library and its original linked folders.

The active database is switched only after staged media and managed files are in place. A supervisor restart retries an incomplete activation journal. Do not delete either persistent volume to force startup.

## Disaster-recovery check

A backup is not proven until a separate test deployment can:

- pass `/healthz`;
- open the restored database;
- list expected projects;
- read representative original files;
- produce or read representative thumbnails;
- pair or authenticate a test client as expected.
