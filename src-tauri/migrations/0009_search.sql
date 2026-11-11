-- Migration 0009: the search index (master prompt, sections 18 and 19;
-- Phase 9, ADR 0020).
--
-- Shipped migrations are never changed. New schema changes get a new file.
--
-- One row of search_entries per document, medium and subnet; its id is the
-- rowid of the element in both FTS5 tables. Everything the database knows
-- (titles, tags, subnets, metadata, the description of a medium or subnet)
-- is kept current by the triggers below, in the transaction of the change.
-- The body of a document lives in its file: Rust writes it in the
-- transaction that records the new content and notes the checksum of the
-- content it indexed (indexed_sha256). Opening a library fills in what is
-- missing or stale, for example right after this migration.
--
-- Archive and trash do not touch the index: a search reads the state from
-- documents and media. Deleting an element for good deletes its entry
-- (ON DELETE CASCADE) and the entry trigger its FTS rows.
CREATE TABLE search_entries (
    id             INTEGER PRIMARY KEY,
    document_id    TEXT UNIQUE REFERENCES documents (id) ON DELETE CASCADE,
    media_id       TEXT UNIQUE REFERENCES media (id) ON DELETE CASCADE,
    subnet_id      TEXT UNIQUE REFERENCES subnets (id) ON DELETE CASCADE,
    indexed_sha256 TEXT
                   CHECK (indexed_sha256 IS NULL
                          OR (length(indexed_sha256) = 64 AND indexed_sha256 NOT GLOB '*[^0-9a-f]*')),
    CHECK ((document_id IS NOT NULL) + (media_id IS NOT NULL) + (subnet_id IS NOT NULL) = 1),
    CHECK (indexed_sha256 IS NULL OR document_id IS NOT NULL)
) STRICT;

-- Section 19, unchanged: words and word beginnings ...
CREATE VIRTUAL TABLE search_words USING fts5(
    title, body, tags, subnets, meta,
    tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3'
);

