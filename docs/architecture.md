# Architektur – Project Write

Stand: Phase 2 (Speicherkern), 3. November 2026. Entscheidungen im Detail stehen in `docs/adr/`.

## 1. Überblick

Project Write ist eine Tauri-2-Desktop-Anwendung. Ein Rust-Prozess besitzt alle Daten und alle Zugriffe auf Datenbank und Dateisystem. Die Oberfläche ist eine React-Anwendung im System-Webview (WebView2 unter Windows, WKWebView unter macOS, WebKitGTK unter Linux). Beide sprechen nur über typisierte Befehle miteinander.

```text
┌──────────────────────── Webview (nicht vertrauenswürdig) ────────────────────────┐
│  Oberfläche (React-Komponenten, src/app, src/components, src/modules/*)          │
│        │                                                                          │
│  Frontend-Dienste (src/services) ── einzige Stelle mit invoke()                   │
└────────┼──────────────────────────────────────────────────────────────────────────┘
         │  IPC: Befehlsname + { request } → Ergebnis | { code, messageKey, details? }
┌────────┼──────────────────────── Rust-Prozess (vertrauenswürdig) ─────────────────┐
│  Typisierte Befehle (src-tauri/src/commands) ── Input<T>: deserialisieren + Validate│
│        │                                                                          │
│  Rust-Dienste (src-tauri/src/services)                                            │
│        │                                                                          │
│  Repositories / Datenbankschicht (src-tauri/src/database)                         │
│        │                                                                          │
│  SQLite (mitgebaut, SQLx)        Dateisystem (src/filesystem, src/security)       │
└───────────────────────────────────────────────────────────────────────────────────┘
```

Keine Schicht wird übersprungen (Abschnitt 8). ESLint verbietet `@tauri-apps/api/core` außerhalb von `src/services/ipc.ts` und jedes `@tauri-apps/plugin-*` im Frontend.

## 2. Laufzeit

1. `main.rs` ruft `project_write_lib::run()`.
2. `run()` registriert zuerst `tauri-plugin-single-instance` (ein zweiter Programmstart übergibt an die laufende Instanz und endet; ADR 0006), im Debug-Build mit Feature `e2e` den eingebetteten WebDriver (ADR 0003), sonst keine Plugins.
3. `setup` bestimmt die App-Ordner (Konfiguration, Logs, Dokumente; im `e2e`-Build aus `PW_E2E_APP_DIRS`), startet den Datei-Logger, lädt `settings.json` und erzeugt den `AppState`. Nennt die Einstellung eine Bibliothek, wird sie geöffnet (Sperre, Prüfungen, gegebenenfalls Sicherung und Migration).
4. Das Fenster `main` lädt das gebaute Frontend aus `dist/` über das Tauri-Protokoll. Tauri setzt dabei die CSP als Header und ergänzt Hashes für eigene Skripte.
5. `main.tsx` setzt die Farb-Tokens (System-Theme) und rendert den Startbildschirm. Er fragt `library_status` ab und zeigt die Bibliotheksansicht; die Systemprüfung läuft gegen die Datenbank der geöffneten Bibliothek (ohne Bibliothek gegen eine In-Memory-Datenbank) und erneut, wenn eine andere Bibliothek geöffnet wird.
6. Beim Beenden (`RunEvent::Exit`) schließt die App die Bibliothek: Datenbank zu, Sperre frei.

## 3. Backend (`src-tauri/`)

