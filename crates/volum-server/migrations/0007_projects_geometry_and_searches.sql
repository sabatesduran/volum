ALTER TABLE models ADD COLUMN bundle_mode TEXT NOT NULL DEFAULT 'automatic';

-- Older scans always assigned an asset to one model, but the original join table
-- did not encode that invariant. Keep the first assignment if an early database
-- somehow contains more than one before enforcing project ownership.
DELETE FROM model_assets
WHERE rowid NOT IN (
  SELECT MIN(rowid)
  FROM model_assets
  GROUP BY asset_id
);

CREATE UNIQUE INDEX idx_model_assets_asset_project ON model_assets(asset_id);
CREATE INDEX idx_models_bundle_mode ON models(bundle_mode);

UPDATE model_assets
SET role = CASE (
  SELECT extension FROM assets WHERE assets.id = model_assets.asset_id
)
  WHEN 'step' THEN 'source'
  WHEN 'stp' THEN 'source'
  WHEN '3mf' THEN 'plate'
  ELSE 'printable'
END;

CREATE TABLE saved_searches (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  query_json TEXT NOT NULL,
  sort_order REAL NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE asset_geometry (
  asset_id TEXT PRIMARY KEY NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
  source_fingerprint TEXT NOT NULL,
  algorithm_version TEXT NOT NULL,
  geometry_hash TEXT NOT NULL,
  similarity_key TEXT NOT NULL,
  dimensions_json TEXT NOT NULL,
  surface_area REAL NOT NULL,
  volume REAL,
  triangle_count INTEGER NOT NULL,
  descriptor_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_asset_geometry_hash ON asset_geometry(geometry_hash);
CREATE INDEX idx_asset_geometry_similarity ON asset_geometry(similarity_key);

CREATE TABLE duplicate_dismissals (
  match_key TEXT PRIMARY KEY NOT NULL,
  created_at TEXT NOT NULL
);
