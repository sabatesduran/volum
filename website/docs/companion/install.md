---
title: Install the mobile beta
description: Understand the current iOS and Android beta distribution options.
---

# Install the mobile beta

Volum Companion is currently an early-access application. Public App Store and Google Play listings are not available yet.

## Recommended: invited beta

If you received an official testing invitation, install the build using the platform instructions included with that invitation. Keep automatic beta updates enabled so the app remains compatible with the server protocol.

Do not install mobile packages from unofficial mirrors. Pairing tokens grant access to private library metadata and should only be handled by builds you trust.

## Build from source

Developers can build the Tauri mobile project in the `mobile/` directory. You will need:

- Node.js 20 or newer;
- stable Rust;
- Tauri 2 mobile prerequisites;
- Android Studio and the Android SDK/NDK for Android;
- macOS, Xcode, and an Apple Development team for iOS device builds.

See [Mobile development](../development/mobile) for exact repository commands.

## Before opening the app

1. Confirm the mobile device can reach the Volum host. Installing [Tailscale](https://tailscale.com/) on both devices is the easiest option.
2. If you use a LAN, WireGuard, Twingate, ZeroTier, or another VPN, confirm its private route is active. Public routes require trusted HTTPS.
3. Confirm the Volum host is online.
4. Ask the host owner to create a new pairing code.

The mobile app requests camera access only when you scan a pairing or project QR code. Photo and notification permissions are requested only when the corresponding feature needs them.
