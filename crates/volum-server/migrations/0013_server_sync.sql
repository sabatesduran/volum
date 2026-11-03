CREATE TABLE sync_changes (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  change_kind TEXT NOT NULL,
  revision TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX idx_sync_changes_entity ON sync_changes(entity_type, entity_id, sequence DESC);

CREATE TRIGGER sync_models_insert
AFTER INSERT ON models
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_models_update
AFTER UPDATE ON models
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.id, CASE WHEN NEW.trashed_at IS NULL THEN 'upsert' ELSE 'trash' END, NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_models_delete
AFTER DELETE ON models
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', OLD.id, 'delete', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_assets_insert
AFTER INSERT ON assets
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('asset', NEW.id, 'upsert', NEW.modified_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_assets_update
AFTER UPDATE ON assets
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('asset', NEW.id, CASE WHEN NEW.missing_since IS NULL THEN 'upsert' ELSE 'missing' END, NEW.modified_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_assets_delete
AFTER DELETE ON assets
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('asset', OLD.id, 'delete', OLD.modified_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_collections_insert
AFTER INSERT ON collections
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('collection', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_collections_update
AFTER UPDATE ON collections
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('collection', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_collections_delete
AFTER DELETE ON collections
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('collection', OLD.id, 'delete', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_tags_insert
AFTER INSERT ON tags
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('tag', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_tags_update
AFTER UPDATE ON tags
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('tag', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_tags_delete
AFTER DELETE ON tags
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('tag', OLD.id, 'delete', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_collection_items_insert
AFTER INSERT ON collection_items
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_collection_items_delete
AFTER DELETE ON collection_items
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', OLD.model_id, 'upsert', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_model_tags_insert
AFTER INSERT ON model_tags
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_model_tags_delete
AFTER DELETE ON model_tags
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', OLD.model_id, 'upsert', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_model_assets_insert
AFTER INSERT ON model_assets
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_model_assets_delete
AFTER DELETE ON model_assets
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', OLD.model_id, 'upsert', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_materials_insert
AFTER INSERT ON materials
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('material', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_materials_update
AFTER UPDATE ON materials
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('material', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_materials_delete
AFTER DELETE ON materials
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('material', OLD.id, 'delete', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_saved_searches_insert
AFTER INSERT ON saved_searches
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('saved-search', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_saved_searches_update
AFTER UPDATE ON saved_searches
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('saved-search', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_saved_searches_delete
AFTER DELETE ON saved_searches
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('saved-search', OLD.id, 'delete', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_custom_fields_insert
AFTER INSERT ON custom_field_definitions
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('custom-field', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_custom_fields_update
AFTER UPDATE ON custom_field_definitions
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('custom-field', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_custom_fields_delete
AFTER DELETE ON custom_field_definitions
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('custom-field', OLD.id, 'delete', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_project_photos_insert
AFTER INSERT ON project_photos
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_project_photos_update
AFTER UPDATE ON project_photos
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_project_photos_delete
AFTER DELETE ON project_photos
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', OLD.model_id, 'upsert', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_project_custom_values_insert
AFTER INSERT ON project_custom_values
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_project_custom_values_update
AFTER UPDATE ON project_custom_values
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_project_custom_values_delete
AFTER DELETE ON project_custom_values
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', OLD.model_id, 'upsert', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_project_bom_insert
AFTER INSERT ON project_bom_items
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_project_bom_update
AFTER UPDATE ON project_bom_items
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_project_bom_delete
AFTER DELETE ON project_bom_items
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', OLD.model_id, 'upsert', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_assembly_steps_insert
AFTER INSERT ON assembly_steps
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_assembly_steps_update
AFTER UPDATE ON assembly_steps
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_assembly_steps_delete
AFTER DELETE ON assembly_steps
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', OLD.model_id, 'upsert', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_cost_estimates_insert
AFTER INSERT ON cost_estimates
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_cost_estimates_update
AFTER UPDATE ON cost_estimates
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', NEW.model_id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_cost_estimates_delete
AFTER DELETE ON cost_estimates
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('project', OLD.model_id, 'upsert', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_web_sources_insert
AFTER INSERT ON web_sources
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('web-source', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_web_sources_update
AFTER UPDATE ON web_sources
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('web-source', NEW.id, 'upsert', NEW.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;

CREATE TRIGGER sync_web_sources_delete
AFTER DELETE ON web_sources
BEGIN
  INSERT INTO sync_changes(entity_type, entity_id, change_kind, revision, created_at)
  VALUES ('web-source', OLD.id, 'delete', OLD.updated_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
END;
