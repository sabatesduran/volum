# Contributing to Volum

Thank you for helping improve Volum.

## Before opening a change

- Keep the filesystem canonical: indexing must never rename or move a model automatically.
- Keep core functionality local and account-free.
- Prefer focused changes that preserve the editorial-industrial visual direction.
- New file formats need parser tests and small, legally redistributable fixtures.
- Avoid broad shell or filesystem permissions in the webview.

## Development checks

Install the repository's secret-scanning hook once before committing:

```bash
pre-commit install
```

The hook and CI use Gitleaks to reject credentials, private keys, and tokens. Never commit real secrets, local `.env` files, signing material, credential stores, or sensitive fixtures. Use explicit placeholders in examples. If a real secret is ever committed, revoke or rotate it immediately; deleting it in a later commit is not sufficient.

```bash
npm ci
npm run build
npm test
npm --prefix mobile ci
npm --prefix mobile run build
npm --prefix mobile test
npm --prefix website ci
npm run website:build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
```

User and operator documentation lives under `website/docs/` and is built with VitePress at `/docs/`. Internal implementation plans remain under `docs/`. Run the public site locally with `npm run website:docs:dev`. Keep commands copyable, mark early-access behavior clearly, and update the relevant guide whenever a user-facing workflow changes.

Use conventional, descriptive commits. Include platform details and reproduction steps for filesystem bugs.
