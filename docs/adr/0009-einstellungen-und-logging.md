# ADR 0009 – App- und Bibliothekseinstellungen, Logging

- Status: angenommen
- Datum: 2026-11-03
- Phase: 2

## Kontext

Abschnitt 9: bibliotheksweite Einstellungen in `application_settings`, appweite (Bibliothekspfad, Fensterzustand, Sprache, Tastenkürzel) im Konfigurationsordner des Systems. Abschnitt 8: „Logs bleiben lokal, werden rotiert und enthalten keine Dokumentinhalte.“

## Entscheidungen

### Appweite Einstellungen

- Eine Datei `settings.json` im Konfigurationsordner (`app_config_dir`): atomar geschrieben, `formatVersion`, unbekannte Felder bleiben erhalten.
- Beschädigte Datei: wird unter `settings.json.damaged-<Zeit>` aufbewahrt, die App startet mit Standardwerten und zeigt einen Hinweis. Datei einer neueren Version: wird gelesen, aber nie überschrieben.
- Phase 2 speichert den Bibliothekspfad. Fensterzustand, Sprache und Tastenkürzel kommen mit der Oberfläche (Phase 3) hinzu. `tauri-plugin-window-state` wurde verworfen: eigene Datei außerhalb unserer Konfiguration, nicht atomar, und in E2E-Tests unter Windows nicht in die Sandbox umlenkbar.

### Bibliothekseinstellungen

- Jede Einstellung ist ein Rust-Typ mit Schlüssel, Werttyp, Standardwert und Prüfung (`LibrarySetting`). Keine Zeile = Standardwert. Unbrauchbare gespeicherte Werte ergeben den Standardwert und bleiben unverändert; Schlüssel neuerer Versionen werden nicht angefasst.
- Erste Einstellung: `storage.acceptedLocation` (ADR 0006). Weitere Schlüssel entstehen mit ihren Phasen (database.md).

### Logging

- Eigener, kleiner Logger (`src/logging.rs`, keine neue Abhängigkeit) statt `tauri-plugin-log`: nur Rust schreibt, das Frontend erhält keinen Log-Befehl.
- `project-write.log` im Log-Ordner des Systems; bei 1 MiB Rotation, drei ältere Dateien bleiben. Eine Zeile je Eintrag, Zeilenumbrüche in Meldungen werden maskiert. Release: `Info`, Debug-Build: `Debug`. SQL- und Fensterbibliotheken (`sqlx`, `tao`, `wry` …) nur ab `Warn`.
- Meldungen nennen IDs, Zählerstände und bibliotheksrelative Pfade, nie Dokumentinhalte oder Titel.

## Folgen

- Die E2E-Tests lesen den Log (Start, sauberes Schließen der Bibliothek beim Beenden).
- Fehlerdetails, die nicht in die Oberfläche gehören (SQL-Meldungen, absolute Pfade), stehen nur im Log.
