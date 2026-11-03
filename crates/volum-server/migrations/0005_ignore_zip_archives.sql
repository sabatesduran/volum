-- ZIP archives are containers, not model-library items. Preserve the source files
-- on disk while removing previously indexed ZIP records from Volum's database.

UPDATE web_sources
SET status = 'saved',
    root_id = NULL,
    relative_path = NULL,
    model_id = NULL,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE model_id IN (
  SELECT m.id
  FROM models m
  WHERE EXISTS (
    SELECT 1
    FROM model_assets ma
    JOIN assets a ON a.id = ma.asset_id
    WHERE ma.model_id = m.id AND lower(a.extension) = 'zip'
  )
  AND NOT EXISTS (
    SELECT 1
    FROM model_assets ma
    JOIN assets a ON a.id = ma.asset_id
    WHERE ma.model_id = m.id AND lower(a.extension) != 'zip'
  )
);

DELETE FROM model_search
WHERE model_id IN (
  SELECT m.id
  FROM models m
  WHERE EXISTS (
    SELECT 1
    FROM model_assets ma
    JOIN assets a ON a.id = ma.asset_id
    WHERE ma.model_id = m.id AND lower(a.extension) = 'zip'
  )
  AND NOT EXISTS (
    SELECT 1
    FROM model_assets ma
    JOIN assets a ON a.id = ma.asset_id
    WHERE ma.model_id = m.id AND lower(a.extension) != 'zip'
  )
);

DELETE FROM models
WHERE id IN (
  SELECT m.id
  FROM models m
  WHERE EXISTS (
    SELECT 1
    FROM model_assets ma
    JOIN assets a ON a.id = ma.asset_id
    WHERE ma.model_id = m.id AND lower(a.extension) = 'zip'
  )
  AND NOT EXISTS (
    SELECT 1
    FROM model_assets ma
    JOIN assets a ON a.id = ma.asset_id
    WHERE ma.model_id = m.id AND lower(a.extension) != 'zip'
  )
);

UPDATE models
SET primary_asset_id = (
  SELECT a.id
  FROM model_assets ma
  JOIN assets a ON a.id = ma.asset_id
  WHERE ma.model_id = models.id AND lower(a.extension) != 'zip'
  ORDER BY CASE a.extension
    WHEN '3mf' THEN 0
    WHEN 'stl' THEN 1
    WHEN 'obj' THEN 2
    WHEN 'step' THEN 3
    WHEN 'stp' THEN 3
    ELSE 4
  END,
  a.filename
  LIMIT 1
)
WHERE primary_asset_id IN (
  SELECT id FROM assets WHERE lower(extension) = 'zip'
);

DELETE FROM assets WHERE lower(extension) = 'zip';
