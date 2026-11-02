# Changelog

Alle wesentlichen Änderungen an Project Write. Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionen nach [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Phase 1 – Grundgerüst

#### Hinzugefügt

- Tauri-2-Anwendung mit React 19, TypeScript 6 (strict), Vite 8, Tailwind CSS 4 und Zustand.
- Startbildschirm im Project-Write-Design mit Systemprüfung (Datenspeicher, Datenbankschema, Datenverknüpfungen, Volltextsuche), Dark und Light passend zum System, Deutsch und Englisch.
- Design-Tokens aus Abschnitt 11 als einzige Farbquelle, Kontrastprüfung aller verwendeten Paare. Themes werden über das Attribut `data-theme` und daraus erzeugte CSS-Regeln umgeschaltet.
- Sichere Kommunikationsschnittstelle: `Input<T>` mit Pflicht-Validierung, einheitliches Fehlerformat `{ code, messageKey, details? }`, aus Rust erzeugte TypeScript-Typen (`ts-rs`), App-Manifest und eine Capability mit genau den eigenen Befehlen.
- Befehl `system_check`.
- SQLite-Anbindung über SQLx 0.9 (mitgebaut): Schreibverbindung und Lese-Pool, `foreign_keys`, `synchronous=FULL`, `busy_timeout`, WAL oder Rollback-Journal, Migrationen mit Ablehnung neuerer Schemata.
- Migration 0001: Tabelle `application_settings`.
- FTS5-Probe mit den Tabellendefinitionen aus Abschnitt 19 und Quoting von Suchbegriffen.
- Strikte CSP nach Abschnitt 20, `withGlobalTauri: false`, `freezePrototype: true`, Asset-Protokoll aus, `dragDropEnabled: false`.
- Tests: Rust-Unit- und Integrationstests, Vitest, WebdriverIO-Rauchtest mit eingebettetem WebDriver (nur Debug-Build, Feature `e2e`).
- CI auf Windows, macOS und Linux; Prüfung des Release-Builds auf Testplugins; `cargo audit`, `cargo deny`, `npm audit`.
- Dokumentation: Architektur, Datenbank, Abhängigkeiten, ADRs 0001–0005, Lizenzliste.
- Windows: Common-Controls-v6-Manifest wird auch in Test-Executables gelinkt.

#### Behoben

- E2E-Farbmessungen unter macOS und Windows lieferten veraltete Werte, weil die CI-Fenster nicht von selbst rendern. Die Tests messen Farben jetzt an Screenshot-Pixeln (F-1, F-2 in `docs/status.md`).
