# Volum Companion and Server Plan

**Status:** In progress — Desktop hosting, dedicated `volumd`, secure mobile browsing, organization, uploads, offline snapshots, and touch 3D inspection implemented
**Last updated:** 2026-11-03
**Products:** Volum Desktop, Volum for iOS and Android (companion role), and the dedicated `volumd` server
**Primary architecture:** One transport-neutral Rust core used by local Desktop, Desktop-hosted access, and a standalone headless server, with trusted clients sharing one versioned API

---

## 1. Decision summary

Volum has an optional Desktop-hosted server, a standalone `volumd` server, and one mobile companion built with Tauri 2 for both iOS and Android.

The recommended product shape is:

- One canonical Desktop or `volumd` server owns the filesystem, SQLite database, indexing, parsing, previews, and host-only destructive operations.
- Desktop Companion Access is disabled by default and can be enabled from **Settings → Mobile access**.
- Clients pair with the canonical host by scanning or pasting a short-lived payload.
- Desktop Companion Access uses automatic Tailscale discovery by default, but Tailscale is not required. Custom mode separates the private listener IP from the private HTTP or trusted HTTPS URL clients use. LAN, WireGuard, Twingate, ZeroTier, and reverse proxies are supported. Volum adds server-identity verification, one-time pairing, per-device tokens, and revocation.
- Volum mobile is a full mobile client delivered in safe phases, not only a read-only companion. The selected canonical host remains authoritative for storage, indexing, parsing, and host-only destructive operations.
- Authenticated mobile writes begin with managed 3D-file uploads, favorites, notes, manual collections, and collection membership, then expand to project documentation and phone photos.
- Touch-friendly 3D inspection and explicit offline availability are part of the mobile product scope.
- Destructive operations, library configuration, scanning, backups, slicer launching, and filesystem-wide actions remain on the canonical host.
- A Desktop host must be running, but it can remain available through menu-bar or system-tray mode after its window closes. `volumd` provides the always-on NAS or home-server path.

### Implementation snapshot

Current implementation includes:

- [x] Cloneable backend service seam for projects, folders, collections, tags, saved searches, photos, and thumbnails.
- [x] Authenticated Axum server with persisted enabled state, automatic Tailscale discovery for Desktop, and configurable headless binding.
- [x] Five-minute, one-use QR pairing secrets; hashed device tokens; last-seen tracking; immediate revocation.
- [x] Sanitized network DTOs that omit absolute and relative asset paths.
- [x] Desktop Companion Access settings, QR display, status, device list, and revocation.
- [x] One Tauri React mobile codebase plus generated iOS and Android native projects.
- [x] QR/manual pairing, native encrypted credential storage, native HTTPS/private-network Rust transport, library/favorites/collections/search, and project details.
- [x] Project photos, custom fields, BOM, assembly steps, files, tags, source attribution, and cover thumbnails.
- [x] Bounded, streaming STL/3MF/OBJ/STEP/STP and photo uploads with progress and native cancellation.
- [x] Editable paired server origin with HTTP port `43821` defaulting, paired-server identity verification, and pull-to-refresh/loading states.
- [x] Native display name and layered platform icons use the Volum brand; “companion” remains a role and protocol name.
- [x] First authenticated organization writes: conflict-checked favorites and notes, manual collection creation, and collection membership.
- [x] Collections have a designed empty state and mobile creation flow.
- [x] Android arm64 debug APK, unsigned release APK, and unsigned iOS device archive builds.
- [x] Live Tailnet pairing/authentication round-trip test.
- [x] Automatic Tailscale and Custom Desktop network modes with separate bind IP and advertised origin.
- [x] Backward-compatible `tailscale-http`, provider-neutral `private-http`, and `https` pairing transports.
- [x] Any trusted HTTPS origin; private-only HTTP with an explicit mobile warning; public HTTP rejection; redirects disabled.
- [x] Editable advertised URL before QR generation, automatic Tailscale/HTTPS pairing, and editable address review after a failed automatic attempt.
- [x] `/identity` verification before a pairing secret or stored token is sent to a changed origin.
- [x] Mobile frontend tests/build, Rust checks/tests, visual regression, Android release build, and unsigned iOS simulator-build CI jobs.
- [x] Offline metadata/media snapshots, mobile localization, revision-safe writes, photo uploads, revision polling, and an interactive 3D viewer.
- [x] Add revision/ETag-aware transient media caching with bounded LRU eviction.
- [x] Complete English, Catalan, and Spanish mobile interface, validation, and server-error localization with source-scanned coverage tests.
- [x] Add authenticated router tests for write-scope downgrade and immediate token revocation.
- [x] Add Playwright visual regression coverage for pairing, library, filters, project detail, and localized settings.
- [x] Install and validate the current debug APK on physical Android, including reconnect, detail, touch 3D, resume, and memory checks.
- [x] Produce and install a current Apple Development-signed iOS IPA on a physical iPhone without replacing the protected Desktop app; retained credentials reconnect to the 589-project library after a cold launch.
- [x] Generate and validate the OpenAPI contract and typed client in CI, including request IDs, resource limits, and plate thumbnails.
- [x] Keep existing beta pairings through a verified one-time Stronghold-to-native-vault migration.
- [x] Separate photo-upload authorization from organization writes and report effective upload capabilities accurately.
- [x] Build and interaction-test the browser administrator, and smoke-test the hardened Docker image and Compose configuration in CI.
- [ ] Add push-style live change events.

### Remaining public-release gates

The remaining blockers are deliberately manual or owner decisions; a green build does not satisfy them:

- [ ] Obtain owner/legal approval for the GPL/App Store distribution strategy and record the decision.
- [ ] Certify iOS 15 and Android API 24 as public minimums on the oldest supported physical devices.
- [ ] Complete QR camera, VoiceOver, TalkBack, network-change, low-memory/storage, large-model GPU/battery, and store-candidate testing on physical devices.
- [ ] Complete App Store and Play privacy disclosures, signed store distribution, and TestFlight/Play internal acceptance.

### Confirmed mobile product scope

The mobile app will include all of the following, delivered incrementally:

1. Library organization: create/edit collections, favorites, tags, and notes.
2. Project management: names, custom fields, BOM items, and assembly steps.
3. Phone capture: create projects and add camera/photo-library images.
4. Touch 3D inspection: viewer meshes and 3MF plate/profile details.
5. Offline access: users explicitly mark individual projects, folders, or collections as **Available offline**. The app shows download, last-sync, stale, and error states and never implies that unmarked content is available offline.

### Non-negotiable design decision

The companion must look and feel like Volum, not like a generic mobile dashboard or a separate product.

It will share the desktop application's:

- warm neutral surfaces;
- orange-red accent;
- light, dark, and system themes;
- typography and numeral treatment;
- borders, radii, shadows, cards, thumbnails, artwork, skeletons, and status treatments;
- restrained editorial-industrial personality;
- English, Catalan, and European Spanish localization.

Layouts and interactions will be mobile-native, but the visual language will be the same. “Same styling” does not mean shrinking the desktop window: the sidebar becomes bottom navigation, inspectors become stacked detail sections, menus become sheets, hover actions become visible touch actions, and all targets meet mobile accessibility sizes.

---

## 2. Why this is worth building

The companion supports a clear workshop use case:

> “Let me check what I have, find the right project, and follow its information while I am beside the printer or assembling the object.”

The current product is particularly suited to this because it already stores:

- visual thumbnails and project cover photos;
- dimensions, file formats, file sizes, geometry metadata, and 3MF profile data;
- favorites, tags, collections, smart collections, and saved searches;
- project notes;
- material and cost estimates;
- bills of materials;
- ordered assembly steps with linked parts and photos;
- source attribution from MakerWorld and Printables.

These are useful away from the desktop even when the phone is not allowed to manipulate the underlying files.

---

## 3. Product goals

### 3.1 Goals

1. Let a user browse and search their Volum library from iOS or Android.
2. Make project photos, BOMs, assembly instructions, notes, dimensions, and print metadata useful at the workbench.
3. Preserve Volum's private, local-first positioning.
4. Ship one mobile codebase without compromising the established visual identity.
5. Keep the desktop filesystem security boundary intact.
6. Make pairing understandable and revocable by a non-technical user.
7. Provide useful last-synced information when the desktop is temporarily unavailable.
8. Leave a clean path to an optional NAS or home-server edition later.

