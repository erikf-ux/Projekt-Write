# Bericht Phase 2 – Speicherkern

Stand: 3. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig; Abschnitt 24 sieht einen Zweig je Phase vor). Entwicklungsumgebung: Cloud-Container, Ubuntu 24.04, Rust 1.99.1, Node.js 22 (kein Referenzgerät nach Abschnitt 6). CI: GitHub Actions `windows-latest` und `ubuntu-24.04`; macOS ist auf Wunsch des Auftraggebers zurückgestellt.

## Was wurde implementiert?

- **Bibliothek** (Abschnitt 9): anlegen (Standard `Dokumente/ProjectWrite`, nur in leeren Ordnern, sonst im Unterordner `ProjectWrite`), öffnen, schließen, umziehen. `library.json` mit ID und Formatversion (neuere Formate abgelehnt, unbekannte Felder erhalten); `library.json` entsteht zuletzt.
- **Sperre:** `library.lock` mit Betriebssystem-Dateisperre (keine veraltete Sperre nach Absturz) und `tauri-plugin-single-instance` (zweiter Programmstart übergibt an die laufende Instanz).
- **Speicherort:** Sync-Ordner (Windows Cloud Files API, Anbieter-Konfiguration, Ordnernamen) und Netzlaufwerke (Windows Laufwerkstyp/UNC, Linux Einhängetabelle) werden erkannt; Anlegen und Umziehen dorthin nur nach Bestätigung; dort Rollback-Journal statt WAL, bei jedem Öffnen neu bestimmt.
- **Atomares Speichern:** Temporärdatei im selben Ordner, `fsync`, Umbenennen (Windows `MoveFileExW` mit Write-Through), Ordner-`fsync` unter Unix.
- **Pfadprüfung:** bibliotheksrelative Pfade mit festen Namensregeln, kein `..`, keine absoluten Pfade, keine reservierten Namen; jede Pfadstufe ohne symbolische Links oder Junctions; Dateipfade werden aus ID und Typ abgeleitet, nie gespeichert.
- **Öffnen:** Verknüpfungen an festen Pfaden abgelehnt, `tmp/` geleert, `quick_check`, neuere Schemata abgelehnt, **Sicherung vor jeder Migration** (`VACUUM INTO`, `integrity_check`, die fünf neuesten bleiben), Dateien ohne Datenbankeintrag gemeldet statt gelöscht.
- **Datenbank:** vollständiges geplantes Schema aller Tabellen aus Abschnitt 18 in `docs/database.md`; Migration 0002 `documents`; Migrationstests ab jeder früheren Version; Prüfsummen ausgelieferter Migrationen festgeschrieben.
- **Dokument-Repository:** anlegen, lesen, speichern (unveränderter Inhalt wird nicht neu geschrieben), umbenennen (nur Titel), Papierkorb. Erst Datei, dann Zeile; veraltete Prüfsummen werden beim Lesen korrigiert.
- **Umzug:** kopieren (mit Prüfsummen), prüfen (erneutes Lesen, `integrity_check`, Schema und Dokumentanzahl), umschalten; Fortschritt und Abbruch; alte `library.json` erhält `movedTo`; Abbruch und Fehler entfernen nur, was der Umzug angelegt hat.
- **Einstellungen:** `settings.json` im Konfigurationsordner (Bibliothekspfad; beschädigte Datei wird aufbewahrt, neuere nie überschrieben); typisierte Bibliothekseinstellungen in `application_settings` (erste: bestätigter Speicherort).
- **Logging:** lokaler, rotierender Datei-Log, nur Rust-seitig, ohne Dokumentinhalte.
- **IPC:** neun Bibliotheksbefehle; Ordnerwahl im nativen Dialog aus Rust (`rfd`), das Frontend erhält Tokens und Anzeigetexte und sendet nie Pfade; Anfragen müssen JSON-Objekte sein; 22 neue Fehlercodes mit deutschen und englischen Meldungen.
- **Oberfläche:** Bibliotheksansicht auf dem Startbildschirm: anlegen, öffnen, umziehen mit Fortschritt und Abbruch, Warndialog für Sync-Ordner und Netzlaufwerke, Rückfrage bei umgezogenen Bibliotheken, Hinweise aus dem letzten Öffnen; Deutsch und Englisch.
- **Referenzbibliothek:** `scripts/reference-library.mjs`, reproduzierbar aus einem Startwert, 10.000 Dokumente (eines mit 50.000 Wörtern), Fingerabdruck und `--check`.

## Welche Dateien wurden erstellt oder verändert?

Neu in `src-tauri/src/`: `clock.rs`, `logging.rs`, `security/` (`paths.rs`), `filesystem/` (`atomic.rs`, `location.rs`, `windows.rs`), `services/library/` (`mod.rs`, `layout.rs`, `lock.rs`, `maintenance.rs`, `manifest.rs`, `relocate.rs`), `services/documents.rs`, `services/session.rs`, `services/dialog.rs`, `services/settings/` (`app.rs`, `library.rs`), `commands/library.rs`, `database/documents.rs`, `database/settings.rs`; `migrations/0002_documents.sql`; `examples/reference_library/`; Tests `tests/{path_security,library,library_lock,documents,crash_safety,migrations,relocate,settings,reference_library}.rs`, `tests/common/mod.rs`.

