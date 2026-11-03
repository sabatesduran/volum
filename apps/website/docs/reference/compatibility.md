---
title: Compatibility
description: Understand Desktop, mobile, server, and protocol compatibility.
---

# Compatibility

Volum's network protocol is versioned independently from Desktop, mobile, and server marketing versions.

## Protocol negotiation

Pairing exchanges the minimum and maximum supported protocol versions. A client refuses pairing when there is no overlap and indicates which side needs an update.

The current branch implements protocol version `1` and the `/api/v1` route family.

## Capabilities

Clients read server capability flags rather than assuming every endpoint is enabled. Capabilities cover areas such as:

- organization writes;
- file and photo uploads;
- viewer meshes;
- offline and synchronization support.

A host permission change can remove a capability immediately. Clients must handle server rejection even when their local interface is older.

## Release guidance

- Keep Desktop hosts and remote Desktop clients on supported releases.
- Keep mobile beta builds current.
- Pin dedicated servers to a known release for production-like use.
- Read release notes before changing a server that owns important managed files.
- Take a coordinated server backup before an update that may run migrations.

Exact mobile operating-system minimums and public store compatibility will be published before general availability.
