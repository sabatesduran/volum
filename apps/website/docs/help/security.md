---
title: Report a security issue
description: Privately report a security vulnerability in Volum.
---

# Report a security issue

Do not publish an exploitable path-validation, authentication, archive, updater, upload, or arbitrary-command issue in a public GitHub issue.

Use [GitHub Security Advisories](https://github.com/sabatesduran/volum/security/advisories/new) on the canonical repository.

## Include

- the Volum Desktop, mobile, or server version;
- the operating system and architecture;
- whether the library was local, Desktop-hosted, or dedicated server;
- minimal reproduction steps;
- a small, legally shareable, non-sensitive fixture when needed;
- the expected and observed security boundary.

## Do not include

- bearer tokens or pairing payloads;
- private keys, certificates, or signing material;
- real private library paths or model files without permission;
- personal network names, private addresses, notes, or project photos;
- unredacted server logs containing private content.

If a real secret was exposed, revoke or rotate it immediately. Removing it from a later commit or message does not invalidate the original secret.
