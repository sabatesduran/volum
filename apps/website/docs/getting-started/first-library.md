---
title: Create your first library
description: Choose linked folders or a managed library and complete the first scan.
---

# Create your first library

Volum can index files where they already live or manage a library folder for you. The choice controls what file operations Volum may perform.

## Option 1: link an existing folder

Use a linked folder when your current folder layout should remain authoritative.

1. Open Volum.
2. Under **Use my existing projects**, select **Choose a project folder**.
3. Choose a folder containing STL, 3MF, OBJ, STEP, or STP files.
4. Leave Volum open while the initial scan builds metadata and previews.

Volum watches linked folders but does not automatically rename or reorganize their files. Collections, tags, favorites, notes, and project documentation live in the Volum database.

## Option 2: create a managed library

Use a managed library when Volum should safely own the project layout and support uploads, renames, Trash, and restore.

1. Open Volum.
2. Under **Start a new Volum library**, select **Choose library location**.
3. Choose an empty or dedicated folder.
4. Let Volum create its `Projects`, `Inbox`, and `Trash` structure.

Do not independently rearrange files inside a managed library while Volum is running.

After onboarding, add another linked folder from **Settings → Library → Add linked folder**. If no managed library is configured yet, the same page offers a **Choose…** action for the managed Volum library.

## During the first scan

Projects appear progressively. Metadata and thumbnails can continue in the background after the first items are visible. Large libraries and STEP files take longer because Volum parses actual geometry.

You can pause or resume indexing from the Desktop interface. Missing removable or mounted folders retain their metadata and can be reconnected later.

## Protect the library

- Back up original model folders using your normal filesystem or NAS backup.
- Use Volum backups for the database, preferences, attribution, and user-owned media.
- Never synchronize a live `volum.sqlite3`, `-wal`, or `-shm` file.
- For a managed server library, back up the database and managed files together.

For a deeper comparison, see [Linked and managed libraries](../desktop/libraries).
