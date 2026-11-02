# Projektstatus – Project Write

Diese Datei ist das Arbeitsgedächtnis. Zu Beginn jeder Sitzung lesen, am Ende aktualisieren.

## Aktuelle Phase

**Phase 1 – Grundgerüst** (Zweig `claude/project-write-phase-1-6sh7n2`): umgesetzt, **wartet auf Freigabe** für Phase 2.

## Stand Phase 1

| Schritt | Inhalt                                                                                                  | Stand                                             |
| ------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| 1       | Auftrag als `docs/master-prompt.md`                                                                     | erledigt                                          |
| 2       | Versionen und Kompatibilität (ADR 0001, `docs/dependencies.md`)                                         | erledigt                                          |
| 3       | Frontend-Gerüst: Vite 8, React 19, TypeScript 6 strict, Tailwind 4, Zustand 5, ESLint, Prettier, Vitest | erledigt                                          |
| 4       | Tauri-2-Gerüst: App-Manifest in `build.rs`, eine Capability, CSP, Härtung                               | erledigt                                          |
| 5       | IPC-Vertrag: `Input<T>` + `Validate`, `CommandError`, `ts-rs` (ADR 0002)                                | erledigt                                          |
| 6       | SQLx-Schicht: Schreibverbindung + Lese-Pool, Pragmas, Migration 0001, FTS5-Probe (ADR 0004)             | erledigt                                          |
| 7       | Befehl `system_check`, Startbildschirm (de/en, Dark/Light)                                              | erledigt                                          |
| 8       | Tests: Rust (41), Vitest (66), WebdriverIO (7)                                                          | erledigt, CI-Lauf 19 grün                         |
| 9       | CI: Windows, macOS, Linux; Release-Prüfung; Audits                                                      | erledigt, CI-Lauf 19 auf allen drei Systemen grün |
| 10      | Doku: Architektur, Datenbank, Abhängigkeiten, ADRs 0001–0005, CHANGELOG, Lizenzliste                    | erledigt                                          |

## Behobene Fehler

| Nr. | Beschreibung                                                                                                                                                                                                                       | Ursache und Lösung                                                                                                                                                                                                                                                                                                                                                                                                     |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F-1 | E2E meldete nach einem Theme-Wechsel zur Laufzeit veraltete Textfarben (macOS, Windows)                                                                                                                                            | Kein Fehler der App. Die CI-Fenster unter macOS und Windows rendern nicht von selbst; `getComputedStyle` lieferte dort Werte von vor der Änderung (CI-Lauf 15), unter macOS teils auch nach einem erzwungenen Frame (CI-Lauf 17). Die gerenderten Pixel stimmten jedes Mal. Die E2E-Tests messen Farben jetzt an Screenshot-Pixeln (ADR 0003)                                                                          |
| F-2 | E2E meldete den Fokusring als Weiß auf Weiß (macOS, Windows)                                                                                                                                                                       | Dieselbe Ursache. Der Fokusring wird jetzt an Pixeln geprüft (≥ 2 px Akzentfarbe, ≥ 3:1 gegen die Umgebung). Windows war damit in CI-Lauf 16 vollständig grün, macOS in Lauf 17 beim Fokusring                                                                                                                                                                                                                         |
| F-3 | `cargo test --manifest-path src-tauri/Cargo.toml` (README, CI) schrieb die erzeugten TypeScript-Typen nach `src-tauri/bindings/`; die CI-Prüfung „Generated TypeScript types are up to date“ verglich deshalb unveränderte Dateien | Cargo sucht Konfiguration ab dem aktuellen Ordner, nicht ab dem Manifest; `src-tauri/.cargo/config.toml` wurde vom Projektstamm aus nicht gelesen. Konfiguration liegt jetzt in `.cargo/config.toml` im Projektstamm; CI löscht `src/types/generated` vor den Rust-Tests, sodass die Prüfung nur besteht, wenn die Typen neu geschrieben wurden. Die bisher eingecheckten Typen waren aktuell (neu erzeugt, identisch) |

## Offene Aufgaben

### Vor bzw. in Phase 2

- Bibliothek anlegen, öffnen, sperren (`tauri-plugin-single-instance` + `library.lock`), umziehen; `AppState` von In-Memory auf Bibliotheksdatenbank umstellen (ADR 0004).
- Vollständiges Datenbankschema planen und in `docs/database.md` dokumentieren.
- Sicherung vor jeder Migration; Migrationstests ab jeder früheren Schemaversion.
- Logging: nur Rust-seitig, lokal, rotierend, ohne Dokumentinhalte (Abschnitt 8).
- Appweite Einstellungen im Konfigurationsordner des Systems.
- `src/filesystem/` und `src/security/` (Pfadprüfung, symbolische Links ablehnen).

### Später

- Echte Tastaturnavigation (Tab) testen: Der eingebettete WebDriver sendet nur synthetische Ereignisse (ADR 0003). Für Phase 3 eine native Eingabeprüfung einplanen.
- Unter macOS schreibt der Webview der E2E-Tests Caches in den Benutzerordner der CI-Maschine (ADR 0003).
- npm-Audit-Funde in Entwicklungswerkzeugen bei jedem Update von WebdriverIO/ESLint neu bewerten (`docs/dependencies.md`).
- TypeScript 7 und WebdriverIO 10, sobald `typescript-eslint` und `@wdio/tauri-service` sie unterstützen (ADR 0001).
- PDF.js und WebAssembly-Decoder vs. CSP prüfen (Phase 8, `docs/architecture.md` Abschnitt 10).
- Isolation Pattern vor Phase 8 und vor Phase 14 neu bewerten (ADR 0005).

### Entscheidungen des Auftraggebers

- macOS vorerst zurückgestellt (2. November 2026): CI läuft nur auf Windows und Linux. Priorität: zuerst Windows, dann Linux. macOS-spezifische Arbeiten ruhen, bis der Auftraggeber sie wieder freigibt.
- Erledigt: Bundle-Kennung `app.projectwrite.desktop` bestätigt; App-Icon festgelegt (Schreibmaschine, größte Größe 256 × 256 px, erzeugt mit `scripts/app-icons.mjs`).
- CI-Lauf 22 (Commit `a89afae`, App-Icon): Linux, Windows, Release-Prüfung und Audits grün. Der macOS-Job bekam zweimal keinen Runner zugeteilt und wurde nach 15 Minuten abgebrochen, ohne einen Schritt auszuführen – kein Codefehler. Das Icon (jetzt 256 × 256 px) unter macOS beim nächsten CI-Lauf prüfen.

Noch offen (vor Phase 14):

- D4 Lizenz der App, inkl. Bewertung der MPL-2.0-Crates im Tauri-Baum.
- D11 Signierung, Apple-Developer-Konto.

## Nächster Schritt

Freigabe für Phase 2 abwarten. Danach: Zweig für Phase 2 anlegen, Abschnitte 9 und 18 lesen, Schema planen.
