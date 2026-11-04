---
title: Volum on mobile
description: Browse, organize, and capture projects from iPhone, iPad, or Android.
---

# Volum on mobile

Volum Companion is the mobile role of the Volum app. It connects directly to a Desktop host or `volumd` server through Tailscale, another private network or VPN, or trusted HTTPS; there is no Volum account, hosted database, or public relay.

::: warning Beta availability
The mobile app is in early access and is not yet available through the App Store or Google Play. Android users can install the signed APK from the Volum website. iPhone and iPad users can [join the public beta through TestFlight](https://testflight.apple.com/join/Vcet13mC).
:::

## What you can do

- Browse, search, filter, and favorite projects.
- Open collections, folders, and recent projects.
- Inspect photos, files, metadata, 3MF plates, BOMs, and assembly steps.
- Rotate and zoom supported models in the touch 3D viewer.
- Create share-ready project photos and animated videos in Photo Room.
- Edit approved organization and documentation fields while online.
- Take project photos or choose them from the photo library.
- Upload STL, 3MF, OBJ, STEP, and STP files into a managed library.
- Keep selected projects, folders, or collections available offline.

## What remains on the canonical machine

Mobile does not configure library roots, run backups, control scanning, launch slicers, clean duplicates, or perform host administration. Destructive and filesystem-wide maintenance stays on the canonical Desktop or server.

## Requirements

- A supported Volum Desktop with Companion Access enabled, or a compatible `volumd` server.
- A route from the mobile device to the host. Tailscale is the automatic, recommended setup; private LAN/VPN routes and HTTPS are also supported.
- A pairing payload created by the host.
- Write permission on the host for edits and uploads.

## How data moves

Library requests travel directly between the mobile device and your canonical Volum service. HTTPS or your selected VPN supplies transport protection; Volum supplies server identity checks, per-device authentication, and revocation. Plain HTTP is accepted only on private addresses and networks.

Offline snapshots and media stay on the mobile device. The app labels cached information and does not imply it is current while the server is unavailable.

Next, [choose a network](./networking), [install the beta](./install), [pair a device](./pair), or learn to [create photos and videos](./photo-room).
