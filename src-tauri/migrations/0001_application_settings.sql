-- Migration 0001: library-wide settings (master prompt, section 9 "Einstellungen").
--
-- Shipped migrations are never changed. New schema changes get a new file.
-- Conventions (section 18): STRICT tables, UUID text ids, UTC ISO 8601 times.
--
-- Settings are a key/value store. Values are JSON documents so that each
-- setting keeps its own type; SQLite validates the JSON on every write.
CREATE TABLE application_settings (
    key        TEXT NOT NULL PRIMARY KEY
               CHECK (length(key) BETWEEN 1 AND 128),
    value      TEXT NOT NULL
               CHECK (json_valid(value)),
    updated_at TEXT NOT NULL
               CHECK (updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z')
) STRICT, WITHOUT ROWID;
