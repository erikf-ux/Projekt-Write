# Architektur – Project Write

Stand: Phase 1 (Grundgerüst), 2. November 2026. Entscheidungen im Detail stehen in `docs/adr/`.

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
│  SQLite (mitgebaut, SQLx)              Dateisystem (ab Phase 2, src/filesystem)   │
└───────────────────────────────────────────────────────────────────────────────────┘
```

Keine Schicht wird übersprungen (Abschnitt 8). ESLint verbietet `@tauri-apps/api/core` außerhalb von `src/services/ipc.ts` und jedes `@tauri-apps/plugin-*` im Frontend.

## 2. Laufzeit

1. `main.rs` ruft `project_write_lib::run()`.
2. `run()` baut die Tauri-App. Im Debug-Build mit Feature `e2e` kommt der eingebettete WebDriver hinzu (ADR 0003), sonst keine Plugins.
3. `setup` erzeugt den `AppState`. In Phase 1 öffnet er eine In-Memory-Datenbank und migriert sie (ADR 0004).
4. Das Fenster `main` lädt das gebaute Frontend aus `dist/` über das Tauri-Protokoll. Tauri setzt dabei die CSP als Header und ergänzt Hashes für eigene Skripte.
5. `main.tsx` setzt die Farb-Tokens (System-Theme), rendert den Startbildschirm, dieser ruft `system_check` auf.

## 3. Backend (`src-tauri/`)

| Datei / Modul                                       | Aufgabe                                                                                                                                                                                                   | Phase        |
| --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| `src/lib.rs`                                        | App-Aufbau, Plugin- und Befehlsregistrierung, `compile_error!`-Schutz gegen Testplugins im Release                                                                                                        | 1            |
| `src/state.rs`                                      | `AppState` (geteilte Dienste)                                                                                                                                                                             | 1            |
| `src/error.rs`                                      | `ErrorCode`, `CommandError` – einheitliches Fehlerformat                                                                                                                                                  | 1            |
| `src/commands/mod.rs`                               | `COMMAND_NAMES`, `handler()` mit `generate_handler!`                                                                                                                                                      | 1            |
| `src/commands/input.rs`                             | `Input<T>` und `Validate` (ADR 0002)                                                                                                                                                                      | 1            |
| `src/commands/system.rs`                            | Befehl `system_check`                                                                                                                                                                                     | 1            |
| `src/services/system.rs`                            | Systemprüfung: Datenspeicher, Schema, Fremdschlüssel, Volltextsuche                                                                                                                                       | 1            |
| `src/database/mod.rs`                               | `Database`: Schreibverbindung + Lese-Pool, Pragmas, Migrationen                                                                                                                                           | 1            |
| `src/database/diagnostics.rs`                       | Lesende Prüfungen (`quick_check`, `integrity_check`, Pragmas, Compile-Optionen)                                                                                                                           | 1            |
| `src/database/fts.rs`                               | FTS5-Definitionen aus Abschnitt 19, `quote_term`, FTS5-Probe                                                                                                                                              | 1 (Suche: 9) |
| `migrations/`                                       | Einziger Ort für Migrationen, eingebettet mit `sqlx::migrate!`                                                                                                                                            | 1            |
| `capabilities/main-window.json`                     | Einzige Capability: genau die eigenen Befehle                                                                                                                                                             | 1            |
| `build.rs`, `windows-app-manifest.xml`              | App-Manifest: jeder eigene Befehl ist ohne Capability verboten. Unter Windows wird das Common-Controls-v6-Manifest in App **und** Test-Executables gelinkt (sonst `STATUS_ENTRYPOINT_NOT_FOUND` in Tests) | 1            |
| `src/filesystem/`, `src/security/`, `src/language/` | entstehen in Phase 2, 2/8 und 12                                                                                                                                                                          | –            |

## 4. Frontend (`src/`)

| Pfad                                                 | Aufgabe                                                                                        |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `main.tsx`                                           | Einstieg, Theme vor dem ersten Rendern                                                         |
| `app/App.tsx`, `app/StartScreen.tsx`, `app/Logo.tsx` | App-Hülle und Startbildschirm                                                                  |
| `app/systemStatusStore.ts`                           | Zustand (Zustand-Store) der Systemprüfung                                                      |
| `services/ipc.ts`                                    | Typisierte Befehlsaufrufe, Fehlernormalisierung                                                |
| `services/system.ts`                                 | Aufruf `system_check`                                                                          |
| `i18n/`                                              | Wörterbücher `de.json`, `en.json`, `translate`, `useT`, Spracherkennung                        |
| `modules/themes/`                                    | Design-Tokens, Kontrastberechnung, Anwenden als CSS-Variablen                                  |
| `types/generated/`                                   | Aus Rust erzeugte Typen (`ts-rs`), nie von Hand ändern                                         |
| `test/setup.ts`                                      | Vitest-Setup (jsdom)                                                                           |
| `index.css`                                          | Tailwind 4, Zuordnung Tokens → Utility-Farben, Basisstile, Fokusring, `prefers-reduced-motion` |

**Theming:** `modules/themes/tokens.ts` ist die einzige Quelle der Farben. `applyTheme` erzeugt daraus einmal ein `<style>` mit Regeln `[data-theme="dark"|"light"] { --pw-color-*: …; color-scheme: … }` und schaltet nur noch das Attribut `data-theme` an `<html>` um. Tailwind-Klassen wie `bg-surface` oder `text-muted` verweisen per `@theme inline` auf die Variablen. Hintergrund: Das direkte Umschreiben der Variablen als Inline-Stil an `<html>` hinterließ in WebKit unter macOS veraltete berechnete Farben (im E2E-Test belegt); das Umschalten über Selektoren wird von allen Engines zuverlässig neu berechnet. Importierbare Theme-Dateien (Phase 3) liefern andere Werte für dieselben Tokens.

**Sprache:** Deutsch und Englisch, standardmäßig nach `navigator.languages`. Fehlermeldungen kommen als `messageKey` aus Rust und werden im Frontend übersetzt. Ein Rust-Test prüft, dass jeder `messageKey` in beiden Sprachdateien steht.

## 5. IPC-Vertrag

Siehe ADR 0002. Kurzfassung:

- Aufruf: `invoke(name, { request })`. `request` ist ein aus Rust erzeugter Typ.
- Erfolg: aus Rust erzeugter Ergebnistyp.
- Fehler: immer `CommandError { code: ErrorCode, messageKey: string, details?: Record<string, string> }`.
- `ErrorCode`: `INVALID_INPUT`, `DATABASE`, `SCHEMA_TOO_NEW`, `INTERNAL`, `IPC_REJECTED` (nur Frontend, für Ablehnungen durch Tauri selbst).

| Befehl         | Request                                                                      | Response                                                                                | Phase |
| -------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ----- |
| `system_check` | `SystemCheckRequest { checks: SystemCheckId[] }` (nicht leer, ohne Doppelte) | `SystemCheckResponse { appVersion, platform, sqliteVersion, schemaVersion, results[] }` | 1     |

## 6. Sicherheitsmodell (Abschnitt 20)

| Maßnahme                            | Umsetzung                                                                   | Geprüft durch                                                                                                         |
| ----------------------------------- | --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| CSP exakt nach Abschnitt 20         | `tauri.conf.json` → `app.security.csp`                                      | `tests/security_config.rs` (Wortlaut), E2E (Inline-Skript läuft nicht, Webbild lädt nicht, `securitypolicyviolation`) |
| Kein globales Tauri-Objekt          | `withGlobalTauri: false`                                                    | Konfigurationstest, E2E (`window.__TAURI__` undefiniert)                                                              |
| Eingefrorene Prototypen             | `freezePrototype: true`                                                     | Konfigurationstest, E2E (`Object.isFrozen(Object.prototype)`)                                                         |
| Asset-Protokoll aus                 | `assetProtocol.enable: false`                                               | Konfigurationstest                                                                                                    |
| Keine Plugin-Rechte                 | Capability enthält nur `allow-system-check`; keine Plugins außer Testplugin | Konfigurationstest, E2E (`fs`, `shell`, `sql`, `path`, `opener` abgelehnt)                                            |
| IPC nur von eigenen Seiten          | keine `remote`-Capability; Tauri lehnt fremde Ursprünge ab                  | `tests/ipc.rs` (`https://example.com` u. a. abgelehnt), Konfigurationstest                                            |
| Eigene Befehle nur nach Freigabe    | App-Manifest in `build.rs`                                                  | Konfigurationstest                                                                                                    |
| Eingaben validiert                  | `Input<T>` + `Validate`, `deny_unknown_fields`                              | `tests/ipc.rs`                                                                                                        |
| Keine internen Details in Fehlern   | `CommandError` ohne Quelltext; Frontend verwirft fremde Fehlertexte         | Unit-Tests Rust und Vitest                                                                                            |
| Testplugins nur im Debug-Build      | Feature `e2e`, `compile_error!`, CI-Job `release-check`                     | CI                                                                                                                    |
| Kein dynamisches SQL ohne Prüfmarke | SQLx 0.9 `SqlSafeStr`                                                       | Compiler                                                                                                              |
| Verbotene Abhängigkeiten            | kein `rusqlite`, `tauri-plugin-sql/fs/shell/http`                           | Konfigurationstest, `cargo deny` (`[bans]`)                                                                           |
| Lieferkette                         | `cargo audit`, `cargo deny`, `npm audit`                                    | CI-Job `audit`                                                                                                        |
| Isolation Pattern                   | nicht aktiviert, siehe ADR 0005                                             | ADR 0005                                                                                                              |