### 3.2 Non-goals for the first release

- Cloud accounts, hosted sync, or a Volum relay.
- Public internet exposure or automatic router configuration.
- Printer control, job monitoring, or slicer integration on mobile.
- Editing, repairing, converting, or slicing model files.
- Renaming, moving, deleting, importing, exporting, or downloading project files from mobile.
- Configuring desktop libraries, backups, materials, apps, or indexing from mobile.
- Full desktop feature parity.
- Running the full desktop indexer inside the mobile application.
- A standalone server daemon in the first release.
- Native iOS/Android home-screen widgets in 0.1. Widgets are deferred until stale-data labeling, shared native snapshot storage, and platform maintenance cost are validated.
- Background polling, background uploads, silent push, and background refresh in 0.1. The app refreshes on foreground/resume; completion alerts are local and only scheduled by foreground work.
- Keeping Desktop alive invisibly. Companion availability ends when Volum quits unless a later, explicit tray/start-at-login option is enabled by the user.

---

## 4. Product principles

1. **Desktop is canonical.** The desktop database and approved library roots remain the source of truth.
2. **Local by default.** Companion Access is opt-in and does not require an account.
3. **No path leakage.** Network responses never include absolute filesystem paths, credentials, or application paths.
4. **Safe before broad.** Start read-only and add narrowly scoped mutations only after connection and conflict behavior are proven.
5. **One visual identity.** Shared tokens and primitives prevent mobile styling from drifting away from desktop.
6. **Mobile is contextual.** Optimize for quick lookup and instructions rather than duplicating every desktop control.
7. **Offline is honest.** Cached data is labeled with its last refresh time; the app never implies it is live when the server is unavailable.
8. **No hidden background behavior.** Desktop clearly shows whether Companion Access is running and when a device last connected.

---

## 5. Current architecture assessment

Volum is well positioned for this work, but the backend needs a service boundary before adding HTTP.

### 5.1 Existing strengths

- `src/lib/tauri/api.ts` already provides a single typed frontend facade.
- `src/types/index.ts` contains a broad TypeScript representation of the domain.
- Rust owns SQLite and filesystem access, so the security boundary is already in the correct layer.
- Model listing already supports cursor pagination and a rich `ModelQuery`.
- Thumbnails, project photos, plate thumbnails, and viewer meshes are already produced as binary responses.
- Project documentation already exposes reusable database functions such as `documentation_for_project` and `custom_field_definitions`.
- The React app already uses TanStack Query, which maps naturally to a remote companion client.
- The Vite configuration already supports `TAURI_DEV_HOST`, which is required for physical-device Tauri development.
- The desktop visual identity is centralized in CSS variables at the top of `apps/desktop/src/styles/global.css`.

### 5.2 Initial gaps that drove the server extraction

- Many Tauri commands combine transport, validation, SQL, filesystem work, and event emission.
- Several command responses are desktop-specific. For example, `ModelDetail` includes `rootPath` and assets include relative paths.
- Thumbnail generation emits directly through `tauri::AppHandle`.
- The global stylesheet mixes tokens, primitives, desktop layout, and feature-specific rules in one file.
- Presentational components such as `ModelCard` call the desktop Tauri API and desktop Zustand store directly.
- The desktop app has no companion-server lifecycle, background/tray behavior, device registry, or pairing UI.
- There is no versioned network contract or generated API client.

The plan therefore introduces narrow reusable service functions and network DTOs rather than serving existing Tauri commands directly.

---

## 6. Implemented target architecture

```text
                   Private network/VPN or trusted HTTPS
┌────────────────────┐       authenticated API      ┌─────────────────────────┐
│ Volum Companion    │ <─────────────────────────> │ Desktop host or volumd  │
│ Tauri iOS/Android  │                              │                         │
│                    │                              │ Axum API adapter        │
│ React mobile UI    │                              │ Tauri command adapter   │
│ Query/cache layer  │                              │          │              │
│ Rust HTTP transport│                              │          ▼              │
│ Secure token store │                              │ Shared library services │
└────────────────────┘                              │          │              │
                                                    │ SQLite + files + cache  │
                                                    └─────────────────────────┘
```

### 6.1 Backend layers

```text
Tauri commands ─┐
                ├── validated library services ── SQLite / filesystem
HTTP routes ────┘
```

The HTTP layer must not invoke Tauri commands or duplicate their SQL. Tauri commands and HTTP routes should be thin adapters over shared service functions.

### 6.2 Mobile layers

```text
React screens and shared Volum UI
                │
TanStack Query + offline repository
                │
Typed companion API client
                │
Tauri commands for restricted private HTTP/trusted HTTPS and secure storage
                │
Canonical Volum host: Desktop or volumd
```

The mobile WebView does not make ordinary JavaScript network requests. A small Rust transport accepts trusted HTTPS and HTTP only for loopback, private IPv4/IPv6, link-local, Tailscale, `.local`, `.ts.net`, or names that resolve entirely to private addresses. It disables redirects, executes requests, and returns typed JSON or binary data to the React layer.

### 6.3 Deployment modes

| Mode | Initial support | Behavior |
| --- | --- | --- |
| Same trusted Wi-Fi | Supported | Custom mode binds an explicit private IP; private HTTP shows a warning and relies on the LAN boundary. |
| Tailscale/WireGuard/ZeroTier | Supported | Tailscale is automatic; other VPN routes use Custom mode. The tunnel protects private HTTP. |
| Twingate/private DNS | Supported | Private HTTP names must resolve entirely to private addresses; trusted HTTPS is also accepted. |
| Public internet | HTTPS only | A trusted reverse proxy must terminate TLS. Public HTTP remains blocked; there is no Volum relay or UPnP. |
| Desktop process | Required when Desktop is canonical | The embedded server runs while Volum is running. |
| Desktop background/tray | Supported | Closing the window can keep indexing and Companion Access active; explicit Quit stops the process. |
| Headless NAS/server | Supported | `apps/volumd` uses the transport-neutral server crate without Tauri or a GUI runtime. |

---

## 7. Repository structure

The product applications now live under `apps/`, while cross-application source lives under `packages/`. Each JavaScript application keeps an independent manifest and lockfile so contributors and deployment systems can install only the product they are working on. The root Makefile is the supported orchestration interface.

```text
volum/
├── apps/
│   ├── desktop/                      # Desktop React app and thin Tauri adapter
│   │   ├── src/
│   │   └── src-tauri/                # Desktop-only native adapter
│   ├── mobile/                       # Tauri iOS/Android companion
│   ├── server-admin/                 # Browser administrator bundled with volumd
│   ├── volumd/                       # Standalone headless daemon
│   └── website/                      # Landing page and public VitePress docs
├── crates/
│   └── volum-server/                 # Transport-neutral Rust services and migrations
├── packages/
│   ├── companion-api/                # OpenAPI, generated types, and client
│   ├── design/                       # Semantic tokens and CSS primitives
│   ├── photo-room/                   # Shared Desktop/Mobile editor and core
│   ├── shared/                       # QR, i18n, formatting, and cache policy
│   └── viewer/                       # Shared viewer parsing and materials
├── assets/brand/                     # Master brand artwork
├── docs/plans/                       # Internal product and implementation plans
├── metadata/                         # Canonical App Store metadata
├── packaging/                        # Distribution packaging
├── Makefile                          # Root development interface
└── package.json                      # npm command aliases
```

### 7.1 Package boundaries

#### `@volum/photo-room`

- Shared Photo Room editor, document model, transforms, localization, and styles.
- Consumed by Desktop and Mobile through a local package dependency.
- Receives platform-specific model renderers and delivery callbacks from each app.

#### Additional package boundaries

- `@volum/design` owns shared theme tokens and portable visual primitives.
- `@volum/shared` owns formatting, localization infrastructure, QR handling, and cache policies.
- `@volum/companion-api` owns the checked-in OpenAPI contract, generated network DTOs, and typed client.
- `@volum/viewer` owns shared mesh parsing, materials, bounds, and normalization.

### 7.2 Rust structure strategy

Shared Rust services live in the transport-neutral `crates/volum-server`. Desktop provides Tauri, backup, event, and credential-store adapters; `apps/volumd` links the core without GUI or native credential-store dependencies. The root Cargo workspace owns dependency versions, profiles, and the single lockfile.

Long-term optional structure:

