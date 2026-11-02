---
title: Menu bar and system tray
description: Keep Volum running after the Desktop window closes.
---

# Menu bar and system tray

Volum can remain active in the macOS menu bar or the Windows and Linux system tray. This is useful when a Desktop hosts Companion Access.

## Enable background window behavior

1. Open **Settings**.
2. Find **Menu bar**.
3. Enable **Show Volum in the menu bar**.

Closing the main window now hides it instead of quitting the process. Indexing, filesystem watching, scheduled work, and Companion Access can continue.

## Tray controls

The menu reports library and Companion status and provides actions to:

- open the Volum window;
- scan libraries now;
- run a configured backup;
- quit Volum explicitly.

## Quit completely

Choose **Quit Volum** from the menu-bar or tray menu. Quitting stops Companion Access and makes the Desktop unavailable to remote clients.

::: info Start at login
Automatic start at login is not currently part of the public workflow. After a reboot, open Volum before expecting Companion clients to reconnect.
:::
