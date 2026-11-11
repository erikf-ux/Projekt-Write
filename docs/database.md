# Datenbank – Project Write

Stand: Phase 9 (11. November 2026). Diese Datei enthält das **vollständige geplante Schema** (Abschnitt 18) und kennzeichnet, was bereits per Migration existiert. Tabellen entstehen in der Phase, die sie braucht; Spalten, die eine spätere Phase braucht, kommen mit einer eigenen Migration hinzu.

## 1. Konventionen (Abschnitt 18)

- SQLite, mit der App gebaut (`libsqlite3-sys` 0.37.0 über SQLx 0.9.0, Feature `sqlite-bundled`). Kein zweites Crate auf `libsqlite3-sys`.
- Alle Tabellen `STRICT`. Ausnahme: die interne Tabelle `_sqlx_migrations` von SQLx.
- IDs als UUID-Text in Kleinbuchstaben (`CHECK` auf Länge, Bindestriche und Hex-Zeichen). Zeiten in UTC nach ISO 8601, immer mit Millisekunden: `2026-11-03T10:00:00.000Z` (feste Breite, die lexikalische Ordnung entspricht der zeitlichen).
- Weiches Löschen: `deleted_at` (Papierkorb) und `archived_at` (Archiv) statt `DELETE`. Endgültig gelöscht wird nur beim Leeren des Papierkorbs (Phase 6); dann greifen die `ON DELETE CASCADE` der Zuordnungstabellen.
- Fremdschlüssel für jede Verbindung, `PRAGMA foreign_keys=ON` auf jeder Verbindung.
- Dateipfade werden **nie gespeichert**, sondern aus ID und Typ abgeleitet (`documents/markdown/<id>.md`). Eine manipulierte Datenbank kann so keinen Pfad außerhalb der Bibliothek ins Spiel bringen.
- SQL steht nur in `src-tauri/src/database/` (Repositories `documents.rs`, `settings.rs`). SQLx 0.9 akzeptiert nur statisches SQL; dynamisches SQL braucht `AssertSqlSafe` (nur in Tests).
- Werte von Einstellungen sind JSON (`json_valid`), damit jede Einstellung ihren eigenen Typ behält.

## 2. Verbindungen

|                | Bibliotheksdatenbank                                                                                 | In-Memory (Systemprüfung ohne Bibliothek, Tests) |
| -------------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| Ort            | `<Bibliothek>/database/projectwrite.db`                                                              | –                                                |
| Schreiben      | 1 Verbindung (`max_connections = 1`); alle Schreiber einer Bibliothek zusätzlich hinter einer Sperre | 1 Verbindung für alles, nie erneuert             |
| Lesen          | Pool mit 4 Verbindungen, `read_only`                                                                 | dieselbe Verbindung                              |
| `journal_mode` | `WAL` auf lokalen Datenträgern, `DELETE` in Sync-Ordnern und auf Netzlaufwerken (ADR 0006)           | `MEMORY`                                         |
| `synchronous`  | `FULL`                                                                                               | `FULL`                                           |
| `foreign_keys` | `ON`                                                                                                 | `ON`                                             |
| `busy_timeout` | 5000 ms                                                                                              | 5000 ms                                          |

Der Modus wird bei **jedem** Öffnen aus dem Speicherort bestimmt. Eine Bibliothek, die der Nutzer von Hand in einen Sync-Ordner verschiebt, wechselt beim nächsten Öffnen von WAL in den Rollback-Modus (Test `a_library_moved_into_a_sync_folder_switches_from_wal_to_rollback`).

## 3. Migrationen

- Ort: `src-tauri/migrations/NNNN_beschreibung.sql`, eingebettet zur Übersetzungszeit (`sqlx::migrate!`).
- Nur vorwärts, jede Migration in einer eigenen Transaktion (Test: keine Migration schaltet die Transaktion ab; eine fehlerhafte Migration hinterlässt nichts).
- **Ausgelieferte Migrationen werden nie geändert.** SQLx prüft eine SHA-384-Prüfsumme je Migration; zusätzlich sind die Prüfsummen in `tests/migrations.rs` festgeschrieben.
- Vor jeder Änderung: Ist die Datenbank neuer als die App, meldet das Öffnen `SCHEMA_TOO_NEW` und ändert nichts – auch keine Sicherung.
- **Sicherung vorab:** Stehen Migrationen an und hat die Datenbank schon ein Schema (Version ≥ 1), kopiert die App sie mit `VACUUM INTO` nach `backups/pre-migration/projectwrite-schema-<alt>-<Zeit>.db`, prüft die Kopie mit `PRAGMA integrity_check` und migriert erst dann. Scheitert die Sicherung, wird nicht migriert (`LIBRARY_BACKUP_FAILED`). Die fünf neuesten Kopien bleiben, ältere entfernt die App (nur eigene Dateien nach diesem Namensmuster). Ab Phase 10 ersetzt ein vollständiges `.pwbackup` diese Kopie (Abschnitt 21).
- **Tests ab jeder früheren Version:** `tests/migrations.rs` legt für jede Version 0 … n−1 eine Datenbank an, füllt die Tabellen dieser Version mit Testdaten, migriert auf die aktuelle Version und prüft: Schema identisch zu einer frisch migrierten Datenbank, Daten erhalten, `integrity_check` = ok, `foreign_key_check` leer. Jede neue Migration bringt eine Testzeile für ihre Tabellen mit.

