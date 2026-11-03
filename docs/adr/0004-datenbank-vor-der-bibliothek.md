# ADR 0004 – Datenbank vor der Bibliothek: In-Memory in Phase 1, erste Migration

- Status: angenommen; der Laufzeit-Teil (In-Memory statt Bibliotheksdatenbank) ist seit Phase 2 durch [ADR 0006](0006-bibliothek-sperre-und-speicherort.md) abgelöst
- Datum: 2026-11-02
- Phase: 1

## Kontext

Phase 1 verlangt „SQLite über SQLx anbinden, erste Migration“. Die Datenbankdatei gehört aber in die Bibliothek (`<Bibliothek>/database/projectwrite.db`, Abschnitt 9), und Bibliotheken entstehen erst in Phase 2. Den Speicherort wählt der Nutzer beim ersten Start (Phase 2/3).

Zugleich verbietet Abschnitt 2 Tabellen „auf Vorrat“, und Abschnitt 18 sagt, Tabellen entstehen in der Phase, die sie braucht. Phase 1 hat noch keine fachlichen Daten. **Das ist eine Spannung zwischen Phasenliste und Abschnitt 2/18, die hier gemeldet wird.**

## Optionen für den Speicherort in Phase 1

1. Datei im Datenordner des Systems (`app_data_dir`). Würde in Phase 2 zu einer verwaisten Datei außerhalb jeder Bibliothek.
2. Datei unter `Dokumente/ProjectWrite` ohne Bibliothekslogik. Schreibt ohne Zustimmung in einen Nutzerordner und nimmt Phase 2 vorweg.
3. **In-Memory-Datenbank** im laufenden Programm, Dateidatenbanken nur in Tests.

## Entscheidung

- Option 3. `AppState::initialize()` öffnet in Phase 1 eine private In-Memory-Datenbank und führt die Migrationen aus. Die App schreibt in Phase 1 nichts auf den Datenträger. Der Startbildschirm prüft damit die mitgebaute SQLite im echten Programm (Systemprüfung).
- `Database::open_file()` mit WAL- und Rollback-Modus ist vollständig implementiert und durch Integrationstests mit Dateien in `src-tauri/target/tmp/` abgedeckt. Phase 2 schaltet beim Öffnen einer Bibliothek darauf um.
- Für In-Memory gibt es genau eine Verbindung, die nie geschlossen oder erneuert wird (`idle_timeout`/`max_lifetime` aus), weil sonst die Datenbank verloren ginge. Für Dateien gilt: eine Schreibverbindung, vier Leseverbindungen mit `read_only`.
- Migration 0001 legt nur `application_settings` an (Abschnitt 9, „Einstellungen“). Begründung: Sie ist die allgemeinste Tabelle der Liste aus Abschnitt 18, Phase 2 nutzt sie sofort für die bibliotheksweiten Einstellungen, und sie ist klein genug, um die Migrationsmechanik (Transaktion, Idempotenz, Ablehnung neuerer Schemata, `STRICT`, `CHECK`) zu belegen. Weitere Tabellen entstehen erst in ihren Phasen.

## Folgen

- Die Systemprüfung zeigt in Phase 1 den Zustand der In-Memory-Datenbank. Ab Phase 2 prüft sie die Datenbank der geöffneten Bibliothek.
- Phase 2 ergänzt vor jeder Migration eine Sicherung (Abschnitt 18). Migrationen laufen deshalb nicht automatisch beim Öffnen, sondern über `Database::migrate()`.
- Das Prüfen neuerer Schemata geschieht vor jeder Änderung: Ist die höchste angewandte Version größer als die höchste bekannte, meldet `migrate()` `SchemaTooNew`, ohne etwas zu schreiben.