| Datei / Modul                                           | Aufgabe                                                                                                                                                                                                   | Phase        |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| `src/lib.rs`                                            | App-Aufbau, Plugin- und Befehlsregistrierung, `compile_error!`-Schutz gegen Testplugins im Release                                                                                                        | 1            |
| `src/state.rs`                                          | `AppState` (geteilte Dienste)                                                                                                                                                                             | 1            |
| `src/error.rs`                                          | `ErrorCode`, `CommandError` – einheitliches Fehlerformat                                                                                                                                                  | 1            |
| `src/commands/mod.rs`                                   | `COMMAND_NAMES`, `handler()` mit `generate_handler!`                                                                                                                                                      | 1            |
| `src/commands/input.rs`                                 | `Input<T>` und `Validate` (ADR 0002)                                                                                                                                                                      | 1            |
| `src/commands/system.rs`                                | Befehl `system_check`                                                                                                                                                                                     | 1            |
| `src/services/system.rs`                                | Systemprüfung: Datenspeicher, Schema, Fremdschlüssel, Volltextsuche                                                                                                                                       | 1            |
| `src/database/mod.rs`                                   | `Database`: Schreibverbindung + Lese-Pool, Pragmas, Migrationen                                                                                                                                           | 1            |
| `src/database/diagnostics.rs`                           | Lesende Prüfungen (`quick_check`, `integrity_check`, Pragmas, Compile-Optionen)                                                                                                                           | 1            |
| `src/database/fts.rs`                                   | FTS5-Definitionen aus Abschnitt 19, `quote_term`, FTS5-Probe                                                                                                                                              | 1 (Suche: 9) |
| `migrations/`                                           | Einziger Ort für Migrationen, eingebettet mit `sqlx::migrate!`                                                                                                                                            | 1            |
| `capabilities/main-window.json`                         | Einzige Capability: genau die eigenen Befehle                                                                                                                                                             | 1            |
| `build.rs`, `windows-app-manifest.xml`                  | App-Manifest: jeder eigene Befehl ist ohne Capability verboten. Unter Windows wird das Common-Controls-v6-Manifest in App **und** Test-Executables gelinkt (sonst `STATUS_ENTRYPOINT_NOT_FOUND` in Tests) | 1            |
| `src/clock.rs`                                          | Zeitstempel (UTC, feste Breite) und IDs; austauschbar für Tests und die Referenzbibliothek                                                                                                                | 2            |
| `src/logging.rs`                                        | Rust-seitiger Datei-Logger, rotierend (ADR 0009)                                                                                                                                                          | 2            |
| `src/security/paths.rs`                                 | Bibliotheksrelative Pfade: Namensprüfung, kein `..`, keine Verknüpfungen (ADR 0006)                                                                                                                       | 2            |
| `src/filesystem/atomic.rs`                              | Atomares Schreiben: Temporärdatei, `fsync`, Umbenennen (ADR 0007)                                                                                                                                         | 2            |
| `src/filesystem/location.rs`                            | Sync-Ordner und Netzlaufwerke erkennen → WAL oder Rollback-Journal (ADR 0006)                                                                                                                             | 2            |
| `src/filesystem/windows.rs`                             | Einzige Stelle mit `unsafe`: `MoveFileExW`, `GetDriveTypeW`, `CfGetSyncRootInfoByPath`                                                                                                                    | 2            |
| `src/services/library/`                                 | Bibliothek: `library.json`, `library.lock`, Anlegen/Öffnen/Schließen, Aufräumen beim Öffnen, Sicherung vor Migrationen, Umzug (`relocate.rs`)                                                             | 2            |
| `src/services/documents.rs`                             | Dokument-Repository: Datei + Zeile, Reihenfolge und Absturzverhalten (ADR 0007)                                                                                                                           | 2            |
| `src/services/settings/`                                | `settings.json` (appweit) und typisierte Bibliothekseinstellungen (ADR 0009)                                                                                                                              | 2            |
| `src/services/session.rs`                               | Die Bibliothek der Instanz: Status, Tokens für gewählte Ordner, Anlegen/Öffnen/Umziehen, Fehlerzuordnung                                                                                                  | 2            |
| `src/services/dialog.rs`                                | Ordnerdialog über `rfd` (`SystemPicker`), Testvariante `ScriptedPicker` (ADR 0008)                                                                                                                        | 2            |
| `src/commands/library.rs`                               | Befehle der Bibliothek                                                                                                                                                                                    | 2            |
| `src/database/documents.rs`, `src/database/settings.rs` | SQL der Tabellen `documents` und `application_settings`                                                                                                                                                   | 2            |
| `migrations/0002_documents.sql`                         | Tabelle `documents`                                                                                                                                                                                       | 2            |
| `examples/reference_library/`                           | Erzeugt die Referenzbibliothek (ADR 0010), gestartet von `scripts/reference-library.mjs`                                                                                                                  | 2            |
| `src/language/`                                         | entsteht in Phase 12                                                                                                                                                                                      | –            |

