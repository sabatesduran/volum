---
title: Introduction
description: Understand Volum's local-first library and the three supported ways to run it.
---

# Introduction

Volum is a private library for STL, 3MF, OBJ, STEP, and STP projects. It groups related files, builds visual previews, and keeps notes, tags, collections, photos, bills of materials, and assembly steps beside each project.

You do not need an account or a hosted Volum cloud. Start with the Desktop app on one computer, then add mobile access or move the canonical library to a server when you need it.

::: warning Early access
Volum Companion and the dedicated server are in early access. The Desktop app remains the normal starting point. Mobile store distribution, broad hardware validation, and some release operations are still being completed.
:::

## The three ways to run Volum

| Setup | Where the canonical library lives | Best for |
| --- | --- | --- |
| Local Desktop | On the computer running Volum | One computer, removable drives, or mounted folders |
| Desktop host | On a Desktop that has Companion Access enabled | Reaching an existing library from a phone or another Desktop |
| Dedicated `volumd` | On a NAS or home server | An always-on managed library shared by trusted devices |

The **canonical library** owns the database, organization, and authoritative project files. Choose one canonical machine. Do not synchronize or merge `volum.sqlite3` between computers.

## Local-first does not mean one device

Local-first means your chosen machine remains authoritative and Volum does not require a hosted account or relay. Optional connections are direct:

- Desktop and mobile clients authenticate to your Desktop host or `volumd` server.
- Tailscale is the easiest encrypted route; other private networks/VPNs and trusted HTTPS are supported.
- Pairing uses a five-minute, single-use secret.
- Each paired device receives its own revocable token.
- Selected files and photos upload directly to your canonical library, never through a Volum cloud.

## A good first path

1. [Install Volum Desktop](./install-desktop).
2. [Create a linked or managed library](./first-library).
3. Use it locally until the organization matches how you work.
4. [Enable Companion Access](../desktop/companion-access) if you want mobile access.
5. Consider [`volumd`](../server/) only when you want an always-on canonical library.

## What to read next

- [Choose a setup](./choose-a-setup) compares the tradeoffs.
- [Linked and managed libraries](../desktop/libraries) explains file ownership.
- [Volum Companion](../companion/overview) covers the mobile beta.
- [Install with Docker](../server/docker) starts a dedicated server.
- [Architecture](../reference/architecture) explains how the pieces communicate.
