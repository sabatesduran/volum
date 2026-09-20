# Contributing to Volum

Thank you for helping improve Volum.

## Before opening a change

- Keep the filesystem canonical: indexing must never rename or move a model automatically.
- Keep core functionality local and account-free.
- Prefer focused changes that preserve the editorial-industrial visual direction.
- New file formats need parser tests and small, legally redistributable fixtures.
- Avoid broad shell or filesystem permissions in the webview.

## Development checks

```bash
npm ci
npm run build
npm test
cd src-tauri
cargo fmt --check
cargo test
```

Use conventional, descriptive commits. Include platform details and reproduction steps for filesystem bugs.