Geändert: `lib.rs`, `state.rs`, `error.rs`, `commands/{mod,input,system}.rs`, `database/{mod,diagnostics}.rs`, `Cargo.toml`, `build.rs`, `capabilities/main-window.json`, `tests/{ipc,database}.rs`.

Frontend: neu `src/modules/library/` (`LibraryPanel.tsx`, `libraryStore.ts`, `format.ts`, Tests), `src/services/library.ts`; geändert `src/services/ipc.ts`, `src/app/StartScreen.tsx` (+ Test), `src/i18n/{de,en}.json`. E2E: neu `tests/e2e/{sandbox,session}.ts`, `specs/{library,restart}.e2e.ts`, `manual/`; geändert `wdio.conf.ts`, `specs/smoke.e2e.ts`. Skripte: `scripts/reference-library.mjs`. CI: `.github/workflows/ci.yml`. Doku: `docs/database.md`, `docs/architecture.md`, `docs/dependencies.md`, ADRs 0006–0010, `docs/status.md`, dieser Bericht, `CHANGELOG.md`, `README.md`, `THIRD-PARTY-LICENSES`.

## Welche Tests wurden durchgeführt?

Umfang: **146 Rust-Tests** (71 Unit, 75 Integration; dazu 2 Hilfsprozesse, die die Tests selbst starten), **78 Vitest-Tests**, **14 WebdriverIO-Tests** (Phase 1: 41, 66, 7).

CI-Ergebnis: siehe Abschnitt „CI“ unten.

Lokal (Linux-Container, Xvfb, D-Bus-Sitzung), alle ausgeführt und bestanden:

| Prüfung                                                                                    | Ergebnis                                                              |
| ------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- |
| Prettier, ESLint, TypeScript, Vitest (78)                                                  | bestanden                                                             |
| rustfmt, Clippy `-D warnings` (Linux, Linux + `e2e`, Windows-Ziel `x86_64-pc-windows-gnu`) | bestanden                                                             |
| Rust-Tests (146)                                                                           | bestanden                                                             |
| 100 harte Abbrüche während des Speicherns                                                  | bestanden: 100 von 100 Abbrüchen trafen ein laufendes Speichern, 26 s |
| Referenzbibliothek, 10.000 Dokumente, zweimal gebaut                                       | bestanden: gleicher Fingerabdruck, 31,5 s je Bau, 81 MB Text          |
| E2E (14): Bibliothek, Neustart/zweite Instanz, Rauchtest Phase 1                           | bestanden                                                             |
| Echter Ordnerdialog (manuell, ohne Testantworten)                                          | GTK-Dialog erscheint (Bildschirmfoto)                                 |

## Abnahmekriterien Phase 2

| Kriterium                                                                                                | Stand                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| -------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 100 harte Prozessabbrüche während des Speicherns: keine beschädigte Datei, `integrity_check` meldet `ok` | **erfüllt.** `tests/crash_safety.rs`: Ein Hilfsprozess speichert ohne Pause und wird 100-mal hart beendet. Nach jedem Abbruch meldet `integrity_check` `ok`, jede Datei ist vollständig (alte oder neue Fassung mit passender Prüfsumme), keine Temporärdatei bleibt liegen, kein Dokument verliert seine Datei. Lokal trafen 100 von 100 Abbrüchen ein laufendes Speichern; der Test verlangt mindestens 75. Bestanden lokal und in CI unter Windows und Linux                                       |
| Eine zweite Instanz kann dieselbe Bibliothek nicht öffnen                                                | **erfüllt.** `tests/library_lock.rs`: Ein zweiter Prozess erhält „gesperrt“, nach geordnetem Ende und nach hartem Abbruch des ersten bleibt keine veraltete Sperre. `tests/library.rs`: Ein zweites Öffnen im selben Prozess wird abgelehnt. E2E `restart.e2e.ts`: Ein zweiter Programmstart übergibt an die laufende Instanz und endet mit Exit-Code 0                                                                                                                                               |
| Sync-Ordner lösen die Warnung aus; wird trotzdem dort gespeichert, läuft die Datenbank im Rollback-Modus | **erfüllt.** `tests/library.rs`: Rollback-Journal im Sync-Ordner, Wechsel von WAL auf Rollback nach dem Umzug in einen Sync-Ordner. E2E `library.e2e.ts`: Der Vorschlag `Dokumente/ProjectWrite` im Sync-Ordner zeigt die Warnung; nach dem bestätigten Umzug dorthin läuft die Datenbank im Rollback-Modus. Erkannt über Ordnernamen; echte Sync-Clients fehlen in CI (siehe unten)                                                                                                                  |
| Pfadangriffe wie `..` und symbolische Links werden abgelehnt                                             | **erfüllt.** `tests/path_security.rs`: `..`, absolute Pfade, fremde Laufwerke, reservierte Namen, symbolische Links auf Ordner und Dateien, unter Windows Junctions; nichts entsteht außerhalb der Bibliothek. `tests/library.rs`: Verknüpfungen an festen Pfaden und in `tmp/` werden abgelehnt bzw. nicht verfolgt. `tests/ipc.rs`: Ordner kommen nur aus dem Dialog und werden über Tokens angesprochen; E2E `smoke.e2e.ts`: Das Frontend hat keinen Zugriff auf Dateisystem, Shell, SQL und Pfade |
| Die Referenzbibliothek entsteht reproduzierbar                                                           | **erfüllt für den Umfang von Phase 2** (10.000 Dokumente; Medien, Subnetze, Tags und Verbindungen folgen mit ihren Tabellen, ADR 0010). CI baut sie zweimal und vergleicht den Fingerabdruck (Linux 10.000, Windows 1.000 Dokumente); `tests/reference_library.rs` prüft eine kleine Bibliothek                                                                                                                                                                                                       |

