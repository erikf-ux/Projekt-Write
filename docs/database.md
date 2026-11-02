# Datenbank – Project Write

Stand: Phase 1. Das **vollständige Schema** wird in Phase 2 geplant und hier dokumentiert (Abschnitt 18). Diese Fassung beschreibt Konventionen, Verbindungsaufbau, Migrationsmechanik und die Tabellen, die bereits existieren.

## Konventionen (Abschnitt 18)

- SQLite, mit der App gebaut (`libsqlite3-sys` 0.37.0 über SQLx 0.9.0, Feature `sqlite-bundled`). Kein zweites Crate auf `libsqlite3-sys`.
- Alle Tabellen `STRICT`. Ausnahme: die interne Tabelle `_sqlx_migrations` von SQLx.
- IDs als UUID-Text, Zeiten in UTC nach ISO 8601 mit `Z` (`2026-11-02T10:00:00Z`, optional mit Sekundenbruchteilen).
- Weiches Löschen über `deleted_at` und `archived_at` (ab Phase 6).
- Fremdschlüssel für jede Verbindung, sinnvolle Indizes, eindeutige Constraints, `CHECK`-Bedingungen für Formate.
- SQL steht nur in `src-tauri/src/database/` und den späteren Repositories. SQLx 0.9 akzeptiert nur statisches SQL; dynamisches SQL braucht `AssertSqlSafe` (nur in Tests).

## Verbindungen

|                | Dateidatenbank                                                           | In-Memory (Phase 1, Tests)           |
| -------------- | ------------------------------------------------------------------------ | ------------------------------------ |
| Schreiben      | 1 Verbindung (`max_connections = 1`)                                     | 1 Verbindung für alles, nie erneuert |
| Lesen          | Pool mit 4 Verbindungen, `read_only`                                     | dieselbe Verbindung                  |
| `journal_mode` | `WAL` lokal, `DELETE` für Sync-Ordner/Netzlaufwerke (Auswahl ab Phase 2) | `MEMORY`                             |
| `synchronous`  | `FULL`                                                                   | `FULL`                               |
| `foreign_keys` | `ON`                                                                     | `ON`                                 |
| `busy_timeout` | 5000 ms                                                                  | 5000 ms                              |

## Migrationen

- Ort: `src-tauri/migrations/NNNN_beschreibung.sql`, eingebettet zur Übersetzungszeit (`sqlx::migrate!`).
- Nur vorwärts. Jede Migration läuft in einer eigenen Transaktion.
- Ausgelieferte Migrationen werden nie geändert. SQLx speichert eine Prüfsumme je Migration und bricht bei Abweichung ab.
- `Database::migrate()` prüft vor jeder Änderung, ob die Datenbank eine höhere Version kennt als die App. Dann meldet es `SchemaTooNew` (Fehlercode `SCHEMA_TOO_NEW`) und ändert nichts.
- Ein zweiter Lauf ändert nichts (Test vergleicht Schema und Migrationstabelle).
- Ab Phase 2: Sicherung vor jeder Migration, Tests ab jeder früheren Schemaversion.

| Version | Datei                           | Inhalt                         | Phase |
| ------- | ------------------------------- | ------------------------------ | ----- |
| 1       | `0001_application_settings.sql` | Tabelle `application_settings` | 1     |

## Tabellen

### `application_settings` (Version 1)

Bibliotheksweite Einstellungen (Abschnitt 9). Appweite Einstellungen liegen dagegen im Konfigurationsordner des Systems.

| Spalte       | Typ                   | Regeln                       |
| ------------ | --------------------- | ---------------------------- |
| `key`        | TEXT, Primärschlüssel | 1–128 Zeichen                |
| `value`      | TEXT                  | gültiges JSON (`json_valid`) |
| `updated_at` | TEXT                  | ISO 8601 UTC mit `Z`         |

`STRICT, WITHOUT ROWID`.

## Volltextsuche (geplant für Phase 9)

Phase 1 belegt, dass FTS5 aktiv ist: `pragma_compile_options` nennt `ENABLE_FTS5`, beide Tabellen aus Abschnitt 19 lassen sich mit genau dieser Definition anlegen, `"C++"` und ein einzelnes `"` funktionieren, `Muller` findet `Müller`, `haus` findet `Krankenhaus`. Die Definitionen stehen in `src-tauri/src/database/fts.rs` und werden in Phase 9 per Migration angelegt.

## Integritätsprüfungen

- `PRAGMA quick_check` – Systemprüfung „Datenspeicher“ (in Phase 2 beim Öffnen der Bibliothek).
- `PRAGMA integrity_check` – nach Backup und Wiederherstellung (Phase 10), in Tests nach Migrationen.
