# Architecture

## Boundary

Volum is a Tauri 2 application. React owns presentation and transient interaction state. Rust owns filesystem access, SQLite, indexing, parsing, archive safety, native file actions, and persisted user data. Frontend commands pass IDs and narrow typed inputs; they never receive unrestricted filesystem access.

## Data flow

1. A native folder dialog returns a user-approved root.
2. Rust canonicalizes and stores the root, then traverses supported files with `walkdir`.
3. Lightweight records appear progressively while bounded parsing extracts metadata.
4. Files are grouped deterministically by folder and normalized basename.
5. SQLite in WAL mode serves paged/search queries. FTS5 indexes names, paths, project files, collections, tags, notes, and formats. Manual collections store membership; smart collections store a versioned, validated all/any rule set evaluated by the same paged project query. Saved searches persist the ordinary filter and sort state separately from collections.
6. `notify` events are debounced and treated as reconciliation hints.
7. A bounded two-worker Rust queue creates persistent PNG thumbnails. It uses a safe embedded 3MF thumbnail/plate image when available and otherwise rasterizes actual mesh geometry. Jobs continue after their originating React view unmounts.
8. Interactive viewer geometry is parsed off the UI thread in Rust, including external 3MF component files and build transforms, then transferred as a compact binary indexed mesh. Three.js computes display normals and owns camera/orbit interaction only.
9. Slicer discovery stays local. Rust detects a catalog of common installed slicers and caches native application icons; the persisted configuration only stores enabled IDs, the default ID, and explicitly chosen custom application paths.
10. Exact duplicate candidates are found with the inexpensive partial fingerprint and size, then confirmed with full BLAKE3 hashes. Every supported geometry file also receives a versioned, translation/orientation-invariant and scale-sensitive surface descriptor. Duplicate browsing classifies exact bytes separately from matching geometry; conservative normalized version keys remain a lower-confidence relationship. Cleanup transfers project metadata transactionally and can optionally move only redundant matching files to the operating system Trash.
11. Web imports use provider adapters for public MakerWorld and Printables metadata. Saved links and attribution live in SQLite; user-selected files are copied atomically into a configurable folder inside an indexed library, then enter the normal scanner, parser, duplicate, and preview pipelines. Volum does not store provider credentials or bypass protected downloads.
12. Backups use SQLite `VACUUM INTO` for a consistent WAL-safe snapshot, then package the database and user-owned media in a checksummed ZIP. Destinations can be ordinary or mounted folders and WebDAV collections. WebDAV passwords live in the operating system credential manager and are never included in SQLite or backup archives.

## Identity and recovery

Paths are display/location data, not the only identity. A partial BLAKE3 fingerprint and size provide a fast candidate match; full BLAKE3 is computed when resolving a likely move or confirming a duplicate candidate. Project identity lives in the existing `models` record for migration compatibility, while `model_assets` owns the source/printable/plate/version files in a bundle. Manual bundles are never undone by a rescan. Missing records are retained instead of immediately deleting favorites, notes, tags, or collection membership. The database is copied before migrations.

Restore is restart-based. Archives are bounded, path-checked, checksum-verified, and subjected to SQLite integrity and schema checks before staging. Volum creates a separate safety archive of the current state, writes a pending-restore marker, and swaps the database and user-media directory before opening SQLite on the next launch. Original model files and regenerable thumbnail caches are never part of the archive.

## Storage

- Database: platform app-data directory, `volum.sqlite3`
- Thumbnail cache: app-data `thumbnails/`
- Logs: Tauri log plugin’s platform log directory
- User-owned media: app-data `media/`
- Restore safety archives: app-data `recovery/`
- Model files: never copied, renamed, or modified

Thumbnail cache entries are keyed by the asset fingerprint and source modification time, have no time-based expiry, and are regenerated only when the source revision changes. Cache reads update the LRU timestamp; the storage-pressure cleanup starts evicting only after 1 GB.

## Format strategy

- STL: binary/ASCII bounds and triangle count
- OBJ: bounds, face count, and referenced material names
- 3MF: archive validation, model XML bounds, object count, per-plate thumbnails and object names, printer/nozzle/layer/profile metadata, explicit sliced time and `used_g` values, and browser rendering
- ZIP archives are ignored as library items; ZIP-based 3MF containers still receive central-directory safety validation
- STEP/STP: parsed and tessellated with headless Open CASCADE through `cadrum`; generated meshes and thumbnails are cache data, while dimensions, area, volume, and geometry descriptors are indexed metadata
