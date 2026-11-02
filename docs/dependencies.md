# Abhängigkeiten – Project Write

Stand: 2. November 2026 (Phase 1). Jede neue Abhängigkeit wird hier mit Zweck, Version und Lizenz begründet (Abschnitt 2). Versionen sind exakt festgelegt (`.npmrc`: `save-exact=true`, Lockfiles committet). Kompatibilitätsentscheidungen: ADR 0001.

## Werkzeuge der Entwicklungsumgebung

| Werkzeug | Version         | Hinweis                               |
| -------- | --------------- | ------------------------------------- |
| Rust     | 1.97.0 (stable) | Edition 2024, `rust-version = "1.88"` |
| Node.js  | 22.x (LTS)      | getestet mit 22.23.4                  |
| npm      | 10.x            |                                       |

## Rust – Laufzeit (`src-tauri/Cargo.toml`, `[dependencies]`)

| Crate                         | Version | Lizenz            | Zweck                                                                                                                                       |
| ----------------------------- | ------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `tauri`                       | 2.12.5  | Apache-2.0 OR MIT | Desktop-Rahmen, Webview, IPC (stabile Linie 2.x, nicht 3.0-alpha)                                                                           |
| `serde`                       | 1.0.229 | MIT OR Apache-2.0 | Serialisierung der Befehlsdaten                                                                                                             |
| `serde_json`                  | 1.0.154 | MIT OR Apache-2.0 | JSON für IPC und Konfigurationstests                                                                                                        |
| `log`                         | 0.4.35  | MIT OR Apache-2.0 | Protokoll-Fassade (Ausgabeziel folgt in Phase 2)                                                                                            |
| `sqlx`                        | 0.9.0   | MIT OR Apache-2.0 | SQLite-Zugriff, Pools, Migrationen. Features: `runtime-tokio`, `sqlite-bundled`, `migrate`, `macros` – bewusst ohne `sqlite-load-extension` |
| `thiserror`                   | 2.0.21  | MIT OR Apache-2.0 | Fehlertypen                                                                                                                                 |
| `ts-rs`                       | 12.0.1  | MIT               | Erzeugt TypeScript-Typen aus Rust-Typen (Abschnitt 8)                                                                                       |
| `tauri-plugin-wdio-webdriver` | 1.5.1   | MIT               | **Nur Feature `e2e`, nur Debug.** Eingebetteter WebDriver-Server für E2E-Tests (ADR 0003)                                                   |

## Rust – Build und Tests

| Crate                    | Version | Lizenz            | Zweck                                    |
| ------------------------ | ------- | ----------------- | ---------------------------------------- |
| `tauri-build`            | 2.7.4   | Apache-2.0 OR MIT | Build-Skript, App-Manifest, Capabilities |
| `tauri` (Feature `test`) | 2.12.5  | Apache-2.0 OR MIT | Mock-Runtime für IPC-Integrationstests   |
| `tempfile`               | 3.27.0  | MIT OR Apache-2.0 | Temporäre Testordner in `target/`        |
| `tokio`                  | 1.x     | MIT               | `#[tokio::test]`                         |

## npm – Laufzeit (`dependencies`)

| Paket                | Version | Lizenz            | Zweck                                                                         |
| -------------------- | ------- | ----------------- | ----------------------------------------------------------------------------- |
| `@tauri-apps/api`    | 2.12.4  | Apache-2.0 OR MIT | `invoke` (nur in `src/services/ipc.ts`), `mockIPC` in Tests                   |
| `react`, `react-dom` | 19.3.1  | MIT               | Oberfläche                                                                    |
| `zustand`            | 5.0.16  | MIT               | Zustandsverwaltung (Abschnitt 7)                                              |
| `lucide-react`       | 1.66.0  | ISC               | Lokale Symbolbibliothek (Abschnitt 11); nur genutzte Symbole landen im Bundle |

## npm – Entwicklung (`devDependencies`)

