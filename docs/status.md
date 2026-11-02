# Projektstatus – Project Write

Diese Datei ist das Arbeitsgedächtnis. Zu Beginn jeder Sitzung lesen, am Ende aktualisieren.

## Aktuelle Phase

**Phase 1 – Grundgerüst** (Zweig `claude/project-write-phase-1-6sh7n2`), in Arbeit.

## Plan Phase 1

| Schritt | Inhalt                                                                                       | Stand     |
| ------- | -------------------------------------------------------------------------------------------- | --------- |
| 1       | Auftrag als `docs/master-prompt.md` sichern                                                  | erledigt  |
| 2       | Versionen und Kompatibilität prüfen, `docs/dependencies.md`                                  | in Arbeit |
| 3       | Frontend-Gerüst: Vite, React, TypeScript strict, Tailwind, Zustand, ESLint, Prettier, Vitest | offen     |
| 4       | Tauri-2-Gerüst: `src-tauri/`, App-Manifest in `build.rs`, Capabilities, CSP                  | offen     |
| 5       | Fehlerformat `{ code, messageKey, details? }`, eigene Argument-Auswertung, `ts-rs`-Typen     | offen     |
| 6       | SQLx-Datenbankschicht: Schreibverbindung + Lese-Pool, Pragmas, Migration 0001, FTS5-Prüfung  | offen     |
| 7       | Befehl `system_status` und Startbildschirm im Project-Write-Design (de/en, Dark/Light)       | offen     |
| 8       | Tests: Rust, Vitest (inkl. Kontrast), WebdriverIO-Rauchtest, CSP- und Rechte-Test            | offen     |
| 9       | CI: GitHub Actions auf Windows, macOS, Linux; Audits; Release-Prüfung auf Testplugins        | offen     |
| 10      | Doku: architecture.md, database.md, ADRs, CHANGELOG, THIRD-PARTY-LICENSES, Bericht           | offen     |

## Offene Aufgaben (phasenübergreifend)

- Siehe Plan oben.

## Nächster Schritt

Frontend- und Tauri-Gerüst anlegen.
