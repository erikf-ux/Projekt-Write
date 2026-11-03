# Project Write

Lokale, plattformübergreifende Schreib- und Wissensmanagement-Anwendung für Windows, macOS und Linux. Keine Cloud, keine Konten, keine Telemetrie.

> Stand: **Phase 2 – Speicherkern**. Der vollständige Auftrag steht in [`docs/master-prompt.md`](docs/master-prompt.md), der aktuelle Stand in [`docs/status.md`](docs/status.md).

## Voraussetzungen

- Rust (stable, ≥ 1.89), Node.js 22
- Linux: `libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev` (für E2E zusätzlich `xvfb` und `dbus`)
- Windows: WebView2 (in Windows 10/11 enthalten), Visual Studio Build Tools
- macOS: Xcode Command Line Tools

## Befehle

| Zweck                                          | Befehl                                                                                                                                                   |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Abhängigkeiten installieren                    | `npm ci`                                                                                                                                                 |
| App im Entwicklungsmodus                       | `npm run tauri dev`                                                                                                                                      |
| Alle Frontend-Prüfungen                        | `npm run check` (Prettier, ESLint, TypeScript, Vitest)                                                                                                   |
| Rust-Tests (erzeugt auch die TypeScript-Typen) | `cargo test --manifest-path src-tauri/Cargo.toml`                                                                                                        |
| Debug-Build für E2E                            | `npm run build:e2e`                                                                                                                                      |
| E2E-Tests                                      | `npm run test:e2e` (Linux ohne Bildschirm: `dbus-run-session -- xvfb-run -a npm run test:e2e`)                                                           |
| Echten Ordnerdialog prüfen (manuell)           | `npx wdio run tests/e2e/manual/wdio.manual.conf.ts` (unter Linux mit `xvfb-run`; speichert ein Bildschirmfoto)                                           |
| Referenzbibliothek erzeugen                    | `node scripts/reference-library.mjs` (Standard: 10.000 Dokumente in `src-tauri/target/reference-library`; `--seed`, `--documents`, `--force`, `--check`) |
| Lizenzliste aktualisieren                      | `node scripts/third-party-licenses.mjs`                                                                                                                  |
| Release-Binary                                 | `npx tauri build --no-bundle`                                                                                                                            |
| App-Icons neu erzeugen                         | `node scripts/app-icons.mjs` (Quelle `src-tauri/icons/app-icon.png`, 256 × 256 px)                                                                       |

## Dokumentation

- [Architektur](docs/architecture.md)
- [Datenbank](docs/database.md)
- [Abhängigkeiten](docs/dependencies.md)
- [Architekturentscheidungen](docs/adr/README.md)
- [Changelog](CHANGELOG.md)
- [Lizenzen Dritter](THIRD-PARTY-LICENSES)

## Die Bibliothek

Project Write speichert Dokumente in einer **Bibliothek**: einem Ordner mit Datenbank und Dateien (Standard `Dokumente/ProjectWrite`). Eine Bibliothek ist immer nur in einer Instanz geöffnet. Liegt sie in einem Sync-Ordner (OneDrive, Dropbox, iCloud Drive, Google Drive) oder auf einem Netzlaufwerk, warnt die App und arbeitet dort im Rollback-Modus der Datenbank; empfohlen ist ein lokaler Ort. Ein Umzug kopiert, prüft und schaltet erst dann um; der alte Ordner bleibt, bis Sie ihn entfernen.

## Datenschutz

Project Write arbeitet vollständig lokal. Die Bibliothek ist in Version 1.0 nicht verschlüsselt; empfohlen wird die Datenträgerverschlüsselung des Betriebssystems (BitLocker, FileVault, LUKS).