```text
crates/
├── volum-domain
├── volum-library
├── volum-preview
└── volum-server
```

This long-term extraction is not a prerequisite for the mobile MVP.

---

## 8. Design system and mobile UX specification

### 8.1 Shared visual source of truth

Extract the existing tokens without changing their values:

```css
--canvas: #f4f1ea;
--surface: #fcfaf5;
--surface-strong: #fffefb;
--surface-subtle: #ece8df;
--ink: #191916;
--muted: #74736d;
--faint: #99978f;
--border: #ddd8ce;
--border-strong: #c7c1b5;
--accent: #ff5a36;
--accent-ink: #b52c11;
--success: #2f8f64;
--sidebar: #eae6dc;
--stage: #e8e3d9;
```

The existing dark values, shadows, typography stack, transition timing, and reduced-motion behavior must also move into the shared package unchanged.

Add named semantic tokens for currently hard-coded destructive and warning colors before mobile uses them:

```css
--danger: ...;
--warning: ...;
--scrim: ...;
```

No mobile component may introduce an independent brand palette.

### 8.2 Shared versus platform-specific UI

### Share directly

- `Brand` and app icon treatment.
- `ModelArt` fallback silhouettes.
- Theme tokens and base styles.
- Model/project card visual primitive.
- Thumbnail and photo frame visual primitives.
- Buttons, icon buttons, fields, skeletons, empty states, badges, and metadata rows.
- Formatting and localization behavior.
- Three.js viewer rendering after its data loader is injected.

### Keep desktop-specific

- Sidebar and desktop top bar.
- Drag selection and desktop contextual menus.
- File pickers, slicer menus, reveal/open actions, backup settings, and indexing status.
- Desktop split-pane model detail layout.

### Build mobile-specific

- Safe-area-aware app shell.
- Bottom navigation.
- Mobile search and filter sheet.
- Pairing and server status screens.
- Pull to refresh and offline banners.
- Touch action sheets and confirmation sheets.
- Stacked project detail screen.
- Mobile cache settings.

### 8.3 Mobile adaptation rules

| Desktop pattern | Mobile companion pattern |
| --- | --- |
| 224 px sidebar | Bottom navigation with 4–5 destinations. |
| Top search field | Sticky search bar or dedicated Search destination. |
| Hover card actions | Visible favorite action plus overflow sheet; no hover dependency. |
| Right-side inspector | Full-width stacked project detail. |
| Fixed popover menus | Bottom sheets or anchored menus with touch-sized rows. |
| Dialog | Bottom sheet on phones; centered dialog on tablets. |
| Dense 7–12 px metadata | Preserve hierarchy but use readable mobile sizes. |
| 36–38 px controls | Minimum 44×44 pt on iOS and 48×48 dp where Android guidance requires it. |
| Keyboard-first navigation | Touch-first with complete screen-reader and external-keyboard support. |
| Scroll-to-zoom hint | Pinch to zoom and drag to rotate. |

### 8.4 Mobile information architecture

Recommended bottom navigation:

1. **Library** — recent projects, collection shortcuts, and the main grid.
2. **Favorites** — quick workshop access.
3. **Collections** — manual and smart collections.
4. **Search** — search, formats, tags, folders, and date filters.
5. **Settings** — paired server, cache, theme, language, privacy, and about.

On compact widths, keep five icons with short labels. On tablets, use the same navigation state in a compact rail if it improves space without recreating the desktop sidebar.

### 8.5 Key screens

### Pairing/onboarding

- Volum brand and the existing dark contour artwork adapted to portrait.
- Explain that the desktop stays the source and must be running.
- Primary action: **Scan pairing code**.
- Secondary action: **Enter address manually**.
- Request camera and local-network permissions only when needed.
- Show the desktop name and library project count before completing pairing.

### Library

- Warm canvas and desktop-style view header.
- Connection state beside the header, not as a permanent alarming banner.
- Two-column model grid on ordinary phones.
- Three columns where width permits; tablet layout follows available card width.
- Same 4:3 preview, radius, border, shadow, format label, title, and context line as desktop.
- Collection row follows the desktop home treatment and scrolls horizontally when necessary.

### Project detail

- Cover/thumbnail stage first; interactive 3D is a later enhancement.
- Back, favorite, and overflow controls float over the stage using the existing translucent treatment.
- Project title, format/file count eyebrow, and location summary.
- Stacked sections for details, source, 3MF profile, files, photos, custom fields, BOM, assembly guide, tags, collections, cost, and notes.
- Project photos open into a full-screen gallery.
- Assembly steps receive larger typography and photos because this is a primary mobile use case.
- Desktop-only actions are omitted rather than shown disabled.

### Settings

- Connected desktop identity and current reachability.
- Last successful refresh.
- Cache size and clear-cache action.
- Theme and language using the same controls and values as desktop.
- Unpair action with a clear explanation.
- Privacy copy stating that no cloud account or relay is used.

### 8.6 Visual acceptance criteria

- [ ] Light and dark screenshots visibly belong to the same design system as Desktop.
- [ ] Shared colors are imported from `@volum/design`; no duplicated palette exists under `apps/mobile/`.
- [ ] Cards use the same aspect ratio, visual hierarchy, radii, and thumbnail treatment.
- [ ] The Volum brand, icon, eyebrow style, accent use, and warm surfaces match Desktop.
- [ ] iOS and Android layouts respect safe areas and system bars.
- [ ] No essential action depends on hover, right click, or a hardware keyboard.
- [ ] Text remains readable at the supported accessibility text scale.
- [ ] Touch targets meet the chosen platform minimums.
- [ ] Reduced motion disables nonessential transitions and loading movement.
- [ ] WCAG AA contrast is maintained in both themes.

---

## 9. Release scope

### 9.1 Companion 0.1 — secure read-only library

Required:

- QR pairing and manual pairing fallback.
- One paired desktop at a time, with data structures that permit multiple servers later.
- Library, recent, favorites, folders, collections, and search.
- Cursor-paginated project grid.
- Project detail and asset metadata.
- Project photos and cover photos.
- Custom values, BOM, and assembly guide.
- Notes, tags, collection membership, cost estimate, web source, and 3MF profile as read-only data.
- Thumbnail and photo cache.
- Last-synced offline browsing.
- Light, dark, and system themes.
- English, Catalan, and European Spanish.
- Desktop device list and revoke action.
- Clear server-unavailable and token-revoked states.

Not in 0.1:

- Mobile mutations.
- Phone uploads.
- Interactive 3D.
- Desktop background/tray mode.
- Automatic mDNS discovery if QR-provided addresses are sufficient.

### 9.2 Companion 0.2 — safe organization changes

Candidate scope after 0.1 proves stable:

- Toggle favorite.
- Add/remove manual collection membership.
- Add/remove existing tags.
- Edit notes.
- Pull live change notifications while connected.
- Explicit `write:organization` permission granted by the desktop owner.
- Optimistic updates with rollback and server-authoritative refresh.

No destructive file actions are added.

### 9.3 Companion 0.3 — workshop capture and richer viewing

- Capture or select project photos from the phone.
- Upload with progress, cancellation, server-side size/type validation, and explicit project selection.
- Edit photo caption and request cover status.
- Interactive Three.js viewer using the existing `VLM1`/`VLM2` mesh payload.
- Plate selection for multi-plate 3MF projects.
- Optional device biometrics before opening a cached library.
- Optional multiple paired desktops.

### 9.4 Desktop 0.x follow-up — availability

- Tray/menu-bar status.
- Optional “Keep Volum running for Companion Access.”
- Optional start at login.
- Sleep/wake and interface-change recovery.
- Clear quit behavior that stops the server.

### 9.5 Implemented — headless Volum server

- [x] Extract transport-neutral library services into `crates/volum-server/`.
- [x] Build `apps/volumd/` for an always-on home server or NAS without a GUI runtime.
- [x] Provide explicit `/config` and `/library` container volumes plus operator guidance.
- [x] Reuse the same companion API and pairing protocol as Desktop hosting.
- [x] Keep the server an independent early-access deployment choice rather than a condition for mobile launch.

---

## 10. Companion API contract

### 10.1 Contract rules

