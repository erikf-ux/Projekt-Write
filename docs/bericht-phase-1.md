# Bericht Phase 1 – Grundgerüst

Stand: 2. November 2026. Zweig `claude/project-write-phase-1-6sh7n2`. Entwicklungsumgebung: Cloud-Container, Ubuntu 24.04, 4 Kerne, 15 GB RAM (kein Referenzgerät nach Abschnitt 6). CI: GitHub Actions (`ubuntu-24.04`, `windows-latest`, `macos-latest`).

## Was wurde implementiert?

- Tauri-2.12-App mit React 19, TypeScript 6 (strict), Vite 8, Tailwind CSS 4, Zustand 5.
- Sichere Kommunikationsschnittstelle: `Input<T>` mit Pflicht-Validierung und `deny_unknown_fields`, einheitliches Fehlerformat `{ code, messageKey, details? }` auch für Tauri-eigene Ablehnungen (Normalisierung zu `IPC_REJECTED`), aus Rust erzeugte TypeScript-Typen (`ts-rs`), App-Manifest, eine Capability mit genau `allow-system-check`.
- SQLite über SQLx 0.9 (mitgebaut): Schreibverbindung + Lese-Pool, `foreign_keys=ON`, `synchronous=FULL`, `busy_timeout=5000`, WAL bzw. Rollback-Journal, Migrationen mit Ablehnung neuerer Schemata; Migration 0001 (`application_settings`); FTS5-Probe mit den Tabellen aus Abschnitt 19.
- Befehl `system_check` und Startbildschirm mit Systemprüfung (Deutsch/Englisch, Dark/Light nach System).
- CSP exakt nach Abschnitt 20, `withGlobalTauri: false`, `freezePrototype: true`, Asset-Protokoll aus, `dragDropEnabled: false`.
- Tests: 41 Rust-Tests, 66 Vitest-Tests, 7 WebdriverIO-Tests. CI mit Release-Prüfung auf Testplugins und Audits.
- Dokumentation: `architecture.md` (mit Prüfung der Gesamtarchitektur), `database.md`, `dependencies.md`, ADRs 0001–0005, `CHANGELOG.md`, `THIRD-PARTY-LICENSES` (erzeugt).

## Welche Dateien wurden erstellt oder verändert?

Alle Dateien sind neu (leeres Repository). Wichtigste Orte: `src-tauri/src/` (Befehle, Fehler, Datenbank, Dienste), `src-tauri/tests/`, `src-tauri/migrations/`, `src-tauri/capabilities/`, `src-tauri/tauri.conf.json`, `src-tauri/build.rs`, `src/` (App, Dienste, i18n, Themes), `tests/e2e/`, `.github/workflows/ci.yml`, `scripts/`, `docs/`.

## Welche Tests wurden durchgeführt?

Letzter vollständiger CI-Lauf: Run 13 (Commit `39bb969`).

| Prüfung                                                             | Linux     | Windows                  | macOS                    |
| ------------------------------------------------------------------- | --------- | ------------------------ | ------------------------ |
| Prettier, ESLint, TypeScript                                        | bestanden | bestanden                | bestanden                |
| Vitest (66)                                                         | bestanden | bestanden                | bestanden                |
| rustfmt, Clippy `-D warnings` (mit/ohne `e2e`)                      | bestanden | bestanden                | bestanden                |
| Rust-Tests (41)                                                     | bestanden | bestanden                | bestanden                |
| Erzeugte Typen / Lizenzliste aktuell                                | bestanden | bestanden                | bestanden                |
| E2E-Build                                                           | bestanden | bestanden                | bestanden                |
| E2E: App startet, Startbildschirm                                   | bestanden | bestanden                | bestanden                |
| E2E: CSP (Inline-Skript, Webbild)                                   | bestanden | bestanden                | bestanden                |
| E2E: `fs`/`shell`/`sql`/`path`/`opener` abgelehnt                   | bestanden | bestanden                | bestanden                |
| E2E: Kontrast im Start-Theme                                        | bestanden | bestanden                | bestanden                |
| E2E: Tabulator-Reihenfolge, Fokusring 3:1                           | bestanden | **fehlgeschlagen (F-2)** | **fehlgeschlagen (F-2)** |
| E2E: Kontrast nach Wechsel zu Dark                                  | bestanden | **fehlgeschlagen (F-1)** | **fehlgeschlagen (F-1)** |
| E2E: Kontrast nach Wechsel zu Light                                 | bestanden | bestanden                | **fehlgeschlagen (F-1)** |
| Release ohne Testplugins (Job auf Linux)                            | bestanden | –                        | –                        |
| `cargo audit`, `cargo deny`, `npm audit --omit=dev` (Job auf Linux) | bestanden | –                        | –                        |