| Version | Datei                           | Inhalt                                                                                                        | Phase |
| ------- | ------------------------------- | ------------------------------------------------------------------------------------------------------------- | ----- |
| 1       | `0001_application_settings.sql` | Tabelle `application_settings`                                                                                | 1     |
| 2       | `0002_documents.sql`            | Tabelle `documents`                                                                                           | 2     |
| 3       | `0003_document_import.sql`      | `documents.original_name`                                                                                     | 3     |
| 4       | `0004_favorites_and_recent.sql` | `documents.favorited_at`, `documents.last_opened_at`, Index `ix_documents_recent`                             | 5     |
| 5       | `0005_document_links.sql`       | Tabelle `document_links`                                                                                      | 5     |
| 6       | `0006_document_history.sql`     | Tabelle `document_history`                                                                                    | 6     |
| 7       | `0007_subnets_and_tags.sql`     | Tabellen `subnets`, `subnet_memberships`, `tags`, `document_tags`, Zyklus-Trigger                             | 7     |
| 8       | `0008_media.sql`                | Tabellen `media`, `media_tags`, `document_media`; Neuaufbau von `subnet_memberships` mit `media_id`           | 8     |
| 9       | `0009_search.sql`               | Tabelle `search_entries`, FTS5-Tabellen `search_words` und `search_parts`, drei Ansichten, Trigger (ADR 0020) | 9     |

Geplante Migrationen (Nummern vergibt die jeweilige Phase):

| Phase | Inhalt                                                                                                                                     |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| 8     | `documents.import_folder` (mit dem Ordnerimport, ADR 0015)                                                                                 |
| 11    | `mesh_nodes` (mit Triggern und Nachtrag für bestehende Elemente), `mesh_connections`, `mesh_suggestions`, `mesh_layouts`, `mesh_positions` |
| 12    | `dictionaries`, `dictionary_words`, `document_dictionaries`, `subnet_dictionaries`, `language_profiles`, `autocorrect_rejections`          |

## 4. Gesamtschema

Legende: **vorhanden** = per Migration angelegt; **geplant (Phase n)** = Entwurf, wird in Phase n angelegt und bei Bedarf dort präzisiert (Änderungen am Plan werden hier nachgetragen, nicht still geändert).

### Übersicht

| Tabelle                        | Zweck                                            | Stand              |
| ------------------------------ | ------------------------------------------------ | ------------------ |
| `application_settings`         | Bibliotheksweite Einstellungen                   | vorhanden (v1)     |
| `documents`                    | Dokumente (TXT, MD, `.pwrite`)                   | vorhanden (v2–v4)  |
| `document_links`               | Interne Links zwischen Dokumenten (Rückverweise) | vorhanden (v5)     |
| `document_history`             | Versionen eines Dokuments                        | vorhanden (v6)     |
| `subnets`                      | Hierarchische Gruppen                            | vorhanden (v7)     |
| `subnet_memberships`           | Zuordnung Element → Subnetz                      | vorhanden (v7, v8) |
| `tags`                         | Stichwörter                                      | vorhanden (v7)     |
| `document_tags`                | Tags eines Dokuments                             | vorhanden (v7)     |
| `media`                        | Bilder, PDF, Anhänge                             | vorhanden (v8)     |
| `media_tags`                   | Tags eines Mediums                               | vorhanden (v8)     |
| `document_media`               | Medien, die ein Dokument einbindet oder verweist | vorhanden (v8)     |
| `search_entries`               | Zuordnung FTS-Zeile → Element                    | vorhanden (v9)     |
| `search_words`, `search_parts` | FTS5-Index (Abschnitt 19)                        | vorhanden (v9)     |
| `mesh_nodes`                   | Knoten des Mesh (ein Eintrag je Element/Subnetz) | geplant (Phase 11) |
| `mesh_connections`             | Bestätigte Verbindungen                          | geplant (Phase 11) |
| `mesh_suggestions`             | Vorschläge mit Wert, Begründung und Status       | geplant (Phase 11) |
| `mesh_layouts`                 | Layout je Ansicht                                | geplant (Phase 11) |
| `mesh_positions`               | Knotenpositionen je Layout                       | geplant (Phase 11) |
| `dictionaries`                 | Eigene und Hunspell-Wörterbücher                 | geplant (Phase 12) |
| `dictionary_words`             | Einträge eigener Wörterbücher                    | geplant (Phase 12) |
| `document_dictionaries`        | Wörterbücher eines Dokuments                     | geplant (Phase 12) |
| `subnet_dictionaries`          | Wörterbücher eines Subnetzes                     | geplant (Phase 12) |
| `language_profiles`            | Sprachprofil je Dokument                         | geplant (Phase 12) |
| `autocorrect_rejections`       | Abgelehnte Autokorrekturen                       | geplant (Phase 12) |