- Base path: `/api/v1`.
- JSON uses camelCase, matching the current frontend.
- IDs remain opaque UUID strings.
- Project terminology is used publicly even if the legacy database table is named `models`.
- Pagination uses opaque cursors; offsets are not used by the mobile client.
- Default page size is 48; server maximum is 100 for mobile endpoints.
- Every response includes an API version header and request ID.
- Errors use a stable machine-readable shape.
- Binary responses include `Content-Type`, `Content-Length`, `ETag`, and conservative cache headers.
- The API never returns `rootPath`, slicer paths, backup locations, credentials, database paths, cache paths, or unrestricted relative paths.
- Exposed folder/location text is a deliberate display DTO, not a raw database row.

Suggested error shape:

```json
{
  "error": {
    "code": "project_not_found",
    "message": "Project not found",
    "requestId": "...",
    "retryable": false
  }
}
```

### 10.2 Network DTOs

Create companion-specific Rust DTOs. Do not serialize `ModelDetail` directly.

Minimum DTO set:

- `ServerIdentity`
- `ServerStatus`
- `ServerCapabilities`
- `PairingRequest`
- `PairingResult`
- `CompanionProjectSummary`
- `CompanionProjectDetail`
- `CompanionAsset`
- `CompanionFolder`
- `CompanionCollection`
- `CompanionTag`
- `CompanionDocumentation`
- `CompanionPhoto`
- `CompanionPage<T>`
- `ApiError`

Important field decisions:

- Keep `rootName`, `folderName`, and a sanitized display folder when useful.
- Omit absolute roots and OS-native paths.
- Omit asset `relativePath` initially; expose `filename`, role, extension, size, modified date, parse state, and metadata.
- Keep public source URLs because they are intentional user-facing attribution links.
- Add stable revision fields for covers, photos, and projects to drive cache invalidation.

### 10.3 Initial endpoints

### Pairing and identity

| Method | Endpoint | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/identity` | None | Minimal server ID, name, version, and protocol range. No library data. |
| `POST` | `/pair` | One-time secret | Register a device and return its scoped bearer token once. |

### Read-only library

| Method | Endpoint | Purpose |
| --- | --- | --- |
| `GET` | `/api/v1/status` | Server version, capabilities, project count, index freshness, and library availability summary. |
| `GET` | `/api/v1/projects` | Search/filter/sort with opaque cursor pagination. |
| `GET` | `/api/v1/projects/{id}` | Sanitized project detail and asset metadata. |
| `GET` | `/api/v1/projects/{id}/documentation` | Photos, custom values, BOM, and assembly steps. |
| `GET` | `/api/v1/projects/{id}/cover` | Resolved cover photo or generated primary-asset thumbnail. |
| `GET` | `/api/v1/assets/{id}/thumbnail` | Asset thumbnail where needed. |
| `GET` | `/api/v1/assets/{id}/plates/{index}/thumbnail` | Embedded 3MF plate image. |
| `GET` | `/api/v1/photos/{id}` | Full or thumbnail project photo selected by query parameter. |
| `GET` | `/api/v1/folders` | Sanitized hierarchy and project counts. |
| `GET` | `/api/v1/collections` | Manual and smart collection summaries. |
| `GET` | `/api/v1/tags` | Tag summaries. |
| `GET` | `/api/v1/saved-searches` | Saved query definitions supported by the mobile filter contract. |
| `GET` | `/api/v1/custom-fields` | Reusable field definitions required to label custom values. |

### Organization mutation endpoints

| Method | Endpoint | Required Desktop permission |
| --- | --- | --- |
| `PATCH` | `/api/v1/projects/{id}` | `write:organization` for a conflict-checked rename, favorite, notes, or tag membership change. |
| `POST` | `/api/v1/collections` | `write:organization` |
| `PUT` / `DELETE` | `/api/v1/collections/{collectionId}/projects/{projectId}` | `write:organization` |
| `POST` | `/api/v1/tags` | `write:organization` |
| `PUT` | `/api/v1/projects/{id}/custom-fields/{fieldId}` | `write:organization` |
| `POST` / `DELETE` | `/api/v1/projects/{id}/bom[/{itemId}]` | `write:organization` |
| `POST` / `DELETE` | `/api/v1/projects/{id}/assembly-steps[/{stepId}]` | `write:organization` |
| `POST` | `/api/v1/projects/{id}/photos` | Future `write:photos` permission. |

Do not add remote endpoints for open, reveal, slicer launch, trash, permanent delete, root changes, backup restore, or diagnostics export. Managed renames must continue through Desktop's authoritative `project_storage` pipeline.

### 10.4 Media behavior

- Cover requests resolve to the user-selected photo first, then the primary asset thumbnail.
- If a thumbnail exists, return it immediately.
- If generation is queued, return `202 Accepted` with `Retry-After` and a placeholder state rather than holding a mobile request open for an unbounded render.
- Keep the existing two-worker rendering limit and pending-queue cap.
- Support conditional requests through `ETag`/`If-None-Match`.
- Enforce response-size limits before exposing viewer meshes.
- Use IDs to resolve files and retain all existing path validation.
- Do not offer arbitrary file-serving routes.

### 10.5 API schema generation

Use an OpenAPI document as the contract source:

1. Annotate network DTOs and routes in Rust, preferably with `utoipa` or an equivalent maintained crate.
2. Commit the generated OpenAPI JSON.
3. Generate TypeScript types in `packages/companion-api`.
4. Fail CI if regeneration changes committed output.
5. Maintain additive compatibility within `/api/v1`.
6. Report server and client protocol ranges during pairing and status checks.

The desktop's internal TypeScript domain types may continue to exist. Network DTOs must remain explicitly separate so private fields cannot be exposed accidentally.

---

## 11. Connection, pairing, and security design

### 11.1 Threat model

Protect against:

- another device on the LAN browsing the library without approval;
- interception of project names, notes, photos, or tokens on shared Wi-Fi;
- a malicious local website probing the server;
- replay or brute force of a pairing code;
- a stolen phone retaining access after revocation;
- path traversal or arbitrary file reads through media endpoints;
- secrets leaking into logs, backups, diagnostics, crash output, or QR history;
- an attacker substituting a different server during pairing.

The first version does not attempt to protect data on an already-unlocked, compromised desktop or phone.

### 11.2 Private-network and HTTPS transport

The release supports three backward-compatible transport identifiers:

1. `tailscale-http`: automatic Desktop discovery and all existing QR payloads; Tailscale's WireGuard tunnel protects application HTTP.
2. `private-http`: an explicit LAN, WireGuard, Twingate, ZeroTier, or other private route. HTTP is allowed only for private/link-local/Tailscale IPs or names that resolve entirely to private addresses. Mobile shows an explicit warning because the network or VPN supplies confidentiality.
3. `https`: any origin whose TLS certificate is trusted by the operating system. This is required for public or otherwise non-private routes.

Desktop creates a stable random Volum server ID. Automatic mode binds only to the discovered Tailscale IP. Custom mode binds only to an explicit private IP and separately advertises the origin clients use; wildcard and public listener IPs are rejected. Public HTTP is always rejected, redirects are disabled, and no mode uses UPnP or a hosted Volum relay.

### 11.3 Pairing flow

1. User enables Companion Access in Desktop settings.
2. User selects **Pair a device**.
3. Desktop creates a cryptographically random, one-use pairing secret with a five-minute lifetime.
4. Desktop displays a QR code containing protocol version, server ID, editable server origin, transport identifier, and the one-time secret.
5. The companion scans the QR using Tauri's mobile barcode-scanner plugin.
6. Tailscale and HTTPS payloads attempt connection automatically. Private HTTP opens an address-review screen with an explicit LAN/VPN trust confirmation; a failed automatic attempt opens the same review screen for editing and retrying.
7. Mobile validates the transport policy and calls unauthenticated `/identity`; it checks server ID and protocol compatibility before sending the secret.
8. Mobile calls `/pair` with device name and platform. The canonical host verifies and consumes the one-time secret.
9. The host returns a random 256-bit bearer token once.
10. Server stores only a token hash and device metadata.
11. Mobile stores the token and server identity in a Rust-side authenticated vault keyed by iOS Keychain or Android Keystore.
12. The canonical host lists the device with paired and last-seen timestamps.

Manual entry parses the same payload and preserves the same HTTPS/private-HTTP policy. Later address edits perform `/identity` verification before a stored token is used against the new origin.

### 11.4 Authorization

Initial scope:

- `read:library`

Later optional scopes:

- `write:organization`
- `write:photos`

Rules:

- Every `/api/v1` request requires a bearer token.
- Tokens belong to one device and can be revoked independently.
- Scope checks happen before route handlers.
- Revoked or unknown devices receive `401 unauthorized`.
- A paired device can revoke itself with `DELETE /api/v1/device`; successful self-revocation returns `204 No Content`.
- Pairing endpoints have strict attempt and time limits.
- Do not use cookies; this avoids ambient browser credentials and CSRF behavior.
- CORS is denied by default. Development-only browser mocks run separately.

### 11.5 Companion security state storage

Do not put pairing secrets or device tokens in `volum.sqlite3`, because that database is included in portable backups.

Use a separate local security store under the app-data directory for:

- server ID;
- enabled state and selected port;
- paired device metadata;
- token hashes and scopes;
- last-seen timestamps.

The separate companion store is excluded from Volum backups and restore archives. Restoring a library backup must not silently copy trusted devices to another machine.

### 11.6 Network behavior

- Bind only after explicit enablement.
- Default to the active Tailscale interface, with explicit Custom mode for another private listener IP.
- Persist an automatically selected available port; permit reset if another application claims it.
- Do not bind to wildcard or public addresses. Custom mode may bind an explicit LAN, VPN, loopback, or link-local IP.
- Do not advertise through public discovery services.
- Add Bonjour/mDNS (`_volum._tcp`) only after QR pairing works reliably.
- Treat mDNS as convenience discovery, never as authentication.
- Recover after Wi-Fi interface changes and desktop sleep/wake.
- Explain operating-system firewall prompts in the Desktop UI.
- Never use UPnP or automatically modify a router.

### 11.7 Logging and diagnostics

- Assign request IDs but do not log bearer tokens, pairing secrets, query strings containing searches, notes, filenames, or photo contents.
- Log route templates, status, duration, and redacted device IDs.
- Mobile diagnostics omit tokens and certificate private data.
- Desktop diagnostics redact local IP addresses by default.
- No analytics are introduced.

---

## 12. Desktop server implementation

### 12.1 Service extraction

Create transport-independent functions for the first server routes:

- list projects;
- get project detail;
- list roots/folders in sanitized form;
- list collections, tags, and saved searches;
- load project documentation and field definitions;
- read project photos;
- read/request project thumbnails;
- read plate thumbnails;
- later read viewer meshes.

Each existing Tauri command becomes an adapter that receives `tauri::State`, calls a service, and converts the output when necessary.

Introduce a cloneable `LibraryServices` or similarly named context containing only the resources required by both transports:

- `SqlitePool`;
- data directory;
- thumbnail worker semaphore;
- thumbnail pending/cancellation state;
- validated asset/photo path resolver;
- a transport-neutral event publisher.

Keep filesystem watchers, scan pause state, native app launching, and other desktop-only state outside this context.

### 12.2 Event decoupling

Replace direct `AppHandle` dependence in reusable work with a small event abstraction or broadcast channel:

```text
Preview worker
  └── publishes preview-ready / preview-error
        ├── Tauri event bridge
        └── optional companion change stream
