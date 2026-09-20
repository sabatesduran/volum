# Volum

> **A beautiful home for your 3D models.**

Product, design, and engineering plan for a private, local-first desktop library for 3D-printing files, built with Tauri for macOS, Windows, and Linux.

**Status:** product definition  
**Working app identifier:** `dev.didac.volum`  
**Primary platforms:** macOS, Windows, Linux  
**Distribution:** open source; no account required  

---

## 1. The name

### Chosen name: Volum

**Volum** is short, visual, memorable, and directly connected to three-dimensional objects without sounding like a slicer or engineering utility. It works particularly well for a design-led product: the app is about the *things* in the library, not merely their filenames.

It is also natural in Catalan and understandable internationally through its relationship to “volume.”

**Tagline:** A beautiful home for your 3D models.

**Secondary product line:** Your folders, beautifully organized.

**Brand idea:** a simple `V` formed from three stacked contour layers or shelves. It should remain recognizable as a 16 px sidebar icon and should not use a generic cube or printer nozzle.

The initial web search did not reveal a directly competing 3D-model library called Volum. Before a public launch, complete a proper EUIPO trademark search, domain check, app-store search, and social-handle check. Treat the name as chosen for development but not legally cleared.

---

## 2. Product thesis

3D-printing folders are functional but unpleasant to browse. Existing organizers often make the library feel like a database or file manager with thumbnails added to it.

Volum should feel closer to **Photos, Things, and a beautifully designed media library**:

- Models are the visual focus.
- The user’s real files and folders remain the source of truth.
- The app discovers and updates the library automatically.
- Collections provide a second, flexible organization layer without moving files.
- Useful details—format, dimensions, material estimate, and plastic cost—are available when needed but do not clutter browsing.
- Everything works locally, without an account or cloud dependency.

### The core rule

> **Folders are automatic. Collections are manual.**

If a user reorganizes files in Finder, Explorer, or their Linux file manager, Volum reflects the change. If the user adds a model to “Garage,” “Gifts,” and “To print,” the underlying file remains exactly where it was.

---

## 3. Product principles

1. **The filesystem is canonical.** Volum indexes and enriches files; it does not imprison them.
2. **Visual first, metadata second.** The default view is for recognition and discovery, not inspection.
3. **No organization twice.** Existing folder structure appears automatically.
4. **Fast enough to disappear.** Startup, search, scrolling, and file changes should feel immediate.
5. **Local by default.** No account, telemetry, or network access is required for the core product.
6. **Native where it matters.** Folder pickers, menus, keyboard shortcuts, file opening, drag-and-drop, updates, and window behavior should respect each OS.
7. **Progressive disclosure.** Advanced metadata lives in the detail view, inspector, or contextual actions—not on every card.
8. **Honest data.** Estimated values show their source and never pretend to be exact.

---

## 4. Target user and primary job

### Primary user

A hobbyist or maker with tens, hundreds, or thousands of STL/3MF files spread across local disks or mounted NAS folders. They use Bambu Studio, OrcaSlicer, PrusaSlicer, Cura, or similar tools and want to recognize, find, and reopen models quickly.

### Primary job

> “Show me what I have, let me find it visually, and open the right file without digging through folders.”

### Secondary jobs

- Keep cross-folder lists such as Favorites, To print, Garage, Gifts, or Baby.
- Know roughly how much the plastic for a model or batch costs.
- Inspect dimensions and available variants/formats.
- Reveal the source file or open it in a chosen slicer.

---

## 5. Scope

### V1 must include

- Multiple local or mounted library roots.
- Automatic folder hierarchy mirroring.
- Recursive scanning and continuous filesystem watching.
- Model discovery for STL, 3MF, and OBJ.
- STEP indexing from day one; rich STEP rendering only after the cross-platform rendering spike succeeds.
- ZIP archive inspection without permanently extracting its contents.
- Generated thumbnails and an interactive 3D preview.
- Embedded 3MF thumbnails and slicer metadata when available.
- Search by model name, filename, folder, collection, format, and notes.
- Recent models and Favorites.
- User-created collections.
- Add-to-collection from a menu, drag-and-drop, or multi-select action.
- Model details, file variants, dimensions, size, and modified date.
- Plastic-only cost calculator with saved materials.
- Open with the default app or a configured slicer.
- Reveal in Finder, Explorer, or the Linux file manager.
- Grid density control and light/dark/system appearance.
- Keyboard navigation and contextual menus.
- Local SQLite metadata database and thumbnail cache.
- Background indexing with visible progress, pause, resume, and recoverability.
- Signed installers and in-app updates.

