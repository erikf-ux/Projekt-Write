# Projektstatus – Project Write

Diese Datei ist das Arbeitsgedächtnis. Zu Beginn jeder Sitzung lesen, am Ende aktualisieren.

## Aktuelle Phase

**Phase 1 – Grundgerüst** (Zweig `claude/project-write-phase-1-6sh7n2`): umgesetzt, **wartet auf Freigabe** für Phase 2.

## Stand Phase 1

| Schritt | Inhalt                                                                                                  | Stand                            |
| ------- | ------------------------------------------------------------------------------------------------------- | -------------------------------- |
| 1       | Auftrag als `docs/master-prompt.md`                                                                     | erledigt                         |
| 2       | Versionen und Kompatibilität (ADR 0001, `docs/dependencies.md`)                                         | erledigt                         |
| 3       | Frontend-Gerüst: Vite 8, React 19, TypeScript 6 strict, Tailwind 4, Zustand 5, ESLint, Prettier, Vitest | erledigt                         |
| 4       | Tauri-2-Gerüst: App-Manifest in `build.rs`, eine Capability, CSP, Härtung                               | erledigt                         |
| 5       | IPC-Vertrag: `Input<T>` + `Validate`, `CommandError`, `ts-rs` (ADR 0002)                                | erledigt                         |
| 6       | SQLx-Schicht: Schreibverbindung + Lese-Pool, Pragmas, Migration 0001, FTS5-Probe (ADR 0004)             | erledigt                         |
| 7       | Befehl `system_check`, Startbildschirm (de/en, Dark/Light)                                              | erledigt                         |
| 8       | Tests: Rust (41), Vitest (66), WebdriverIO (7)                                                          | erledigt, Ergebnis siehe Bericht |
| 9       | CI: Windows, macOS, Linux; Release-Prüfung; Audits                                                      | siehe Bericht                    |
| 10      | Doku: Architektur, Datenbank, Abhängigkeiten, ADRs 0001–0005, CHANGELOG, Lizenzliste                    | erledigt                         |

## Behobene Fehler

| Nr. | Beschreibung                                                                            | Ursache und Lösung                                                                                                                                                                                                                                                                                                                                                                    |
| --- | --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F-1 | E2E meldete nach einem Theme-Wechsel zur Laufzeit veraltete Textfarben (macOS, Windows) | Kein Fehler der App. Die CI-Fenster unter macOS und Windows rendern nicht von selbst; `getComputedStyle` lieferte dort bis zum nächsten Frame Werte von vor der Änderung. Belegt in CI-Lauf 15: vor einem erzwungenen Frame veraltet, danach alle Werte korrekt. Die E2E-Tests erzwingen jetzt vor jeder Messung einen Frame (Screenshot) und prüfen zusätzlich echte Bildschirmpixel |
| F-2 | E2E meldete den Fokusring als Weiß auf Weiß (macOS, Windows)                            | Dieselbe Ursache. Der Fokusring wird jetzt auf einem gerenderten Frame geprüft: berechnete Farbe gleich Akzent-Token, gerenderte Ringpixel gegen die Umgebung mindestens 3:1                                                                                                                                                                                                          |

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

### Entscheidungen des Auftraggebers (vor Phase 14)

- D4 Lizenz der App, inkl. Bewertung der MPL-2.0-Crates im Tauri-Baum.
- D11 Signierung, Apple-Developer-Konto.
- Endgültige Bundle-Kennung (derzeit vorläufig `app.projectwrite.desktop`, ADR 0001).

## Nächster Schritt

Freigabe für Phase 2 abwarten. Danach: Zweig für Phase 2 anlegen, Abschnitte 9 und 18 lesen, Schema planen.
