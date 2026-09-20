PRAGMA foreign_keys = ON;

CREATE TABLE library_roots (
  id TEXT PRIMARY KEY NOT NULL,
  path TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  volume_key TEXT,
  status TEXT NOT NULL DEFAULT 'online',
  last_scan_at TEXT,
  scan_cursor TEXT,
  ignore_json TEXT NOT NULL DEFAULT '[]',
  case_sensitive INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE folders (
  id TEXT PRIMARY KEY NOT NULL,
  root_id TEXT NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
  parent_id TEXT REFERENCES folders(id) ON DELETE CASCADE,
  relative_path TEXT NOT NULL,
  name TEXT NOT NULL,
  modified_at TEXT,
  status TEXT NOT NULL DEFAULT 'available',
  UNIQUE(root_id, relative_path)
);

CREATE TABLE models (
  id TEXT PRIMARY KEY NOT NULL,
  folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
  display_name TEXT NOT NULL,
  primary_asset_id TEXT,
  notes TEXT NOT NULL DEFAULT '',
  favorite INTEGER NOT NULL DEFAULT 0,
  grouping_key TEXT NOT NULL,
  grouping_override TEXT,
  added_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_opened_at TEXT,
  missing_since TEXT,
  UNIQUE(folder_id, grouping_key)
);

CREATE TABLE assets (
  id TEXT PRIMARY KEY NOT NULL,
  root_id TEXT NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
  folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
  relative_path TEXT NOT NULL,
  filename TEXT NOT NULL,
  extension TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  modified_at TEXT NOT NULL,
  partial_fingerprint TEXT,
  content_hash TEXT,
  parse_status TEXT NOT NULL DEFAULT 'pending',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  missing_since TEXT,
  UNIQUE(root_id, relative_path)
);

CREATE TABLE model_assets (
  model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
  asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
  role TEXT NOT NULL DEFAULT 'variant',
  sort_order INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY(model_id, asset_id)
);

CREATE TABLE thumbnails (
  id TEXT PRIMARY KEY NOT NULL,
  asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
  fingerprint TEXT NOT NULL,
  renderer_version TEXT NOT NULL,
  preset TEXT NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  cache_path TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(asset_id, fingerprint, renderer_version, preset, width, height)
);

CREATE TABLE collections (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  symbol TEXT NOT NULL DEFAULT 'box',
  color TEXT NOT NULL DEFAULT '#ff5a36',
  sort_order REAL NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE collection_items (
  collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
  position REAL NOT NULL DEFAULT 0,
  added_at TEXT NOT NULL,
  PRIMARY KEY(collection_id, model_id)
);

CREATE TABLE materials (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  material_type TEXT NOT NULL,
  color_name TEXT,
  color_hex TEXT,
  spool_price_minor INTEGER NOT NULL,
  currency TEXT NOT NULL,
  spool_weight_g REAL NOT NULL,
  density REAL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE cost_estimates (
  id TEXT PRIMARY KEY NOT NULL,
  model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
  asset_id TEXT REFERENCES assets(id) ON DELETE SET NULL,
  material_id TEXT REFERENCES materials(id) ON DELETE SET NULL,
  plastic_g REAL NOT NULL,
  quantity INTEGER NOT NULL,
  source TEXT NOT NULL,
  source_fingerprint TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE user_overrides (
  id TEXT PRIMARY KEY NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  key TEXT NOT NULL,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(entity_type, entity_id, key)
);

CREATE VIRTUAL TABLE model_search USING fts5(
  model_id UNINDEXED,
  display_name,
  filenames,
  folder_path,
  collections,
  notes,
  formats,
  tokenize = 'unicode61 remove_diacritics 2'
);

CREATE INDEX idx_folders_root_parent ON folders(root_id, parent_id);
CREATE INDEX idx_models_folder ON models(folder_id, missing_since);
CREATE INDEX idx_models_recent ON models(last_opened_at DESC);
CREATE INDEX idx_models_added ON models(added_at DESC);
CREATE INDEX idx_assets_root_missing ON assets(root_id, missing_since);
CREATE INDEX idx_assets_fingerprint ON assets(root_id, partial_fingerprint, byte_size);
CREATE INDEX idx_model_assets_asset ON model_assets(asset_id);
CREATE INDEX idx_collection_items_model ON collection_items(model_id);
