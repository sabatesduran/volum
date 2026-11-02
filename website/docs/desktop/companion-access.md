---
title: Companion Access
description: Make a Desktop library available to trusted mobile and Desktop clients.
---

# Companion Access

Companion Access turns an existing Volum Desktop into the canonical host for trusted devices. It is disabled by default. Automatic mode listens only on the detected Tailscale address; Custom mode listens on a private IP you choose and advertises a separate private HTTP or HTTPS URL.

## Before you begin

For the easiest setup, install and sign in to [Tailscale](https://tailscale.com/) on:

- the Desktop that owns the library;
- each phone, tablet, or remote Desktop that will connect.

The devices must belong to a Tailnet allowed by your Tailscale ACLs. Tailscale is recommended but not required: LAN, WireGuard, Twingate, ZeroTier, and trusted HTTPS routes are supported through Custom mode.

## Enable access

1. Open **Settings** on the canonical Desktop.
2. Open **Companion Access**.
3. Keep **Automatic** for Tailscale, or choose **Custom**.
4. In Custom mode, enter the private IP on this computer that Volum should listen on, then enter the URL clients will use. These values are separate so a reverse proxy or VPN hostname can advertise a different address.
5. Save the network settings and turn Companion Access on.
6. Wait for **Available on your selected network** and confirm the displayed URL.

If Automatic mode cannot find a Tailscale address, verify that Tailscale is connected or switch to Custom mode. Volum refuses wildcard and public listener IPs. Public routes must use HTTPS.

## Choose write permissions

**Allow organization changes** controls whether paired clients may change project organization and managed files.

When enabled, supported clients can edit names, notes, favorites, tags, collections, custom fields, bills of materials, assembly steps, and approved uploads. When disabled, those actions are rejected by the server even if an older client still displays a control.

Keep writes disabled when a device only needs browsing access.

## Pair a device

1. Select **Pair a device**.
2. Review or edit the connection address shown above the QR code.
3. Leave the pairing dialog open and scan it in Volum on mobile, or copy the pairing payload into another Desktop.
4. Complete pairing within five minutes.

The payload works once. It contains the server identity, address, protocol range, and a temporary secret. It does not contain a permanent device token.

## Revoke a device

Open **Settings → Companion Access**, find the device, and choose revoke. Its token stops working on the next request.

Turning Companion Access off closes the listener immediately. Paired-device records remain until you revoke them.

## Keep access available after closing the window

Enable [menu-bar or system-tray operation](./menu-bar). Closing the main window will then leave Volum running for indexing and Companion Access. Use the tray menu's explicit **Quit Volum** command when you want the process and server to stop.