-- ... and parts of words (trigram, detail=full as FTS5's default).
CREATE VIRTUAL TABLE search_parts USING fts5(
    title, body,
    tokenize = 'trigram remove_diacritics 1'
);

-- What the index holds of an element, besides the body of a document.
-- Tags and subnets are names separated by a space; meta is the file name at
-- import, the import folder and the format.
CREATE VIEW search_document_fields AS
SELECT d.id AS id,
       d.title AS title,
       (SELECT ifnull(group_concat(t.name, ' '), '')
          FROM document_tags AS dt JOIN tags AS t ON t.id = dt.tag_id
         WHERE dt.document_id = d.id) AS tags,
       (SELECT ifnull(group_concat(s.name, ' '), '')
          FROM subnet_memberships AS m JOIN subnets AS s ON s.id = m.subnet_id
         WHERE m.document_id = d.id) AS subnets,
       trim(ifnull(d.original_name, '') || ' ' || d.format) AS meta
  FROM documents AS d;

CREATE VIEW search_media_fields AS
SELECT x.id AS id,
       x.title AS title,
       x.description AS body,
       (SELECT ifnull(group_concat(t.name, ' '), '')
          FROM media_tags AS mt JOIN tags AS t ON t.id = mt.tag_id
         WHERE mt.media_id = x.id) AS tags,
       (SELECT ifnull(group_concat(s.name, ' '), '')
          FROM subnet_memberships AS m JOIN subnets AS s ON s.id = m.subnet_id
         WHERE m.media_id = x.id) AS subnets,
       trim(x.original_name || ' ' || ifnull(x.import_folder, '') || ' ' || x.extension) AS meta
  FROM media AS x;

-- A subnet is found by its name and description; its subnets column holds
-- the name of its parent.
CREATE VIEW search_subnet_fields AS
SELECT s.id AS id,
       s.name AS title,
       s.description AS body,
       ifnull((SELECT p.name FROM subnets AS p WHERE p.id = s.parent_id), '') AS subnets
  FROM subnets AS s;

-- What exists already. The bodies of documents follow when the library opens.
INSERT INTO search_entries (document_id) SELECT id FROM documents ORDER BY created_at, id;
INSERT INTO search_entries (media_id) SELECT id FROM media ORDER BY created_at, id;
INSERT INTO search_entries (subnet_id) SELECT id FROM subnets ORDER BY created_at, id;

INSERT INTO search_words (rowid, title, body, tags, subnets, meta)
SELECT e.id, f.title, '', f.tags, f.subnets, f.meta
  FROM search_entries AS e JOIN search_document_fields AS f ON f.id = e.document_id;
INSERT INTO search_words (rowid, title, body, tags, subnets, meta)
SELECT e.id, f.title, f.body, f.tags, f.subnets, f.meta
  FROM search_entries AS e JOIN search_media_fields AS f ON f.id = e.media_id;
INSERT INTO search_words (rowid, title, body, tags, subnets, meta)
SELECT e.id, f.title, f.body, '', f.subnets, ''
  FROM search_entries AS e JOIN search_subnet_fields AS f ON f.id = e.subnet_id;

INSERT INTO search_parts (rowid, title, body)
SELECT e.id, f.title, '' FROM search_entries AS e JOIN search_document_fields AS f ON f.id = e.document_id;
INSERT INTO search_parts (rowid, title, body)
SELECT e.id, f.title, f.body FROM search_entries AS e JOIN search_media_fields AS f ON f.id = e.media_id;
INSERT INTO search_parts (rowid, title, body)
SELECT e.id, f.title, f.body FROM search_entries AS e JOIN search_subnet_fields AS f ON f.id = e.subnet_id;

-- Entries ------------------------------------------------------------------

CREATE TRIGGER tr_search_entries_delete
AFTER DELETE ON search_entries
BEGIN
    DELETE FROM search_words WHERE rowid = OLD.id;
    DELETE FROM search_parts WHERE rowid = OLD.id;
END;

-- Documents ----------------------------------------------------------------

CREATE TRIGGER tr_search_documents_insert
AFTER INSERT ON documents
BEGIN
    INSERT INTO search_entries (document_id) VALUES (NEW.id);
    INSERT INTO search_words (rowid, title, body, tags, subnets, meta)
    SELECT e.id, f.title, '', f.tags, f.subnets, f.meta
      FROM search_entries AS e JOIN search_document_fields AS f ON f.id = e.document_id
     WHERE e.document_id = NEW.id;
    INSERT INTO search_parts (rowid, title, body)
    SELECT e.id, NEW.title, '' FROM search_entries AS e WHERE e.document_id = NEW.id;
END;

CREATE TRIGGER tr_search_documents_update
AFTER UPDATE OF title, original_name, format ON documents
BEGIN
    UPDATE search_words
       SET title = NEW.title,
           meta = (SELECT meta FROM search_document_fields WHERE id = NEW.id)
     WHERE rowid = (SELECT id FROM search_entries WHERE document_id = NEW.id);
    UPDATE search_parts SET title = NEW.title
     WHERE rowid = (SELECT id FROM search_entries WHERE document_id = NEW.id);
END;

CREATE TRIGGER tr_search_document_tags_insert
AFTER INSERT ON document_tags
BEGIN
    UPDATE search_words SET tags = (SELECT tags FROM search_document_fields WHERE id = NEW.document_id)
     WHERE rowid = (SELECT id FROM search_entries WHERE document_id = NEW.document_id);
END;

CREATE TRIGGER tr_search_document_tags_delete
AFTER DELETE ON document_tags
BEGIN
    UPDATE search_words SET tags = (SELECT tags FROM search_document_fields WHERE id = OLD.document_id)
     WHERE rowid = (SELECT id FROM search_entries WHERE document_id = OLD.document_id);
END;

-- Media --------------------------------------------------------------------

CREATE TRIGGER tr_search_media_insert
AFTER INSERT ON media
BEGIN
    INSERT INTO search_entries (media_id) VALUES (NEW.id);
    INSERT INTO search_words (rowid, title, body, tags, subnets, meta)
    SELECT e.id, f.title, f.body, f.tags, f.subnets, f.meta
      FROM search_entries AS e JOIN search_media_fields AS f ON f.id = e.media_id
     WHERE e.media_id = NEW.id;
    INSERT INTO search_parts (rowid, title, body)
    SELECT e.id, NEW.title, NEW.description FROM search_entries AS e WHERE e.media_id = NEW.id;
END;

CREATE TRIGGER tr_search_media_update
AFTER UPDATE OF title, description, original_name, import_folder, extension ON media
BEGIN
    UPDATE search_words
       SET title = NEW.title,
           body = NEW.description,
           meta = (SELECT meta FROM search_media_fields WHERE id = NEW.id)
     WHERE rowid = (SELECT id FROM search_entries WHERE media_id = NEW.id);
    UPDATE search_parts SET title = NEW.title, body = NEW.description
     WHERE rowid = (SELECT id FROM search_entries WHERE media_id = NEW.id);
END;

CREATE TRIGGER tr_search_media_tags_insert
AFTER INSERT ON media_tags
BEGIN
    UPDATE search_words SET tags = (SELECT tags FROM search_media_fields WHERE id = NEW.media_id)
     WHERE rowid = (SELECT id FROM search_entries WHERE media_id = NEW.media_id);
END;

CREATE TRIGGER tr_search_media_tags_delete
AFTER DELETE ON media_tags
BEGIN
    UPDATE search_words SET tags = (SELECT tags FROM search_media_fields WHERE id = OLD.media_id)
     WHERE rowid = (SELECT id FROM search_entries WHERE media_id = OLD.media_id);
END;

-- Tags ---------------------------------------------------------------------

CREATE TRIGGER tr_search_tags_update
AFTER UPDATE OF name ON tags
BEGIN
    UPDATE search_words
       SET tags = (SELECT f.tags FROM search_entries AS e
                     JOIN search_document_fields AS f ON f.id = e.document_id
                    WHERE e.id = search_words.rowid)
     WHERE rowid IN (SELECT e.id FROM search_entries AS e
                       JOIN document_tags AS dt ON dt.document_id = e.document_id
                      WHERE dt.tag_id = NEW.id);
    UPDATE search_words
       SET tags = (SELECT f.tags FROM search_entries AS e
                     JOIN search_media_fields AS f ON f.id = e.media_id
                    WHERE e.id = search_words.rowid)
     WHERE rowid IN (SELECT e.id FROM search_entries AS e
                       JOIN media_tags AS mt ON mt.media_id = e.media_id
                      WHERE mt.tag_id = NEW.id);
END;

-- Subnets and memberships ----------------------------------------------------

CREATE TRIGGER tr_search_subnets_insert
AFTER INSERT ON subnets
BEGIN
    INSERT INTO search_entries (subnet_id) VALUES (NEW.id);
    INSERT INTO search_words (rowid, title, body, tags, subnets, meta)
    SELECT e.id, f.title, f.body, '', f.subnets, ''
      FROM search_entries AS e JOIN search_subnet_fields AS f ON f.id = e.subnet_id
     WHERE e.subnet_id = NEW.id;
    INSERT INTO search_parts (rowid, title, body)
    SELECT e.id, NEW.name, NEW.description FROM search_entries AS e WHERE e.subnet_id = NEW.id;
END;

CREATE TRIGGER tr_search_subnets_update
AFTER UPDATE OF name, description, parent_id ON subnets
BEGIN
    UPDATE search_words
       SET title = NEW.name,
           body = NEW.description,
           subnets = (SELECT subnets FROM search_subnet_fields WHERE id = NEW.id)
     WHERE rowid = (SELECT id FROM search_entries WHERE subnet_id = NEW.id);
    UPDATE search_parts SET title = NEW.name, body = NEW.description
     WHERE rowid = (SELECT id FROM search_entries WHERE subnet_id = NEW.id);
    -- The name also stands in every member and child subnet.
    UPDATE search_words
       SET subnets = (SELECT f.subnets FROM search_entries AS e
                        JOIN search_document_fields AS f ON f.id = e.document_id
                       WHERE e.id = search_words.rowid)
     WHERE OLD.name IS NOT NEW.name
       AND rowid IN (SELECT e.id FROM search_entries AS e
                       JOIN subnet_memberships AS m ON m.document_id = e.document_id
                      WHERE m.subnet_id = NEW.id);
    UPDATE search_words
       SET subnets = (SELECT f.subnets FROM search_entries AS e
                        JOIN search_media_fields AS f ON f.id = e.media_id
                       WHERE e.id = search_words.rowid)
     WHERE OLD.name IS NOT NEW.name
       AND rowid IN (SELECT e.id FROM search_entries AS e
                       JOIN subnet_memberships AS m ON m.media_id = e.media_id
                      WHERE m.subnet_id = NEW.id);
    UPDATE search_words SET subnets = NEW.name
     WHERE OLD.name IS NOT NEW.name
       AND rowid IN (SELECT e.id FROM search_entries AS e
                       JOIN subnets AS c ON c.id = e.subnet_id
                      WHERE c.parent_id = NEW.id);
END;

CREATE TRIGGER tr_search_memberships_insert
AFTER INSERT ON subnet_memberships
BEGIN
    UPDATE search_words SET subnets = (SELECT subnets FROM search_document_fields WHERE id = NEW.document_id)
     WHERE NEW.document_id IS NOT NULL
       AND rowid = (SELECT id FROM search_entries WHERE document_id = NEW.document_id);
    UPDATE search_words SET subnets = (SELECT subnets FROM search_media_fields WHERE id = NEW.media_id)
     WHERE NEW.media_id IS NOT NULL
       AND rowid = (SELECT id FROM search_entries WHERE media_id = NEW.media_id);
END;

CREATE TRIGGER tr_search_memberships_delete
AFTER DELETE ON subnet_memberships
BEGIN
    UPDATE search_words SET subnets = (SELECT subnets FROM search_document_fields WHERE id = OLD.document_id)
     WHERE OLD.document_id IS NOT NULL
       AND rowid = (SELECT id FROM search_entries WHERE document_id = OLD.document_id);
    UPDATE search_words SET subnets = (SELECT subnets FROM search_media_fields WHERE id = OLD.media_id)
     WHERE OLD.media_id IS NOT NULL
       AND rowid = (SELECT id FROM search_entries WHERE media_id = OLD.media_id);
END;

CREATE TRIGGER tr_search_memberships_update
AFTER UPDATE OF subnet_id, document_id, media_id ON subnet_memberships
BEGIN
    UPDATE search_words SET subnets = (SELECT subnets FROM search_document_fields WHERE id = OLD.document_id)
     WHERE OLD.document_id IS NOT NULL
       AND rowid = (SELECT id FROM search_entries WHERE document_id = OLD.document_id);
    UPDATE search_words SET subnets = (SELECT subnets FROM search_media_fields WHERE id = OLD.media_id)
     WHERE OLD.media_id IS NOT NULL
       AND rowid = (SELECT id FROM search_entries WHERE media_id = OLD.media_id);
    UPDATE search_words SET subnets = (SELECT subnets FROM search_document_fields WHERE id = NEW.document_id)
     WHERE NEW.document_id IS NOT NULL
       AND rowid = (SELECT id FROM search_entries WHERE document_id = NEW.document_id);
    UPDATE search_words SET subnets = (SELECT subnets FROM search_media_fields WHERE id = NEW.media_id)
     WHERE NEW.media_id IS NOT NULL
       AND rowid = (SELECT id FROM search_entries WHERE media_id = NEW.media_id);
END;