Abgeleitete Verbindungen (Referenz, Tag, Enthalten-in, Abschnitt 15) werden **nicht** gespeichert, sondern aus `document_links`, `document_media`, `document_tags`/`media_tags` und `subnet_memberships` berechnet.

### `application_settings` – vorhanden (Version 1)

| Spalte       | Typ                   | Regeln                        |
| ------------ | --------------------- | ----------------------------- |
| `key`        | TEXT, Primärschlüssel | 1–128 Zeichen, `bereich.name` |
| `value`      | TEXT                  | gültiges JSON (`json_valid`)  |
| `updated_at` | TEXT                  | ISO 8601 UTC                  |

`STRICT, WITHOUT ROWID`. Jeder Schlüssel ist in Rust ein Typ mit Standardwert und Prüfung (`services/settings/library.rs`). Eine Zeile, deren Wert nicht (mehr) passt, ergibt den Standardwert und bleibt unverändert; Schlüssel neuerer Versionen werden nie angefasst.

| Schlüssel                   | Wert                                                     | Phase |
| --------------------------- | -------------------------------------------------------- | ----- |
| `storage.acceptedLocation`  | bestätigter Sync-Ordner/Netzlaufwerk (`StorageLocation`) | 2     |
| `trash.retentionDays`       | Aufbewahrung im Papierkorb, Standard 30                  | 6     |
| `history.*`                 | Aufbewahrungsregeln der Versionen (Abschnitt 13)         | 6     |
| `statistics.wordsPerMinute` | Lesezeit, Standard 200                                   | 4     |
| `backup.*`                  | Ziel, Automatik, Anzahl (Abschnitt 21)                   | 10    |
| `suggestions.threshold`     | Schwellenwert der Vorschläge, Standard 0,35              | 11    |

Appweite Einstellungen (Bibliothekspfad, später Fensterzustand, Sprache, Tastenkürzel) liegen **nicht** hier, sondern in `settings.json` im Konfigurationsordner des Systems (ADR 0009).

### `documents` – vorhanden (Versionen 2 bis 4)

| Spalte           | Typ                   | Regeln                                                               | Phase |
| ---------------- | --------------------- | -------------------------------------------------------------------- | ----- |
| `id`             | TEXT, Primärschlüssel | UUID in Kleinbuchstaben                                              | 2     |
| `title`          | TEXT                  | 1–300 Zeichen; Rust: getrimmt, ohne Steuerzeichen                    | 2     |
| `format`         | TEXT                  | `txt`, `md` oder `pwrite`                                            | 2     |
| `size_bytes`     | INTEGER               | ≥ 0                                                                  | 2     |
| `content_sha256` | TEXT                  | 64 Hex-Zeichen                                                       | 2     |
| `created_at`     | TEXT                  | ISO 8601 UTC                                                         | 2     |
| `updated_at`     | TEXT                  | ISO 8601 UTC                                                         | 2     |
| `archived_at`    | TEXT NULL             | ISO 8601 UTC                                                         | 2     |
| `deleted_at`     | TEXT NULL             | ISO 8601 UTC (Papierkorb)                                            | 2     |
| `favorited_at`   | TEXT NULL             | Favorit seit                                                         | 5     |
| `last_opened_at` | TEXT NULL             | für „Zuletzt verwendet“                                              | 5     |
| `original_name`  | TEXT NULL             | Dateiname beim Import (Abschnitt 9), 1–255 Zeichen                   | 3     |
| `import_folder`  | TEXT NULL             | Importordner als Anzeigetext (Metadatum); kommt mit dem Ordnerimport | 8     |

`original_name` kam mit dem Drag-&-Drop-Import in Phase 3 statt wie geplant in Phase 5 (Migration 0003). Abgelegte Dateien kommen ohne Ordner an (der Webview nennt nur den Namen); `import_folder` entsteht deshalb erst mit dem Ordnerimport, der laut Abschnitt 25 zu Phase 8 gehört (ADR 0015, Punkt 1). `favorited_at` und `last_opened_at` kamen mit Migration 0004 (Phase 5); beide sind ISO 8601 UTC wie die übrigen Zeitspalten, `last_opened_at` setzt das Öffnen in einem Tab, `favorited_at` ändert `updated_at` nicht. Bibliotheksdokumente in TXT und MD sind UTF-8 mit LF-Zeilenenden; der Import erkennt UTF-8 (mit und ohne BOM), UTF-16 und Windows-1252 und wandelt CRLF und CR um (ADR 0015, Punkt 16).

Indizes: `ix_documents_active_updated_at (updated_at) WHERE deleted_at IS NULL`, `ix_documents_recent (last_opened_at) WHERE last_opened_at IS NOT NULL AND deleted_at IS NULL`. Datei: `documents/text/<id>.txt`, `documents/markdown/<id>.md`, `documents/richtext/<id>.pwrite` (Hülle: `docs/pwrite-format.md`).

**Umwandeln** (ADR 0015, Punkt 13): Die ID bleibt. Rust schreibt zuerst die Datei des neuen Formats, ändert dann in einer Transaktion `format`, `size_bytes`, `content_sha256`, `updated_at` und die Links und löscht zuletzt die alte Datei. Ein Absturz dazwischen hinterlässt höchstens eine verwaiste Datei, die die Prüfung beim Öffnen meldet – nie eine Zeile ohne Datei.

