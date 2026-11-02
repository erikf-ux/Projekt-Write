# Project Write

Lokale, plattformübergreifende Schreib- und Wissensmanagement-Anwendung für Windows, macOS und Linux. Keine Cloud, keine Konten, keine Telemetrie.

> Stand: **Phase 1 – Grundgerüst**. Der vollständige Auftrag steht in [`docs/master-prompt.md`](docs/master-prompt.md), der aktuelle Stand in [`docs/status.md`](docs/status.md).

## Voraussetzungen

- Rust (stable, ≥ 1.88), Node.js 22
- Linux: `libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev` (für E2E zusätzlich `xvfb`)
- Windows: WebView2 (in Windows 10/11 enthalten), Visual Studio Build Tools
- macOS: Xcode Command Line Tools

## Befehle

| Zweck                                          | Befehl                                                                     |
| ---------------------------------------------- | -------------------------------------------------------------------------- |
| Abhängigkeiten installieren                    | `npm ci`                                                                   |
| App im Entwicklungsmodus                       | `npm run tauri dev`                                                        |
| Alle Frontend-Prüfungen                        | `npm run check` (Prettier, ESLint, TypeScript, Vitest)                     |
| Rust-Tests (erzeugt auch die TypeScript-Typen) | `cargo test --manifest-path src-tauri/Cargo.toml`                          |
| Debug-Build für E2E                            | `npm run build:e2e`                                                        |
| E2E-Tests                                      | `npm run test:e2e` (Linux ohne Bildschirm: `xvfb-run -a npm run test:e2e`) |
| Lizenzliste aktualisieren                      | `node scripts/third-party-licenses.mjs`                                    |
| Release-Binary                                 | `npx tauri build --no-bundle`                                              |

## Dokumentation

- [Architektur](docs/architecture.md)
- [Datenbank](docs/database.md)
- [Abhängigkeiten](docs/dependencies.md)
- [Architekturentscheidungen](docs/adr/README.md)
- [Changelog](CHANGELOG.md)
- [Lizenzen Dritter](THIRD-PARTY-LICENSES)

## Datenschutz

Project Write arbeitet vollständig lokal. Die Bibliothek ist in Version 1.0 nicht verschlüsselt; empfohlen wird die Datenträgerverschlüsselung des Betriebssystems (BitLocker, FileVault, LUKS).