### V1 should include if time permits

- User notes.
- Custom cover image or saved preview angle.
- Duplicate detection using content hashes.
- Tags, if real testing shows collections alone are insufficient.
- Quick filters: format, folder, collection, has cost, missing preview.
- Ignore rules for folders and extensions.
- Export/import of Volum metadata without copying model files.

### Explicit V1 non-goals

- Printer control or Bambu account integration.
- Print history and job monitoring.
- Filament inventory or spool remaining estimates.
- MakerWorld/Printables downloading or scraping.
- Cloud sync, accounts, teams, or social features.
- Editing, repairing, slicing, or converting models.
- AI features.
- Mobile apps.

These can be reconsidered only after the core library is excellent.

---

## 6. Information architecture

### App shell

The sidebar should remain deliberately small:

```text
VOLum

Library
Recent
Favorites

Folders
Collections

                  +
Settings
```

Expanded folder trees belong inside the Folders view, not permanently inside the global sidebar. User collections can appear in the sidebar after the first few are created, with overflow behind “Show all.”

### Main destinations

| Destination | Purpose |
| --- | --- |
| Library | Visual overview, recently added models, top folders, and collections |
| Recent | Models ordered by added, modified, or last opened |
| Favorites | Built-in system collection |
| Folders | The actual filesystem hierarchy |
| Collections | User-defined groups independent of folders |
| Search | Instant global results with filters |
| Model detail | Preview, variants, metadata, cost, notes, and file actions |
| Settings | Libraries, apps, materials, appearance, indexing, updates, privacy |

### Mental model

```text
Library root
└── Folder hierarchy (automatic)
    └── Model
        ├── Primary 3D asset
        ├── Related files / variants
        ├── Preview and metadata
        └── Cost estimate

Collection (manual)
└── References models from any folder
```

---

## 7. Key user flows

### 7.1 First launch

1. Welcome screen explains: “Volum indexes your files locally. It does not move or upload them.”
2. User chooses one or more folders.
3. Volum immediately opens the library; scanning continues in the background.
4. Cards appear progressively instead of waiting for the entire scan.
5. A small status item shows discovered models, preview generation, errors, pause, and retry.

### 7.2 Browse folders

1. Open Folders.
2. Select a root or folder from a collapsible tree.
3. See child folders first, followed by a visual model grid.
4. Breadcrumbs expose the real location.
5. Changes made outside Volum update automatically.

### 7.3 Create and fill a collection

1. Click `+` next to Collections or use `New collection` from a selected model.
2. Choose a name, optional symbol, and color.
3. Add models with drag-and-drop, contextual menu, or multi-select.
4. Removing a model from a collection never touches its file.

### 7.4 Inspect a model

1. Open a card into a large detail view.
2. Rotate, zoom, reset, and switch between file variants.
3. See dimensions, formats, source folder, modified date, and file size.
4. Open in a configured slicer, open with the OS default, or reveal the file.
5. Add to collections, favorite it, add notes, or calculate material cost.

### 7.5 Calculate plastic cost

1. Volum pre-fills filament usage from trustworthy 3MF metadata when available.
2. Otherwise the user enters grams manually.
3. Select a saved material or enter spool price and spool weight.
4. Set quantity.
5. Volum displays one-piece and batch cost with the source marked as `From file` or `Manual`.

Formula:

```text
cost_per_piece = (plastic_grams / spool_weight_grams) × spool_price
batch_cost     = cost_per_piece × quantity
```

Example:

```text
Plastic used      84 g
Spool price       €15.99
Spool weight      1,000 g
Quantity          10
────────────────────────
One piece         €1.34
Batch             €13.43
```

Rounding should occur only for display; calculations retain full precision.

---

## 8. Visual and interaction design

### Design direction: editorial industrial

Volum should combine the calm hierarchy of an editorial library with the precision of a well-made workshop tool. It must not resemble a Windows file utility, a slicer, or a generic Tailwind dashboard.

### Visual personality

- Large model imagery with generous whitespace.
- Warm neutral surfaces rather than cold blue-gray SaaS panels.
- Near-black ink, bone/light canvas, and a confident orange-red accent.
- Fine borders, restrained shadows, and strong typographic hierarchy.
- Rounded corners used consistently but not excessively.
- Small moments of depth in model renders, not decorative 3D UI chrome.
- Subtle motion: 120–180 ms for controls, 180–240 ms for view transitions.