Bewusst **ohne** `updated_at >= created_at`: Eine Zeitkorrektur der Systemuhr darf ein Speichern nie scheitern lassen. Sprache und Autokorrektur je Dokument stehen in `language_profiles`.

Schreibreihenfolge und Absturzverhalten: Abschnitt 6 und ADR 0007.

### `document_links` – vorhanden (Version 5)

| Spalte      | Typ  | Regeln                                        |
| ----------- | ---- | --------------------------------------------- |
| `source_id` | TEXT | → `documents(id)` ON DELETE CASCADE           |
| `target_id` | TEXT | UUID in Kleinbuchstaben; ungleich `source_id` |

`STRICT, WITHOUT ROWID`, Primärschlüssel `(source_id, target_id)`, Index `ix_document_links_target (target_id, source_id)` für die Rückverweise (Seitenleiste „Verbindungen“). Rust leitet die Zeilen bei jedem Speichern, Anlegen, Import, Kopieren und Umwandeln aus dem Inhalt ab und schreibt sie in derselben Transaktion wie die Metadaten: in Markdown jedes `pwrite://<id>`, in `.pwrite` die Link-Markierungen mit `pwrite://<id>`; TXT hat keine Links.

Abweichungen vom Entwurf: `target_id` hat **keinen** Fremdschlüssel, weil ein Link auf ein (noch) fehlendes Dokument ein Link bleibt (zum Beispiel aus einer anderen Bibliothek importiert); die Spalte `count` entfällt, weil die Verbindungen nur zählen, ob ein Dokument verweist. Rückverweise aus dem Papierkorb zeigt die Abfrage nicht.

### `document_history` – vorhanden (Version 6)

| Spalte           | Typ                   | Regeln                                                                                     |
| ---------------- | --------------------- | ------------------------------------------------------------------------------------------ |
| `id`             | TEXT, Primärschlüssel | UUID (Kleinbuchstaben, `CHECK`)                                                            |
| `document_id`    | TEXT                  | → `documents(id)` ON DELETE CASCADE                                                        |
| `kind`           | TEXT                  | `auto`, `close`, `beforeConversion`, `beforeRestore`, `named` (`CHECK`)                    |
| `name`           | TEXT NULL             | 1–200 Zeichen; gesetzt genau bei `named` (`CHECK ((kind = 'named') = (name IS NOT NULL))`) |
| `format`         | TEXT                  | `txt`, `md`, `pwrite`: das Format zum Zeitpunkt der Version                                |
| `content`        | BLOB                  | Inhalt der Datei, komprimiert                                                              |
| `compression`    | TEXT                  | `deflate` (`flate2`, ADR 0016); weitere Verfahren nur mit neuer Migration                  |
| `size_bytes`     | INTEGER               | unkomprimierte Größe, ≥ 0                                                                  |
| `content_sha256` | TEXT                  | SHA-256 des unkomprimierten Inhalts (64 Hex-Zeichen); beim Lesen geprüft                   |
| `created_at`     | TEXT                  | ISO 8601 UTC (`CHECK` auf das Muster)                                                      |

`STRICT`, Index `ix_document_history_document (document_id, created_at)`.

Regeln in Rust (`services/history.rs`, ADR 0016, Entscheidung 4–7):

- **Entstehen:** `auto` beim Speichern, wenn die neueste Version älter als 10 Minuten ist; `close` beim Schließen eines Tabs und der App; `beforeConversion` vor jedem Umwandeln (scheitert sie, wird nicht umgewandelt); `beforeRestore` vor jeder Wiederherstellung (Version, Entwurf); `named` mit „Version speichern …“.
- **Keine Doppel:** Gleicht der Inhalt (Prüfsumme) der neuesten Version, entsteht keine Zeile; eine benannte Version gibt dann der neuesten ihren Namen.
- **Aufbewahrung** (Bibliothekseinstellungen `history.keepAllHours` = 24, `history.dailyDays` = 30, `history.weeklyDays` = 365): alle Versionen der letzten 24 Stunden, danach die neueste je UTC-Tag bis 30 Tage, danach die neueste je ISO-Woche (ab Montag) bis 365 Tage; älteres fällt weg. Benannte Versionen und die neueste Version eines Dokuments bleiben immer. Angewandt nach jeder neuen Version und beim Öffnen der Bibliothek; getestet mit simulierter Zeit (`ManualClock`, `tests/history.rs`).
- Endgültiges Löschen des Dokuments entfernt seine Versionen (`ON DELETE CASCADE`).

### `subnets` – vorhanden (Version 7)