```

The 0.1 mobile client may poll or refresh on foreground. A live stream is not required until 0.2.

### 12.3 HTTP server

Recommended stack:

- `axum` for routing and extractors;
- `tower`/`tower-http` for limits, request IDs, tracing, and timeouts;
- Tailscale/WireGuard as the automatic path, explicit private-network routes, or trusted HTTPS;
- `tokio-util::CancellationToken` for clean start/stop;
- a maintained QR encoder for the Desktop pairing dialog;
- an explicit rate limiter for pairing and authentication failures.

Add a `CompanionServerController` to desktop state that can:

- start once;
- stop cleanly;
- report network mode, listener/address, transport, server identity, and status;
- create and consume short-lived pairing sessions;
- revoke a device;
- shut down during application exit.

### 12.4 Desktop settings UI

Add a **Companion Access** section near Privacy & data:

- enable/disable toggle;
- concise explanation of local access and desktop availability;
- running/stopped/error status;
- local address and port as diagnostic information;
- **Pair a device** button;
- QR dialog with expiration countdown and cancel action;
- paired-device rows with device name, platform, scopes, paired date, last seen, and revoke;
- automatic Tailscale and Custom LAN/VPN/HTTPS address controls;
- later background/tray controls.

Turning access off must close the listener immediately while retaining device records unless the user explicitly revokes or resets them.

Resetting server identity must warn that every companion will be disconnected.

---

## 13. Mobile application implementation

### 13.1 Tauri application identity

Proposed values, subject to store availability checks:

- Product name: **Volum Companion**
- iOS bundle ID: `com.dsd.volum.companion`
- Android application ID: `com.dsd.volum.companion`
- URL scheme: `volum://`

The home-screen display name may be shortened to **Volum** if store and platform metadata clearly describe it as the companion.

### 13.2 Tauri mobile responsibilities

Use Tauri for:

- one React UI codebase on iOS and Android;
- QR scanning through the official barcode-scanner plugin;
- Rust-side authenticated secret storage backed by iOS Keychain or Android Keystore;
- a custom Rust command restricted to trusted HTTPS or private HTTP hosts;
- opening intentional external URLs;
- optional haptics and biometrics later;
- platform build/signing integration.

Do not compile desktop indexing, OpenCascade parsing, filesystem watching, backup, or slicer logic into the mobile app.

### 13.3 Required platform permissions

### iOS

- `NSCameraUsageDescription` for QR pairing.
- `NSLocalNetworkUsageDescription` for connecting to Volum Desktop.
- `NSBonjourServices` if `_volum._tcp` discovery is added.
- Photo library/camera usage descriptions only when phone photo upload ships.

### Android

- Internet/network access.
- Camera permission for QR pairing.
- Multicast/nearby-network permissions only if discovery requires them for supported Android versions.
- Photo picker/camera permissions only when uploads ship; prefer the system photo picker where possible.

Request permissions just in time and provide a manual fallback where possible.

### 13.4 Mobile data and state

- TanStack Query manages in-memory server state.
- Zustand or a similarly small store manages navigation, selected server, theme, language, and transient UI state.
- Secure storage contains the paired server identity, address, device ID, and token only.
- IndexedDB stores sanitized explicit offline snapshots and a separate transient media cache.
- Transient thumbnails, photos, and viewer meshes use a bounded LRU store; explicit offline media is never evicted by it.
- Clearing the cache does not unpair the device.
- Unpairing removes tokens, pins, API snapshots, and cached media for that server.

### 13.5 Offline behavior

The 0.1 app is online-first with a read-only cache:

- Cache each successful list page and project detail/documentation response.
- Cache thumbnails and photos using server ID, entity ID, revision/ETag, and requested size.
- Show cached content immediately, then refresh when reachable.
- Display **Last updated …** when data is stale.
- Provide pull to refresh and a retry action.
- Distinguish desktop unavailable, wrong network, revoked device, changed server identity, and incompatible protocol.
- Never queue mutations offline until a conflict model is designed.

Initial transient media budget:

- default cap: 96 MB and 192 entries;
- user-selectable smaller cap later;
- LRU eviction of media only;
- metadata snapshots retained unless the user clears cache or unpairs.

### 13.6 Interactive viewer reuse

When the viewer milestone begins:

1. Refactor `ModelViewer` to accept a mesh loader instead of importing the desktop API directly.
2. Reuse `parseViewerMesh`, normalization, materials, lighting, and camera views.
3. Add touch-specific OrbitControls configuration.
4. Limit mesh payload size and triangle count on the server.
5. Cache only a small number of recent meshes.
6. Test WebGL behavior and memory pressure on physical iOS and Android devices.
7. Keep thumbnails as a fallback when a mesh is too large or WebGL initialization fails.

The viewer is not allowed to block the read-only launch.

---

## 14. Phased implementation plan

Each phase should land as one or more reviewable pull requests and preserve all existing desktop tests.

### Phase 0 — decisions and technical spike

**Purpose:** retire the highest-risk unknowns before restructuring production code.

