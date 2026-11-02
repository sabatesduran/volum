---
title: Connect Desktop to a server
description: Use Volum Desktop as a client of another Desktop host or volumd server.
---

# Connect Desktop to a server

A Desktop can browse and edit a library owned by another Volum Desktop or a dedicated `volumd` server. Its previous local library remains intact.

To transfer the current local library and make a new NAS canonical, do not use ordinary pairing alone. Follow [Move a Desktop library to a NAS](./move-library-to-nas).

## Pair and connect

On the canonical host:

1. Enable Companion Access or start `volumd` with pairing enabled.
2. Create or copy the five-minute pairing payload.

On the client Desktop:

1. Open **Settings → Connection**.
2. Paste the complete pairing payload.
3. Select **Pair and connect**.
4. Verify the server name and connection status.

The client stores its bearer token in the operating-system credential store. The webview does not receive it.

## Remote capabilities

Remote Desktop supports normal browsing and most organization work, including:

- search, filters, folders, recents, favorites, collections, and saved searches;
- project creation and managed-file uploads;
- project names, notes, tags, custom fields, BOMs, assembly steps, materials, and estimates;
- project photos, thumbnails, plate images, and viewer meshes;
- Trash, restore, and permanent delete where server permissions allow;
- read-only duplicate review.

Some operations stay on the canonical machine:

- adding or reconnecting linked roots;
- scan administration and linked-to-managed conversion;
- merge, split, primary-file changes, and duplicate cleanup;
- package import or export and web-import ingestion;
- server backups and Companion host administration.

The client hides unsupported controls and does not fall back to mutating its local library.

## Switch back to local mode

Open **Settings → Connection** and choose **Use local library**. Volum returns to the database and folders that were already on that Desktop.

The remote server remains configured until you choose **Forget server**, so you can switch back later without pairing again.

This switch is intentionally unavailable after a completed NAS migration. The retired Desktop library is then a rollback copy, not a second writable library.

## Open original files remotely

Thumbnails and viewer meshes stream from the server. Opening an original file in a slicer or file manager requires a verified local copy. Configure a [managed mirror](./managed-mirror) for that workflow.
