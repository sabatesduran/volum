---
title: Managed mirror
description: Keep verified original files on a remote Desktop for offline opening and safe synchronization.
---

# Local project files

A managed mirror stores verified copies of original project files on a remote Desktop. It is optional and never contains a second Volum database.

## Choose what stays local

1. Pair the Desktop with a server.
2. Open **Settings → Remote library**.
3. Under **Local project files**, choose **Selected projects** or **Entire library**.
4. Choose a private folder.
5. Select **Sync now** for the initial download.

Use an empty or dedicated folder outside Volum's app-data directory. Make sure it has enough free space for the server's managed project files.

With **Selected projects**, use **Keep on this Desktop** from a project card, project details, or a multi-project selection. Folder and collection controls are live rules: new projects added later are downloaded automatically, including projects that newly match a smart collection. Removing a project from local storage creates an exclusion if a broader rule would otherwise include it.

## What the mirror stores

The folder contains ordinary project files plus private synchronization state under `.volum/`. Partial downloads are resumable. A file becomes active only after its size and BLAKE3 `ETag` match the server revision.

The mirror does not store `volum.sqlite3`, tags, collections, notes, or other canonical metadata. Metadata browsing still requires the server.

## Synchronization rules

| Server copy | Mirrored copy | Result |
| --- | --- | --- |
| Unchanged | Unchanged | No transfer |
| Changed | Unchanged | Download and verify the server copy |
| Unchanged | Changed | Upload only if the server still has the downloaded revision |
| Changed | Changed | Preserve the local copy under `Conflicts/`, then download the server copy |
| Deleted or no longer selected | Unchanged | Remove the mirrored copy |
| Deleted or no longer selected | Changed | Preserve the edited copy under `Conflicts/` |

`Conflicts/` is preservation, not automatic merging. Review preserved files manually and explicitly upload or import the version you want to keep.

## Offline behavior

When the server is unavailable, Volum lists verified mirrored files and can open or reveal them. Search, metadata browsing, and edits remain unavailable until the canonical server reconnects.

## Disable a mirror

Disabling synchronization does not delete the folder. Remove mirror files manually only after confirming that the canonical server has the copies you need.