## 4. Frontend (`src/`)

| Pfad                                                 | Aufgabe                                                                                             |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `main.tsx`                                           | Einstieg, Theme vor dem ersten Rendern                                                              |
| `app/App.tsx`, `app/StartScreen.tsx`, `app/Logo.tsx` | App-Hülle und Startbildschirm                                                                       |
| `app/systemStatusStore.ts`                           | Zustand (Zustand-Store) der Systemprüfung                                                           |
| `modules/library/`                                   | Bibliotheksansicht (`LibraryPanel`), Store (`libraryStore.ts`), Größenformat; Phase 2               |
| `services/library.ts`                                | Aufrufe der Bibliotheksbefehle                                                                      |
| `services/ipc.ts`                                    | Typisierte Befehlsaufrufe, Fehlernormalisierung, Fortschritt über Tauri-Kanäle (`callWithProgress`) |
| `services/system.ts`                                 | Aufruf `system_check`                                                                               |
| `i18n/`                                              | Wörterbücher `de.json`, `en.json`, `translate`, `useT`, Spracherkennung                             |
| `modules/themes/`                                    | Design-Tokens, Kontrastberechnung, Anwenden als CSS-Variablen                                       |
| `types/generated/`                                   | Aus Rust erzeugte Typen (`ts-rs`), nie von Hand ändern                                              |
| `test/setup.ts`                                      | Vitest-Setup (jsdom)                                                                                |
| `index.css`                                          | Tailwind 4, Zuordnung Tokens → Utility-Farben, Basisstile, Fokusring, `prefers-reduced-motion`      |

**Theming:** `modules/themes/tokens.ts` ist die einzige Quelle der Farben. `applyTheme` erzeugt daraus einmal ein `<style>` mit Regeln `[data-theme="dark"|"light"] { --pw-color-*: …; color-scheme: … }` und schaltet nur noch das Attribut `data-theme` an `<html>` um. Tailwind-Klassen wie `bg-surface` oder `text-muted` verweisen per `@theme inline` auf die Variablen. Hintergrund: Das direkte Umschreiben der Variablen als Inline-Stil an `<html>` hinterließ in WebKit unter macOS veraltete berechnete Farben (im E2E-Test belegt); das Umschalten über Selektoren wird von allen Engines zuverlässig neu berechnet. Importierbare Theme-Dateien (Phase 3) liefern andere Werte für dieselben Tokens.

**Sprache:** Deutsch und Englisch, standardmäßig nach `navigator.languages`. Fehlermeldungen kommen als `messageKey` aus Rust und werden im Frontend übersetzt. Ein Rust-Test prüft, dass jeder `messageKey` in beiden Sprachdateien steht.

## 5. IPC-Vertrag

Siehe ADR 0002. Kurzfassung:

- Aufruf: `invoke(name, { request })`. `request` ist ein aus Rust erzeugter Typ.
- Erfolg: aus Rust erzeugter Ergebnistyp.
- Fehler: immer `CommandError { code: ErrorCode, messageKey: string, details?: Record<string, string> }`.
- `ErrorCode`: `INVALID_INPUT`, `DATABASE`, `SCHEMA_TOO_NEW`, `INTERNAL`, `IPC_REJECTED` (nur Frontend, für Ablehnungen durch Tauri selbst) und seit Phase 2 die Codes der Bibliothek (`LIBRARY_LOCKED`, `LIBRARY_MOVED`, `LOCATION_NOT_ACCEPTED` …; vollständige Liste in `src-tauri/src/error.rs`, jede mit deutscher und englischer Meldung).
- Seit Phase 2: `request` muss ein JSON-Objekt sein (serde würde sonst auch ein Array der Feldwerte annehmen). Befehle ohne Parameter nehmen `{}` (`EmptyRequest`).
- Ordner werden in Rust gewählt; das Frontend erhält ein **Token** und einen Anzeigetext und schickt nur das Token zurück (ADR 0008).

