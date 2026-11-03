---
title: Linked and managed libraries
description: Understand how Volum treats files in linked folders and managed libraries.
---

# Linked and managed libraries

Both library types use the same project model, search, previews, collections, and documentation. The difference is who owns the filesystem layout.

Manage linked folders and the optional managed library under **Settings → Library** on the canonical Desktop.

## Linked folders

In a linked folder, your existing directory structure remains authoritative.

Volum can:

- index and watch supported files;
- detect moved files using fingerprints;
- group related files into projects;
- generate cacheable thumbnails and viewer meshes;
- open or reveal files through the operating system.

Volum does not automatically rename or reorganize linked source files. Operations that require ownership of a project folder are intentionally limited.

Linked folders are suitable for existing archives, removable drives, and mounted NAS shares.

## Managed libraries

In a managed library, Volum owns a predictable layout:

```text
Volum Library/
├── Projects/
├── Inbox/
└── Trash/
```

This allows Volum to:

- create empty projects;
- import files from Desktop or Companion;
- rename projects through the storage pipeline;
- move projects to Trash, restore them, or permanently delete them;
- keep project files together when remote clients make approved changes.

Use a dedicated folder and let Volum perform managed operations. Avoid moving managed files manually while the canonical service is running.

## Convert a linked project

The canonical Desktop can copy or move a linked project into a managed library:

1. Open the project.
2. Choose **Organize in Volum**.
3. Select a managed library.
4. Preview the operation.
5. Choose copy or move and confirm.

This operation remains host-only because it changes authoritative filesystem storage.

## Backups are different from model-file backups

Desktop backup archives contain Volum data and user-owned media, not the original linked model files. Continue backing up linked and managed project folders using your normal storage backup.

For `volumd`, snapshot the server database and managed library together. See [Storage and backups](../server/storage-and-backups).
