---
title: Security model
description: Review Volum's trust boundaries, pairing, transport, and data handling.
---

# Security model

Volum is private and local-first, but optional remote access still creates a network security boundary. Prefer Tailscale or trusted HTTPS; use private HTTP only on a trusted LAN or VPN. Pair only devices you control and keep canonical storage protected.

## Authentication

- Pairing payloads expire after five minutes and work once.
- A successful pairing returns a random per-device bearer token.
- The server stores a hash of the token, not the token itself.
- Devices can be revoked independently.
- Revocation is checked server-side on every protected request.
- Cookies and ambient browser authentication are not used.
- The `volumd` web administrator exchanges the configured administrator password for a separate revocable bearer token. The browser stores that token in IndexedDB and attaches it explicitly only to Volum API requests.
- Administrator passwords are Argon2id-hashed under `/config`. Login attempts are throttled and credentials are never written to request logs.

## Transport

Desktop Companion Access uses automatic Tailscale detection by default. Custom mode listens only on an explicitly selected private IP and can advertise a separate private HTTP or HTTPS origin.

Desktop and mobile accept trusted HTTPS. Plain HTTP is restricted to loopback, private IPv4/IPv6, link-local, Tailscale, or names that resolve entirely to private addresses. Public HTTP is blocked. Private HTTP relies on the selected LAN or VPN for confidentiality.

Before sending a one-time secret, clients call `/identity` and verify the expected server ID and protocol. Address changes perform the same check before a stored token is used. Native HTTP clients do not follow redirects.

The dedicated server has a configurable listener because container operators control the host network. Firewall it to intended private clients or place it behind a trusted HTTPS reverse proxy. Never expose application-level plain HTTP to the public internet.

## Filesystem protection

Network responses omit canonical absolute and relative asset paths. File and media routes accept opaque IDs, resolve them in Rust, and validate the final path against approved roots.

Uploads are streamed with size and type limits, then validated before managed import. An authenticated upload does not grant arbitrary filesystem placement.

## Secret storage

- Desktop remote tokens use the operating-system credential manager.
- Mobile credentials use Stronghold.
- Server token hashes and identity are stored separately from portable Desktop backup archives.
- Pairing payloads, bearer tokens, and signing keys must not be committed or logged.

## Backups and offline copies

Desktop backup ZIPs contain private metadata such as paths, notes, tags, and source links. They are checksummed but not encrypted; protect their storage permissions.

Mobile offline data and Desktop mirrors can expose project information on an unlocked device. Use platform device encryption and revoke lost devices from the canonical host.

## Explicitly out of scope

Volum does not protect data from an attacker who already controls an unlocked canonical host or an unlocked paired device. It does not provide multi-user authorization domains or end-to-end content encryption above the selected transport.

Report vulnerabilities through the [private security process](../help/security).
