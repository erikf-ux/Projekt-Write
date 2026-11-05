-- Migration 0006: document history (master prompt, section 13; Phase 6,
-- ADR 0016).
--
-- Shipped migrations are never changed. New schema changes get a new file.
--
-- One row per version of a document: the content of its file at that time,
-- compressed (deflate). A version whose content equals the newest version
-- of the document is not stored. Named versions are kept by the retention
-- rules; the others thin out with age (Rust, services/history.rs).
CREATE TABLE document_history (
    id             TEXT NOT NULL PRIMARY KEY
                   CHECK (id GLOB '????????-????-????-????-????????????'
                          AND id NOT GLOB '*[^0-9a-f-]*'),
    document_id    TEXT NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    kind           TEXT NOT NULL
                   CHECK (kind IN ('auto', 'close', 'beforeConversion', 'beforeRestore', 'named')),
    name           TEXT
                   CHECK (name IS NULL OR length(name) BETWEEN 1 AND 200),
    format         TEXT NOT NULL
                   CHECK (format IN ('txt', 'md', 'pwrite')),
    content        BLOB NOT NULL,
    compression    TEXT NOT NULL
                   CHECK (compression IN ('deflate')),
    size_bytes     INTEGER NOT NULL
                   CHECK (size_bytes >= 0),
    content_sha256 TEXT NOT NULL
                   CHECK (length(content_sha256) = 64
                          AND content_sha256 NOT GLOB '*[^0-9a-f]*'),
    created_at     TEXT NOT NULL
                   CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    CHECK ((kind = 'named') = (name IS NOT NULL))
) STRICT;

CREATE INDEX ix_document_history_document ON document_history (document_id, created_at);