### Initial color tokens

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| Canvas | `#F4F1EA` | `#11110F` | Main background |
| Surface | `#FCFAF5` | `#1A1A17` | Cards and panels |
| Ink | `#191916` | `#F4F1EA` | Primary text |
| Muted | `#74736D` | `#A4A29A` | Secondary text |
| Border | `#DDD8CE` | `#31312C` | Dividers and outlines |
| Accent | `#FF5A36` | `#FF6A48` | Selection and primary actions |
| Success | `#2F8F64` | `#54C995` | Complete/available states |

These are a starting system, not a substitute for testing contrast in both themes.

### Typography

- Use a high-quality variable sans with open numerals for the interface; begin with Inter as a development-safe fallback.
- Use tabular numerals for dimensions, grams, prices, and file sizes.
- Avoid an ornamental display typeface in the product UI. The brand wordmark can be custom.

### Model grid

- Responsive cards with a large 4:3 preview region.
- Default card text: model name and one quiet context line.
- Optional context line changes with the view: folder in search, date in recent, or collection membership where useful.
- Format badges, dimensions, file size, and cost do not appear by default.
- Hover/focus actions: favorite and `…`; never show a row of persistent buttons.
- Grid density: Comfortable, Compact, and Large Preview.
- Virtualize long lists so design quality does not cost performance.

### Model detail layout

```text
┌──────────────────────────────────┬─────────────────────┐
│                                  │ Model name          │
│                                  │ Folder / breadcrumb │
│       INTERACTIVE PREVIEW        │                     │
│                                  │ [Open in slicer]    │
│                                  │                     │
│                                  │ Details             │
│                                  │ Files / variants    │
│                                  │ Collections         │
│                                  │ Material cost       │
│                                  │ Notes               │
└──────────────────────────────────┴─────────────────────┘
```

The preview gets most of the available space. On narrow windows, the inspector becomes a slide-over panel.

### Empty, loading, and error states

- Empty states should explain one action, not advertise features.
- Scanning uses skeleton cards and incremental results.
- Broken or unsupported models get a restrained placeholder with a retry action.
- Missing external drives or NAS mounts remain visible as offline libraries rather than silently deleting their models.

### Accessibility

- Complete keyboard traversal and visible focus.
- Minimum WCAG AA contrast.
- Never encode status using color alone.
- Respect reduced motion.
- Use semantic controls and accessible names in the webview.
- Support UI scaling and long translated strings from the start.

---

## 9. Technical architecture

### Recommended stack

| Layer | Choice | Reason |
| --- | --- | --- |
| Desktop runtime | Tauri 2 | Small cross-platform shell, native capabilities, Rust backend |
| Frontend | React + TypeScript + Vite | Fast iteration and a strong ecosystem for a highly designed interface |
| Styling | CSS variables + CSS Modules or vanilla CSS | Full visual control without a generic component-library appearance |
| Accessible primitives | Radix Primitives | Behavior and accessibility without imposing visual style |
| State | Zustand | Small UI state surface and easy separation from server/index state |
| Async data | TanStack Query | Cache/invalidation for paged library queries and background events |
| Grid virtualization | TanStack Virtual | Smooth large libraries |
| Interactive 3D | Three.js via React Three Fiber | Mature STL/OBJ rendering and controllable presentation |
| Backend | Rust | Filesystem, parsing, indexing, hashing, database, and native integration |
| Database | SQLite through `sqlx` in Rust | Durable local index, migrations, full-text search, transactions |
| File watching | Rust `notify` crate | Cross-platform filesystem events |
| Traversal | Rust `ignore`/`walkdir` | Fast recursive scanning with ignore support |
| Archive reading | Rust `zip` crate | Inspect ZIP/3MF containers without permanent extraction |
| XML parsing | `quick-xml` | Parse 3MF relationships, model data, and metadata |
| Hashing | BLAKE3 | Fast content fingerprints for moves and duplicates |
| Logging | `tracing` + Tauri log plugin | Structured diagnostics with a user-exportable log |

### Why business logic stays in Rust

The frontend should not receive unrestricted filesystem or database access. React requests narrow operations through typed Tauri commands; Rust validates paths against approved library roots and performs scanning, metadata extraction, writes, and file actions.

This gives Volum:

