CREATE TABLE web_sources (
  id TEXT PRIMARY KEY NOT NULL,
  provider TEXT NOT NULL,
  remote_id TEXT,
  canonical_url TEXT NOT NULL UNIQUE,
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
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_web_sources_status ON web_sources(status, updated_at DESC);
CREATE INDEX idx_web_sources_attachment ON web_sources(root_id, relative_path);
CREATE INDEX idx_web_sources_model ON web_sources(model_id);
