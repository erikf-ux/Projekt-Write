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

Letzter vollständiger CI-Lauf: **Run 19 (Commit `9c750a1`) – auf allen drei Systemen grün.**

| Prüfung                                                             | Linux     | Windows   | macOS     |
| ------------------------------------------------------------------- | --------- | --------- | --------- |
| Prettier, ESLint, TypeScript                                        | bestanden | bestanden | bestanden |
| Vitest (66)                                                         | bestanden | bestanden | bestanden |
| rustfmt, Clippy `-D warnings` (mit/ohne `e2e`)                      | bestanden | bestanden | bestanden |
| Rust-Tests (41)                                                     | bestanden | bestanden | bestanden |
| Erzeugte Typen / Lizenzliste aktuell                                | bestanden | bestanden | bestanden |
| E2E-Build                                                           | bestanden | bestanden | bestanden |
| E2E: App startet, Startbildschirm                                   | bestanden | bestanden | bestanden |
| E2E: CSP (Inline-Skript, Webbild)                                   | bestanden | bestanden | bestanden |
| E2E: `fs`/`shell`/`sql`/`path`/`opener` abgelehnt                   | bestanden | bestanden | bestanden |
| E2E: Kontrast im Start-Theme (Pixel)                                | bestanden | bestanden | bestanden |
| E2E: Tabulator-Reihenfolge, Fokusring ≥ 2 px, 3:1 (Pixel)           | bestanden | bestanden | bestanden |
| E2E: Kontrast nach Wechsel zu Dark (Pixel)                          | bestanden | bestanden | bestanden |
| E2E: Kontrast nach Wechsel zu Light (Pixel)                         | bestanden | bestanden | bestanden |
| Release ohne Testplugins (Job auf Linux)                            | bestanden | –         | –         |
| `cargo audit`, `cargo deny`, `npm audit --omit=dev` (Job auf Linux) | bestanden | –         | –         |

Zusätzlich lokal (Linux, Xvfb): alle Tests bestanden; Release-Binary 4,96 MB; `cargo check --release --features e2e` bricht wie vorgesehen ab; Positivkontrolle des Testplugin-Scans (E2E-Binary enthält die Kennung 78-mal); E2E schreibt nichts außerhalb des Projektordners (Linux geprüft); pixelbasierte Kontrastmessung in Chromium bei 1- und 2-facher Skalierung gegen die Token-Werte geprüft (exakt).

### Behobene Fehler während der Phase

| Befund                                                                  | Ursache                                                                                          | Lösung                                                                                                   |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| Lizenzliste in CI abweichend                                            | `CARGO_TERM_COLOR=always` färbte `cargo tree`                                                    | `--color never`, ANSI-Codes entfernt, `cargo` ohne Shell                                                 |
| Windows: Test-Executables starten nicht (`STATUS_ENTRYPOINT_NOT_FOUND`) | Common-Controls-v6-Manifest nur im App-Binary                                                    | Manifest per `build.rs` für alle Ziele gelinkt                                                           |
| Windows: IPC-Tests abgelehnt                                            | Lokaler Ursprung dort `http://tauri.localhost`                                                   | Ursprung je Plattform; neuer Test: fremde Ursprünge werden abgelehnt                                     |
| Windows: E2E-Worker stürzt ab                                           | Worker löschte den WebView2-Ordner der laufenden App                                             | Aufräumen nur im Launcher                                                                                |
| macOS/Windows: veraltete Farben im E2E (F-1, F-2)                       | CI-Fenster rendern verzögert; `getComputedStyle` und einzelne Screenshots zeigten Zwischenstände | Farben werden an einem eingeschwungenen Screenshot gemessen (Pixel). Die Farben der App waren nie falsch |

## Abnahmekriterien Phase 1

| Kriterium                                                                 | Stand                                                                                                          |
| ------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| App startet auf allen drei Systemen, WebdriverIO-Rauchtest grün           | erfüllt (CI-Lauf 19)                                                                                           |
| Testbefehl: typisiertes Ergebnis bzw. einheitliches Fehlerformat          | erfüllt (alle drei Systeme)                                                                                    |
| Migration auf leerer DB, zweiter Lauf ändert nichts, Fremdschlüssel aktiv | erfüllt (alle drei Systeme)                                                                                    |
| FTS5 aktiv, beide Tabellen, `"C++"`                                       | erfüllt (alle drei Systeme)                                                                                    |
| CSP: Inline-Skript läuft nicht, Webbild lädt nicht                        | erfüllt (alle drei Systeme)                                                                                    |
| Frontend-Zugriff auf Dateisystem-Befehle abgelehnt                        | erfüllt (alle drei Systeme)                                                                                    |
| Startbildschirm erfüllt in Dark und Light die Kontrastwerte               | erfüllt: Token-Paare (Unit-Tests) und gerenderte Pixel in Start-Theme, Dark und Light (E2E, alle drei Systeme) |
| ADR zum Isolation Pattern                                                 | erfüllt (ADR 0005, mit Versuch)                                                                                |

Abweichung, offen benannt: Der Fokusring wird unter macOS 3 px statt der in `index.css` gesetzten 2 px breit gezeichnet. Der Test verlangt deshalb „mindestens 2 px“ statt exakt 2 px.

## Was konnte nicht getestet werden und warum?

- Echte Tab-Navigation: Der eingebettete WebDriver erzeugt nur synthetische Tastatureingaben (ADR 0003).
- `cargo audit`/`cargo deny` lokal: Ausführen heruntergeladener Binärdateien wurde vom Berechtigungssystem abgelehnt; in CI ausgeführt und bestanden.
- Messungen auf dem Referenzgerät: nicht verfügbar; Leistungswerte sind Thema von Phase 14.
- Unter macOS lässt sich der WebKit-Datenordner der E2E-Tests nicht in den Projektordner umlenken (ADR 0003).

## Offene Fehler

Keine.

## Gefundene Widersprüche im Auftrag

1. `@wdio/tauri-service` verlangt `withGlobalTauri: true` (Abschnitt 7 vs. 20) – gelöst ohne Abweichung (ADR 0003).
2. „Erste Migration“ vs. „keine Tabellen auf Vorrat“ (Phase 1 vs. Abschnitte 2/18) – gelöst mit genau einer Tabelle (ADR 0004).
3. Isolation Pattern vs. `frame-src 'none'` (beides Abschnitt 20) – Isolation Pattern nicht aktiviert (ADR 0005).

## Entscheidungen (ADR)

0001 Versionen (TypeScript 6 statt 7, WebdriverIO 9 statt 10, SQLx ohne `load-extension`), 0002 IPC-Vertrag, 0003 E2E ohne `withGlobalTauri`, 0004 In-Memory-Datenbank bis zur Bibliothek, 0005 Isolation Pattern nicht aktiviert.

## Entscheidungen des Auftraggebers

- Bundle-Kennung `app.projectwrite.desktop` bestätigt (Nachtrag in ADR 0001).
- App-Icon: Schreibmaschine nach Vorlage des Auftraggebers, größte Größe 256 × 256 px (nichts hochskaliert). Das Windows-Logo der Vorlage wurde entfernt (Markenrecht, Icon erscheint auf allen drei Systemen); Herkunft und Bearbeitung in `docs/dependencies.md`.

## Nächster Schritt

Nach Freigabe: Phase 2 (Speicherkern) – Bibliothek anlegen/öffnen/sperren/umziehen, atomares Speichern, vollständiges Schema in `docs/database.md`, Einstellungen, Dokument-Repository, Referenzbibliothek-Skript.