- one security boundary;
- predictable background concurrency;
- fewer cross-platform path bugs;
- easier unit testing of the indexer;
- no large file contents moving across IPC;
- a future path to reuse the indexing core in a CLI.

### High-level architecture

```text
React UI
  ├── Library views and virtualized cards
  ├── Search, collections, settings
  └── Three.js preview renderer
          │ typed Tauri commands/events
Rust core
  ├── Library service
  ├── Scanner + filesystem watcher
  ├── Model grouping and metadata parsers
  ├── Thumbnail orchestration
  ├── Collection and cost services
  ├── SQLite repository
  └── Native open/reveal/update actions
          │
Filesystem + SQLite + thumbnail cache
```

### Tauri capabilities and plugins

Use only the permissions each window needs:

- Dialog: choose library roots and custom cover images.
- Opener: open files and reveal URLs where appropriate.
- Updater: signed application updates.
- Window State: restore size and position.
- Single Instance: focus the existing instance and handle files opened from the OS.
- OS and Process: platform-specific behavior when required.
- Log: persisted diagnostics.

Do not expose a broad shell permission to the frontend. “Open in slicer” should be implemented by a validated Rust command using a stored application path and an asset path already known to the index.

### Suggested repository structure

```text
volum/
├── src/                         # React application
│   ├── app/
│   ├── components/
│   ├── features/
│   │   ├── library/
│   │   ├── folders/
│   │   ├── collections/
│   │   ├── model-detail/
│   │   ├── cost-calculator/
│   │   ├── search/
│   │   └── settings/
│   ├── lib/tauri/
│   ├── styles/
│   └── types/
├── src-tauri/
│   ├── migrations/
│   └── src/
│       ├── commands/
│       ├── db/
│       ├── domain/
│       ├── indexing/
│       ├── parsers/
│       ├── previews/
│       ├── services/
│       └── platform/
├── fixtures/                    # Small legal test models and corrupt cases
├── tests/
└── .github/workflows/
```

---

## 10. Indexing and filesystem behavior

### Library roots

Each approved root stores:

- absolute path;
- platform/volume identity where available;
- display name;
- online/offline state;
- last complete scan cursor/time;
- ignore configuration;
- case-sensitivity behavior.

Never assume paths are case-sensitive. Normalize for comparison according to the root/platform while preserving the original spelling for display.

### Initial scan pipeline

1. Traverse supported paths without following symlink loops.
2. Insert lightweight file records quickly in batches.
3. Group related assets into models.
4. Prioritize visible items for metadata and preview generation.
5. Parse remaining items in a bounded worker queue.
6. Commit in batches so the UI updates progressively.
7. Save a checkpoint so interrupted scans resume safely.

### Filesystem watcher

- Debounce bursts from downloads, archive extraction, and slicer saves.
- Coalesce create/modify/rename/delete events by path and file identity.
- Treat watcher events as hints; schedule a small reconciliation scan because events can be missed.
- Run a full reconciliation on startup and optionally on a low-frequency schedule while the app is open.

### Stable identity and moves

Collections cannot break every time a user renames a folder.

Use this identity strategy:

1. Store path, size, modified time, and a fast partial fingerprint initially.
2. Compute a full BLAKE3 hash lazily when identity resolution or duplicate detection needs it.
3. On disappearance plus appearance, match likely moves using filesystem ID when available, then full hash and size.
4. Retain missing records for a grace period instead of immediately deleting metadata.
5. If an offline root returns, reconcile it before treating files as removed.

### Model grouping

The app presents **models**, not an undifferentiated list of files. V1 needs deterministic, reversible heuristics:

1. A `.3mf` is a model and may contain multiple build items/plates.
2. Files with the same normalized basename in the same directory are grouped as variants (`hook.stl`, `hook.3mf`, `hook.obj`).
3. Numbered parts such as `part_1` and `part_2` remain separate unless a 3MF or manifest explicitly groups them.
4. A folder containing only one primary model plus source/preview files may be represented as one model bundle.
5. Ambiguous cases remain separate. Never merge aggressively and lose clarity.
6. Later, add explicit Merge and Split actions; preserve those overrides in SQLite.

The original files are never renamed or moved by automatic grouping.

### Ignore behavior

Ignore common temporary/system artifacts, hidden cache folders, partial downloads, and configurable patterns. Provide a per-root “Show ignored files” diagnostic rather than making omissions mysterious.

---

## 11. File formats and previews

### Support matrix

