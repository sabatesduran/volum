# Security policy

## Reporting

Please do not publish exploitable path-validation, archive, updater, or arbitrary-command issues in a public issue. Report them privately through GitHub Security Advisories on the canonical repository.

Include the Volum version, operating system, reproduction steps, and a minimal non-sensitive fixture where possible. Never include a real private library path or model without permission.

## Scope

The latest release receives security fixes. Volum does not require an account or upload model files to a hosted Volum cloud. When a user explicitly uses Companion or remote Desktop uploads, selected files travel directly to the paired canonical Desktop or `volumd` server. Approved library roots, app-owned data, authenticated remote routes, IPC validation, upload limits, archive and geometry limits, OS Trash validation, and update signatures are security boundaries and should not be bypassed.

Backup ZIPs contain private library metadata such as paths, notes, tags, and source links, but not original model files. They are not encrypted; protect backup folders with appropriate filesystem or NAS permissions. Direct WebDAV credentials are stored in the operating system credential manager and are excluded from both SQLite and backups.

See the full [security model](apps/website/docs/reference/security.md) and [private reporting instructions](apps/website/docs/help/security.md).
