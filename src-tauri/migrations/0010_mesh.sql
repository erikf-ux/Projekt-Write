-- Phase 11: the Mesh (section 15, ADR 0022). Nodes, confirmed connections,
-- suggestions, and the layout of each view. Derived connections (links,
-- media in documents, subnet memberships, tags) are not stored here: they
-- are computed from their own tables (section 15).

-- One node per document, medium and subnet; the id is the element's id.
CREATE TABLE mesh_nodes (
    id          TEXT NOT NULL PRIMARY KEY,
    kind        TEXT NOT NULL
                CHECK (kind IN ('document', 'media', 'subnet')),
    document_id TEXT UNIQUE REFERENCES documents (id) ON DELETE CASCADE,
    media_id    TEXT UNIQUE REFERENCES media (id) ON DELETE CASCADE,
    subnet_id   TEXT UNIQUE REFERENCES subnets (id) ON DELETE CASCADE,
    CHECK (
        (kind = 'document' AND document_id = id AND media_id IS NULL AND subnet_id IS NULL)
        OR (kind = 'media' AND media_id = id AND document_id IS NULL AND subnet_id IS NULL)
        OR (kind = 'subnet' AND subnet_id = id AND document_id IS NULL AND media_id IS NULL)
    )
) STRICT, WITHOUT ROWID;

INSERT INTO mesh_nodes (id, kind, document_id) SELECT id, 'document', id FROM documents;
INSERT INTO mesh_nodes (id, kind, media_id) SELECT id, 'media', id FROM media;
INSERT INTO mesh_nodes (id, kind, subnet_id) SELECT id, 'subnet', id FROM subnets;

CREATE TRIGGER tr_mesh_node_document AFTER INSERT ON documents
BEGIN
    INSERT INTO mesh_nodes (id, kind, document_id) VALUES (NEW.id, 'document', NEW.id);
END;

CREATE TRIGGER tr_mesh_node_media AFTER INSERT ON media
BEGIN
    INSERT INTO mesh_nodes (id, kind, media_id) VALUES (NEW.id, 'media', NEW.id);
END;

CREATE TRIGGER tr_mesh_node_subnet AFTER INSERT ON subnets
BEGIN
    INSERT INTO mesh_nodes (id, kind, subnet_id) VALUES (NEW.id, 'subnet', NEW.id);
END;

-- Confirmed connections only: drawn by the user (manual) or an accepted
-- suggestion (similarity). Undirected: the ends are stored sorted, so a
-- pair has one row per kind and a node cannot connect to itself.
CREATE TABLE mesh_connections (
    id         TEXT NOT NULL PRIMARY KEY
               CHECK (id GLOB '????????-????-????-????-????????????'
                      AND id NOT GLOB '*[^0-9a-f-]*'),
    node_a     TEXT NOT NULL REFERENCES mesh_nodes (id) ON DELETE CASCADE,
    node_b     TEXT NOT NULL REFERENCES mesh_nodes (id) ON DELETE CASCADE,
    kind       TEXT NOT NULL
               CHECK (kind IN ('manual', 'similarity')),
    note       TEXT
               CHECK (note IS NULL OR length(note) <= 2000),
    created_at TEXT NOT NULL
               CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    CHECK (node_a < node_b),
    UNIQUE (node_a, node_b, kind)
) STRICT;

CREATE INDEX ix_mesh_connections_b ON mesh_connections (node_b);

-- Suggestions of the grouping analysis, apart from confirmed connections.
-- One row per pair whatever its status: a rejected pair keeps its row and
-- so never comes back; "Später" leaves it open.
CREATE TABLE mesh_suggestions (
    id         TEXT NOT NULL PRIMARY KEY
               CHECK (id GLOB '????????-????-????-????-????????????'
                      AND id NOT GLOB '*[^0-9a-f-]*'),
    node_a     TEXT NOT NULL REFERENCES mesh_nodes (id) ON DELETE CASCADE,
    node_b     TEXT NOT NULL REFERENCES mesh_nodes (id) ON DELETE CASCADE,
    score      REAL NOT NULL
               CHECK (score >= 0 AND score <= 1),
    reason     TEXT NOT NULL
               CHECK (json_valid(reason)),
    status     TEXT NOT NULL DEFAULT 'open'
               CHECK (status IN ('open', 'accepted', 'rejected')),
    created_at TEXT NOT NULL
               CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    decided_at TEXT
               CHECK (decided_at IS NULL
                      OR decided_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    CHECK (node_a < node_b),
    CHECK ((status = 'open') = (decided_at IS NULL)),
    UNIQUE (node_a, node_b)
) STRICT;

CREATE INDEX ix_mesh_suggestions_b ON mesh_suggestions (node_b);
CREATE INDEX ix_mesh_suggestions_open ON mesh_suggestions (score DESC) WHERE status = 'open';

-- The layout of one view: the Global View (scope ''), the Subnet View of a
-- subnet, the Focus View of a node.
CREATE TABLE mesh_layouts (
    id         TEXT NOT NULL PRIMARY KEY
               CHECK (id GLOB '????????-????-????-????-????????????'
                      AND id NOT GLOB '*[^0-9a-f-]*'),
    view       TEXT NOT NULL
               CHECK (view IN ('global', 'subnet', 'focus')),
    scope_id   TEXT NOT NULL DEFAULT ''
               CHECK ((view = 'global') = (scope_id = '')),
    algorithm  TEXT NOT NULL DEFAULT ''
               CHECK (length(algorithm) <= 100),
    options    TEXT NOT NULL DEFAULT '{}'
               CHECK (json_valid(options)),
    updated_at TEXT NOT NULL
               CHECK (updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    UNIQUE (view, scope_id)
) STRICT;

-- Node positions of a layout. A node the user moved is pinned: the
-- automatic layout leaves it alone unless asked to.
CREATE TABLE mesh_positions (
    layout_id TEXT NOT NULL REFERENCES mesh_layouts (id) ON DELETE CASCADE,
    node_id   TEXT NOT NULL REFERENCES mesh_nodes (id) ON DELETE CASCADE,
    x         REAL NOT NULL
              CHECK (x BETWEEN -10000000 AND 10000000),
    y         REAL NOT NULL
              CHECK (y BETWEEN -10000000 AND 10000000),
    pinned    INTEGER NOT NULL DEFAULT 0
              CHECK (pinned IN (0, 1)),
    PRIMARY KEY (layout_id, node_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX ix_mesh_positions_node ON mesh_positions (node_id);