| Format | V1 indexing | V1 interactive preview | Metadata |
| --- | --- | --- | --- |
| STL (binary/ASCII) | Yes | Yes | Triangle count, bounds, dimensions |
| 3MF | Yes | Yes | Embedded thumbnail, objects, build items, slicer metadata when present |
| OBJ | Yes | Yes | Bounds and referenced material names; textures are best-effort |
| STEP/STP | Yes | Conditional | File metadata initially; rich preview after converter spike |
| ZIP | Yes | Preview contained supported files | Archive contents and selected primary asset |
| G-code | Later | Embedded thumbnail where available | Time/filament metadata later |

### Rendering strategy

- Parse lightweight metadata in Rust.
- Send compact geometry or a scoped asset URL to the preview renderer; never send arbitrary filesystem access to the webview.
- Normalize model orientation and camera framing from bounds.
- Use consistent studio lighting, neutral material, ground shadow, and transparent background.
- Cache thumbnails as WebP/AVIF at multiple sizes keyed by asset fingerprint + renderer version + theme/render preset.
- Let the user save a preferred camera angle as the cover.
- Prefer an embedded 3MF preview immediately, then replace it with the Volum render if appropriate.

### STEP risk

STEP is not a simple mesh format. Do not block the initial app on pretending it is. Run an early technical spike comparing:

1. Open CASCADE-based conversion bundled as a per-platform sidecar;
2. a maintained Rust STEP tessellator if one is production-ready;
3. indexing STEP with a high-quality placeholder and using a sibling STL/3MF for the model preview.

The decision must consider installer size, LGPL obligations, startup cost, tessellation quality, code signing, and Linux packaging. The fallback is honest STEP indexing in V1 and rich rendering in V1.1.

### Archive safety

- Inspect ZIP central directories before extraction.
- Reject path traversal (`../`) and absolute paths.
- Cap decompressed size, entry count, and nesting.
- Extract individual preview candidates only into the app cache.
- Never execute scripts or binaries from archives.

---

## 12. Search

Use SQLite FTS5 for text search over:

- model display name;
- filenames;
- relative folder path;
- collection names;
- notes;
- file formats.

Search results should appear as the user types, tolerate punctuation differences, and highlight the matched context. Begin with prefix/token search and simple ranking before considering fuzzy search.

### Filters

- Folder/root
- Collection
- Format
- Added/modified date
- Has preview / missing preview
- Has cost estimate
- Favorite
- Available/offline

Persist the most recent sort and grid density per view, not globally.

---

## 13. Collections

### Standard collections

- User-created name, symbol, and optional color.
- A model can belong to any number of collections.
- Reordering inside a collection is optional for V1; if implemented, use fractional position values.
- Deleting a collection removes only its references.

### System collections

- Favorites
- Recent

“To print” should be a suggested normal collection rather than hard-coded product behavior.

### Smart collections: later

After standard collections are proven, add saved queries such as:

- Added in the last 30 days
- STL files in Garage
- Missing preview
- More than 200 g
- Never opened

Do not implement a rule builder in the first release.

---

## 14. Material cost calculator

### Material record

- Name: e.g. Sunlu PLA+
- Material type: PLA, PETG, TPU, ABS, other
- Color name and optional swatch
- Spool price
- Currency
- Net spool weight in grams
- Optional density in g/cm³
- Last updated date

This is a price preset, not inventory. V1 does not track spool balance.

### Estimate record

- Model/variant
- Plastic amount in grams
- Quantity
- Selected material
- Optional manual price override
- Source: 3MF metadata, calculated from volume+density, or manual
- Source asset fingerprint so stale imported estimates can be detected

### Calculation rules

- Prefer explicit weight stored by the slicer.
- If only filament length and diameter are known, calculate volume and apply material density, clearly marking it as estimated.
- If mesh volume can be computed, do **not** present it as print weight without infill, wall, support, and slicer information.
- Use locale-aware currency and decimal formatting.
- Allow cost values on the detail screen; keep them off grid cards unless the user enables a “Show cost” display option.

---

## 15. Data model

Suggested first schema; exact normalization can evolve through migrations.

