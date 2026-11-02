---
title: Mobile development
description: Build and test the Tauri iOS and Android companion application.
---

# Mobile development

The `mobile/` directory contains one React interface and one Tauri Rust transport for iOS and Android. Generated native projects are checked in.

## Install dependencies

```bash
npm --prefix mobile install
```

Build and test the portable frontend:

```bash
npm --prefix mobile run build
npm --prefix mobile test
```

Run visual regression tests after installing Playwright Chromium:

```bash
npm --prefix mobile exec playwright install chromium
npm --prefix mobile run test:visual
```

## Android

Install Android Studio, SDK platform tools, and a compatible NDK. The CI workflow currently builds an arm64 debug APK with Java 17 and the configured Android toolchain.

Run on a configured Android target:

```bash
npm --prefix mobile run tauri -- android dev
```

Build an arm64 debug APK:

```bash
npm --prefix mobile run tauri -- android build --debug --target aarch64 --apk
```

The repository wrapper locates the NDK and supplies the correct cross-compilation archiver for native dependencies.

## iOS

Copy the environment example and add your Apple Development team identifier:

```bash
cp mobile/.env.example mobile/.env
```

Then run on a configured simulator or device:

```bash
npm --prefix mobile run tauri -- ios dev
```

The local `.env` file and signing assets must never be committed.

## Test against a live host

Use a non-sensitive test library. Connect the development phone and host through Tailscale, another private test route, or trusted HTTPS; enable Companion Access and create a fresh pairing payload. Include private HTTP, public-HTTP rejection, identity mismatch, and redirect cases in transport testing.

Exercise pairing, reconnect after cold launch, revocation, app resume, offline packs, upload cancellation, and viewer fallback on physical hardware before a beta release.
