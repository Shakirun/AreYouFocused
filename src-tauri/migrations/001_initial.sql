CREATE TABLE IF NOT EXISTS schema_version (
  version INTEGER PRIMARY KEY
);

CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS captures (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  body TEXT NOT NULL,
  created_at_unix INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS scheduler_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  next_ping_at_unix INTEGER
);

INSERT OR IGNORE INTO schema_version (version) VALUES (1);
INSERT OR IGNORE INTO scheduler_state (id, next_ping_at_unix) VALUES (1, NULL);
INSERT OR IGNORE INTO settings (key, value) VALUES ('ping_min_minutes', '30');
INSERT OR IGNORE INTO settings (key, value) VALUES ('ping_max_minutes', '120');