- [x] Confirm native app name and bundle/application IDs (`Volum`, `com.dsd.volum.companion`).
- [x] Encode provisional minimums from current Tauri/plugin support (iOS 15 and Android API 24); public certification remains a release gate above.
- [ ] Audit repository contributors and choose a mobile/App Store licensing strategy.
- [x] Initialize the production Tauri mobile app and generated iOS/Android projects.
- [x] Build and run it on one physical iPhone and one physical Android device.
- [x] Verify shared CSS tokens and the model card render correctly in both WebViews.
- [ ] Verify official QR scanning on both platforms.
- [x] Prove the Rust mobile command permits trusted HTTPS/private routes, blocks public HTTP, and completes a live pairing/authentication round trip over Tailscale.
- [x] Implement native authenticated-vault persistence for the paired server identity and token.
- [x] Verify binary image transfer and blob/cache rendering on physical iOS and Android devices.
- [ ] Record WebGL availability and a representative `VLM2` mesh result, without committing to viewer launch scope.

**Exit criteria:** pairing transport, secure storage, QR scanning, physical-device builds, and shared styling are all feasible. HTTP remains confined to explicit private network/VPN routes; public routes require HTTPS.

### Phase 1 — workspace and design foundation

**Purpose:** share identity without destabilizing desktop layout.

- [ ] Enable npm workspaces for `mobile` and `packages/*`.
- [x] Create `@volum/design`, `@volum/shared`, and `@volum/companion-api` packages.
- [x] Extract theme variables, base typography, reduced motion, and semantic status colors.
- [x] Extract portable button, field, card, skeleton, thumbnail, and artwork styles.
- [ ] Refactor `Brand` and `ModelArt` so asset paths and translation are injectable or package-safe.
- [ ] Introduce a pure project-card visual component; keep desktop data/actions in a wrapper.
- [x] Extract formatting and the i18n engine/dictionaries without changing current output.
- [x] Keep desktop-specific layout CSS in the desktop app.
- [ ] Add desktop visual regression screenshots before and after extraction.
- [x] Add package build/typecheck scripts to root orchestration.

**Exit criteria:** desktop has no intended visual change, while a mobile story/test page renders the same brand, tokens, cards, and both themes.

### Phase 2 — backend service seam

**Purpose:** separate business logic from Tauri transport.

- [x] Introduce a cloneable shared service context.
- [x] Extract list-projects query and cursor behavior.
- [x] Extract get-project detail assembly.
- [x] Extract folders, collections, tags, and saved searches queries.
- [x] Reuse existing project documentation and custom-field query functions.
- [x] Extract validated project photo reads.
- [x] Extract thumbnail read/request logic.
- [x] Provide thumbnail reads that do not require an `AppHandle`.
- [x] Keep existing Tauri command names and TypeScript behavior unchanged.
- [x] Add service-level tests against temporary migrated databases.

**Exit criteria:** the required read-only use cases can be called without constructing a Tauri command invocation or exposing a raw path.

### Phase 3 — versioned read-only HTTP API

**Purpose:** establish a safe contract before pairing UI exists.

- [x] Add companion-specific DTOs with no absolute or relative asset path fields.
- [x] Add API error and request-ID middleware.
- [x] Add `/identity`, status, project list/detail, documentation, collection, folder, tag, saved-search, custom-field, cover, thumbnail, and photo routes.
- [x] Add a plate-thumbnail route.
- [x] Add request body, query, timeout, concurrency, and response-size limits.
- [x] Add ETags and conditional media requests.
- [ ] Return `202`/`Retry-After` for queued thumbnails.
- [x] Generate and commit OpenAPI.
- [x] Generate TypeScript DTOs/client.
- [x] Add a CI contract drift check.
- [x] Add tests that serialize responses and reject forbidden path fields.
- [x] Add route integration tests using a temporary database and media directory.

**Exit criteria:** an authenticated test client can browse a fixture library entirely through `/api/v1`, and a path-leak regression test passes.

### Phase 4 — server identity, authentication, and pairing

**Purpose:** make the API safe to bind to a real network.

- [x] Implement the separate, backup-excluded `companion.json` security store.
- [x] Generate and persist a stable server ID.
- [x] Bind Automatic mode only to the active Tailscale IPv4 address and Custom mode only to an explicit private IP.
- [x] Implement in-memory one-time pairing sessions with five-minute expiry and one-use consumption.
- [x] Implement device token issuance, hashing, last-seen updates, and revocation; the current API is read-only.
- [x] Limit failed attempts for each pairing session.
- [x] Add a start/stop/restart controller and clean shutdown.
- [x] Discover the active Tailnet address and MagicDNS name without placing tokens or pairing secrets in logs.
- [x] Add trusted HTTPS and private-LAN/VPN HTTP transport policy with server-identity verification before secrets or tokens.
- [x] Add a live ignored integration test for one-use pairing, authenticated access, and immediate revocation.
- [ ] Add non-network unit coverage for expiry and failed-attempt exhaustion.
- [ ] Add platform tests proving the listener never binds to wildcard or public addresses and Custom mode binds only the requested private IP.

**Exit criteria:** no library endpoint is reachable without a valid device token over an approved private or HTTPS transport, and revocation takes effect immediately.

### Phase 5 — Desktop Companion Access UI

**Purpose:** give the owner complete visibility and control.

- [x] Add Companion Access DTOs and Tauri commands for status, enable, disable, create pairing session, list devices, and revoke.
- [ ] Add an explicit reset-server-identity control if needed after hardware testing.
- [x] Add the Settings section using existing Volum settings-card styling.
- [x] Add running/stopped/error status and actionable error copy.
- [x] Add a QR pairing dialog with expiry countdown and close control.
- [x] Add the paired-device list and revoke confirmation.
- [x] Add automatic Tailscale plus Custom listener/advertised-address controls and editable QR origin.
- [x] Add translations in English, Catalan, and Spanish.
- [x] Update privacy copy to distinguish optional companion traffic from cloud uploads.
- [x] Ensure disabling closes the listener immediately.
- [ ] Test firewall-denied and port-conflict states.

**Exit criteria:** a desktop user can enable access, pair/revoke a device, see server state, and disable all network listening without editing configuration files.

### Phase 6 — production mobile shell and pairing

**Purpose:** establish the real iOS/Android app.

- [x] Create the `apps/mobile/` Tauri application and production identifiers.
- [x] Add mobile Vite, TypeScript, Vitest, and Tauri configurations.
- [x] Add initial mobile transport Vitest coverage for server-address normalization and selected-file filename handling.
- [x] Add CI jobs for the mobile frontend, Rust checks/tests, visual regression, Android arm64 release build, and unsigned iOS simulator build.
- [x] Add shared design and localization packages.
- [x] Add a safe-area shell, system light/dark theme application, and mobile navigation.
- [x] Add camera/local-network permission strings and Android manifest entries.
- [x] Add QR scanner and manual-entry flow.
- [x] Implement a Rust transport for trusted HTTPS and private-only HTTP, with DNS validation and redirects disabled.
- [x] Store the server identity and token in the native authenticated vault.
- [x] Add protocol compatibility and server-identity checks during pairing.
- [x] Add editable post-scan address review, explicit private-HTTP warning, and identity checks before pairing or changing address.
- [x] Add user-friendly connection, unavailable, and revoked states.
- [x] Add unpair and re-pair flows.
- [ ] Verify release-mode builds on physical iOS and Android devices.

**Exit criteria:** both platforms can pair, restart the app, reconnect securely, detect revocation, and reject a changed server identity.

### Phase 7 — mobile read-only library

**Purpose:** deliver the first complete product experience.

- [x] Implement the status/bootstrap query.
- [x] Implement Library, Favorites, Collections, and Search queries.
- [x] Add Recent and Folder browsing within Library.
- [x] Implement a cursor-based infinite project grid.
- [x] Implement Volum-style project cards and authenticated thumbnail loading.
- [x] Implement mobile filters supported by the server contract.
- [x] Implement project detail with safe metadata and sanitized file records.
- [x] Implement the project photo gallery.
- [x] Implement custom fields, BOM, and assembly guide.
- [x] Implement source attribution.
- [x] Add intentional, validated external URL opening.
- [x] Implement 3MF profile and plate-thumbnail display.
- [x] Implement empty, loading, error, offline, and revoked states.
- [x] Complete English, Catalan, and Spanish strings, including validation and server-error fallbacks.
- [x] Add accessibility labels and reduced-motion support.
- [ ] Verify traversal order and dynamic text on physical VoiceOver and TalkBack devices.

**Exit criteria:** a user can locate a project and consume all workshop-relevant information on either platform without encountering desktop-only actions.