| Spalte                     | Typ                   | Regeln                                                                               |
| -------------------------- | --------------------- | ------------------------------------------------------------------------------------ |
| `id`                       | TEXT, Primärschlüssel | UUID (Kleinbuchstaben, `CHECK`)                                                      |
| `name`                     | TEXT                  | 1–200 Zeichen                                                                        |
| `name_key`                 | TEXT                  | Unicode-Kleinschreibung des Namens (Rust); eindeutig je übergeordnetem Subnetz       |
| `description`              | TEXT                  | 0–2000 Zeichen, Zeilenumbrüche erlaubt                                               |
| `color`                    | TEXT                  | `blue`, `green`, `violet`, `orange`, `grey` (Cluster-Token) oder `#rrggbb` (`CHECK`) |
| `parent_id`                | TEXT NULL             | → `subnets(id)` ON DELETE RESTRICT; `CHECK (parent_id IS NULL OR parent_id <> id)`   |
| `created_at`, `updated_at` | TEXT                  | ISO 8601 UTC                                                                         |

`STRICT`. Eindeutiger Index `uq_subnets_sibling_name (ifnull(parent_id, ''), name_key)` (zwei Geschwister heißen nie gleich, auch auf der obersten Ebene), Index `ix_subnets_parent`. Der Trigger `tr_subnets_no_cycle` (`BEFORE UPDATE OF parent_id`) sucht mit einer rekursiven Abfrage unter den Vorfahren des neuen übergeordneten Subnetzes das Subnetz selbst und bricht mit `RAISE(ABORT, 'subnet cycle')` ab; Rust prüft vorher und meldet `SUBNET_CYCLE`.

Abweichungen vom Entwurf (ADR 0017, Punkt 2): kein `sort_key` (Geschwister stehen nach Namen), kein `archived_at`/`deleted_at` (Subnetze sind keine Elemente; ein gelöschtes Subnetz bringt das Struktur-Undo zurück), dafür `name_key`. Eine eigene Farbe muss gegen Seitenleiste und Schreibfläche beider mitgelieferter Designs 3:1 erreichen (Rust).

Löschen eines Subnetzes (Abschnitt 16), eine Transaktion: Untersubnetze rücken eine Ebene hoch – ein dort vergebener Name bekommt „ (2)“, „ (3)“ … – oder werden mitgelöscht (tiefste zuerst); dann geht das Subnetz, `ON DELETE CASCADE` nimmt seine Zuordnungen mit, Dokumente bleiben; wo die primäre Zuordnung entfiel, rückt die älteste verbleibende nach.

### `subnet_memberships` – vorhanden (Version 7)

Das geprüfte Muster aus Abschnitt 18 für Dokumente, ergänzt um `created_at`:

| Spalte        | Typ     | Regeln                                              |
| ------------- | ------- | --------------------------------------------------- |
| `subnet_id`   | TEXT    | → `subnets(id)` ON DELETE CASCADE                   |
| `document_id` | TEXT    | → `documents(id)` ON DELETE CASCADE                 |
| `is_primary`  | INTEGER | 0 oder 1                                            |
| `created_at`  | TEXT    | ISO 8601 UTC; „die älteste verbleibende rückt nach“ |

`STRICT`. Eindeutige Indizes `uq_member_doc (subnet_id, document_id)` und `uq_primary_doc (document_id) WHERE is_primary = 1`; Index `ix_memberships_document (document_id, created_at)`. Genau eine Zuordnung je Dokument ist primär, die erste von selbst (Rust in jeder Transaktion). Papierkorb und Archiv lassen die Zeilen stehen (ADR 0016); erst endgültiges Löschen entfernt sie.

In Version 7 war `document_id` `NOT NULL`, ein `media_id` gab es noch nicht. Migration 0008 baut die Tabelle nach dem geprüften Muster aus Abschnitt 18 neu auf (neue Tabelle, Daten kopieren, alte entfernen, umbenennen; `created_at` bleibt): `document_id` und `media_id` sind beide `NULL`-fähig, `CHECK ((document_id IS NULL) <> (media_id IS NULL))` verlangt genau eines. Dazu je Elementart die eindeutigen Indizes `uq_member_doc`/`uq_member_media (subnet_id, …)` und `uq_primary_doc`/`uq_primary_media (… ) WHERE is_primary = 1` sowie `ix_memberships_document`/`ix_memberships_media`. Getestet wie jede Migration (Fixture der Version 7 mit Zuordnungen).

### `tags`, `document_tags` – vorhanden (Version 7); `media_tags` – vorhanden (Version 8)

`tags`: `id` (UUID), `name` (1–100 Zeichen, ohne Steuerzeichen und Komma), `name_key` (Unicode-Kleinschreibung in Rust, `UNIQUE` – `Äpfel` und `äpfel` sind dasselbe Tag; `COLLATE NOCASE` faltet nur ASCII), `created_at`, `updated_at`; `STRICT`.

`document_tags (document_id, tag_id, created_at)`: Primärschlüssel über beide IDs, `STRICT, WITHOUT ROWID`, Fremdschlüssel mit `ON DELETE CASCADE`, Index `ix_document_tags_tag (tag_id, document_id)`. Zusammenführen ist eine Transaktion (Zuordnungen umhängen, Doppel verwerfen, altes Tag löschen); Löschen eines Tags lässt die Dokumente stehen. In `.pwrite` schreibt Rust die Namen beim Speichern in die Hülle; beim Import werden die Tags der Hülle zugeordnet.

