# Changelog

All notable changes to Volum are documented here.

## [0.3.0] - 2026-11-03

### Added

- Companion Access for securely pairing trusted phones, tablets, and other desktops over Tailscale, private networks, or HTTPS, with scoped permissions and immediate device revocation.
- The Volum Companion 1.0 beta for iPhone, iPad, and Android, including library browsing, search, project documentation, offline access, touch 3D inspection, photo capture, file uploads, and organization tools.
- The headless `volumd` server, a browser administrator, and Docker Compose deployment for an always-on library on a NAS or home server.
- Managed project storage, portable project packages, remote Desktop access, and guided library migration to a new local or network location.
- Object-level selection, isolation, and visibility controls for inspecting multipart models.

### Improved

- Project details now support photos, custom fields, bills of materials, assembly steps, notes, cost information, and source attribution across supported clients.
- Desktop, mobile, server, and shared components now live in one validated monorepo with a generated OpenAPI contract and shared design, viewer, photo-room, and utility packages.
- Release automation now validates versions, changelog-derived notes, installers, mobile builds, container startup, API drift, visual regressions, and bundle budgets.

### Security

- Pairing uses short-lived one-use secrets, hashed and revocable device tokens, server-identity verification, native credential vaults, bounded requests, and independently configurable organization, photo, and model-upload permissions.
- Existing Companion beta pairings migrate from Stronghold to the native credential vault only after the replacement credentials have been verified.

## [0.2.0] - 2026-09-23

### Added

- Project bundles that keep printable files, source geometry, plates, and versions together, with manual merge, split, and primary-preview controls that persist across rescans.
- Geometry-aware duplicate detection across formats and orientations, with keeper recommendations, metadata transfer, false-match dismissal, and optional OS Trash cleanup.
- Versioned all/any smart-collection rules and reusable saved searches.
- OpenCascade-backed STEP/STP metadata, interactive tessellation, thumbnails, and geometry analysis on every supported desktop platform.

### Improved

- Library and detail interfaces now present projects rather than treating every file as an isolated model.
- Duplicate cleanup preserves companion files and organization metadata, and never permanently deletes source files.

## [0.1.10] - 2026-09-23

### Added

- Portable, checksummed ZIP backups containing a consistent SQLite snapshot and user-owned media, with bounded validation and restart-safe restore.
- Manual, daily, weekly, or monthly backups to local folders, mounted network shares, and direct WebDAV destinations.
- Per-destination retention, connection testing, backup history, remote archive browsing, and automatic retry when a scheduled destination becomes available.
- WebDAV secrets stored in the operating system credential manager rather than Volum's database.

### Improved

- Theme, language, grid density, and sort preferences now mirror into the database so they survive a full restore.
- Backups can be restored from the first-run screen, and offline libraries can be reconnected at a new path without losing their metadata.
- Missing mounted backup folders now fail safely and retry later instead of being recreated on the local disk.

## [0.1.9] - 2026-09-22

### Improved

- The Folders header action now opens the native folder picker, adds the selected library, and starts indexing it immediately.
- Quick filters are larger and easier to read, with a custom Volum calendar replacing the inconsistent system date picker.
- The library header now always shows the total number of indexed models.
- The product website has clearer navigation, updated sharing artwork and metadata, corrected download/version information, and privacy-safe website analytics.

### Accessibility

- Date pickers now support outside-click dismissal, Escape-to-close with focus restoration, localized labels, and constrained From/To ranges.
- Mobile website navigation now supports active-section states, Escape dismissal, outside-click dismissal, and improved focus behavior.

## [0.1.8] - 2026-09-22

### Fixed

- Model cards no longer overlap after a library finishes loading or the grid is resized.
- Linux filesystem reads no longer trigger continuous rescans or leave the library stuck on “Building previews.”

[0.3.0]: https://github.com/sabatesduran/volum/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/sabatesduran/volum/compare/v0.1.10...v0.2.0
[0.1.10]: https://github.com/sabatesduran/volum/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/sabatesduran/volum/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/sabatesduran/volum/compare/v0.1.7...v0.1.8
