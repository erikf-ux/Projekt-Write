# Projektstatus – Project Write

Diese Datei ist das Arbeitsgedächtnis. Zu Beginn jeder Sitzung lesen, am Ende aktualisieren.

## Aktuelle Phase

**Phase 2 – Speicherkern** (Abschnitte 9 und 18): abgeschlossen am 3. November 2026, Bericht in `docs/bericht-phase-2.md`. **Wartet auf die Freigabe von Phase 3.**

Zweig: weiterhin `claude/project-write-phase-1-6sh7n2`. Abschnitt 24 sieht einen Zweig je Phase vor; die Sitzung erlaubt aber nur Pushes auf diesen Zweig.

## Stand Phase 2

| Schritt | Inhalt                                                                                                | Stand                                                                                     |
| ------- | ----------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| 1       | Pfadprüfung (`security/`), atomares Schreiben (`filesystem/`), Windows-Systemaufrufe                  | erledigt                                                                                  |
| 2       | Erkennung von Sync-Ordnern und Netzlaufwerken                                                         | erledigt                                                                                  |
| 3       | Bibliothek: anlegen, öffnen, sperren, Rollback-Modus, Migrationen mit Sicherung vorab, Migration 0002 | erledigt                                                                                  |
| 4       | Dokument-Repository, Absturztest (100 harte Abbrüche)                                                 | erledigt                                                                                  |
| 5       | Einstellungen (App und Bibliothek), Logging                                                           | erledigt                                                                                  |
| 6       | Bibliothek umziehen (kopieren, prüfen, umschalten)                                                    | erledigt                                                                                  |
| 7       | Befehle, Einzelinstanz, Ordnerdialog                                                                  | erledigt                                                                                  |
| 8       | Bibliotheksansicht, E2E-Tests                                                                         | erledigt                                                                                  |
| 9       | Referenzbibliothek                                                                                    | erledigt für Dokumente; Medien, Subnetze, Tags, Verbindungen in Phase 7, 8, 11 (ADR 0010) |
| 10      | Doku, CI, Bericht                                                                                     | erledigt, CI-Lauf 44 grün (Windows, Linux)                                                |

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
| F-4 | Windows: Umbenennen beim atomaren Speichern scheiterte mit Fehler 123 (ungültiger Name), wenn der Pfad `/` enthielt                                                                                                                | Pfade mit `\\?\`-Präfix werden von Windows nicht normalisiert. Pfade werden vor dem Präfix mit `std::path::absolute` normalisiert; abgedeckt durch die Rust-Tests unter Windows in CI                                                                                                                                                                                                                                  |
| F-5 | E2E: `library.e2e.ts` wurde in CI-Lauf 42 (Windows, Linux) als fehlgeschlagen gemeldet, obwohl die Datei ihre Tests ausführte                                                                                                      | Kein Fehler der App. Der letzte Test schließt das Fenster; die App endet und mit ihr der eingebettete WebDriver-Server. Der Testläufer schickte danach noch `DELETE /session`, bekam `ECONNREFUSED` und brach die Datei ab. Der Test ersetzt `deleteSession` nach dem Schließen durch eine leere Operation (Commit `1047c4c`)                                                                                          |

## Offene Aufgaben

### In Phase 3

- Fensterzustand, Sprache und Tastenkürzel in `settings.json` (ADR 0009).
- Erster Start: Bibliotheksort (Anlegen und Öffnen gibt es seit Phase 2), Sprache, Autokorrektur.
- Echte Tastaturnavigation (Tab) testen: Der eingebettete WebDriver sendet nur synthetische Ereignisse (ADR 0003); eine native Eingabeprüfung einplanen.

### Später

- Referenzbibliothek ergänzen: Medien, Subnetze, Tags und Verbindungen mit ihren Tabellen (Phasen 7, 8, 11), `.pwrite`-Dokumente mit dem Format (Phase 4/5; ADR 0010).
- Echten Ordnerdialog unter Windows einmal von Hand prüfen; CI kann native Dialoge nicht bedienen (ADR 0008).
- Echte Sync-Clients (OneDrive, Dropbox) und Netzlaufwerke auf einem Testgerät prüfen; CI hat keine.
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

Nach Freigabe: Phase 3 (Oberfläche, Abschnitt 11).