Die Bibliothek ist in Version 1.0 nicht verschlüsselt. Die Dokumentation für Nutzer (Phase 14) empfiehlt die Datenträgerverschlüsselung des Systems.

## 7. Datenhaltung

Siehe `docs/database.md` und ADR 0004. Phase 1: In-Memory-Datenbank zur Laufzeit, Dateidatenbanken in Tests. Pragmas: `foreign_keys=ON`, `synchronous=FULL`, `busy_timeout=5000`, `journal_mode=WAL` (lokal) oder `DELETE` (Sync-Ordner, ab Phase 2).

## 8. Tests und CI

| Ebene            | Werkzeug                                                                     | Ort                   | Inhalt Phase 1                                                                                                     |
| ---------------- | ---------------------------------------------------------------------------- | --------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Rust-Unit        | `cargo test`                                                                 | `src-tauri/src/**`    | Fehlerformat, `Input<T>`, Validierung, FTS-Quoting, Systemprüfung                                                  |
| Rust-Integration | `cargo test`                                                                 | `src-tauri/tests/`    | Migrationen, Fremdschlüssel, Pragmas, FTS5, neuere Schemata, IPC über Tauri-Mock-Runtime, Sicherheitskonfiguration |
| Frontend         | Vitest + jsdom + `mockIPC`                                                   | `src/**/*.test.ts(x)` | Kontrast aller Token-Paare, i18n-Vollständigkeit, IPC-Wrapper, Startbildschirm                                     |
| E2E              | WebdriverIO + `@wdio/tauri-service` (embedded)                               | `tests/e2e/`          | Start, CSP, abgelehnte Zugriffe, Kontrast im echten Rendering (Dark und Light), Fokus                              |
| Statisch         | ESLint (strict-type-checked), `tsc`, Prettier, rustfmt, Clippy `-D warnings` | –                     | –                                                                                                                  |