```text
library_roots
  id, path, display_name, volume_key, status,
  last_scan_at, created_at, updated_at

folders
  id, root_id, parent_id, relative_path, name,
  modified_at, status

models
  id, folder_id, display_name, primary_asset_id,
  notes, favorite, grouping_key, grouping_override,
  added_at, updated_at, last_opened_at, missing_since

assets
  id, root_id, folder_id, relative_path, filename,
  extension, byte_size, modified_at, partial_fingerprint,
  content_hash, parse_status, metadata_json, missing_since

model_assets
  model_id, asset_id, role, sort_order

thumbnails
  id, asset_id, fingerprint, renderer_version,
  preset, width, height, cache_path, created_at

collections
  id, name, symbol, color, sort_order, created_at, updated_at

collection_items
  collection_id, model_id, position, added_at

materials
  id, name, material_type, color_name, color_hex,
  spool_price_minor, currency, spool_weight_g, density,
  created_at, updated_at

cost_estimates
  id, model_id, asset_id, material_id, plastic_g,
  quantity, source, source_fingerprint, created_at, updated_at

user_overrides
  id, entity_type, entity_id, key, value_json, updated_at
```

Store money as integer minor units plus currency. Store physical values in explicit base units, not formatted strings.

---

## 16. Rust command surface

Keep commands narrow and typed. An illustrative API:

```text
add_library_root(path)
remove_library_root(root_id, keep_metadata)
start_scan(root_id)
pause_scan(root_id)
get_scan_status(root_id)

list_library(query, cursor)
list_folder(folder_id, query, cursor)
get_model(model_id)
search_models(query, filters, cursor)

create_collection(input)
update_collection(id, input)
delete_collection(id)
add_models_to_collection(collection_id, model_ids)
remove_models_from_collection(collection_id, model_ids)

save_material(input)
save_cost_estimate(input)
calculate_cost(input)

open_asset(asset_id, app_id?)
reveal_asset(asset_id)
regenerate_preview(asset_id)
set_custom_cover(model_id, image_path)
```

Rust emits events such as `scan-progress`, `models-changed`, `root-status-changed`, and `preview-ready`. Events contain IDs and small summaries, never model bytes.

Generate TypeScript types from Rust definitions or validate IPC payloads on both sides to stop the boundary drifting.

---

## 17. Performance targets

Targets for a release build on a normal recent laptop and an SSD:

| Interaction | Target |
| --- | --- |
| App shell visible | under 1 second warm start |
| Existing library usable | under 1.5 seconds |
| Search feedback | under 100 ms for 10,000 models |
| Folder change reflected | usually under 2 seconds |
| Grid scroll | 60 fps under normal load |
| Detail view open | under 150 ms when preview is cached |
| Memory at 10,000 models | bounded; no geometry retained for off-screen cards |

Implementation requirements:

- cursor pagination, not loading the whole database into React;
- virtualized grids;
- bounded scan and preview queues;
- cancellation and priority for visible models;
- batched database writes;
- WAL mode and appropriate indexes;
- lazy full hashes;
- cache size controls and LRU cleanup;
- benchmark fixtures for 1k, 10k, and deep NAS-style trees.

---

## 18. Privacy, security, and recovery

- No account and no required network connection.
- No model uploads.
- No analytics by default. If crash reporting is later offered, make it explicit opt-in and exclude paths/model data.
- Scope filesystem access to user-approved roots and app-owned cache/config paths.
- Validate every path again in Rust; do not trust frontend input.
- Use a strict Content Security Policy and no remote scripts.
- Sign updater artifacts and verify signatures.
- Use atomic writes and SQLite transactions.
- Back up the small metadata database before migrations.
- Provide `Export Volum metadata` and `Rebuild index` recovery actions.
- Rebuilding the index must preserve recoverable user metadata by matching fingerprints where possible.
- Provide a diagnostics export with paths optionally redacted.

---

## 19. Cross-platform details

### macOS

- Signed and notarized app bundle and DMG.
- Apple Silicon and Intel builds, or a universal build if size remains reasonable.
- Correct sandbox/bookmark strategy if distributing through the Mac App Store; direct distribution is simpler for arbitrary folders and NAS volumes.
- Native menu bar, standard shortcuts, window restoration, and Finder reveal.

### Windows

- Signed NSIS or MSI installer.
- Test long paths, UNC/network paths, removable volumes, case-insensitive behavior, and antivirus scanning impact.
- Use the system WebView2 runtime and provide a clear prerequisite path where needed.
- Explorer reveal and configurable slicer executables.

### Linux

- Begin with AppImage plus `.deb`; add RPM/Flatpak when demand justifies it.
- Test Wayland and X11, file chooser portals, removable mounts, and several desktop file managers.
- Expect system-webview differences; set a supported distro/runtime baseline.
- Provide manual downloads even if automatic updating differs by package type.