| Befehl                     | Request                                                                      | Response                                                                                | Phase |
| -------------------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ----- |
| `system_check`             | `SystemCheckRequest { checks: SystemCheckId[] }` (nicht leer, ohne Doppelte) | `SystemCheckResponse { appVersion, platform, sqliteVersion, schemaVersion, results[] }` | 1     |
| `library_status`           | `{}`                                                                         | `LibraryStatus` (`closed` mit Grund, `open` mit `LibraryInfo`, `moving`)                | 2     |
| `library_default_location` | `{}`                                                                         | `LocationChoice` für `Dokumente/ProjectWrite` oder `null`                               | 2     |
| `library_choose_location`  | `{ purpose: create \| open \| move, title }`                                 | `LocationChoice` (Token, Anzeigepfad, Inhalt, Ort) oder `null` (abgebrochen)            | 2     |
| `library_create`           | `{ location: Token, acceptWarning }`                                         | `LibraryStatus`                                                                         | 2     |
| `library_open`             | `{ location: Token, acceptMoved }`                                           | `LibraryStatus`                                                                         | 2     |
| `library_reopen`           | `{}`                                                                         | `LibraryStatus` (Bibliothek aus den Einstellungen erneut öffnen)                        | 2     |
| `library_move`             | `{ location: Token, acceptWarning }` + Kanal `onProgress: MoveProgress`      | `LibraryStatus`                                                                         | 2     |
| `library_cancel_move`      | `{}`                                                                         | `boolean` (lief ein Umzug?)                                                             | 2     |
| `library_accept_location`  | `{}`                                                                         | `LibraryStatus` (Sync-Ordner/Netzlaufwerk bestätigt)                                    | 2     |

## 6. Sicherheitsmodell (Abschnitt 20)

| Maßnahme                            | Umsetzung                                                                                                                      | Geprüft durch                                                                                                         |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| CSP exakt nach Abschnitt 20         | `tauri.conf.json` → `app.security.csp`                                                                                         | `tests/security_config.rs` (Wortlaut), E2E (Inline-Skript läuft nicht, Webbild lädt nicht, `securitypolicyviolation`) |
| Kein globales Tauri-Objekt          | `withGlobalTauri: false`                                                                                                       | Konfigurationstest, E2E (`window.__TAURI__` undefiniert)                                                              |
| Eingefrorene Prototypen             | `freezePrototype: true`                                                                                                        | Konfigurationstest, E2E (`Object.isFrozen(Object.prototype)`)                                                         |
| Asset-Protokoll aus                 | `assetProtocol.enable: false`                                                                                                  | Konfigurationstest                                                                                                    |
| Keine Plugin-Rechte                 | Capability enthält genau die eigenen Befehle; einziges Plugin außer dem Testplugin ist `single-instance` (ohne Befehle)        | Konfigurationstest, E2E (`fs`, `shell`, `sql`, `path`, `opener`, `dialog` abgelehnt)                                  |
| Keine Pfade aus dem Frontend        | Ordnerwahl in Rust, Tokens statt Pfaden (ADR 0008)                                                                             | IPC-Tests (Pfad statt Token → `INVALID_INPUT`), E2E (`library_open` mit Pfad abgelehnt)                               |
| Pfadangriffe, Verknüpfungen         | `SafeRoot`: Namensregeln, kein `..`, jede Pfadstufe ohne Verknüpfung; `O_NOFOLLOW` unter Unix; Pfade aus ID und Typ abgeleitet | `tests/path_security.rs`, `tests/library.rs`, `tests/documents.rs`, `tests/relocate.rs` (Symlinks, Junctions)         |
| Eine Instanz je Bibliothek          | Dateisperre `library.lock`, `tauri-plugin-single-instance`                                                                     | `tests/library_lock.rs` (zweiter Prozess, harter Abbruch), IPC-Test, E2E (zweiter Start endet mit Code 0)             |
| IPC nur von eigenen Seiten          | keine `remote`-Capability; Tauri lehnt fremde Ursprünge ab                                                                     | `tests/ipc.rs` (`https://example.com` u. a. abgelehnt), Konfigurationstest                                            |
| Eigene Befehle nur nach Freigabe    | App-Manifest in `build.rs`                                                                                                     | Konfigurationstest                                                                                                    |
| Eingaben validiert                  | `Input<T>` + `Validate`, `deny_unknown_fields`                                                                                 | `tests/ipc.rs`                                                                                                        |
| Keine internen Details in Fehlern   | `CommandError` ohne Quelltext; Frontend verwirft fremde Fehlertexte                                                            | Unit-Tests Rust und Vitest                                                                                            |
| Testplugins nur im Debug-Build      | Feature `e2e`, `compile_error!`, CI-Job `release-check`                                                                        | CI                                                                                                                    |
| Kein dynamisches SQL ohne Prüfmarke | SQLx 0.9 `SqlSafeStr`                                                                                                          | Compiler                                                                                                              |
| Verbotene Abhängigkeiten            | kein `rusqlite`, `tauri-plugin-sql/fs/shell/http`                                                                              | Konfigurationstest, `cargo deny` (`[bans]`)                                                                           |
| Lieferkette                         | `cargo audit`, `cargo deny`, `npm audit`                                                                                       | CI-Job `audit`                                                                                                        |
| Testnähte nur im Debug-Build        | `PW_E2E_APP_DIRS`, `PW_E2E_PICK_FOLDERS` nur mit Feature `e2e`                                                                 | `compile_error!` im Release, CI-Job `release-check`                                                                   |
| Isolation Pattern                   | nicht aktiviert, siehe ADR 0005                                                                                                | ADR 0005                                                                                                              |

