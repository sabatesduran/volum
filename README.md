# Volum

**A beautiful home for your 3D models.**

Volum is a private, local-first desktop library for STL, 3MF, OBJ, and STEP/STP files. It mirrors the folders you already have, adds visual previews and searchable metadata, and lets you create collections without moving source files.

![Volum icon](src-tauri/icons/128x128.png)

The master icon artwork is [`volum-appicon.png`](volum-appicon.png); platform assets under `src-tauri/icons/` are generated from it with `npx tauri icon volum-appicon.png`.

## What works

- Multiple local, removable, or mounted library roots
- Progressive recursive indexing with pause/resume, recovery, and filesystem watching
- Stable move detection with fingerprints and a metadata grace period for unavailable files
- STL, OBJ, and 3MF dimensions/triangle metadata; ignored ZIP archives; honest STEP placeholders
- Interactive 3D detail view and persistent background-rendered thumbnails
- FTS5 search, folders, recents, favorites, quick filters, and virtualized grids
- Collections via menus, multi-select, and drag-and-drop
- Material presets and plastic/batch cost estimates
- Configurable slicer, OS-default open, and Finder/Explorer/file-manager reveal
- English, Catalan, and European Spanish interfaces with system-language detection
- Light/dark/system themes, keyboard navigation, portable ZIP backup/restore, and approval-based signed updates
- Scheduled backups to local folders, mounted NAS shares, or direct WebDAV destinations with retention controls
- No account, analytics, model uploads, or required network connection

The original product specification is in [`Volum-product-engineering-plan.md`](Volum-product-engineering-plan.md).

## Development

Requirements:

- Node.js 20+
- Rust stable
- [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/)

```bash
npm install
npm run tauri dev
```

Browser-only UI preview (uses a representative local mock library):

```bash
npm run dev
```

## Downloads

- **macOS:** universal, signed, and notarized DMG from [GitHub Releases](https://github.com/sabatesduran/volum/releases/latest)
- **Linux x86_64:** AppImage and Arch Linux package from [GitHub Releases](https://github.com/sabatesduran/volum/releases/latest)
- **Windows x64:** native [NSIS installer](https://github.com/sabatesduran/volum/releases/latest/download/Volum_windows_x86_64_setup.exe)
- **Windows ARM64:** native [NSIS installer](https://github.com/sabatesduran/volum/releases/latest/download/Volum_windows_arm64_setup.exe)

The Linux AppImage is built on Ubuntu 22.04 for broad glibc compatibility. Automated publication of the `volum-bin` package to the AUR is prepared and will be enabled after its maintainer account is configured.

Windows installers are not yet Authenticode-signed, so Microsoft Defender SmartScreen may ask for confirmation. Tauri updater signatures and SHA-256 checksum files are published beside both installers. The Windows 11 ARM64 release has been exercised natively, and the x64 release through Windows' built-in emulation, with STL, OBJ, 3MF, Explorer reveal, search, and installed slicers.

Validation:

```bash
npm run build
npm test
cd src-tauri && cargo test
```

## Release signing

- The updater public key is committed in `src-tauri/tauri.conf.json`; its private key is stored only as a GitHub Actions secret.
- macOS releases are universal Developer ID builds, notarized by Apple, and stapled before publication.
- Linux AppImages and both Windows installers include Tauri updater signatures and SHA-256 checksums.
- Windows installers are not yet Authenticode-signed. Authenticode credentials are still required for a warning-free SmartScreen experience.
- The updater endpoint uses the canonical [`sabatesduran/volum`](https://github.com/sabatesduran/volum) repository.

Private signing keys must never be committed.

## Privacy and security

The React webview has no general filesystem or shell permission. File paths received over IPC are resolved from database asset IDs and revalidated against user-approved roots in Rust. Archive traversal and decompression limits are enforced when reading ZIP-based 3MF containers.

See [`SECURITY.md`](SECURITY.md) for vulnerability reporting.

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE).
