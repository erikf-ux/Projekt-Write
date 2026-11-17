-- Language check (Phase 12, section 14, ADR 0025): the user's own
-- dictionaries with their words and targets, rejected autocorrections and
-- the language profile of a document. The language dictionaries themselves
-- are files (ADR 0024), not rows.

-- An own dictionary. `for_all` assigns it to every document of the library
-- ("Gesamte Anwendung"); documents and subnets are targets below. Several
-- targets at once are allowed (section 14).
CREATE TABLE user_dictionaries (
    id         TEXT NOT NULL PRIMARY KEY
               CHECK (id GLOB '????????-????-????-????-????????????'
                      AND id NOT GLOB '*[^0-9a-f-]*'),
    name       TEXT NOT NULL
               CHECK (length(name) BETWEEN 1 AND 100),
    name_key   TEXT NOT NULL
               CHECK (length(name_key) BETWEEN 1 AND 400),
    for_all    INTEGER NOT NULL DEFAULT 0
               CHECK (for_all IN (0, 1)),
    created_at TEXT NOT NULL
               CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    updated_at TEXT NOT NULL
               CHECK (updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z')
) STRICT;

CREATE UNIQUE INDEX uq_user_dictionaries_name ON user_dictionaries (name_key);

-- A word of an own dictionary: a technical term, a name, an abbreviation
-- or any other word. One word per entry, no white space.
CREATE TABLE user_dictionary_words (
    dictionary_id TEXT NOT NULL REFERENCES user_dictionaries (id) ON DELETE CASCADE,
    word          TEXT NOT NULL
                  CHECK (length(word) BETWEEN 1 AND 100
                         AND word NOT GLOB '*[ ' || char(9) || char(10) || char(13) || ']*'),
    category      TEXT NOT NULL DEFAULT 'word'
                  CHECK (category IN ('word', 'term', 'name', 'abbreviation')),
    created_at    TEXT NOT NULL
                  CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    PRIMARY KEY (dictionary_id, word)
) STRICT, WITHOUT ROWID;

-- A document or a subnet an own dictionary is assigned to. A subnet's
-- dictionary also applies to the documents of its child subnets.
CREATE TABLE user_dictionary_targets (
    dictionary_id TEXT NOT NULL REFERENCES user_dictionaries (id) ON DELETE CASCADE,
    document_id   TEXT REFERENCES documents (id) ON DELETE CASCADE,
    subnet_id     TEXT REFERENCES subnets (id) ON DELETE CASCADE,
    CHECK ((document_id IS NULL) <> (subnet_id IS NULL))
) STRICT;

CREATE UNIQUE INDEX uq_dictionary_targets_document
    ON user_dictionary_targets (dictionary_id, document_id) WHERE document_id IS NOT NULL;
CREATE UNIQUE INDEX uq_dictionary_targets_subnet
    ON user_dictionary_targets (dictionary_id, subnet_id) WHERE subnet_id IS NOT NULL;
CREATE INDEX ix_dictionary_targets_document ON user_dictionary_targets (document_id);
CREATE INDEX ix_dictionary_targets_subnet ON user_dictionary_targets (subnet_id);

-- A correction the user took back with Ctrl+Z: the autocorrect never makes
-- it again (condition 7).
CREATE TABLE autocorrect_rejections (
    word       TEXT NOT NULL CHECK (length(word) BETWEEN 1 AND 100),
    correction TEXT NOT NULL CHECK (length(correction) BETWEEN 1 AND 100),
    created_at TEXT NOT NULL
               CHECK (created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z'),
    PRIMARY KEY (word, correction)
) STRICT, WITHOUT ROWID;

-- The language profile of a document: its languages (the first is the main
-- language; an empty list means the default of the app) and whether the
-- autocorrect works in it. No row: the defaults.
CREATE TABLE document_language (
    document_id TEXT NOT NULL PRIMARY KEY REFERENCES documents (id) ON DELETE CASCADE,
    languages   TEXT NOT NULL DEFAULT '[]'
                CHECK (json_valid(languages) AND json_type(languages) = 'array'
                       AND json_array_length(languages) <= 5),
    autocorrect INTEGER NOT NULL DEFAULT 1
                CHECK (autocorrect IN (0, 1)),
    updated_at  TEXT NOT NULL
                CHECK (updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z')
) STRICT, WITHOUT ROWID;
