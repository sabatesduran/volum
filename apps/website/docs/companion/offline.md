---
title: Offline projects
description: Keep selected projects and media available when the Volum host cannot be reached.
---

# Offline projects

Volum is online-first, but selected content can be stored for workshop access when the host sleeps or leaves the network.

## Mark content available offline

Use the **Available offline** control on a project, folder, or collection. Folder and collection selections expand into a project and media manifest that can be refreshed later.

Keep Volum open while the first offline pack downloads. The app shows progress, storage use, last synchronization time, and errors.

## What remains available

An offline selection can include sanitized project metadata and downloaded media required by that selection. Availability depends on what completed successfully before the connection was lost.

The app distinguishes:

- explicit offline content, which remains until you remove it;
- recently viewed media in the transient cache, which may be evicted;
- live server content, which needs a connection.

## Offline limitations

- Organization changes require a live server.
- New photos and model files cannot upload offline.
- Cached information may be older than the canonical library.
- Content not explicitly downloaded may be unavailable.

## Manage storage

Open **Settings → Offline storage** to inspect and clear offline projects. **Recent media cache** is separate and automatically limited; clearing it does not remove explicit offline projects or unpair the device.

Unpairing attempts to remove both server credentials and server-specific cached content.
