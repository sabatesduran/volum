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
make setup
make check
```

Run `make help` to target an individual application. The underlying npm and Cargo commands remain available for environments without Make.

Rust work uses the root workspace. Run `make volumd-check` for the no-GUI daemon boundary and `make rust-check` for all adapters and features. API contract changes must update `packages/companion-api/openapi.json`, regenerate `generated.ts`, and pass `generate:check`; do not hand-edit generated types.

User and operator documentation lives under `apps/website/docs/` and is built with VitePress at `/docs/`. Internal implementation plans remain under `docs/plans/`. Run the public site locally with `make website-docs-dev`. Keep commands copyable, mark early-access behavior clearly, and update the relevant guide whenever a user-facing workflow changes.

Use conventional, descriptive commits. Include platform details and reproduction steps for filesystem bugs.