## Was konnte nicht getestet werden und warum?

- **macOS:** auf Wunsch des Auftraggebers zurückgestellt; der Code ist plattformneutral geschrieben, aber unter macOS weder gebaut noch getestet. Netzlaufwerke werden unter macOS noch nicht erkannt (nur Ordnernamen und iCloud).
- **Echter Ordnerdialog unter Windows:** CI kann native Dialoge nicht bedienen; die E2E-Tests beantworten den Dialog über die Testnaht (ADR 0008). Unter Linux manuell belegt.
- **Echte Sync-Clients und Netzlaufwerke:** In CI gibt es weder OneDrive noch Netzlaufwerke. Geprüft sind die Erkennung über Ordnernamen (echte Ordner), die Einhängetabelle (Beispieldaten) und die Rangfolge der Signale (simuliertes System). Die Windows-Aufrufe (`CfGetSyncRootInfoByPath`, `GetDriveTypeW`) laufen in CI, finden dort aber keinen Sync-Ordner.
- **Stromausfall:** Der Absturztest bricht Prozesse ab; ein Stromausfall (Daten im Gerätecache) ist nicht simulierbar. Dagegen schützen `fsync`, Ordner-`fsync` und Write-Through.
- **Messungen auf dem Referenzgerät:** nicht verfügbar.

## Offene Fehler

Keine bekannten.

## Behobene Fehler

- **F-4** Windows: Umbenennen beim atomaren Speichern scheiterte mit Fehler 123, wenn der Pfad `/` enthielt (Pfade mit `\\?\`-Präfix normalisiert Windows nicht). Pfade werden jetzt vorher mit `std::path::absolute` normalisiert.
- **F-5** E2E: CI-Lauf 42 meldete `library.e2e.ts` unter Windows und Linux als fehlgeschlagen. Kein Fehler der App: Der letzte Test schließt das Fenster, die App endet und mit ihr der eingebettete WebDriver-Server; der Testläufer schickte danach noch `DELETE /session` und brach mit `ECONNREFUSED` ab. Der Test ersetzt `deleteSession` nach dem Schließen durch eine leere Operation (Commit `1047c4c`).

Einzelheiten in `docs/status.md`.

## Gefundene Widersprüche und Abweichungen

1. **Referenzbibliothek** (Abschnitt 6 vs. Abschnitt 18): Das Skript soll 5.000 Medien, 500 Subnetze, 1.000 Tags und 30.000 Verbindungen erzeugen; diese Tabellen entstehen aber erst in den Phasen 7, 8 und 11. Phase 2 erzeugt die 10.000 Dokumente; die übrigen Elemente ergänzt das Skript mit ihren Tabellen (ADR 0010).
2. **Ein Zweig je Phase** (Abschnitt 24): Die Sitzung erlaubt nur Pushes auf `claude/project-write-phase-1-6sh7n2`; Phase 2 liegt deshalb auf diesem Zweig.
3. **Isolierte Testdatei für den Mehrprozess-Test:** Unter Unix teilt ein gerade startender Kindprozess bis zum `exec` die offenen Dateien des Elternprozesses; parallel laufende Tests konnten dadurch kurz eine fremde Sperre sehen. Kein Fehler der App, aber eine Testgrenze (ADR 0006).

## Entscheidungen (ADR)

0006 Bibliothek: Sperre, Speicherort und Rollback-Modus · 0007 Atomares Speichern und Dokument-Repository · 0008 Ordnerwahl in Rust, Tokens statt Pfaden · 0009 App- und Bibliothekseinstellungen, Logging · 0010 Referenzbibliothek. ADR 0004 ist im Laufzeit-Teil durch 0006 abgelöst.

## CI

Der CI-Lauf zum letzten Commit läuft noch; das Ergebnis wird hier eingetragen.

## Nächster Schritt

Nach Freigabe: Phase 3 (Oberfläche, Abschnitt 11) – Hauptlayout, Menüleiste, Toolbar, Activity Bar, Explorer, Tabs, Statusleiste, Themes, Command Palette, Tastenkürzel, Kontextmenüs, Drag & Drop, Seitenleisten, erster Start (Bibliotheksort aus Phase 2, Sprache, Autokorrektur), Sprachdateien, Fokusmodus; dabei Fensterzustand, Sprache und Tastenkürzel in `settings.json`.