CI (`.github/workflows/ci.yml`): Job `checks` auf `ubuntu-24.04`, `windows-latest`, `macos-latest` (Format, Lint, Typen, Vitest, Rust-Tests, erzeugte Typen aktuell, Lizenzliste aktuell, E2E-Build, E2E); Job `release-check` (Linux); Job `audit` (Linux).

Tests schreiben nur in temporäre Ordner innerhalb des Projekts (`src-tauri/target/tmp/`, `tests/e2e/.tmp/`).

## 9. Projektstruktur Phase 1

```text
.
├── .github/workflows/ci.yml
├── docs/                      Auftrag, Status, Architektur, Datenbank, Abhängigkeiten, ADRs
├── scripts/                   Lizenzliste, Release-Prüfung
├── src/
│   ├── app/                   App-Hülle, Startbildschirm, Store
│   ├── i18n/                  de.json, en.json, index.ts
│   ├── modules/themes/        Tokens, Kontrast, applyTheme
│   ├── services/              ipc.ts, system.ts
│   ├── test/                  Vitest-Setup
│   └── types/generated/       ts-rs-Ausgabe
├── src-tauri/
│   ├── capabilities/          main-window.json
│   ├── icons/
│   ├── migrations/            0001_application_settings.sql
│   ├── src/                   commands/, database/, services/, error.rs, state.rs, lib.rs, main.rs
│   └── tests/                 database.rs, ipc.rs, security_config.rs
└── tests/e2e/                 wdio.conf.ts, specs/smoke.e2e.ts
```

Abweichungen von Abschnitt 23 (klein, ohne eigene ADR): `src-tauri/src/error.rs` und `state.rs` liegen auf oberster Ebene, weil alle Schichten sie nutzen; `src/test/` enthält das Vitest-Setup; `src/app/` enthält den Startbildschirm, bis Phase 3 das Hauptlayout in `src/components/layout/` baut. Leere Zielordner (`components/`, `modules/editor/` …) entstehen erst in ihren Phasen.

## 10. Prüfung der Gesamtarchitektur (alle Phasen)

Ergebnis der Prüfung des gesamten Auftrags auf technische Machbarkeit. Risiken werden in der genannten Phase erneut bewertet.

| Bereich                              | Bewertung                                                                                            | Risiko / Maßnahme                                                                                                                                                          | Phase |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- |
| Tauri 2 + strikte CSP                | machbar, in Phase 1 belegt                                                                           | Jede spätere Lockerung braucht eine ADR                                                                                                                                    | alle  |
| SQLite + FTS5 + Trigram              | machbar, in Phase 1 belegt (`unicode61 remove_diacritics 2`, `trigram remove_diacritics 1`, `"C++"`) | `ß`/`ss` und Umlaut-Umschreibungen müssen in Rust erweitert werden (Abschnitt 19)                                                                                          | 9     |
| Speicherkern                         | machbar                                                                                              | Atomares Umbenennen unter Windows (`MoveFileEx` mit `REPLACE_EXISTING`), `fsync` des Ordners unter Unix; 100 harte Abbrüche als Kindprozess-Test                           | 2     |
| Einzelinstanz                        | `tauri-plugin-single-instance` 2.5.5 (stabil) + `library.lock`                                       | Lock-Datei muss nach Absturz erkennbar veraltet sein (PID/Zeitstempel)                                                                                                     | 2, 5  |
| Logging                              | noch nicht vorhanden                                                                                 | Nur Rust-seitig, rotierend, ohne Inhalte; `tauri-plugin-log` nur ohne Frontend-Rechte                                                                                      | 2     |
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
