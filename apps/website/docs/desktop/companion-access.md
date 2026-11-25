---
title: Companion Access
description: Make a Desktop library available to trusted mobile and Desktop clients.
---

# Companion Access

Companion Access turns an existing Volum Desktop into the canonical host for trusted devices. It is disabled by default. **Volum Tailscale** gives the Desktop app its own private Tailscale identity and MagicDNS address. **System Tailscale** uses an already installed Tailscale app. **Custom** listens on a private IP you choose and advertises a separate private HTTP or HTTPS URL.

## Before you begin

For the easiest setup, install and sign in to [Tailscale](https://tailscale.com/) on each phone, tablet, or remote Desktop that will connect. Volum Desktop can join the tailnet itself, so the separate Tailscale desktop app is not required when **Volum Tailscale** is available.

The devices and Volum host must belong to a tailnet allowed by your Tailscale ACLs. Tailscale is recommended but not required: LAN, WireGuard, Twingate, ZeroTier, and trusted HTTPS routes are supported through Custom mode.

## Enable access

1. Open **Settings** on the canonical Desktop.
2. Open **Mobile access**. The page heading is **Companion Access**.
3. Keep **Volum Tailscale**, choose a preferred hostname, and select **Sign in to Tailscale**.
4. Authorize the new Volum device in the browser. If your tailnet requires device approval, approve it in the Tailscale admin console.
5. Return to Volum and wait for **Available on your selected network**. Volum displays the assigned address, such as `http://volum.example-tailnet.ts.net`.
6. Alternatively, choose **System Tailscale** to use the separately installed Tailscale app, or choose **Custom** and enter the private listener IP plus the URL clients will use.

Tailscale may add a suffix such as `volum-1` when the preferred hostname is already in use. Volum always advertises the address actually assigned. Embedded Tailscale is experimental and is shown only on platforms supported by the bundled Rust implementation. If it is unavailable, use System Tailscale or Custom. Volum refuses wildcard and public listener IPs. Public routes must use HTTPS.

The embedded address currently uses HTTP inside the encrypted Tailscale tunnel. Do not expose it directly to the public internet.

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

Open **Settings → Mobile access**, find the device, and choose revoke. Its token stops working on the next request.

Turning Companion Access off closes the listener immediately. Paired-device records remain until you revoke them.

## Keep access available after closing the window

Enable [menu-bar or system-tray operation](./menu-bar). Closing the main window will then leave Volum running for indexing and Companion Access. Use the tray menu's explicit **Quit Volum** command when you want the process and server to stop.
