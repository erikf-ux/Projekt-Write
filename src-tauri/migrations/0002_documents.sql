-- Migration 0002: documents (master prompt, sections 9, 13 and 18).
--
-- Shipped migrations are never changed. New schema changes get a new file.
--
-- One row per document of the library. The content lives in a file named
-- after the id: documents/text/<id>.txt, documents/markdown/<id>.md or
-- documents/richtext/<id>.pwrite. The path is derived from id and format and
-- never stored, so a manipulated database cannot point outside the library.
--
-- size_bytes and content_sha256 describe the file as last saved. If a crash
-- happens between renaming the file and updating this row, the next read
-- notices the difference and corrects the row.
--
-- archived_at and deleted_at implement "Archiv" and "Papierkorb" (section 4)
-- without deleting rows (section 18).
CREATE TABLE documents (
    id             TEXT NOT NULL PRIMARY KEY
                   CHECK (id GLOB '????????-????-????-????-????????????'
                          AND id NOT GLOB '*[^0-9a-f-]*'),
    title          TEXT NOT NULL
                   CHECK (length(title) BETWEEN 1 AND 300),
    format         TEXT NOT NULL
                   CHECK (format IN ('txt', 'md', 'pwrite')),
    size_bytes     INTEGER NOT NULL
                   CHECK (size_bytes >= 0),
    content_sha256 TEXT NOT NULL
                   CHECK (length(content_sha256) = 64
                          AND content_sha256 NOT GLOB '*[^0-9a-f]*'),
    created_at     TEXT NOT NULL
                   CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    updated_at     TEXT NOT NULL
                   CHECK (updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    archived_at    TEXT
                   CHECK (archived_at IS NULL
                          OR archived_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    deleted_at     TEXT
                   CHECK (deleted_at IS NULL
                          OR deleted_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z')
) STRICT;

-- "Zuletzt geändert" lists and the explorer (Phases 3 and 5) read active
-- documents by modification time.
CREATE INDEX ix_documents_active_updated_at
    ON documents (updated_at)
    WHERE deleted_at IS NULL;
