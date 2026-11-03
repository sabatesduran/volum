# Volum documentation plan

**Canonical public URL:** <https://volum.didac.dev/docs/>

This file tracks documentation coverage. It is excluded from the public VitePress build.

The unchecked items below are real publication or manual-verification work, not missing application code. Store/legal decisions, public minimum-OS claims, physical accessibility certification, and hardware performance guidance must not be checked from simulator or CI results alone.

## Documentation principles

1. Start from the user's goal, not the repository structure.
2. Keep local Desktop as the default path.
3. Mark Companion and dedicated server behavior as early access until their release gates pass.
4. Separate user tasks, operator tasks, reference material, and development instructions.
5. Use commands that are exercised by CI or a documented manual verification.
6. Never include real pairing payloads, tokens, private paths, Tailnet names, or signing data.
7. Keep one canonical page per topic and link to it from README files instead of duplicating long instructions.

## Initial foundation

- [x] Publish a VitePress site under `/docs/` alongside the existing landing page.
- [x] Add local search, responsive sidebars, edit links, page metadata, and generated sitemap support.
- [x] Add a visible documentation link to the product website.
- [x] Add introduction, setup comparison, Desktop installation, and first-library guides.
- [x] Add Companion Access, remote Desktop, mirror, and tray guides.
- [x] Add mobile pairing, browsing, offline, photo, and file-upload guides.
- [x] Add Docker installation, networking, storage, backup, update, and recovery runbooks.
- [x] Document resumable NAS migration, verified abandonment, cutover, and rollback.
- [x] Add architecture, security, compatibility, environment, troubleshooting, and developer references.

## Desktop feature guides

- [ ] Library navigation, search syntax, filters, sorting, and saved searches.
- [ ] Project bundling, file roles, primary previews, merge, and split.
- [ ] Collections, smart rules, tags, notes, favorites, and batch organization.
- [ ] 3MF plates and profiles, STEP metadata, and interactive viewer controls.
- [ ] Photos, custom fields, BOMs, assembly steps, materials, and estimates.
- [ ] Duplicate review, confidence levels, keeper selection, and Trash-safe cleanup.
- [ ] Web imports from MakerWorld and Printables.
- [ ] Project package export and import.
- [ ] Desktop backup destinations, schedules, retention, validation, and restore.
- [ ] Slicer discovery, custom slicers, opening, and revealing files.
- [ ] Language, theme, density, updates, diagnostics, and accessibility controls.

## Companion release documentation

- [ ] Publish confirmed minimum iOS, iPadOS, and Android versions.
- [ ] Replace beta-installation copy with live TestFlight, App Store, and Play Store paths as each becomes available.
- [ ] Add store privacy disclosures and a plain-language mobile privacy page.
- [ ] Add VoiceOver and TalkBack instructions after physical accessibility review.
- [ ] Document low-storage, low-memory, and large-model behavior after device testing.
- [ ] Add screenshots for pairing, Library, Capture, offline content, and project details.
- [ ] Define the support path for firewall, camera permission, revocation, and protocol mismatch issues.
- [ ] Document live-change events if and when they ship.

## Server and NAS documentation

- [ ] Test and publish a pinned version-tag installation example.
- [ ] Add verified Synology Container Manager instructions.
- [ ] Add verified Unraid template instructions.
- [ ] Add verified TrueNAS SCALE instructions.
- [ ] Add tested Caddy and Nginx HTTPS reverse-proxy examples for Desktop clients.
- [ ] Add private-network sidecar examples when host-level Tailscale, WireGuard, or ZeroTier is unavailable.
- [ ] Publish resource guidance after 10,000-project and large-STEP benchmarks.
- [ ] Document server log retention and the `sync_changes` compaction policy after implementation.
- [ ] Add a tested named-volume export and restore recipe.
- [ ] Add server migration and rollback notes to every release that changes storage.

## API and integration reference

- [x] Generate and commit an OpenAPI document for `/api/v1`.
- [ ] Publish endpoint, error-code, pagination, ETag, upload, and capability references.
- [ ] Document protocol compatibility and deprecation policy.
- [x] Add contract-drift validation to CI.
- [x] Document the independent organization-write and photo-upload host policies.
- [ ] Decide whether third-party clients are supported before presenting the API as a public integration surface.

## Website integration

- [x] Reposition the landing page from Desktop-only to the local-first Desktop, mobile, and self-hosted ecosystem.
- [x] Add a Companion early-access section with real product screenshots.
- [x] Add a three-mode setup comparison and link each mode to its guide.
- [x] Add a self-hosting section with a Docker documentation action.
- [ ] Add remote Desktop, managed mirror, and tray availability coverage.
- [x] Correct existing claims about file moves, uploads, and optional network use.
- [ ] Update structured data, social image, FAQ, and download sections for beta availability.

## Documentation quality and operations

- [ ] Add a link checker for internal and external documentation links.
- [ ] Add spelling and terminology checks for `Desktop`, `Companion Access`, `tailscale-http`, `private-http`, `https`, `volumd`, and canonical library.
- [ ] Validate every shell block in a clean environment where practical.
- [ ] Add screenshot capture and compression instructions.
- [ ] Add page ownership or review expectations for releases.
- [ ] Decide whether user documentation will be localized into Catalan and Spanish.
- [ ] Add versioned documentation only when protocol or product divergence makes latest-only docs unsafe.

## Release documentation checklist

For each release:

1. Compare user-visible changes with the public sidebar topics.
2. Update installation and compatibility notes.
3. Update Docker image tags and migration instructions where relevant.
4. Add or refresh screenshots when a workflow changed materially.
5. Verify every new command on its target platform.
6. Build the landing page and docs together.
7. Check `/`, `/docs/`, search, mobile navigation, sitemap output, and edit links.
8. Keep beta labels until the corresponding release gates are complete.
