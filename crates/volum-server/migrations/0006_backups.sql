CREATE TABLE backup_destinations (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('folder', 'webdav')),
  location TEXT NOT NULL,
  username TEXT,
  credential_saved INTEGER NOT NULL DEFAULT 0,
  schedule TEXT NOT NULL DEFAULT 'weekly' CHECK(schedule IN ('manual', 'daily', 'weekly', 'monthly')),
  retention_count INTEGER NOT NULL DEFAULT 7 CHECK(retention_count BETWEEN 1 AND 1000),
  enabled INTEGER NOT NULL DEFAULT 1,
  last_attempt_at TEXT,
  last_success_at TEXT,
  last_error TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE backup_runs (
  id TEXT PRIMARY KEY NOT NULL,
  destination_id TEXT REFERENCES backup_destinations(id) ON DELETE SET NULL,
  destination_name TEXT NOT NULL,
  reason TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('running', 'complete', 'failed')),
  filename TEXT NOT NULL,
  byte_size INTEGER,
  started_at TEXT NOT NULL,
  completed_at TEXT,
  error TEXT
);

CREATE INDEX idx_backup_destinations_enabled ON backup_destinations(enabled, schedule);
CREATE INDEX idx_backup_runs_started ON backup_runs(started_at DESC);
