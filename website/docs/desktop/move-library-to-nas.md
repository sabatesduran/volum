---
title: Move a Desktop library to a NAS
description: Transfer an existing Volum Desktop library to volumd and make the NAS canonical.
---

# Move a Desktop library to a NAS

Use **Move this library to a NAS** when a Desktop currently owns the library and a new `volumd` server should become its only source of truth.

This is a one-time transfer and cutover. It does not synchronize or merge two SQLite databases.

## Before you start

Confirm all of the following:

- `volumd` is installed, healthy, and reachable from the Desktop;
- the NAS does not contain any projects;
- Desktop and `volumd` run the same Volum release and database schema;
- `/config` and `/library` are persistent and writable;
- `/library` has enough free space for every original file, and `/config` has room for staged media plus the activation database copy;
- Docker or another supervisor will restart `volumd` when activation requests a restart;
- removable disks and network mounts containing linked projects are connected;
- the connection uses a private LAN/VPN or trusted HTTPS.

::: warning Empty NAS required
Volum refuses the migration if the destination contains a project. It never guesses how to merge two canonical libraries.
:::

Take an independent backup of the Desktop library before the first production migration.

## Start the transfer

On the NAS:

1. Open the `volumd` web administrator.
2. Sign in as administrator.
3. Under **Move a Desktop library here**, select **Create transfer code**.
4. Copy the complete one-time pairing data.

On the current canonical Desktop:

1. Open **Settings → Connection**.
2. Select **Move this library to a NAS**.
3. Review project, file, photo, and byte counts.
4. Resolve or explicitly accept any missing linked originals.
5. Paste the NAS transfer data.
6. Select **Copy and make NAS primary**.

Keep Volum Desktop open until activation completes.

## What moves

The migration keeps stable project and asset identities and transfers:

- original managed and linked project files;
- names, notes, favorites, tags, and collections;
- project photos and covers;
- custom fields, BOM items, and assembly documentation;
- materials, estimates, saved searches, and web-source records;
- Trash metadata and available trashed originals.

Derived thumbnails and viewer meshes are not authoritative. The NAS can regenerate them after activation.

Desktop-only host configuration does not move. Backup destinations, watched import folders, menu-bar preferences, server identity, browser sessions, and device tokens remain specific to their original host.

## Linked projects

Linked originals are copied directly from their current locations into managed NAS storage. Volum does not first reorganize or duplicate them inside the Desktop managed library.

After migration, every available project original is managed under `/library`. Existing Desktop folders and removable disks are not changed or deleted.

If an original is unavailable, the assistant shows its project, filename, and source path. Reconnect or locate it whenever possible. If you explicitly continue, its metadata moves and the NAS marks that original as missing.

Missing project photos always block migration because silently dropping documentation would produce an incomplete library.

## Verification and activation

The transfer is chunked and resumable. The NAS persists received offsets, so completed data is not uploaded again after a network interruption.

Before activation, `volumd` verifies:

- the BLAKE3 checksum and declared size of every available original and photo;
- the migration manifest and safe destination paths;
- SQLite `integrity_check` and `foreign_key_check`;
- project, asset, photo, collection, and tag counts;
- the managed library layout and project identity sidecars.

Only a verified job can activate. During activation, `volumd` preserves its previous empty database and library layout, installs the staged database and managed files, and restarts. The NAS keeps its existing server identity, administrator password, and paired-device configuration.

## After cutover

When activation succeeds:

- Desktop switches to remote mode automatically;
- `volumd` becomes canonical;
- the old Desktop index and files remain in place as a migration rollback copy;
- Desktop stops indexing and serving that retired local library;
- **Use local library** is disabled to prevent accidental split-brain edits.

Configure a [managed mirror](./managed-mirror) if the Desktop should keep verified NAS originals for offline or slicer access. A mirror contains files and its private manifest, never another canonical database.

## Resume an interrupted migration

Return to **Settings → Connection → Move this library to a NAS** and select **Resume migration**. Volum reuses the saved job identity and the offsets reported by the NAS.

Do not create a new NAS library or edit the staged directories while a job is resumable.

The saved Desktop snapshot is bound to the original NAS server identity. Volum verifies that identity before sending the migration credential and will not resume the job against another server.

If interruption happens during activation, wait for `volumd` to restart before resuming. Inspect the container with:

```bash
docker compose ps
docker compose logs --tail=200 volumd
```

## Abandon an interrupted migration

Before activation begins, you can select **Abandon migration** if the transfer should not continue. Confirming the action cancels the owned job on the verified NAS, removes its staged files, discards the Desktop's resumable snapshot, restores local writes, and resumes local indexing.

Abandonment is deliberately unavailable once activation is in progress or complete. If the original NAS is unavailable or its identity no longer matches, Volum keeps the local snapshot paused rather than sending its credential or guessing that cancellation succeeded.

## Rollback

The migration never deletes the Desktop originals or old local database. This provides a rollback point, but changes made on the NAS after cutover are not merged back automatically.

For an emergency rollback:

1. Stop `volumd` so clients cannot continue changing the NAS library.
2. In Desktop connection settings, choose the migration rollback action and acknowledge that later NAS changes are not imported.
3. Verify representative projects and originals locally.
4. Keep the NAS storage untouched until the rollback is confirmed.

Never run the restored Desktop library and the migrated NAS as two writable canonical hosts. To retry later, use a fresh empty `volumd` destination.

The NAS also retains its pre-activation database under `/config/migration-rollbacks/` and its previous library layout under `/library/.volum-rollbacks/`. Preserve both `/config` and `/library` when investigating an activation failure.

## Security

The transfer code expires after five minutes and is consumed once. It creates a migration-only credential bound to that transfer job. It cannot browse or edit the normal library API. After verified activation, Volum promotes that same Desktop credential to a normal client while retaining the migration ID so job-specific completion checks and repeated finish requests remain safe. An unrelated transfer credential cannot claim, cancel, or finish the job.

Migration traffic uses the same native Desktop transport rules as normal pairing: private HTTP is allowed only for private/VPN destinations, while public routes require trusted HTTPS.

Do not paste pairing data into issue reports or logs. Before it is consumed or expires, it can create the job-scoped migration credential described above.