Die Bibliothek ist in Version 1.0 nicht verschlüsselt. Die Dokumentation für Nutzer (Phase 14) empfiehlt die Datenträgerverschlüsselung des Systems.

## 7. Datenhaltung

Siehe `docs/database.md` (vollständiges Schema), ADR 0006 (Bibliothek) und ADR 0007 (atomares Speichern). Pragmas: `foreign_keys=ON`, `synchronous=FULL`, `busy_timeout=5000`, `journal_mode=WAL` lokal oder `DELETE` in Sync-Ordnern und auf Netzlaufwerken (bei jedem Öffnen neu bestimmt). Ordner der Bibliothek nach Abschnitt 9; Dateien tragen UUID-Namen, Titel und Originalnamen stehen in der Datenbank.

## 8. Tests und CI

| Ebene            | Werkzeug                                                                     | Ort                   | Inhalt                                                                                                                                                                                                                                     |
| ---------------- | ---------------------------------------------------------------------------- | --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Rust-Unit        | `cargo test`                                                                 | `src-tauri/src/**`    | Fehlerformat, `Input<T>`, Validierung, FTS-Quoting, Systemprüfung, Zeitstempel, Pfadregeln, atomares Schreiben, Ortserkennung, `library.json`, Einstellungen, Logger                                                                       |
| Rust-Integration | `cargo test`                                                                 | `src-tauri/tests/`    | Migrationen ab jeder Version, Bibliothek (anlegen, öffnen, sperren, Rollback, Aufräumen, Sicherung), zweiter Prozess, Pfadangriffe, Dokumente, Umzug, Einstellungen, 100 harte Abbrüche, Referenzbibliothek, IPC, Sicherheitskonfiguration |
| Frontend         | Vitest + jsdom + `mockIPC`                                                   | `src/**/*.test.ts(x)` | Kontrast, i18n, IPC-Wrapper, Startbildschirm, Bibliotheksansicht (alle Abläufe inkl. Fortschritt über Kanal)                                                                                                                               |
| E2E              | WebdriverIO + `@wdio/tauri-service` (embedded)                               | `tests/e2e/`          | Bibliothek anlegen mit Sync-Warnung, Umzug in Rollback-Modus, sauberes Beenden, Neustart, zweite Instanz, umgezogene alte Kopie; Start, CSP, abgelehnte Zugriffe, Kontrast, Fokus                                                          |
| Statisch         | ESLint (strict-type-checked), `tsc`, Prettier, rustfmt, Clippy `-D warnings` | –                     | –                                                                                                                                                                                                                                          |

CI (`.github/workflows/ci.yml`): Job `checks` auf `windows-latest` und `ubuntu-24.04` (macOS auf Wunsch des Auftraggebers vorerst zurückgestellt): Format, Lint, Typen, Vitest, Rust-Tests (vorher werden die erzeugten Typen gelöscht, damit die Prüfung auf aktuelle Typen greift), Referenzbibliothek zweimal gebaut und verglichen, Lizenzliste, E2E-Build, E2E (Linux in einer D-Bus-Sitzung). Job `release-check` (Linux), Job `audit` (Linux).