| Paket                                                                                            | Version | Lizenz            | Zweck                                                                             |
| ------------------------------------------------------------------------------------------------ | ------- | ----------------- | --------------------------------------------------------------------------------- |
| `@tauri-apps/cli`                                                                                | 2.12.4  | Apache-2.0 OR MIT | `tauri build`, `tauri icon`                                                       |
| `vite`                                                                                           | 8.3.5   | MIT               | Dev-Server und Bundler                                                            |
| `@vitejs/plugin-react`                                                                           | 6.2.1   | MIT               | JSX/React für Vite                                                                |
| `tailwindcss`, `@tailwindcss/vite`                                                               | 4.3.3   | MIT               | Styling über Design-Tokens                                                        |
| `typescript`                                                                                     | 6.0.3   | Apache-2.0        | Typprüfung im Strict-Modus (nicht 7.x, ADR 0001)                                  |
| `eslint`                                                                                         | 10.14.0 | MIT               | Lint                                                                              |
| `@eslint/js`                                                                                     | 10.0.1  | MIT               | ESLint-Grundregeln                                                                |
| `typescript-eslint`                                                                              | 8.72.1  | MIT               | Typbasierte Lint-Regeln (`strictTypeChecked`)                                     |
| `eslint-plugin-react-hooks`                                                                      | 7.1.1   | MIT               | Hook-Regeln                                                                       |
| `globals`                                                                                        | 17.14.0 | MIT               | Globale Namen für ESLint                                                          |
| `prettier`                                                                                       | 3.9.10  | MIT               | Formatierung                                                                      |
| `vitest`                                                                                         | 5.0.5   | MIT               | Frontend-Tests                                                                    |
| `jsdom`                                                                                          | 30.1.3  | MIT               | DOM für Vitest                                                                    |
| `@testing-library/react`                                                                         | 16.3.3  | MIT               | Komponententests                                                                  |
| `@testing-library/dom`                                                                           | 10.4.2  | MIT               | Peer von `@testing-library/react`                                                 |
| `@testing-library/jest-dom`                                                                      | 7.0.1   | MIT               | DOM-Matcher                                                                       |
| `@types/react`, `@types/react-dom`                                                               | 19.3.0  | MIT               | Typen                                                                             |
| `@types/node`                                                                                    | 22.20.5 | MIT               | Typen für Konfigurationsdateien und Skripte                                       |
| `@types/mocha`                                                                                   | 10.0.10 | MIT               | Typen für E2E-Specs                                                               |
| `webdriverio`, `@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter` | 9.32.0  | MIT               | E2E-Testlauf (9.x, ADR 0001)                                                      |
| `@wdio/globals`                                                                                  | 9.31.3  | MIT               | `browser`, `$`, `expect` in E2E-Specs                                             |
| `@wdio/tauri-service`                                                                            | 1.5.1   | MIT               | Startet die App und verbindet den eingebetteten WebDriver (Abschnitt 7, ADR 0003) |

## Systempakete (Linux-Build)

`libwebkit2gtk-4.1-dev`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`; für E2E zusätzlich `xvfb`.

## Grafiken

| Datei                                                   | Herkunft                                                               | Bearbeitung                                                                                                                                                                                                                 |
| ------------------------------------------------------- | ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src-tauri/icons/app-icon.png` (Quelle aller App-Icons) | Vom Auftraggeber bereitgestellt, 2. November 2026 (JPEG, 627 × 627 px) | Windows-Logo auf dem Schreibmaschinengehäuse entfernt (Marke von Microsoft, wäre auch im macOS- und Linux-Icon erschienen); Kachel quadratisch zugeschnitten, auf 1024 × 1024 px skaliert, Ecken abgerundet und transparent |
| `src-tauri/icons/*.png`, `icon.ico`, `icon.icns`        | Erzeugt aus `app-icon.png`                                             | `npx tauri icon src-tauri/icons/app-icon.png -o src-tauri/icons`, danach die Varianten für Android, iOS und den Microsoft Store löschen                                                                                     |

Die Quelle ist von 627 auf 1024 px hochskaliert; in sehr großen Darstellungen (macOS 512 pt @2x) kann sie leicht weich wirken. Eine höher aufgelöste Vorlage würde das beheben.

## Lizenzen

- Vollständige Liste aller ausgelieferten Komponenten: `THIRD-PARTY-LICENSES`, erzeugt mit `node scripts/third-party-licenses.mjs`. CI prüft, dass die Datei aktuell ist.
- Phase 1 liefert keine Wörterbücher aus.
- **Hinweis für D4:** Über Tauri (HTML-/CSS-Parser in `tauri-utils`) kommen fünf Crates unter **MPL-2.0**: `cssparser`, `cssparser-macros`, `dtoa-short`, `selectors`, `option-ext`. MPL-2.0 ist eine schwache Copyleft-Lizenz auf Dateiebene: Bleiben die Dateien unverändert, genügt der Hinweis auf den Quellcode. Kein Konflikt mit einem proprietären oder freien Lizenzmodell bekannt, aber vor Phase 14 mit D4 zu bestätigen.
- `cargo deny` erlaubt: MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0 (`src-tauri/deny.toml`).

## Audit-Stand (2. November 2026)

| Prüfung                         | Ergebnis                      | Bewertung                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------------------------- | ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `npm audit --omit=dev`          | 0 Funde                       | Ausgelieferte npm-Pakete sind unauffällig. In CI blockierend                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `npm audit` (inkl. Entwicklung) | 40 Funde (26 mittel, 14 hoch) | Alle in Entwicklungswerkzeugen: WebdriverIO 9 (`@puppeteer/browsers`, `extract-zip`, `basic-ftp`, `glob`, `archiver`), Mocha (`serialize-javascript`, `minimatch`), ESLint/typescript-eslint (`minimatch`). Nichts davon wird ausgeliefert. Die Funde betreffen Denial-of-Service durch präparierte Muster, Pfadangriffe beim Entpacken fremder Archive und FTP-Antworten – Eingaben, die die Werkzeuge in diesem Projekt nicht aus fremden Quellen erhalten. Ein Erzwingen neuer Hauptversionen per `overrides` würde die Werkzeuge brechen. In CI nur berichtet, Neubewertung bei jedem Update von WebdriverIO und ESLint |
| `cargo audit`                   | lokal nicht ausgeführt        | Läuft in CI (Job `audit`). Lokal wurde das Ausführen heruntergeladener Binärdateien vom Berechtigungssystem dieser Umgebung abgelehnt                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `cargo deny`                    | lokal nicht ausgeführt        | wie oben                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