### Phase 8 — offline cache and resilience

**Purpose:** make the companion useful when the desktop sleeps or leaves the network.

- [x] Add sanitized metadata snapshot repository.
- [x] Add revision/ETag-aware image cache.
- [x] Add explicit Available offline controls for projects, folders, and collections.
- [x] Expand a pinned folder or collection into a refreshable project/media manifest.
- [x] Show download progress, last synced, stale, and storage-use states for every offline selection.
- [x] Add LRU media eviction and cache-size reporting.
- [x] Render cached data immediately and refresh in the background.
- [x] Add pull to refresh across refreshable mobile screens.
- [x] Add last-synced indicators.
- [x] Handle app suspend/resume and network interface changes.
- [ ] Handle desktop sleep/wake and server restart.
- [x] Add clear-cache without unpairing.
- [x] Ensure unpair removes all server-specific cached content.
- [x] Test migration of cache schema between mobile app versions.

**Exit criteria:** previously opened library pages and project details remain readable offline, are labeled as cached, and refresh correctly when the server returns.

### Phase 9 — quality, beta, and 0.1 release

**Purpose:** ship read-only companion safely.

- [x] Complete code-level security review and path-leak audit.
- [x] Add automated mobile visual regression baselines and CI comparison.
- [ ] Run performance tests with a 10,000-project fixture.
- [ ] Run network matrix tests on normal Wi-Fi, guest Wi-Fi isolation, IPv4, IPv6, VPN/Tailscale, sleep/wake, and address changes.
- [ ] Run accessibility review on VoiceOver and TalkBack.
- [ ] Test low-memory and low-storage behavior.
- [ ] Add App Store and Play Store privacy disclosures consistent with no analytics/account/cloud relay.
- [ ] Resolve GPL/App Store distribution strategy before submission.
- [ ] Produce screenshots and store metadata.
- [ ] Run TestFlight and Play internal testing.
- [ ] Document Desktop version compatibility and troubleshooting.
- [ ] Define support process for pairing and firewall issues.

**Exit criteria:** signed store candidates pass physical-device, security, accessibility, offline, and compatibility acceptance tests.

### Phase 10 — safe mutations

**Purpose:** add convenience without creating remote file risk.

- [x] Add desktop permission control for `write:organization`, defaulting to disabled and enforced server-side.
- [x] Add conflict-checked favorite, manual collection creation/membership, and notes endpoints.
- [x] Add project rename, custom-value, BOM, and assembly-step endpoints and mobile controls.
- [x] Add tag creation/membership endpoints and mobile controls.
- [x] Add bounded request validation for mutations and uploads.
- [ ] Complete a dedicated audit-log review proving mutation logs never contain content values.
- [x] Add optimistic project, favorite, tag-membership, collection-membership, and collection updates with rollback.
- [x] Require a live Desktop connection for edits.
- [x] Add revision preconditions and explicit conflict errors for project and documentation edits.
- [x] Refresh affected project/list/collection queries after mutation.
- [x] Test scope downgrade and immediate revocation at the authenticated router boundary.

**Exit criteria:** permitted organization changes are reliable, conflicts are explicit, managed renames stay inside the authoritative Desktop storage pipeline, and destructive filesystem actions remain unavailable.

### Phase 11 — phone photos and viewer

**Purpose:** deepen workshop usefulness after the core is stable.

- [x] Add separately grantable `write:photos` host policy and capability.
- [x] Add camera/system photo picker flow.
- [x] Add bounded, streaming upload with progress and cancellation.
- [x] Reuse existing server-side photo validation and thumbnail creation.
- [x] Add caption and cover controls.
- [x] Add authenticated binary viewer transport without exposing source paths.
- [x] Add a bounded viewer-mesh endpoint backed by Desktop's existing mesh cache.
- [x] Add lazy-loaded touch orbit, pinch zoom, reset, and cover/3D switching controls.
- [x] Add 3MF plate selection.
- [ ] Measure battery, memory, GPU use, and large-model failure behavior.

**Exit criteria:** photos and 3D viewing remain optional capabilities and cannot degrade ordinary browsing stability.

### Phase 12 — background and headless availability

**Purpose:** improve availability without weakening user control.

- [x] Formally defer mobile widgets, background polling/uploads, silent push, and background refresh beyond 0.1.
- [x] Keep 0.1 foreground/resume-driven with optional local completion alerts only.
- [x] Add optional tray/menu-bar behavior while server access is enabled.
- [ ] Add optional start-at-login.
- [x] Preserve an explicit Quit action that stops the embedded server.
- [x] Approve and implement a separate daemon project for NAS and home-server use.
- [x] Extract transport-neutral Rust services without adding a GUI runtime to `volumd`.
- [x] Keep API and pairing protocol compatible with existing companions.

---

## 15. Testing strategy

### 15.1 Rust tests

- Service query tests with migrated temporary SQLite databases.
- DTO serialization snapshots.
- Forbidden-field/path-leak tests.
- Router tests for status codes, auth, scopes, request limits, and errors.
- Pairing secret expiry, replay, cancellation, and rate-limit tests.
- Token hash, revocation, and certificate rotation tests.
- Media ID authorization and path traversal tests.
- Thumbnail queued/ready/error behavior.
- Server start/stop and port-conflict tests.
- Backup tests confirming the companion security store and key are excluded.

### 15.2 Shared/frontend tests

- Theme token and component rendering tests.
- Desktop regression tests after component extraction.
- Typed client tests against mocked transport responses.
- Query-key and pagination tests.
- Pairing state-machine tests.
- Offline repository and cache eviction tests.
- Localization key coverage for all three languages.
- Viewer parser tests remain shared and gain mobile transport cases later.

### 15.3 Visual tests

Capture stable light/dark screenshots for:

- pairing;
- library with cards and collections;
- search/filter sheet;
- empty and offline library;
- project detail;
- photos;
- BOM and assembly steps;
- settings and revoked-device state.

Run at representative phone and tablet widths. Desktop screenshots protect against accidental changes while extracting shared CSS.

### 15.4 Physical-device matrix

Final minimum OS versions are chosen in Phase 0. At minimum, test:

- oldest supported iPhone/iOS combination;
- current iPhone/iOS combination;
- oldest supported Android API on a lower-memory device;
- current Android on a Pixel-class device;
- one Samsung-class Android device;
- phone and tablet layout where practical.

Simulators/emulators supplement but do not replace local-network, camera, secure-storage, sleep/wake, and WebGL testing on hardware.

### 15.5 Security tests

- Unauthenticated LAN scan sees no library content.
- Invalid/replayed QR secret is rejected.
- Certificate substitution is rejected.
- Revoked token stops working immediately.
- Token never appears in logs or diagnostics.
- Browser-origin requests cannot acquire or use ambient credentials.
- Malformed IDs cannot escape approved roots or media directories.
- Oversized query/body/upload/mesh requests are rejected before resource exhaustion.
- Restored backups do not restore trusted devices or private server identity.

---

## 16. Performance budgets

Measure on physical devices and a desktop containing a 10,000-project fixture.

Initial targets:

- Authenticated metadata request p95 under 200 ms on a healthy local network, excluding first-time preview generation.
- First cached library content visible under 500 ms after app launch.
- First uncached project page visible under 1.5 seconds on normal Wi-Fi.
- Project list payload under 250 KB for a 48-item page, excluding images.
- Thumbnail files use the existing 480×360 output unless mobile testing justifies a smaller derivative.
- Smooth grid scrolling without downloading off-screen full-resolution photos.
- No unbounded request, render, upload, or mesh queues.
- Mobile cache obeys its configured budget and survives abrupt termination without corruption.
- Viewer payload limits are defined before enabling interactive 3D in production.

These are starting budgets, not promises to hide regressions; CI/load tests should record trends.

---

## 17. CI and release engineering

### 17.1 Continuous integration

CI now runs separate jobs for:

- shared Photo Room tests through the desktop and mobile suites;
- mobile React tests/build and Playwright visual regression;
- desktop Rust server/service tests;
- mobile Rust checks for supported targets;
- Android arm64 release APK build;
- unsigned iOS simulator archive on macOS;
- browser administrator, website/docs, and Docker image builds;
- desktop frontend, Rust, AppImage, and Windows installer builds.

OpenAPI generation, generated TypeScript validation, route-drift checks, Docker smoke testing, and Compose validation are enforced in CI.