Tests schreiben nur in temporäre Ordner innerhalb des Projekts (`src-tauri/target/tmp/`, `tests/e2e/.tmp/`); die App im E2E-Build nutzt für Konfiguration, Logs und Dokumente `tests/e2e/.tmp/OneDrive/`.

## 9. Projektstruktur

```text
.
├── .cargo/config.toml         Ausgabeort der erzeugten Typen (für Befehle aus dem Projektstamm)
├── .github/workflows/ci.yml
├── docs/                      Auftrag, Status, Berichte, Architektur, Datenbank, Abhängigkeiten, ADRs
├── scripts/                   Lizenzliste, Release-Prüfung, Icons, Referenzbibliothek
├── src/
│   ├── app/                   App-Hülle, Startbildschirm, Store der Systemprüfung
│   ├── i18n/                  de.json, en.json, index.ts
│   ├── modules/library/       Bibliotheksansicht und Store (Phase 2)
│   ├── modules/themes/        Tokens, Kontrast, applyTheme
│   ├── services/              ipc.ts, system.ts, library.ts
│   ├── test/                  Vitest-Setup
│   └── types/generated/       ts-rs-Ausgabe
├── src-tauri/
│   ├── capabilities/          main-window.json
│   ├── examples/              reference_library (Referenzbibliothek)
│   ├── icons/
│   ├── migrations/            0001_application_settings.sql, 0002_documents.sql
│   ├── src/                   commands/, database/, filesystem/, security/, services/, clock.rs, error.rs, logging.rs, state.rs, lib.rs, main.rs
│   └── tests/                 Integrationstests (siehe Abschnitt 8)
└── tests/e2e/                 wdio.conf.ts, sandbox.ts, session.ts, specs/, manual/
```

Abweichungen von Abschnitt 23 (klein, ohne eigene ADR): `src-tauri/src/error.rs`, `state.rs`, `clock.rs` und `logging.rs` liegen auf oberster Ebene, weil alle Schichten sie nutzen; `src/test/` enthält das Vitest-Setup; `src/app/` enthält den Startbildschirm, bis Phase 3 das Hauptlayout in `src/components/layout/` baut; `src/modules/library/` ist ein Modul, das Abschnitt 23 nicht nennt (Verwaltung der Bibliothek, später Teil der Einstellungen); `src-tauri/examples/` enthält den Generator der Referenzbibliothek, damit er nicht in die App gelinkt wird. Leere Zielordner (`components/`, `modules/editor/` …) entstehen erst in ihren Phasen.

## 10. Prüfung der Gesamtarchitektur (alle Phasen)

Ergebnis der Prüfung des gesamten Auftrags auf technische Machbarkeit. Risiken werden in der genannten Phase erneut bewertet.

