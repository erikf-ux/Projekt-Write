-- Migration 0004: favourites and "Zuletzt verwendet" (master prompt,
-- section 13; Phase 5, ADR 0015).
--
-- Shipped migrations are never changed. New schema changes get a new file.
--
-- favorited_at: the document is a favourite since then (NULL: not).
-- last_opened_at: last time the document was opened in a tab.
ALTER TABLE documents ADD COLUMN favorited_at TEXT
    CHECK (favorited_at IS NULL
           OR favorited_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z');
ALTER TABLE documents ADD COLUMN last_opened_at TEXT
    CHECK (last_opened_at IS NULL
           OR last_opened_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z');

CREATE INDEX ix_documents_recent ON documents (last_opened_at)
    WHERE last_opened_at IS NOT NULL AND deleted_at IS NULL;
