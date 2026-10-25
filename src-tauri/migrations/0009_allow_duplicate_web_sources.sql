-- A source URL can legitimately belong to more than one independently imported
-- portable project. Keep normal saves deduplicated in application code while
-- allowing package imports to preserve each project's source metadata.
CREATE TABLE web_sources_v2 (
  id TEXT PRIMARY KEY NOT NULL,
  provider TEXT NOT NULL,
  remote_id TEXT,
  canonical_url TEXT NOT NULL,
  title TEXT NOT NULL,
  creator TEXT,
  description TEXT,
  license TEXT,
  image_url TEXT,
  status TEXT NOT NULL DEFAULT 'saved',
  root_id TEXT REFERENCES library_roots(id) ON DELETE SET NULL,
  relative_path TEXT,
  model_id TEXT REFERENCES models(id) ON DELETE SET NULL,
  fetched_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  filament_grams REAL
);

INSERT INTO web_sources_v2 (id, provider, remote_id, canonical_url, title, creator, description, license, image_url, status, root_id, relative_path, model_id, fetched_at, created_at, updated_at, filament_grams)
SELECT id, provider, remote_id, canonical_url, title, creator, description, license, image_url, status, root_id, relative_path, model_id, fetched_at, created_at, updated_at, filament_grams FROM web_sources;

DROP TABLE web_sources;
ALTER TABLE web_sources_v2 RENAME TO web_sources;
CREATE INDEX idx_web_sources_url ON web_sources(canonical_url);
CREATE INDEX idx_web_sources_status ON web_sources(status, updated_at DESC);
CREATE INDEX idx_web_sources_attachment ON web_sources(root_id, relative_path);
CREATE INDEX idx_web_sources_model ON web_sources(model_id);
