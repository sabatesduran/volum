# Changelog

All notable changes to Volum are documented here.

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

[0.1.10]: https://github.com/sabatesduran/volum/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/sabatesduran/volum/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/sabatesduran/volum/compare/v0.1.7...v0.1.8
