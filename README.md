# Volum

**A beautiful home for your 3D models.**

Volum is a private, local-first desktop library for STL, 3MF, OBJ, STEP/STP, and ZIP files. It mirrors the folders you already have, adds visual previews and searchable metadata, and lets you create collections without moving source files.

![Volum icon](src-tauri/icons/128x128.png)

The master icon artwork is [`volum-appicon.png`](volum-appicon.png); platform assets under `src-tauri/icons/` are generated from it with `npx tauri icon volum-appicon.png`.

## What works

- Multiple local, removable, or mounted library roots
- Progressive recursive indexing with pause/resume, recovery, and filesystem watching
- Stable move detection with fingerprints and a metadata grace period for unavailable files
- STL, OBJ, and 3MF dimensions/triangle metadata; safe ZIP inspection; honest STEP placeholders
- Interactive 3D detail view and persistent background-rendered thumbnails
- FTS5 search, folders, recents, favorites, quick filters, and virtualized grids
- Collections via menus, multi-select, and drag-and-drop
- Material presets and plastic/batch cost estimates
- Configurable slicer, OS-default open, and Finder/Explorer/file-manager reveal
- Light/dark/system themes, keyboard navigation, metadata export, and signed updates
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

Validation:

```bash
npm run build
npm test
cd src-tauri && cargo test
```

## Release signing

The updater public key is committed in `src-tauri/tauri.conf.json`. The matching development private key is generated under `.tauri/volum.key` and is gitignored. Before publishing:

1. Replace the development key with a password-protected maintainer key.
2. Update the public key in `src-tauri/tauri.conf.json`.
3. Store `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as GitHub Actions secrets.
4. Configure Apple Developer ID/notarization and Windows code-signing secrets for warning-free installers.
5. Confirm the updater endpoint matches the canonical GitHub repository.

## Privacy and security

The React webview has no general filesystem or shell permission. File paths received over IPC are resolved from database asset IDs and revalidated against user-approved roots in Rust. ZIP traversal and decompression limits are enforced before reading archive contents.

See [`SECURITY.md`](SECURITY.md) for vulnerability reporting.

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE).
