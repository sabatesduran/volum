---
title: Photos and file uploads
description: Send workshop photos and 3D files directly to a managed Volum library.
---

# Photos and file uploads

Uploads travel directly through your selected private network, VPN, or HTTPS route to the canonical Volum host. They do not pass through a Volum cloud or relay.

The host must be online and **Allow organization changes** must be enabled.

## Add a project photo

1. Open **Capture**.
2. Choose **Photo**.
3. Take a new photo or choose one from the photo library.
4. Select the destination project.
5. Add an optional caption.
6. Select **Add photo to project**.

Volum validates the image, stores it with the canonical project documentation, and refreshes the project after completion.

## Upload a 3D file

1. Open **Capture**.
2. Choose **3D file**.
3. Select an STL, 3MF, OBJ, STEP, or STP file.
4. Enter the new project name.
5. Start the upload and keep the app open until import completes.

Mobile accepts one supported 3D file up to 500 MB for this workflow. The canonical host streams and validates it before importing it into a managed library.

## Cancel an upload

Use **Cancel upload** while progress is active. A cancel request cannot undo a file that has already completed import.

## Upload is unavailable

Check that:

- the host is online;
- organization writes are enabled;
- the canonical host has a managed library;
- the selected file uses a supported extension;
- the device has permission to access the selected photo or file;
- enough server storage is available.
