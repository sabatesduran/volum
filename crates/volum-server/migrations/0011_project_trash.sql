ALTER TABLE models ADD COLUMN trashed_at TEXT;
ALTER TABLE models ADD COLUMN trash_original_relative_path TEXT;

CREATE INDEX idx_models_trash ON models(trashed_at, updated_at);
