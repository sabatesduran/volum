---
title: Install the mobile beta
description: Install the Android beta or join the iOS beta through TestFlight.
---

# Install the mobile beta

Volum Companion is currently an early-access application. Android is available as a signed APK, while iPhone and iPad builds are distributed through Apple TestFlight. Public App Store and Google Play listings are not available yet.

## Android: install the signed APK

Volum publishes a signed ARM64 APK with each GitHub release. Open the [mobile download section](https://volum.didac.dev/#mobile-download) on your Android device and select **Download Android APK**.

Android may ask you to allow your browser to install apps from this source. Approve the request for this installation, open the downloaded APK, and confirm the installation. Volum requires Android 7 or later on an ARM64 device.

The APK and its SHA-256 checksum are published together on [GitHub Releases](https://github.com/sabatesduran/volum/releases). Future APK releases must be installed from the website or GitHub because this beta does not use Google Play updates.

## iPhone and iPad: TestFlight

Open the [Volum Companion public beta](https://testflight.apple.com/join/Vcet13mC) on your iPhone or iPad. Install Apple’s free TestFlight app if prompted, then select **Accept** and **Install**.

Keep automatic beta updates enabled so the app remains compatible with the server protocol. Apple may limit access when the beta reaches its tester capacity.

Do not install mobile packages from unofficial mirrors. Pairing tokens grant access to private library metadata and should only be handled by builds you trust.

## Build from source

Developers can build the Tauri mobile project in the `apps/mobile/` directory. You will need:

- Node.js 22.23.4, pinned in `.nvmrc`;
- Rust 1.99.1, pinned in `rust-toolchain.toml`;
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
