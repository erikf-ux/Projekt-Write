-- Migration 0007: subnets, memberships and tags (master prompt, sections
-- 13, 16 and 18; Phase 7, ADR 0017).
--
-- Shipped migrations are never changed. New schema changes get a new file.
--
-- Subnets form a tree through parent_id; a trigger refuses cycles. Names
-- are unique among siblings (name_key: Unicode lower case, made in Rust).
-- Deleting a subnet never deletes an element: memberships go with it
-- (ON DELETE CASCADE), the documents stay. Children must be moved first
-- (ON DELETE RESTRICT); Rust does that in the same transaction.
CREATE TABLE subnets (
    id          TEXT NOT NULL PRIMARY KEY
                CHECK (id GLOB '????????-????-????-????-????????????'
                       AND id NOT GLOB '*[^0-9a-f-]*'),
    name        TEXT NOT NULL
                CHECK (length(name) BETWEEN 1 AND 200),
    name_key    TEXT NOT NULL
                CHECK (length(name_key) BETWEEN 1 AND 800),
    description TEXT NOT NULL DEFAULT ''
                CHECK (length(description) <= 2000),
    color       TEXT NOT NULL
                CHECK (color IN ('blue', 'green', 'violet', 'orange', 'grey')
                       OR (length(color) = 7
                           AND color GLOB '#[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]')),
    parent_id   TEXT REFERENCES subnets (id) ON DELETE RESTRICT,
    created_at  TEXT NOT NULL
                CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    updated_at  TEXT NOT NULL
                CHECK (updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    CHECK (parent_id IS NULL OR parent_id <> id)
) STRICT;

CREATE UNIQUE INDEX uq_subnets_sibling_name ON subnets (ifnull(parent_id, ''), name_key);
CREATE INDEX ix_subnets_parent ON subnets (parent_id);

-- No subnet below itself or one of its descendants.
CREATE TRIGGER tr_subnets_no_cycle
BEFORE UPDATE OF parent_id ON subnets
WHEN NEW.parent_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'subnet cycle')
    WHERE EXISTS (
        WITH RECURSIVE ancestors (id) AS (
            SELECT NEW.parent_id
            UNION
            SELECT s.parent_id
            FROM subnets AS s
            JOIN ancestors AS a ON s.id = a.id
            WHERE s.parent_id IS NOT NULL
        )
        SELECT 1 FROM ancestors WHERE id = NEW.id
    );
END;

-- The tested pattern of section 18 for documents; media join in Phase 8
-- (the table is rebuilt with media_id then). Exactly one membership of a
-- document is primary: the first one; when it goes, the oldest remaining
-- one (Rust keeps that in every transaction).
CREATE TABLE subnet_memberships (
    subnet_id   TEXT NOT NULL REFERENCES subnets (id) ON DELETE CASCADE,
    document_id TEXT NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    is_primary  INTEGER NOT NULL DEFAULT 0
                CHECK (is_primary IN (0, 1)),
    created_at  TEXT NOT NULL
                CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z')
) STRICT;

CREATE UNIQUE INDEX uq_member_doc ON subnet_memberships (subnet_id, document_id);
CREATE UNIQUE INDEX uq_primary_doc ON subnet_memberships (document_id) WHERE is_primary = 1;
CREATE INDEX ix_memberships_document ON subnet_memberships (document_id, created_at);

-- Tags: names unique without regard to case (name_key, section 13).
CREATE TABLE tags (
    id         TEXT NOT NULL PRIMARY KEY
               CHECK (id GLOB '????????-????-????-????-????????????'
                      AND id NOT GLOB '*[^0-9a-f-]*'),
    name       TEXT NOT NULL
               CHECK (length(name) BETWEEN 1 AND 100),
    name_key   TEXT NOT NULL UNIQUE
               CHECK (length(name_key) BETWEEN 1 AND 400),
    created_at TEXT NOT NULL
               CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    updated_at TEXT NOT NULL
               CHECK (updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z')
) STRICT;

CREATE TABLE document_tags (
    document_id TEXT NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    tag_id      TEXT NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL
                CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    PRIMARY KEY (document_id, tag_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_document_tags_tag ON document_tags (tag_id, document_id);