`media_tags (media_id, tag_id, created_at)` wie `document_tags`: Primärschlüssel über beide IDs, `STRICT, WITHOUT ROWID`, Fremdschlüssel mit `ON DELETE CASCADE`, Index `ix_media_tags_tag (tag_id, media_id)`. Zusammenführen und Löschen von Tags behandeln Dokumente und Medien gleich.

### Struktur-Undo (ADR 0017, Punkt 10)

Kein Speicher in der Datenbank: Das Journal hält je Schritt die betroffenen Zeilen von `subnets` und `tags` und Zuordnungen, Tags und Zustand (`deleted_at`, `archived_at`) der betroffenen Dokumente vor und nach dem Schritt, höchstens 100 Schritte, nur im Speicher. Rückgängig prüft, dass der Ist-Stand dem Stand nach dem Schritt gleicht, und schreibt in einer Transaktion den Stand davor: betroffene Subnetze und Tags werden zuerst „abgelöst“ (kein übergeordnetes Subnetz, ein Namensschlüssel mit angehängter ID), dann gelöscht, eingefügt und mit ihren endgültigen Werten geschrieben. So entsteht auf dem Weg weder ein Zyklus noch ein doppelter Name.

### `media` – vorhanden (Version 8)

| Spalte                           | Typ                   | Regeln                                                                           |
| -------------------------------- | --------------------- | -------------------------------------------------------------------------------- |
| `id`                             | TEXT, Primärschlüssel | UUID                                                                             |
| `kind`                           | TEXT                  | `image`, `pdf`, `attachment`                                                     |
| `mime_type`                      | TEXT                  | aus dem Inhalt erkannt, `CHECK` mit der Positivliste (Abschnitt 20)              |
| `extension`                      | TEXT                  | `png`, `jpg`, `jpeg`, `gif`, `webp`, `svg`, `pdf`, `docx`, `xlsx`, `pptx`        |
| `title`                          | TEXT                  | Anzeigename, 1–300 Zeichen, umbenennbar (aus dem Dateinamen ohne Endung)         |
| `description`                    | TEXT                  | 0–2000 Zeichen; Alternativtext beim Einfügen eines Bildes                        |
| `original_name`                  | TEXT                  | Dateiname beim Import, 1–255 Zeichen                                             |
| `import_folder`                  | TEXT NULL             | Importordner (Metadatum)                                                         |
| `size_bytes`                     | INTEGER               | ≥ 0                                                                              |
| `sha256`                         | TEXT                  | Prüfsumme; Index `ix_media_sha256`, **nicht** eindeutig („Trotzdem importieren“) |
| `width`, `height`                | INTEGER NULL          | Bilder (aus dem Dateikopf)                                                       |
| `page_count`                     | INTEGER NULL          | PDF; setzt die PDF-Anzeige beim ersten Öffnen                                    |
| `created_at`, `updated_at`       | TEXT                  | ISO 8601 UTC                                                                     |
| `favorited_at`, `last_opened_at` | TEXT NULL             | Favoriten, „Zuletzt verwendet“ (Index `ix_media_recent`)                         |
| `archived_at`, `deleted_at`      | TEXT NULL             | Archiv, Papierkorb (ADR 0016)                                                    |

Abweichungen vom Entwurf (ADR 0019, Punkt 2): `title` und `description` statt `caption` – eine Bildunterschrift gehört zur einzelnen Verwendung im Dokument, nicht zum Medium; dazu `last_opened_at`.

Datei: `media/images/<id>.<ext>`, `media/pdf/<id>.pdf`, `media/attachments/<id>.<ext>`; Vorschaubilder in `thumbnails/images/<id>.png|jpg` (Rust, beim Import) und `thumbnails/pdf/<id>.png` (Seite 1 aus PDF.js, von Rust geprüft) – abgeleitet, nicht in der Datenbank. Import-Journale `tmp/import-<id>.json` (ADR 0019, Punkt 5) arbeitet das Öffnen der Bibliothek vor dem Leeren von `tmp/` ab; Dateien ohne Zeile und Zeilen ohne Datei meldet es im Protokoll.

### `document_media` – vorhanden (Version 8)

`document_id` → `documents` (CASCADE), `media_id` → `media` (CASCADE), `page` INTEGER NOT NULL DEFAULT 0 (0 = ganzes Medium, sonst PDF-Seite). Primärschlüssel `(document_id, media_id, page)`, `STRICT, WITHOUT ROWID`, Index `ix_document_media_media (media_id, document_id)`. Entsteht beim Speichern aus dem Inhalt, in derselben Transaktion wie `document_links`: in Markdown jedes `pwmedia://<id>` (Bild oder Link, `#page=N` für eine PDF-Seite), in `.pwrite` die Bildknoten und Linkmarken. „Endgültig löschen“ eines Mediums ist nur möglich, solange keine Zeile darauf zeigt.

### `search_entries`, `search_words`, `search_parts` – vorhanden (Version 9)