---

## 20. Testing strategy

### Rust unit tests

- Path normalization and root containment.
- Grouping heuristics.
- Move/rename reconciliation.
- STL/3MF/OBJ metadata parsing.
- ZIP bomb/path traversal guards.
- Cost calculations and rounding.
- Database migrations and repository queries.

### Fixture corpus

Maintain small, legally distributable fixtures for:

- binary and ASCII STL;
- multi-object and multi-plate 3MF;
- different slicer-generated 3MF variants;
- OBJ with/without MTL;
- nested ZIPs;
- corrupt/truncated files;
- Unicode, emoji, very long, and case-colliding filenames;
- folders disappearing and returning;
- duplicates and renames.

### Frontend tests

- Component and accessibility tests with Vitest/Testing Library.
- Critical flows with Playwright against mocked Tauri commands.
- Screenshot regression for cards, empty states, themes, detail view, and calculator.

### End-to-end platform matrix

- macOS on Apple Silicon; Intel before public release.
- Windows 11 on a physical machine or reliable VM.
- Ubuntu LTS Wayland/X11; one Fedora test before release.
- Local SSD, removable drive, SMB-mounted NAS, and unavailable-root scenarios.

Manual visual QA is a release gate. A design-led product cannot rely only on functional tests.

---

## 21. Delivery plan

The work is divided by risk and user-visible value, not by arbitrary backend/frontend completion.

### Phase 0 — Technical and visual spikes (3–5 focused days)

- Scaffold Tauri 2 + React + TypeScript.
- Open and render representative STL, 3MF, and OBJ files on all three OS families.
- Test a 10,000-item virtualized grid.
- Test SQLite FTS and background scan events.
- Decide the STEP approach.
- Create one high-fidelity Library screen and one Model Detail screen.

**Exit criterion:** the architecture can render representative files and the UI direction genuinely looks better than the competitor.

### Phase 1 — Library foundation (1–2 weeks)

- SQLite schema and migrations.
- Add/remove library roots.
- Scanner, checkpoints, and filesystem watcher.
- Folder hierarchy and basic grouping.
- Progressive index status and offline-root handling.
- Library/folder queries with pagination.

**Exit criterion:** point Volum at a real library, close/reopen it, move files externally, and see a correct index.

### Phase 2 — Preview pipeline (1–2 weeks)

- STL, 3MF, and OBJ parsers.
- Embedded 3MF thumbnails.
- Consistent interactive viewer.
- Thumbnail render queue, cache keys, invalidation, and cleanup.
- ZIP inspection and safety limits.
- Error states and retry.

**Exit criterion:** a mixed real-world library becomes visually browsable without manual covers.

### Phase 3 — Designed browsing experience (1–2 weeks)

- Final app shell and navigation.
- Library home, folders, recent, and favorites.
- Virtualized responsive grid and density controls.
- Model detail and file variants.
- Search with FTS and filters.
- Keyboard navigation, menus, open, and reveal.
- Light/dark themes and accessibility pass.

**Exit criterion:** browsing, finding, and opening a model feels polished enough to be the product’s main selling point.

### Phase 4 — Collections and calculator (1 week)

- Collection CRUD and membership.
- Drag-and-drop and multi-select actions.
- Saved material presets.
- 3MF filament-usage extraction where reliable.
- Plastic cost calculator and batch quantity.
- Settings for display, libraries, slicers, and materials.

**Exit criterion:** the complete V1 promise works without touching underlying files.

### Phase 5 — Beta hardening (1–2 weeks)

- Large-library profiling.
- Cross-platform filesystem edge cases.
- Corrupt-file and archive hardening.
- Database backup/recovery and metadata export.
- Signed builds, updater, release notes, and install flow.
- Screenshot regression and a full visual polish pass.

**Exit criterion:** no known data-loss path, acceptable performance on 10,000 models, and reliable signed installers.

### Realistic solo schedule

- **Focused full-time:** approximately 7–10 weeks to a strong private beta.
- **Evenings/weekends:** approximately 12–16 weeks.

The likely schedule risks are STEP previewing, 3MF dialect differences, cross-platform filesystem edge cases, and signing/distribution—not collections or the calculator.

---

## 22. Release definition

V1 is ready when a new user can:

1. Install Volum without security warnings.
2. Add a large existing folder without reorganizing it.
3. Browse useful previews while scanning continues.
4. Find a model quickly by name, folder, or collection.
5. Create collections without moving files.
6. Open a chosen variant in their slicer.
7. Calculate plastic cost for one item or a batch.
8. Rename/move files outside Volum without losing collection membership in normal cases.
9. Disconnect and reconnect a library drive without metadata disappearing.
10. Update the app safely.

### Suggested beta metrics

All metrics should be local unless users explicitly opt into sending anonymous diagnostics.

- Median library size and scan duration (user-reported during beta).
- Preview success rate by format.
- Search latency at 1k/10k models.
- Move/rename reconciliation success.
- Crash-free sessions.
- Tasks from user testing: find model, create collection, open in slicer, calculate cost.
- Visual satisfaction compared directly with ordinary folder browsing and Modelist.

---

## 23. Open-source strategy

Volum should be fully usable from the public source repository, with no paid edition, feature gate, account, or license-key system.

### Recommended project setup

- Host the canonical repository publicly on GitHub.
- Publish signed macOS, Windows, and Linux builds through GitHub Releases.
- Keep the core application and indexing engine in one repository initially.
- Use semantic versioning and maintain a human-readable changelog.
- Publish the roadmap and use issue labels for bugs, formats, platforms, design, and good first issues.
- Include `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, build instructions, and architecture notes before inviting contributions.
- Require every feature to work locally without an account or hosted service.
- Keep update manifests and release artifacts reproducible and signed.

### License recommendation

Use **GPL-3.0-or-later** if the goal is to ensure distributed modifications and competing forks remain open source. Use **MIT** only if maximum reuse—including proprietary reuse—is more important than keeping improvements public.

For Volum, GPL-3.0-or-later is the better default because the design and desktop product are the project’s main value, and it still permits free use, redistribution, modification, and commercial distribution under the license terms.

Confirm license compatibility for every Rust crate, npm dependency, bundled font, icon set, test model, and any optional STEP converter. Generate third-party notices as part of CI.

### Community boundaries

- Maintainers retain final product and design direction; open source should not turn the interface into a collection of unrelated preferences.
- New format support should use a documented parser interface and fixture requirements.
- Feature requests must fit the product principles and V1/V2 scope rather than expanding Volum into a slicer or printer-control platform.
- Avoid a contributor license agreement initially. Signed commits or a Developer Certificate of Origin can be added if contribution volume requires it.

---

## 24. Decisions to lock before implementation

1. Confirm **Volum** after domain, store, and EUIPO checks.
2. Direct-download distribution first vs. Mac/Microsoft stores.
3. Exact V1 STEP promise after the spike.
4. Whether ZIP files appear as models, containers, or both; test with real libraries.
5. Final grouping heuristics using a representative personal dataset.
6. GPL-3.0-or-later vs. MIT before accepting external contributions.
7. Repository governance and the initial maintainer/reviewer policy.
8. Whether tags earn a place in V1 after collections are prototyped.

---

## 25. First implementation backlog

The first tickets should be small enough to verify architecture quickly:

1. Scaffold Tauri 2, React, TypeScript, linting, formatting, and CI.
2. Define design tokens and build the static Library card/grid prototype.
3. Add a folder picker and persist one library root.
4. Add SQLite migrations and root/folder/asset tables.
5. Scan STL/3MF/OBJ paths and stream progress to the UI.
6. Render one STL with normalized camera and studio lighting.
7. Cache one thumbnail keyed by asset fingerprint.
8. Query a paginated virtualized library grid.
9. Watch create/rename/delete events and reconcile them.
10. Build the model detail layout and open/reveal commands.
11. Add collections and drag-to-collection.
12. Add materials and the plastic cost calculator.

Do not begin with settings, community infrastructure, or a complete component library. Prove the scanner-to-beautiful-card loop first; that loop is the product.

---

## 26. Reference documentation

- [Tauri 2 overview](https://v2.tauri.app/start/)
- [Tauri architecture](https://v2.tauri.app/concept/architecture/)
- [Tauri security and capabilities](https://v2.tauri.app/security/)
- [Tauri file system plugin](https://v2.tauri.app/plugin/file-system/)
- [Tauri dialog plugin](https://v2.tauri.app/plugin/dialog/)
- [Tauri opener plugin](https://v2.tauri.app/plugin/opener/)
- [Tauri updater plugin](https://v2.tauri.app/plugin/updater/)
- [Tauri distribution guides](https://v2.tauri.app/distribute/)
