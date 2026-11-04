-- Migration 0005: links between documents (master prompt, section 12;
-- Phase 5, ADR 0015).
--
-- Shipped migrations are never changed. New schema changes get a new file.
--
-- One row per internal link (`pwrite://<id>`) from source_id to target_id.
-- Rust derives the rows from the content on every save, in the same
-- transaction as the metadata; they are never edited by hand. The target
-- need not exist: a link to a missing document stays a link. A link to the
-- document itself is not stored.
CREATE TABLE document_links (
    source_id TEXT NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    target_id TEXT NOT NULL
              CHECK (target_id GLOB '????????-????-????-????-????????????'
                     AND target_id NOT GLOB '*[^0-9a-f-]*'),
    PRIMARY KEY (source_id, target_id),
    CHECK (source_id <> target_id)
) STRICT, WITHOUT ROWID;

-- Backlinks ("Verbindungen"): which documents link to this one.
CREATE INDEX ix_document_links_target ON document_links (target_id, source_id);