| Spalte           | Typ                      | Regeln                                                |
| ---------------- | ------------------------ | ----------------------------------------------------- |
| `id`             | INTEGER, Primärschlüssel | `rowid` der Zeile in beiden FTS5-Tabellen             |
| `document_id`    | TEXT NULL, eindeutig     | → `documents(id)` ON DELETE CASCADE                   |
| `media_id`       | TEXT NULL, eindeutig     | → `media(id)` ON DELETE CASCADE                       |
| `subnet_id`      | TEXT NULL, eindeutig     | → `subnets(id)` ON DELETE CASCADE                     |
| `indexed_sha256` | TEXT NULL                | Prüfsumme des indexierten Inhalts; nur bei Dokumenten |

`STRICT`; genau eine der drei Element-Spalten ist gesetzt (`CHECK`). Die FTS5-Tabellen stehen wörtlich wie in Abschnitt 19 (`search_words`: `title, body, tags, subnets, meta`, `unicode61 remove_diacritics 2`, `prefix = '2 3'`; `search_parts`: `title, body`, `trigram remove_diacritics 1`) – ein Test vergleicht sie mit den Definitionen der Probe aus Phase 1 (`src-tauri/src/database/fts.rs`).

Die Ansichten `search_document_fields`, `search_media_fields` und `search_subnet_fields` beschreiben, was der Index je Element hält (ADR 0020, Punkt 2). Trigger halten es in der Transaktion jeder Änderung aktuell: Anlegen und Ändern von Dokumenten, Medien und Subnetzen, Tags und Zuordnungen; das Löschen eines Eintrags (auch über `ON DELETE CASCADE`) entfernt beide FTS-Zeilen. Den Text eines Dokuments schreibt Rust in der Transaktion, die den Inhalt festhält, und setzt `indexed_sha256`; weicht sie von `documents.content_sha256` ab, trägt das Öffnen der Bibliothek den Text aus der Datei nach. Archiv und Papierkorb ändern den Index nicht; die Suche liest den Zustand aus `documents` und `media`. „Suchindex neu aufbauen“ leert alle drei Tabellen und füllt sie aus den Tabellen und Dateien neu.

### `mesh_nodes` – geplant (Phase 11)

| Spalte        | Typ                   | Regeln                                    |
| ------------- | --------------------- | ----------------------------------------- |
| `id`          | TEXT, Primärschlüssel | gleich der ID des Elements oder Subnetzes |
| `kind`        | TEXT                  | `document`, `media`, `subnet`             |
| `document_id` | TEXT NULL UNIQUE      | → `documents(id)` ON DELETE CASCADE       |
| `media_id`    | TEXT NULL UNIQUE      | → `media(id)` ON DELETE CASCADE           |
| `subnet_id`   | TEXT NULL UNIQUE      | → `subnets(id)` ON DELETE CASCADE         |

`CHECK`: genau eine der drei Spalten ist gesetzt, passend zu `kind`, und gleich `id`. Trigger legen den Knoten beim Anlegen eines Elements an; die Migration trägt Knoten für bestehende Elemente nach.

### `mesh_connections` – geplant (Phase 11)

`id` (UUID), `node_a`, `node_b` (→ `mesh_nodes`, CASCADE), `kind` (`manual` oder `similarity` – angenommener Vorschlag), `note` TEXT NULL, `created_at`. Regeln aus Abschnitt 18: `CHECK (node_a < node_b)` (keine Selbstverbindung, Endpunkte sortiert), `UNIQUE (node_a, node_b, kind)`, Index auf `node_b`. Enthält nur Bestätigtes.

### `mesh_suggestions` – geplant (Phase 11)

`id`, `node_a`, `node_b` (sortiert, `CHECK (node_a < node_b)`), `score` REAL (`0 … 1`), `reason` TEXT (JSON mit den Anteilen, z. B. gemeinsame Tags, Inhaltsähnlichkeit), `status` (`open`, `accepted`, `rejected`), `created_at`, `decided_at` NULL. `UNIQUE (node_a, node_b)`: Abgelehnte Vorschläge bleiben als Zeile stehen und kommen deshalb nicht wieder; „Später“ lässt den Status `open`. Getrennt von `mesh_connections` (Abschnitt 15).

### `mesh_layouts`, `mesh_positions` – geplant (Phase 11)

`mesh_layouts`: `id`, `view` (`global`, `subnet`, `focus`), `scope_id` TEXT NOT NULL DEFAULT `''` (Subnetz bzw. Knoten; leer für die Global View), `algorithm`, `options` (JSON), `updated_at`; `UNIQUE (view, scope_id)`.

`mesh_positions`: `layout_id` → `mesh_layouts` (CASCADE), `node_id` → `mesh_nodes` (CASCADE), `x`, `y` REAL, `pinned` INTEGER (0/1; manuell verschobene Knoten bleiben fixiert), Primärschlüssel `(layout_id, node_id)`.

### Sprachprüfung – geplant (Phase 12)

