ALTER TABLE collections ADD COLUMN kind TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE collections ADD COLUMN rule_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE models ADD COLUMN version_key TEXT NOT NULL DEFAULT '';

CREATE TABLE tags (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL COLLATE NOCASE UNIQUE,
  color TEXT NOT NULL DEFAULT '#ff5a36',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE model_tags (
  model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
  tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  added_at TEXT NOT NULL,
  PRIMARY KEY(model_id, tag_id)
);

CREATE INDEX idx_model_tags_tag ON model_tags(tag_id, model_id);
CREATE INDEX idx_models_version_key ON models(version_key);
CREATE INDEX idx_assets_content_hash ON assets(content_hash) WHERE content_hash IS NOT NULL;
