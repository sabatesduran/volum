CREATE TABLE project_photos (
  id TEXT PRIMARY KEY NOT NULL,
  model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
  media_path TEXT NOT NULL UNIQUE,
  thumbnail_path TEXT NOT NULL UNIQUE,
  original_filename TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL,
  caption TEXT NOT NULL DEFAULT '',
  sort_order REAL NOT NULL DEFAULT 0,
  is_cover INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_project_photos_model_order ON project_photos(model_id, sort_order, created_at);
CREATE UNIQUE INDEX idx_project_photos_one_cover ON project_photos(model_id) WHERE is_cover = 1;

CREATE TABLE custom_field_definitions (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL COLLATE NOCASE UNIQUE,
  field_type TEXT NOT NULL,
  unit TEXT,
  options_json TEXT NOT NULL DEFAULT '[]',
  sort_order REAL NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE project_custom_values (
  model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
  field_id TEXT NOT NULL REFERENCES custom_field_definitions(id) ON DELETE CASCADE,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(model_id, field_id)
);

CREATE INDEX idx_project_custom_values_field ON project_custom_values(field_id, model_id);

CREATE TABLE project_bom_items (
  id TEXT PRIMARY KEY NOT NULL,
  model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
  kind TEXT NOT NULL DEFAULT 'purchased',
  linked_asset_id TEXT REFERENCES assets(id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  quantity REAL NOT NULL DEFAULT 1,
  unit TEXT NOT NULL DEFAULT 'pcs',
  url TEXT,
  notes TEXT NOT NULL DEFAULT '',
  sort_order REAL NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_project_bom_model_order ON project_bom_items(model_id, sort_order, created_at);

CREATE TABLE assembly_steps (
  id TEXT PRIMARY KEY NOT NULL,
  model_id TEXT NOT NULL REFERENCES models(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  instructions TEXT NOT NULL DEFAULT '',
  sort_order REAL NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_assembly_steps_model_order ON assembly_steps(model_id, sort_order, created_at);

CREATE TABLE assembly_step_bom_items (
  step_id TEXT NOT NULL REFERENCES assembly_steps(id) ON DELETE CASCADE,
  bom_item_id TEXT NOT NULL REFERENCES project_bom_items(id) ON DELETE CASCADE,
  sort_order REAL NOT NULL DEFAULT 0,
  PRIMARY KEY(step_id, bom_item_id)
);

CREATE TABLE assembly_step_photos (
  step_id TEXT NOT NULL REFERENCES assembly_steps(id) ON DELETE CASCADE,
  photo_id TEXT NOT NULL REFERENCES project_photos(id) ON DELETE CASCADE,
  sort_order REAL NOT NULL DEFAULT 0,
  PRIMARY KEY(step_id, photo_id)
);

CREATE TABLE project_package_imports (
  model_id TEXT PRIMARY KEY NOT NULL REFERENCES models(id) ON DELETE CASCADE,
  package_project_id TEXT NOT NULL,
  package_export_id TEXT NOT NULL,
  imported_at TEXT NOT NULL
);

CREATE INDEX idx_project_package_source ON project_package_imports(package_project_id, imported_at);