- `dictionaries`: `id`, `name`, `kind` (`custom`, `hunspell`), `language` (BCP 47, NULL = sprachunabhängig), `is_global` (0/1, „gesamte Anwendung“), `package` (Name des Wörterbuch-Datenpakets bei Hunspell), `created_at`, `updated_at`.
- `dictionary_words`: `dictionary_id` → `dictionaries` (CASCADE), `word`, `word_key` (normalisiert), `category` (`word`, `term`, `name`, `abbreviation`), `created_at`; Primärschlüssel `(dictionary_id, word_key)`.
- `document_dictionaries (document_id, dictionary_id)`, `subnet_dictionaries (subnet_id, dictionary_id)`: Primärschlüssel über beide, CASCADE. Für ein Dokument gilt die Vereinigung aus globalen, eigenen und den Wörterbüchern aller Subnetze samt Eltern (Abschnitt 14).
- `language_profiles`: `document_id` (Primärschlüssel, → `documents`, CASCADE), `language`, `detect_paragraphs` (0/1), `autocorrect` (NULL = globale Einstellung, sonst 0/1), `updated_at`.
- `autocorrect_rejections`: `original`, `replacement`, `language`, `created_at`; Primärschlüssel `(original, replacement, language)` (Bedingung 7 der Eindeutigkeitsregel).

Hunspell-Wörterbücher selbst liegen als getrennte Datenpakete vor, nicht in der Datenbank (Abschnitt 7, Lizenzen).

## 5. Papierkorb und Archiv

Umgesetzt in Phase 6 (ADR 0016, Entscheidung 2 und 3):

- Archivieren setzt `archived_at`, Löschen setzt `deleted_at`. Zeile, Datei, Versionen, Zuordnungen und Verbindungen bleiben (Abschnitt 13); Wiederherstellen setzt die Spalte zurück. Rückverweise aus dem Papierkorb zeigt die Abfrage nicht; nach dem Wiederherstellen sind sie wieder da, weil die Zeilen in `document_links` nie entfernt wurden.
- Archivierte Dokumente fehlen in „Favoriten“, „Zuletzt verwendet“, „Alle Dokumente“ und in der Linkauswahl `[[`; sie stehen in der Gruppe „Archiv“ und öffnen normal. Gelöschte Dokumente öffnen nicht (`document_read` meldet `DOCUMENT_NOT_FOUND`).
- Erst das endgültige Entfernen löscht die Zeile: von Hand („Endgültig löschen“, „Papierkorb leeren“, jeweils mit Rückfrage) oder nach `trash.retentionDays` (Bibliothekseinstellung, 1–3650, Standard 30) beim Öffnen der Bibliothek und beim Anzeigen des Papierkorbs. Reihenfolge: Zeile löschen (`ON DELETE CASCADE` entfernt Links, Versionen und künftig Zuordnungen, Knoten und Positionen) → Datei → Recovery-Entwurf. Ein Abbruch dazwischen hinterlässt höchstens eine Datei ohne Zeile, die das Öffnen meldet (Abschnitt 6 unten).
- Standardlisten und Suche blenden archivierte und gelöschte Elemente aus (Indizes mit `WHERE deleted_at IS NULL`).

## 6. Dateien und Datenbank

| Vorgang                  | Reihenfolge                                                                      | Nach einem Absturz dazwischen                                                     |
| ------------------------ | -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Anlegen                  | Datei atomar schreiben → Zeile einfügen                                          | Datei ohne Zeile: beim Öffnen gemeldet, nicht gelöscht                            |
| Speichern                | Datei atomar ersetzen → `size_bytes`/`content_sha256`/`updated_at` aktualisieren | Neue Datei, alte Prüfsumme: das nächste Lesen erkennt es und korrigiert die Zeile |
| Abbruch                  | Temporärdatei `.<name>.<zufall>.pwtmp` bleibt liegen                             | Beim Öffnen entfernt; das Ziel ist unversehrt                                     |
| Endgültig löschen        | Zeile löschen → Datei löschen → Entwurf löschen                                  | Datei ohne Zeile: beim Öffnen gemeldet                                            |
| Version wiederherstellen | Aktuellen Stand als Version `beforeRestore` → Datei speichern                    | Neue Version, alte Datei: der Stand vor dem Wiederherstellen ist erhalten         |

Beim Öffnen vergleicht die App die Dokumentordner mit der Tabelle: Temporärdateien werden entfernt, Dateien ohne Zeile und Zeilen ohne Datei gemeldet (Bibliotheksansicht), nie still gelöscht.

## 7. Integritätsprüfungen

- `PRAGMA quick_check` bei jedem Öffnen einer Bibliothek. Schlägt sie fehl oder ist die Datei keine SQLite-Datenbank, wird nichts verändert (`LIBRARY_DATABASE_DAMAGED`).
- `PRAGMA integrity_check` für jede Kopie: Sicherung vor einer Migration, Datenbankkopie beim Umzug (dort zusätzlich Schemaversion und Dokumentanzahl), ab Phase 10 nach Backup und Wiederherstellung.
- Systemprüfung des Startbildschirms: Datenspeicher (`quick_check`), Schema, Fremdschlüssel, FTS5 – gegen die geöffnete Bibliothek, ohne Bibliothek gegen eine In-Memory-Datenbank.
- Absturztest (Abschnitt 6): 100 harte Prozessabbrüche während des Speicherns, danach `integrity_check` = ok und jede Datei vollständig (`tests/crash_safety.rs`).