Pin compatible Tauri and plugin versions rather than allowing an unreviewed mobile plugin major/minor jump.

### 17.2 Release separation

Desktop and mobile release trains must be independent:

- Desktop continues GitHub Releases and signed updater artifacts.
- iOS uses App Store Connect/TestFlight.
- Android uses Play Console internal testing and production tracks.
- Mobile declares a supported server protocol range, not an exact desktop app version.
- Desktop release notes call out companion compatibility only when the server contract changes.

### 17.3 Store and licensing gate

Volum Desktop is GPL-3.0-or-later. Before distributing an iOS build through the App Store:

1. Audit copyright ownership and external contributions.
2. Obtain legal guidance on GPLv3 compatibility with App Store distribution terms.
3. Decide whether the companion and shared packages need a separate or dual license.
4. Keep the network API boundary explicit.
5. Record the decision in repository licensing documentation.

Do not wait until store submission to resolve this.

---

## 18. Compatibility and versioning

- Protocol versions are independent from desktop and mobile marketing versions.
- Pairing exchanges `protocolMin` and `protocolMax`.
- A companion refuses pairing when no protocol overlap exists and explains which product needs updating.
- `/api/v1` changes are additive whenever possible.
- Removing or changing a field requires a new API version or a compatibility period.
- Capability flags gate optional features such as writes, uploads, events, and meshes.
- Cached records store their server ID and API version so two servers cannot contaminate each other's data.

Suggested capabilities:

```json
{
  "readLibrary": true,
  "organizationWrites": false,
  "photoUploads": false,
  "viewerMeshes": false,
  "liveEvents": false
}
```

---

## 19. Main risks and mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Private HTTP is exposed to a public or untrusted network | Exposes private library data without transport encryption | Reject public listener/client addresses, resolve HTTP DNS names to private IPs, disable redirects, require HTTPS for public routes, firewall the listener, and test release configurations. |
| Desktop must be running | Companion appears unreliable | Explain state clearly, add offline cache, then optional tray/start-at-login. |
| Existing commands are transport-coupled | Server duplicates logic or leaks fields | Extract service functions and create dedicated network DTOs first. |
| Styling diverges | Companion feels like another product | Shared token/package ownership, screenshot tests, and no mobile palette duplication. |
| Mobile UI simply shrinks desktop | Poor usability and accessibility | Share visual primitives, not desktop layout; use bottom navigation and stacked detail. |
| Large libraries overwhelm phone/network | Slow grids and memory pressure | Cursor pagination, lazy media, ETags, LRU cache, payload budgets, virtualization if measured necessary. |
| 3D meshes overwhelm mobile GPU/RAM | Crashes or battery drain | Keep viewer post-MVP, cap payloads, cache sparingly, retain thumbnail fallback. |
| Pairing tokens leak through backups/logs | Unauthorized access | Separate security store, OS credential manager, redacted logging, backup exclusion tests. |
| Network addresses or device names change | Reconnection failures or token disclosure to the wrong host | Store server identity, verify `/identity` before pairing or using a token at a changed origin, and provide explicit re-pairing without weakening host validation. |
| GPL/App Store incompatibility | Blocks iOS distribution | Resolve licensing in Phase 0, before significant shared-code investment. |
| Tauri plugin/mobile regressions | Release instability | Pin versions, test release builds on hardware, keep platform-specific code small. |
| Safe writes create conflicts | Lost notes or confusing state | Ship read-only first; require online edits and revision preconditions later. |

---

## 20. Decisions to lock before implementation

Recommended defaults are shown in bold.

- Mobile framework: **Tauri 2 with React for both iOS and Android**.
- Initial access: **read-only**.
- Connectivity: **Tailscale is the automatic recommended path, not a requirement; explicit private LAN/VPN routes and trusted HTTPS are supported; there is no public relay**.
- Transport: **`tailscale-http` remains backward compatible; `private-http` is limited to explicit private routes with a warning; `https` accepts operating-system-trusted TLS; public HTTP and redirects are blocked**.
- Pairing: **short-lived one-use QR code with manual secure fallback**.
- Desktop lifecycle: **server available only while Volum runs for 0.1**.
- Mobile server count: **one active paired server in 0.1; schema permits more later**.
- Offline: **cached read-only snapshots and media; no offline mutation queue**.
- Interactive viewer: **after read-only release**.
- File operations from mobile: **never in the initial API; reconsider only through a separate threat review**.
- Shared styling: **tokens and presentational primitives are packages; layout remains platform-specific**.
- Analytics: **none**.

Items requiring explicit owner decision:

- [x] Native mobile name and identifiers (`Volum`, `com.dsd.volum.companion`); Companion remains the in-app connection role.
- [ ] Confirm iOS 15 as the public minimum after hardware testing.
- [ ] Confirm Android API 24 as the public minimum after hardware testing.
- [ ] Mobile/shared-code licensing approach.
- [x] Companion 1.0 keeps automatic Tailscale while documenting and supporting LAN, WireGuard, Twingate, ZeroTier, and trusted HTTPS alternatives.
- [x] Phone photo upload is included in the current early-access mobile release behind a separate host permission.
- [ ] Default mobile media cache limit after device testing.

---

## 21. Definition of done for the first public release

The first public companion release is complete when:

- [x] A user can enable Companion Access without editing files or using a terminal.
- [ ] iOS and Android can pair by QR and reconnect after restart.
- [x] Every library endpoint requires a valid, revocable token over an approved private or HTTPS transport.
- [x] Companion API responses omit absolute paths, relative asset paths, and secrets.
- [ ] Library, search, favorites, folders, collections, project detail, photos, BOM, and assembly instructions work on both platforms.
- [ ] Cached project information remains readable while the desktop is unavailable.
- [x] Desktop clearly communicates that it must remain running.
- [x] Disabling access closes the listener; revoking a device works immediately.
- [ ] Light/dark mobile screens match Volum's desktop visual system.
- [x] English, Catalan, and Spanish are complete.
- [ ] VoiceOver, TalkBack, reduced motion, contrast, safe areas, and touch-target requirements pass review.
- [ ] Physical-device tests pass on the chosen minimum and current OS versions.
- [ ] Desktop macOS, Linux, and Windows behavior and release builds remain green.
- [ ] Store licensing and privacy requirements are resolved and documented.
- [x] Troubleshooting covers firewall, local-network permission, desktop sleep/exit, revoked access, and re-pairing.

---

## 22. Suggested pull-request sequence

This is the smallest reviewable order that follows the dependencies above:

1. **Shared design tokens with zero desktop visual change.**
2. **Shared format/i18n and pure project-card primitives.**
3. **Tauri mobile scaffold rendering fixture data in Volum styling.**
4. **Backend read services and tests.**
5. **Sanitized companion DTOs, OpenAPI, and generated TypeScript client.**
6. **Read-only Axum routes behind test authentication.**
7. **Private listener/HTTPS policy, secure pairing, device tokens, and revocation.**
8. **Desktop Companion Access settings and QR flow.**
9. **Mobile private-HTTP/trusted-HTTPS transport, QR pairing, identity verification, and secure storage.**
10. **Mobile library/search/collection browsing.**
11. **Mobile project detail, photos, BOM, and assembly guide.**
12. **Offline metadata/media cache and resilience.**
13. **Security, accessibility, performance, and beta hardening.**
14. **Optional safe writes.**
15. **Optional phone uploads and interactive viewer.**

Each PR should update this checklist, add its own tests, and keep the Desktop application releasable.

---

## 23. Reference documentation

- Tauri prerequisites and mobile targets: <https://v2.tauri.app/start/prerequisites/>
- Tauri mobile development: <https://v2.tauri.app/develop/>
- Tauri barcode scanner: <https://v2.tauri.app/plugin/barcode-scanner/>
- Rust ring AEAD: <https://docs.rs/ring/latest/ring/aead/>
- Current Volum architecture: [`../../apps/website/docs/reference/architecture.md`](../../apps/website/docs/reference/architecture.md)
- Current product plan: [`product-engineering.md`](product-engineering.md)
- Current shared visual source: [`../../apps/desktop/src/styles/global.css`](../../apps/desktop/src/styles/global.css)
- Current frontend API facade: [`../../apps/desktop/src/lib/tauri/api.ts`](../../apps/desktop/src/lib/tauri/api.ts)
