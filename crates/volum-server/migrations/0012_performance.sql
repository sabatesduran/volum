ALTER TABLE models ADD COLUMN asset_modified_at TEXT;
ALTER TABLE models ADD COLUMN asset_count INTEGER NOT NULL DEFAULT 0;

UPDATE models
SET asset_modified_at = (
      SELECT MAX(a.modified_at)
      FROM model_assets ma
      JOIN assets a ON a.id = ma.asset_id
      WHERE ma.model_id = models.id
    ),
    asset_count = (
      SELECT COUNT(*)
      FROM model_assets ma
      WHERE ma.model_id = models.id
    );

CREATE INDEX idx_models_modified_page ON models(trashed_at, COALESCE(asset_modified_at, updated_at) DESC, display_name COLLATE NOCASE, id);
CREATE INDEX idx_models_name_page ON models(trashed_at, display_name COLLATE NOCASE, id);
CREATE INDEX idx_models_added_page ON models(trashed_at, added_at DESC, display_name COLLATE NOCASE, id);
CREATE INDEX idx_models_opened_page ON models(trashed_at, COALESCE(last_opened_at, '') DESC, COALESCE(asset_modified_at, updated_at) DESC, display_name COLLATE NOCASE, id);
CREATE INDEX idx_models_favorite_page ON models(trashed_at, favorite, id);

CREATE TRIGGER model_assets_insert_aggregates
AFTER INSERT ON model_assets
BEGIN
  UPDATE models
  SET asset_count = (SELECT COUNT(*) FROM model_assets WHERE model_id = NEW.model_id),
      asset_modified_at = (SELECT MAX(a.modified_at) FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE ma.model_id = NEW.model_id)
  WHERE id = NEW.model_id;
END;

CREATE TRIGGER model_assets_delete_aggregates
AFTER DELETE ON model_assets
BEGIN
  UPDATE models
  SET asset_count = (SELECT COUNT(*) FROM model_assets WHERE model_id = OLD.model_id),
      asset_modified_at = (SELECT MAX(a.modified_at) FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE ma.model_id = OLD.model_id)
  WHERE id = OLD.model_id;
END;

CREATE TRIGGER model_assets_move_aggregates
AFTER UPDATE OF model_id ON model_assets
BEGIN
  UPDATE models
  SET asset_count = (SELECT COUNT(*) FROM model_assets WHERE model_id = OLD.model_id),
      asset_modified_at = (SELECT MAX(a.modified_at) FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE ma.model_id = OLD.model_id)
  WHERE id = OLD.model_id;
  UPDATE models
  SET asset_count = (SELECT COUNT(*) FROM model_assets WHERE model_id = NEW.model_id),
      asset_modified_at = (SELECT MAX(a.modified_at) FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE ma.model_id = NEW.model_id)
  WHERE id = NEW.model_id;
END;

CREATE TRIGGER assets_modified_aggregate
AFTER UPDATE OF modified_at ON assets
BEGIN
  UPDATE models
  SET asset_modified_at = (SELECT MAX(a.modified_at) FROM model_assets ma JOIN assets a ON a.id = ma.asset_id WHERE ma.model_id = models.id)
  WHERE id IN (SELECT model_id FROM model_assets WHERE asset_id = NEW.id);
END;

ALTER TABLE thumbnails ADD COLUMN byte_size INTEGER NOT NULL DEFAULT 0;
CREATE INDEX idx_thumbnails_lookup ON thumbnails(asset_id, renderer_version, width, height, created_at DESC);
CREATE INDEX idx_cost_estimates_model_updated ON cost_estimates(model_id, updated_at DESC);

ALTER TABLE ignored_asset_paths ADD COLUMN byte_size INTEGER;
ALTER TABLE ignored_asset_paths ADD COLUMN modified_at TEXT;

CREATE TABLE viewer_meshes (
  id TEXT PRIMARY KEY NOT NULL,
  asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
  fingerprint TEXT NOT NULL,
  plate_key TEXT NOT NULL,
  renderer_version TEXT NOT NULL,
  cache_path TEXT NOT NULL UNIQUE,
  byte_size INTEGER NOT NULL,
  last_accessed_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(asset_id, fingerprint, plate_key, renderer_version)
);

CREATE INDEX idx_viewer_meshes_lru ON viewer_meshes(last_accessed_at);
