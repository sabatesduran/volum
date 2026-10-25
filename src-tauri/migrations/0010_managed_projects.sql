ALTER TABLE library_roots ADD COLUMN root_kind TEXT NOT NULL DEFAULT 'linked';
ALTER TABLE models ADD COLUMN storage_mode TEXT NOT NULL DEFAULT 'linked';

CREATE INDEX idx_library_roots_kind ON library_roots(root_kind, status);
CREATE INDEX idx_models_storage_mode ON models(storage_mode, missing_since);

CREATE TABLE asset_origins (
  asset_id TEXT PRIMARY KEY NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
  source_kind TEXT NOT NULL,
  original_path TEXT,
  import_action TEXT NOT NULL,
  original_hash TEXT,
  imported_at TEXT NOT NULL
);

CREATE TABLE ignored_asset_paths (
  id TEXT PRIMARY KEY NOT NULL,
  root_id TEXT NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
  relative_path TEXT NOT NULL,
  content_hash TEXT,
  reason TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(root_id, relative_path)
);

CREATE TABLE file_operation_jobs (
  id TEXT PRIMARY KEY NOT NULL,
  operation TEXT NOT NULL,
  state TEXT NOT NULL,
  payload_json TEXT NOT NULL DEFAULT '{}',
  error TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_file_operation_jobs_state ON file_operation_jobs(state, updated_at);

CREATE TABLE watched_import_folders (
  id TEXT PRIMARY KEY NOT NULL,
  path TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