| Bereich                              | Bewertung                                                                                            | Risiko / Maßnahme                                                                                                                                                          | Phase |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- |
| Tauri 2 + strikte CSP                | machbar, in Phase 1 belegt                                                                           | Jede spätere Lockerung braucht eine ADR                                                                                                                                    | alle  |
| SQLite + FTS5 + Trigram              | machbar, in Phase 1 belegt (`unicode61 remove_diacritics 2`, `trigram remove_diacritics 1`, `"C++"`) | `ß`/`ss` und Umlaut-Umschreibungen müssen in Rust erweitert werden (Abschnitt 19)                                                                                          | 9     |
| Speicherkern                         | umgesetzt (Phase 2)                                                                                  | `MoveFileExW` mit Write-Through, `fsync` des Ordners unter Unix; 100 harte Abbrüche als Kindprozess-Test (ADR 0007)                                                        | 2     |
| Einzelinstanz                        | umgesetzt: `tauri-plugin-single-instance` 2.5.5 + Betriebssystem-Dateisperre auf `library.lock`      | Die Sperre gibt das System beim Prozessende frei, keine veralteten Sperren (ADR 0006); Linux braucht für das Plugin einen D-Bus-Sitzungsbus                                | 2, 5  |
| Logging                              | umgesetzt: eigener Datei-Logger                                                                      | Nur Rust-seitig, rotierend, ohne Inhalte (ADR 0009)                                                                                                                        | 2     |
| Tiptap 3 + `@tiptap/markdown` (Beta) | machbar hinter Adapter                                                                               | Verlustfälle aus Abschnitt 10; `freezePrototype` mit ProseMirror prüfen                                                                                                    | 4     |
| 50.000 Wörter, ≤ 50 ms               | riskant                                                                                              | Messung in Phase 4; Sprachprüfung absatzweise im Hintergrund                                                                                                               | 4, 12 |
| Externe Dateien, Kodierungen         | machbar (`encoding_rs`, Erkennung von BOM/UTF-16/Windows-1252)                                       | Windows-1252 vs. Latin-1-Heuristik testen                                                                                                                                  | 5     |
| Dateizuordnung auf drei Wegen        | machbar                                                                                              | `RunEvent::Opened` nur macOS; Pfadpuffer bis die Oberfläche bereit ist                                                                                                     | 5     |
| Medien-Handler `pwmedia://`          | machbar (`register_asynchronous_uri_scheme_protocol`)                                                | CSP enthält `pwmedia:` und `http://pwmedia.localhost` (Windows/Android-Schema) bereits                                                                                     | 8     |
| PDF.js                               | machbar                                                                                              | Neuere PDF.js-Versionen nutzen WebAssembly für einige Bildformate; das bräuchte `'wasm-unsafe-eval'` in `script-src` (CSP-Lockerung → ADR) oder Verzicht auf diese Decoder | 8     |
| SVG sicher                           | machbar                                                                                              | Nur als `<img>` über Handler mit `Content-Security-Policy: sandbox`                                                                                                        | 8     |
| Druck/PDF-Export                     | riskant unter WebKitGTK                                                                              | `window.print` ist Standard; Test auf allen drei Systemen                                                                                                                  | 10    |
| Backup mit `VACUUM INTO`             | machbar mit SQLx                                                                                     | Prüfen per `integrity_check`; ZIP-Bibliothek mit Pfadprüfung wählen                                                                                                        | 10    |
| React Flow + ELK.js                  | machbar bis ~1.000 Knoten                                                                            | ELK.js steht unter EPL-2.0 – Lizenzprüfung vor jeder Weitergabe (Abschnitt 7)                                                                                              | 11    |
| `spellbook` (Alpha)                  | offen                                                                                                | Vergleich mit Hunspell-Bindings in Phase 12; Wörterbücher (GPL) als getrennte Datenpakete                                                                                  | 12    |
| LanguageTool                         | machbar als Sidecar                                                                                  | Java 17, 1–4 GB RAM; nur `127.0.0.1`, zufälliger Port                                                                                                                      | 12    |
| Updates und Signierung               | Entscheidung nötig                                                                                   | D4, D11, Apple-Konto, Windows-Signatur, Bundle-Kennung (ADR 0001)                                                                                                          | 14    |
| Lizenzen                             | MPL-2.0 im Tauri-Baum                                                                                | `cssparser`, `cssparser-macros`, `dtoa-short`, `selectors`, `option-ext` stehen unter MPL-2.0 (Dateiebene, Quellcode unverändert verfügbar). Für D4 vormerken              | 14    |

### Gefundene Widersprüche und Spannungen

1. **`@wdio/tauri-service` verlangt `withGlobalTauri: true`** (Abschnitt 7 vs. Abschnitt 20). Gelöst ohne Abweichung von Abschnitt 20: nur der eingebettete WebDriver, kein JavaScript-Plugin (ADR 0003).
2. **Erste Migration vs. „keine Tabellen auf Vorrat“** (Phase 1 vs. Abschnitte 2 und 18). Gelöst mit genau einer Tabelle, `application_settings` (ADR 0004).
3. **Isolation Pattern vs. `frame-src 'none'`** (beides Abschnitt 20). Das Isolation Pattern braucht einen Iframe; Tauri ergänzt nur `default-src`, und `frame-src 'none'` verbietet den Iframe. Entscheidung: nicht aktivieren (ADR 0005).
4. **Tastaturtest mit eingebettetem WebDriver**: synthetische `Tab`-Ereignisse bewegen den Fokus nicht. Kein Widerspruch im Auftrag, aber eine Testgrenze für Phase 3 (ADR 0003).
