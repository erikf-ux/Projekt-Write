-- Migration 0008: media (master prompt, sections 9, 17 and 18; Phase 8,
-- ADR 0019).
--
-- Shipped migrations are never changed. New schema changes get a new file.
--
-- One row per medium of the library. The file lives at
-- media/images/<id>.<ext>, media/pdf/<id>.pdf or media/attachments/<id>.<ext>;
-- the path is derived from id, kind and extension and never stored.
-- mime_type and extension come from the check of the content (positive
-- list, section 20). sha256 finds duplicates but is not unique: "trotzdem
-- importieren" is allowed (section 9).
CREATE TABLE media (
    id             TEXT NOT NULL PRIMARY KEY
                   CHECK (id GLOB '????????-????-????-????-????????????'
                          AND id NOT GLOB '*[^0-9a-f-]*'),
    kind           TEXT NOT NULL
                   CHECK (kind IN ('image', 'pdf', 'attachment')),
    mime_type      TEXT NOT NULL
                   CHECK (mime_type IN (
                       'image/png', 'image/jpeg', 'image/gif', 'image/webp', 'image/svg+xml',
                       'application/pdf',
                       'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
                       'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
                       'application/vnd.openxmlformats-officedocument.presentationml.presentation')),
    extension      TEXT NOT NULL
                   CHECK (extension IN ('png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'pdf',
                                        'docx', 'xlsx', 'pptx')),
    title          TEXT NOT NULL
                   CHECK (length(title) BETWEEN 1 AND 300),
    description    TEXT NOT NULL DEFAULT ''
                   CHECK (length(description) <= 2000),
    original_name  TEXT NOT NULL
                   CHECK (length(original_name) BETWEEN 1 AND 255),
    import_folder  TEXT
                   CHECK (import_folder IS NULL OR length(import_folder) BETWEEN 1 AND 4096),
    size_bytes     INTEGER NOT NULL
                   CHECK (size_bytes >= 0),
    sha256         TEXT NOT NULL
                   CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
    width          INTEGER
                   CHECK (width IS NULL OR width > 0),
    height         INTEGER
                   CHECK (height IS NULL OR height > 0),
    page_count     INTEGER
                   CHECK (page_count IS NULL OR page_count > 0),
    created_at     TEXT NOT NULL
                   CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    updated_at     TEXT NOT NULL
                   CHECK (updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    favorited_at   TEXT
                   CHECK (favorited_at IS NULL
                          OR favorited_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    last_opened_at TEXT
                   CHECK (last_opened_at IS NULL
                          OR last_opened_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    archived_at    TEXT
                   CHECK (archived_at IS NULL
                          OR archived_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    deleted_at     TEXT
                   CHECK (deleted_at IS NULL
                          OR deleted_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    -- Kind, type and extension belong together.
    CHECK ((kind = 'image' AND (
               (mime_type = 'image/png' AND extension = 'png')
               OR (mime_type = 'image/jpeg' AND extension IN ('jpg', 'jpeg'))
               OR (mime_type = 'image/gif' AND extension = 'gif')
               OR (mime_type = 'image/webp' AND extension = 'webp')
               OR (mime_type = 'image/svg+xml' AND extension = 'svg')))
           OR (kind = 'pdf' AND mime_type = 'application/pdf' AND extension = 'pdf')
           OR (kind = 'attachment' AND (
               (mime_type = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'
                AND extension = 'docx')
               OR (mime_type = 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'
                   AND extension = 'xlsx')
               OR (mime_type = 'application/vnd.openxmlformats-officedocument.presentationml.presentation'
                   AND extension = 'pptx'))))
) STRICT;

CREATE INDEX ix_media_sha256 ON media (sha256);
CREATE INDEX ix_media_recent ON media (last_opened_at)
    WHERE last_opened_at IS NOT NULL AND deleted_at IS NULL;

-- Media a document uses, derived from its content on every save (section 15:
-- "abgeleitet aus document_links und document_media"). page 0 is the whole
-- medium, otherwise the page of a PDF reference.
CREATE TABLE document_media (
    document_id TEXT NOT NULL REFERENCES documents (id) ON DELETE CASCADE,
    media_id    TEXT NOT NULL REFERENCES media (id) ON DELETE CASCADE,
    page        INTEGER NOT NULL DEFAULT 0
                CHECK (page >= 0),
    PRIMARY KEY (document_id, media_id, page)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_document_media_media ON document_media (media_id, document_id);

CREATE TABLE media_tags (
    media_id   TEXT NOT NULL REFERENCES media (id) ON DELETE CASCADE,
    tag_id     TEXT NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL
               CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    PRIMARY KEY (media_id, tag_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_media_tags_tag ON media_tags (tag_id, media_id);

-- The tested pattern of section 18, now with media: a row belongs to exactly
-- one element. Rebuilt in place (new table, copy, drop, rename); the
-- memberships of migration 0007 keep their created_at.
CREATE TABLE subnet_memberships_v8 (
    subnet_id   TEXT NOT NULL REFERENCES subnets (id) ON DELETE CASCADE,
    document_id TEXT REFERENCES documents (id) ON DELETE CASCADE,
    media_id    TEXT REFERENCES media (id) ON DELETE CASCADE,
    is_primary  INTEGER NOT NULL DEFAULT 0
                CHECK (is_primary IN (0, 1)),
    created_at  TEXT NOT NULL
                CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    CHECK ((document_id IS NULL) <> (media_id IS NULL))
) STRICT;

INSERT INTO subnet_memberships_v8 (subnet_id, document_id, media_id, is_primary, created_at)
SELECT subnet_id, document_id, NULL, is_primary, created_at FROM subnet_memberships;

DROP TABLE subnet_memberships;
ALTER TABLE subnet_memberships_v8 RENAME TO subnet_memberships;

CREATE UNIQUE INDEX uq_member_doc ON subnet_memberships (subnet_id, document_id)
    WHERE document_id IS NOT NULL;
CREATE UNIQUE INDEX uq_member_media ON subnet_memberships (subnet_id, media_id)
    WHERE media_id IS NOT NULL;
CREATE UNIQUE INDEX uq_primary_doc ON subnet_memberships (document_id)
    WHERE is_primary = 1 AND document_id IS NOT NULL;
CREATE UNIQUE INDEX uq_primary_media ON subnet_memberships (media_id)
    WHERE is_primary = 1 AND media_id IS NOT NULL;
CREATE INDEX ix_memberships_document ON subnet_memberships (document_id, created_at)
    WHERE document_id IS NOT NULL;
CREATE INDEX ix_memberships_media ON subnet_memberships (media_id, created_at)
    WHERE media_id IS NOT NULL;
