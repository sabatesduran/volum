---
title: Choose a setup
description: Compare local Desktop, Desktop host, and dedicated server configurations.
---

# Choose a setup

Start with the smallest setup that solves your problem. You can pair more devices later without changing the project format.

## Local Desktop

Choose local Desktop when:

- one computer is the natural home for the library;
- files already live on local, removable, or mounted storage;
- you do not need phone access;
- you want the simplest backup and recovery path.

The interface calls the local Rust core directly. No local HTTP server is involved unless you explicitly enable Companion Access.

## Desktop host

Choose a Desktop host when:

- you already have a working Desktop library;
- the host can stay running while other devices need it;
- you want phone access without moving the canonical library;
- Tailscale, another private LAN/VPN route, or trusted HTTPS is available to clients.

Enable Companion Access under **Settings → Mobile access** on the canonical Desktop. The host itself continues using direct local calls; only remote clients use the authenticated API.

If the window should close while access continues, enable [menu-bar or system-tray operation](../desktop/menu-bar).

## Dedicated server

Choose `volumd` when:

- a NAS or home server is already always on;
- you want one managed library for multiple trusted devices;
- you are comfortable operating Docker, storage permissions, and backups;
- clients will connect through Tailscale, another private network/VPN, or a correctly configured HTTPS reverse proxy.

A dedicated server does not provide a local graphical interface. Use Volum Desktop in remote mode or Volum Companion as the client.

If Desktop already owns the library, use the verified [Move a Desktop library to a NAS](../desktop/move-library-to-nas) assistant. The NAS must be empty; Volum copies linked and managed originals, verifies the staged library, and only then makes `volumd` canonical.

## Capability comparison

| Capability | Local Desktop | Desktop host | Dedicated server |
| --- | --- | --- | --- |
| Desktop interface on canonical machine | Yes | Yes | No |
| Linked folders | Yes | Yes | Not the primary server workflow |
| Managed library | Yes | Yes | Yes |
| Mobile access | No, until enabled | Yes | Yes |
| Remote Desktop access | No, until enabled | Yes | Yes |
| Always-on by default | No | Optional tray behavior | Yes |
| Backups from the Desktop interface | Yes | Yes, on the host | No; use coordinated server snapshots |

## Do not create two canonical libraries

A normal remote connection keeps the previous local library intact and can switch back to it. A completed **Move library to NAS** cutover instead retires that local library and blocks ordinary local switching so the NAS remains the only writable canonical host. This is not database synchronization.

The optional [managed mirror](../desktop/managed-mirror) stores verified original files for offline opening. It never stores a second Volum database.
