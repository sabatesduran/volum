---
title: Browse and organize
description: Use the mobile library, collections, project details, and approved edits.
---

# Browse and organize

The mobile interface is organized around Library, Favorites, Capture, Collections, and Settings.

## Find a project

Use **Library** to browse the latest server state. Search and filters can narrow projects by supported metadata. Pull down on refreshable screens to request current data from the host.

When the host is unavailable, previously cached pages may still appear with stale or offline labeling.

## Inspect a project

A project can include:

- original and printable file records;
- generated previews and a touch 3D view;
- 3MF plate and profile information;
- project photos and source attribution;
- tags, collections, notes, and custom fields;
- bills of materials and assembly steps;
- material and cost information exposed by the server.

Large or unsupported viewer meshes fall back to the project cover rather than blocking the rest of the detail screen.

For supported models, tap **Photo Room** beside **View in 3D** to create a formatted image or model animation. See [Create photos and videos](./photo-room) for formats, scene controls, and export options.

## Make organization changes

Editing requires **Allow organization changes** on the host. Supported edits use server revisions so another device's newer change is not silently overwritten.

If Volum reports a conflict:

1. Refresh the project.
2. Review the current server value.
3. Apply your change again if it is still appropriate.

Edits are not queued while offline.

## Open a Desktop project by QR

A paired Desktop can display a project-specific QR code. On mobile, open the Library QR scanner and scan it to jump directly to that project.

The project code contains only server and project identifiers. It does not contain your bearer token or filesystem paths.