Zusätzlich lokal (Linux, Xvfb): alle 41 Rust-, 66 Vitest- und 7 E2E-Tests bestanden; Release-Binary 4,96 MB; `cargo check --release --features e2e` bricht wie vorgesehen ab; Positivkontrolle des Testplugin-Scans (E2E-Binary enthält die Kennung 78-mal); E2E schreibt nichts außerhalb des Projektordners (Linux geprüft).

## Abnahmekriterien Phase 1

| Kriterium                                                                 | Stand                                                                                                                                                                                                                       |
| ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| App startet auf allen drei Systemen, WebdriverIO-Rauchtest grün           | **Teilweise.** Start und Kerntests auf allen drei Systemen grün; die Suite ist unter Windows (2 Tests) und macOS (3 Tests) rot wegen F-1/F-2                                                                                |
| Testbefehl: typisiertes Ergebnis bzw. einheitliches Fehlerformat          | erfüllt (Rust-IPC-Tests auf allen drei Systemen)                                                                                                                                                                            |
| Migration auf leerer DB, zweiter Lauf ändert nichts, Fremdschlüssel aktiv | erfüllt (alle drei Systeme)                                                                                                                                                                                                 |
| FTS5 aktiv, beide Tabellen, `"C++"`                                       | erfüllt (alle drei Systeme)                                                                                                                                                                                                 |
| CSP: Inline-Skript läuft nicht, Webbild lädt nicht                        | erfüllt (E2E auf allen drei Systemen)                                                                                                                                                                                       |
| Frontend-Zugriff auf Dateisystem-Befehle abgelehnt                        | erfüllt (E2E auf allen drei Systemen)                                                                                                                                                                                       |
| Startbildschirm erfüllt in Dark und Light die Kontrastwerte               | **Teilweise.** Token-Paare beider Themes erfüllt (Unit-Tests, alle Systeme); gerendertes Start-Theme erfüllt (E2E, alle Systeme); nach Theme-Wechsel zur Laufzeit und Fokusring unter Windows/macOS nicht belegt (F-1, F-2) |
| ADR zum Isolation Pattern                                                 | erfüllt (ADR 0005, mit Versuch)                                                                                                                                                                                             |

## Was konnte nicht getestet werden und warum?

- Echte Tab-Navigation: Der eingebettete WebDriver erzeugt nur synthetische Tastatureingaben (ADR 0003).
- `cargo audit`/`cargo deny` lokal: Ausführen heruntergeladener Binärdateien wurde vom Berechtigungssystem abgelehnt; in CI ausgeführt und bestanden.
- Messungen auf dem Referenzgerät: nicht verfügbar; Leistungswerte sind Thema von Phase 14.
- Darstellung unter Windows/macOS als Bildschirmfoto: Artefakte und Log-Downloads sind aus dieser Umgebung über den Proxy nicht erreichbar.

## Offene Fehler

- **F-1** und **F-2**, siehe `docs/status.md`.

## Gefundene Widersprüche im Auftrag

1. `@wdio/tauri-service` verlangt `withGlobalTauri: true` (Abschnitt 7 vs. 20) – gelöst ohne Abweichung (ADR 0003).
2. „Erste Migration“ vs. „keine Tabellen auf Vorrat“ (Phase 1 vs. Abschnitte 2/18) – gelöst mit genau einer Tabelle (ADR 0004).
3. Isolation Pattern vs. `frame-src 'none'` (beides Abschnitt 20) – Isolation Pattern nicht aktiviert (ADR 0005).

## Entscheidungen (ADR)

0001 Versionen (TypeScript 6 statt 7, WebdriverIO 9 statt 10, SQLx ohne `load-extension`), 0002 IPC-Vertrag, 0003 E2E ohne `withGlobalTauri`, 0004 In-Memory-Datenbank bis zur Bibliothek, 0005 Isolation Pattern nicht aktiviert.

## Rückfragen an den Auftraggeber

1. **F-1/F-2:** Abnahmekriterien „Rauchtest grün auf allen Systemen“ und „Kontrast in Dark und Light“ sind unter Windows/macOS nur teilweise belegt (Abschnitt 2, Regel 6). Vorschlag: Phase 1 mit diesen zwei dokumentierten Fehlern freigeben und die Analyse (mit Bildschirmfotos) an den Anfang von Phase 3 (Themes, Fokus) legen – oder vor Phase 2 weiter untersuchen.
2. Bundle-Kennung `app.projectwrite.desktop` vorläufig bestätigen (endgültig vor Phase 14).

## Nächster Schritt

Nach Freigabe: Phase 2 (Speicherkern) – Bibliothek anlegen/öffnen/sperren/umziehen, atomares Speichern, vollständiges Schema in `docs/database.md`, Einstellungen, Dokument-Repository, Referenzbibliothek-Skript.
