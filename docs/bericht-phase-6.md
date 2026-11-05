# Bericht Phase 6 – Wiederherstellung

Stand: 5. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig). Entwicklungsumgebung: Cloud-Container, Ubuntu 24.04, 4 vCPU, 15 GB RAM, keine GPU (Xvfb, Software-Rendering), WebKitGTK, Rust 1.99.1, Node.js 22. CI: GitHub Actions `windows-latest` und `ubuntu-24.04`; macOS ist auf Wunsch des Auftraggebers zurückgestellt. Entwickelt und geprüft wurde lokal unter Linux; den CI-Lauf unter Windows und Linux nennt der Abschnitt „CI“.

## Was wurde implementiert?

Die vier Stufen der Wiederherstellung aus Abschnitt 4, Papierkorb, Archiv und Dokumenthistorie (Abschnitt 13). Oberfläche und Code nennen jeweils die Stufe. Entscheidungen in ADR 0016 (12 Punkte).

- **Stufe 1 – Sitzung:** Offene Tabs (Bibliotheksdokumente und externe Dateien), aktiver Tab, Ansicht (Rich Text, Quelltext, Reintext) und Auswahl jedes Tabs werden 2 s nach einer Änderung der Tabs und beim Beenden in `settings.json` gesichert und beim nächsten Start mit derselben Bibliothek wieder geöffnet. Gesichert wird erst nach dem Wiederherstellen, damit die leere Arbeitsfläche eines Starts die letzte Sitzung nicht überschreibt. Externe Dateien stehen als Pfad nur in Rust; gelöschte Dokumente und verschwundene Dateien fallen beim Wiederherstellen weg. Fenstergröße, Seitenleiste und Zoom bleiben wie seit Phase 3.
- **Stufe 2 – Absturz:** Höchstens 1 s nach einer ungespeicherten Eingabe liegt der ganze Text als Entwurf in `recovery/` der Bibliothek (`<id>.json`, für externe Dateien `ext-<Prüfsumme des Pfads>.json`, mit Kodierung und Zeilenenden), atomar geschrieben. Der Takt ist gedrosselt, nicht entprellt: Auch wer ohne Pause tippt, hat jede Sekunde einen neuen Entwurf. Gespeicherte oder verworfene Tabs entfernen ihren Entwurf; schlägt das Speichern fehl, bleibt er. Beim nächsten Start fragt der Dialog „Wiederherstellung – Stufe 2: Absturz“ je Entwurf „Wiederherstellen“ oder „Verwerfen“ (oder „Später“); Entwürfe, die dem gespeicherten Stand gleichen, entfernt Rust vorher still. Bei einem Bibliotheksdokument sichert Wiederherstellen den jetzigen Stand zuerst als Version und speichert dann den Entwurf; eine externe Datei öffnet mit dem Entwurf als ungespeicherter Änderung und bleibt bis `Strg + S` unverändert. Der Entwurf einer verschwundenen externen Datei wird als Bibliotheksdokument wiederhergestellt.
- **Stufe 3 – Papierkorb:** „Löschen“ (`Entf` im Explorer, Kontextmenü, Befehlspalette) setzt `deleted_at`; Datei, Versionen, Links und Favorit bleiben, die Rückverweise verschwinden aus „Verbindungen“ und kehren mit „Wiederherstellen (Stufe 3)“ zurück. „Endgültig löschen“ und „Papierkorb leeren“ fragen nach. Nach `trash.retentionDays` (Standard 30, einstellbar 1–3650) entfernt die App Dokumente im Papierkorb beim Öffnen der Bibliothek und beim Anzeigen des Papierkorbs: erst die Zeile (mit Links und Versionen), dann Datei und Entwurf.
- **Archiv:** „Archivieren“ blendet ein Dokument aus Favoriten, Zuletzt verwendet, Alle Dokumente und der Linkauswahl `[[` aus; es steht in der Gruppe „Archiv“ und öffnet normal. Archiv und Papierkorb sind am Start zugeklappt.
- **Stufe 4 – Version:** Migration 0006 (`document_history`). Versionen entstehen beim Bearbeiten (beim Speichern, wenn die neueste älter als 10 Minuten ist), beim Schließen eines Tabs und der App, vor jedem Umwandeln (scheitert die Sicherung, wird nicht umgewandelt), vor jeder Wiederherstellung und benannt mit „Version speichern …“. Inhalt `deflate`-komprimiert (`flate2`), mit SHA-256 geprüft; gleicht er der neuesten Version, entsteht keine Zeile (eine benannte Version gibt der neuesten ihren Namen). Aufbewahrung: alles der letzten 24 Stunden, danach die neueste je Tag bis 30 Tage, danach die neueste je Woche (ab Montag) bis 365 Tage; benannte Versionen und die neueste Version bleiben immer. Alle drei Werte und die Papierkorbdauer stehen unter Einstellungen › Bibliothek.
- **Versionsvergleich:** Der Dialog „Versionen“ zeigt zwei Stände nebeneinander (jede Version oder „Aktueller Stand“), entfernte und hinzugefügte Zeilen farbig und als Zeichen (−/+), dazu die Zählung. Zeilenvergleich nach Myers ohne neue Abhängigkeit; `.pwrite` wird für den Vergleich als Markdown gezeigt. Unterscheiden sich zwei Texte in mehr als 4.000 Zeilen, zeigt der Vergleich sie als einen ersetzten Block; die Tabelle zeigt höchstens 5.000 Zeilen und nennt den Rest. „Diese Version wiederherstellen (Stufe 4)“ sichert den aktuellen Stand zuerst als Version „Vor einer Wiederherstellung“ und stellt die Version in ihrem Format wieder her (eine Version von vor dem Umwandeln wandelt zurück).

**Jede Wiederherstellung sichert zuerst den aktuellen Stand:** Stufe 4 und Stufe 2 legen vor dem Schreiben eine Version `beforeRestore` an; scheitert das, wird nichts geschrieben. Eine externe Datei wird beim Wiederherstellen eines Entwurfs nicht geschrieben. Stufe 3 ändert keinen Inhalt (nur `deleted_at`), Stufe 1 öffnet nur Tabs.

## Welche Dateien wurden erstellt oder verändert?

Rust, neu: `services/{history,recovery}.rs`, `database/history.rs`, `commands/{recovery,workspace}.rs`, `migrations/0006_document_history.sql`, Tests `tests/{trash,history}.rs`. Geändert: `clock.rs` (`ManualClock` für simulierte Zeit), `services/documents.rs` (Papierkorb, Archiv, Versionen, Entwürfe), `database/documents.rs`, `services/library/mod.rs` (Aufräumen beim Öffnen), `services/session.rs`, `services/settings/{app,library}.rs` (Sitzung; vier Bibliothekseinstellungen), `services/external.rs`, `commands/{documents,library,mod}.rs`, `error.rs` (`VERSION_NOT_FOUND`, `VERSION_DAMAGED`), `build.rs`, `capabilities/main-window.json`, `tests/{ipc,migrations}.rs`. **20 neue Befehle** (Liste in `docs/architecture.md`).

Frontend, neu: `modules/history/{HistoryDialog.tsx,diff.ts}`, `modules/recovery/RecoveryDialog.tsx`, `modules/documents/sessionTabs.ts`, `modules/settings/LibraryPreferences.tsx`; Tests `modules/history/{HistoryDialog.test.tsx,diff.test.ts}`, `modules/recovery/drafts.test.ts`, `modules/documents/{sessionTabs,trash}.test.ts`. Geändert: `modules/documents/{workspaceStore.ts,Explorer.tsx}`, `modules/editor/{session.ts,DocumentEditor.tsx,InternalLinkPicker.tsx}`, `modules/settings/SettingsDialog.tsx`, `app/Workbench.tsx`, `app/{commands,lifecycle}.ts`, `modules/commands/types.ts`, `services/ipc.ts`, `i18n/{de,en}.json`, erzeugte Typen.

E2E: neu `tests/e2e/editing.ts` (gemeinsame Schreibhilfen), `specs/{session-quit,session-restored,crash,crash-recovered}.e2e.ts`; geändert `specs/documents.e2e.ts` (Papierkorb, Historie), `session.ts` (`waitUntilAppEnded`), `sandbox.ts`, `wdio.conf.ts`. Doku: ADR 0016, `docs/{architecture,database,dependencies,status}.md`, dieser Bericht, `CHANGELOG.md`, `README.md`. **Eine neue Abhängigkeit:** `flate2` 1.1.10 (MIT/Apache-2.0, reines Rust mit `miniz_oxide`; Begründung in `docs/dependencies.md`).

## Welche Tests wurden durchgeführt?

Umfang: **313 Rust-Tests** (202 Unit, 111 Integration; Phase 5: 271), **682 Vitest-Tests** in 35 Dateien (664), **48 WebdriverIO-Tests** in 12 Dateien (42).

Lokal (Linux-Container, Xvfb, D-Bus-Sitzung), alle ausgeführt und bestanden:

| Prüfung                                                                                                                                                                  | Ergebnis                                                                                             |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| Prettier, ESLint (`--max-warnings 0`), TypeScript (App und Node-Teil), Vitest (682)                                                                                      | bestanden                                                                                            |
| rustfmt, Clippy `-D warnings` (Linux, Linux + `e2e`, Windows-Ziel `x86_64-pc-windows-gnu`)                                                                               | bestanden                                                                                            |
| Rust-Tests (313), darunter die 100 harten Abbrüche beim Speichern und die Migrationen ab jeder früheren Version (0 bis 5 → 6)                                            | bestanden                                                                                            |
| Erzeugte TypeScript-Typen neu geschrieben und unverändert; Lizenzliste unverändert                                                                                       | bestanden                                                                                            |
| Referenzbibliothek zweimal gebaut, gleicher Fingerabdruck (10.000 Dokumente)                                                                                             | bestanden                                                                                            |
| E2E (48): Rauchtest (7), Bibliothek (4), Neustart (3), Arbeitsfläche (12), Editor (3), Leistung (5), externe Dateien (6), Dokumente (4), Sitzung (2), harter Abbruch (2) | bestanden, drei vollständige Läufe in Folge                                                          |
| Release-Build ohne Test-Plugins (`check-release-no-test-plugins.sh`)                                                                                                     | bestanden                                                                                            |
| `npm audit --omit=dev` (ausgelieferte Pakete); `npm audit` (mit Entwicklungswerkzeugen, nur berichtet)                                                                   | 0 Funde; 34 Funde (20 mittel, 14 hoch), unverändert seit Phase 5, bewertet in `docs/dependencies.md` |

Was die Tests zeigen, im Einzelnen:

- **Harter Abbruch beim Tippen** (E2E, `crash.e2e.ts` → `crash-recovered.e2e.ts`): Die Seite tippt ein Zeichen alle 100 ms und merkt sich die Zeit jedes Zeichens; nach 5 s beendet der Test den Prozess hart (`SIGKILL`, unter Windows `taskkill /F`). Autosave wartet auf eine Pause von 1 s und kommt nie; nur der Entwurf hält den Text. Der nächste Start bietet den Entwurf an, „Wiederherstellen“ schreibt ihn ins Dokument. Gemessen in drei Läufen: 39, 41 und 39 von 48 Zeichen zurück; verloren war, was in den letzten **887, 674 und 908 ms** vor dem Abbruch getippt wurde (Grenze 2.000 ms). Vitest prüft dazu den Takt mit simulierter Zeit (jede Taste steckt höchstens 1 s später in einem Entwurf, auch bei fehlschlagendem Speichern) und dass ein gespeicherter Tab seinen Entwurf entfernt.
- **Entwürfe in Rust und IPC:** schreiben, lesen, auflisten, entfernen; Schlüssel können keine anderen Dateien benennen; überholte und verwaiste Entwürfe verschwinden beim Auflisten; offene externe Dateien werden nicht angeboten; Wiederherstellen sichert erst den jetzigen Stand; ungültige Anfragen (fremde Felder, falsche IDs und Schlüssel) mit einheitlichem Fehler.
- **Papierkorb** (Rust, IPC, Vitest, E2E): Ein gelöschtes Dokument kehrt mit Links, Rückverweisen und Favorit zurück; endgültiges Löschen nimmt Links und Versionen mit; die Aufbewahrung entfernt mit simulierter Zeit (`ManualClock`) genau die Dokumente, die länger als die eingestellten Tage im Papierkorb liegen, und lässt jüngere liegen. Im echten Programm: `Entf` im Explorer schließt den Tab, die Datei bleibt, „Verbindungen“ des Ziels ist leer; „Wiederherstellen (Stufe 3)“ aus dem Papierkorb bringt den Rückverweis zurück.
- **Archiv** (Rust, IPC, Vitest): archiviert und zurück, Daten bleiben; nicht in Favoriten, Zuletzt verwendet, Alle Dokumente (Explorer-Test) und der Linkauswahl.
- **Historie** (Rust, IPC, Vitest, E2E): eine Version je 10 Minuten Bearbeitung (simulierte Zeit); Versionen beim Schließen, vor dem Umwandeln, benannt; keine Doppel; Prüfsumme beim Lesen; eine Version von vor dem Umwandeln kommt in ihrem Format zurück. Aufbewahrung mit simulierter Zeit: Der Unit-Test legt Versionen in jedem Zeitfenster an (letzte 24 h, Tage, Wochen ab Montag, älter als ein Jahr, benannt) und prüft die genau entfernten; ein Integrationstest simuliert 40 Tage mit vier Sitzungen täglich und strengere Regeln beim nächsten Öffnen. Vitest: Myers-Diff, Ansicht nebeneinander, Zählung. E2E: „Version speichern …“ → Vergleich zeigt „1 Zeilen entfernt, 1 hinzugefügt“ → Wiederherstellen schreibt die alte Fassung in die Datei und den Editor, der vorige Stand steht danach als „Vor einer Wiederherstellung“ in der Liste.
- **Sitzung** (Rust, IPC, Vitest, E2E): Im echten Programm wählt der Test „Mitte“ in einem Dokument, wechselt kurz vor dem Schließen den aktiven Tab (nur das Sichern beim Beenden kennt ihn), schließt das Fenster und prüft `settings.json` (Positionen 8–13, Ansicht `rich`). Nach dem Neustart sind dieselben Tabs in derselben Reihenfolge offen, der zuletzt aktive ist aktiv, und die Statusleiste meldet die Auswahl („Wörter: 1 von 3“); kein Wiederherstellungsdialog nach normalem Ende. Rust und IPC: nur dieselbe Bibliothek, fehlende Dokumente und Dateien ausgelassen, aktiver Tab umgerechnet.
- **Migration 0006:** Testdaten ab jeder früheren Version migriert, Schema identisch, `integrity_check` ok; die Prüfsumme der neuen Migration ist festgeschrieben.

## Abnahmekriterien Phase 6

| Kriterium                                                                                               | Stand                                                                                                                                                                                                                             |
| ------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ein harter Abbruch beim Tippen kostet höchstens 2 s Text; der nächste Start bietet Wiederherstellung an | **erfüllt** unter Linux und Windows (E2E; Angebot beim nächsten Start). Verloren: lokal unter Linux 887, 674 und 908 ms; in CI-Lauf 99 unter Linux 869 ms, unter Windows (`taskkill /F /T`) 508 ms.                               |
| Ein gelöschtes Element kehrt mit Zuordnungen und Verbindungen zurück                                    | **erfüllt** für Dokumente mit Links, Rückverweisen, Favorit und Versionen (Rust, IPC, E2E). Subnetze und Tags gibt es erst ab Phase 7; sie werden wie die Links nie entfernt, solange ein Element im Papierkorb liegt (ADR 0016). |
| Die Aufbewahrung arbeitet richtig, getestet mit simulierter Zeit                                        | **erfüllt** für Papierkorb und Historie (`ManualClock`, Unit- und Integrationstests).                                                                                                                                             |
| Jede Wiederherstellung sichert zuerst den aktuellen Stand                                               | **erfüllt** für Stufe 2 und 4 (Rust, IPC, E2E); Stufe 1 und 3 ändern keinen Inhalt.                                                                                                                                               |

## Was konnte nicht getestet werden und warum?

- **Stromausfall und Systemabsturz:** Geprüft ist der harte Prozessabbruch. Ob ein Entwurf einen Stromausfall übersteht, hängt zusätzlich am Dateisystem; geschrieben wird atomar mit `fsync` von Datei und Ordner (Phase 2). Einen echten Stromausfall kann der Container nicht auslösen.
- **Windows:** nur in CI geprüft (Lauf 99 grün, auch der harte Abbruch mit `taskkill /F /T`); lokal lief nur Clippy für das Windows-Ziel.
- **macOS:** zurückgestellt.
- **Natives Menü:** Die neuen Befehle stehen in Befehlspalette und Kontextmenüs; Menüklicks bleiben in CI unbedienbar (wie bisher).
- **Enter im Namensdialog:** Das synthetische Enter des eingebetteten WebDrivers sendet kein Formular ab (F-24); der E2E-Test klickt „Speichern“. Der Dialog ist ein HTML-Formular, das Enter im Eingabefeld nach dem Standard absendet; mit einer echten Tastatur ist das nicht geprüft.
- **Sehr große Historien:** Aufbewahrung und Vergleich sind mit Dutzenden Versionen und kurzen Texten getestet, nicht mit Hunderten Versionen eines 50.000-Wörter-Manuskripts; Laufzeit und Speicher dafür sind nicht gemessen.

## Offene Fehler

Keine bekannten.

## Behobene Fehler

F-23 bis F-26 in `docs/status.md`: F-23 (Umwandeln-Test prüfte zu früh) und F-24 (synthetisches Enter) waren Fehler der Tests; F-25 (ein Dokument im Papierkorb ließ sich über `document_read` öffnen) fand der IPC-Test. F-26 war ein Prozessfehler von mir: CI-Lauf 98 prüfte einen Zwischenstand, dessen Frontend-Testdaten erst im nächsten Commit kamen (Typprüfung rot); der Code danach war grün.

## Gefundene Widersprüche und Abweichungen

1. **Umfang** (ADR 0016, Punkt 1): Stufe 5 (Backup) folgt in Phase 10, Subnetze und Tags in Phase 7 – nach dem Phasenplan in Abschnitt 25. Das Archiv ist aus dem Explorer ausgeblendet; „aus der Standardsuche“ und „im Mesh grau“ (Abschnitt 4) folgen mit Suche (Phase 9) und Mesh (Phase 11).
2. **„Alle 10 Minuten aktiver Bearbeitung“** (Abschnitt 13) ist umgesetzt als: beim Speichern eine Version, wenn die neueste älter als 10 Minuten ist. Ohne Bearbeitung entsteht keine Version; nach einer Pause hält die erste Speicherung den neuen Stand fest (ADR 0016, Punkt 4).
3. **Entwurf nach 1 s statt 2 s:** Abschnitt 9 und die Leistungstabelle verlangen höchstens 2 s nach der letzten Eingabe; die App schreibt höchstens 1 s nach der ersten ungespeicherten, damit der Verlust bei einem harten Abbruch mit Schreibzeit unter 2 s bleibt (ADR 0016, Punkt 8).
4. **Stufe 1, „Layout und Cursorposition“:** Gesichert werden Tabs, aktiver Tab, Ansicht und Auswahl (Cursor = leere Auswahl). Fenster, Seitenleiste und Zoom hält die App schon seit Phase 3 in `settings.json`.
5. **Ein Zweig je Phase** (Abschnitt 24): weiterhin nur der erlaubte Zweig.

## Entscheidungen (ADR)

0016 Wiederherstellung (12 Punkte: Umfang, Papierkorb, Archiv, Arten und Aufbewahrung der Versionen, Vergleich, Wiederherstellen, Entwürfe, Angebot beim Start, Sitzung, Oberfläche, `flate2`).

## CI

**CI-Lauf 99 ist grün** (von Hand gestartet am 5. November 2026, Commit `90bf17d`, Stand des Berichts):

| Job                             | Ergebnis                                                                                                                                                                                                                                                  |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Windows (`windows-latest`)      | grün: Prettier, ESLint, TypeScript, Vitest, rustfmt, Clippy, Rust-Tests (mit den 100 harten Abbrüchen), Referenzbibliothek zweimal gleich, Typen und Lizenzliste aktuell, E2E alle 12 Dateien (harter Abbruch: 44 von 50 Zeichen zurück, 508 ms verloren) |
| Linux (`ubuntu-24.04`)          | grün: dieselben Schritte, E2E alle 12 Dateien (39 von 48 Zeichen zurück, 869 ms verloren)                                                                                                                                                                 |
| Abhängigkeitsprüfung            | grün: `cargo audit`, `cargo deny` (auch für `flate2`), `npm audit`                                                                                                                                                                                        |
| Release-Build ohne Test-Plugins | grün                                                                                                                                                                                                                                                      |

Zwischenstände trugen `[skip ci]`, um Actions-Minuten zu sparen. **Lauf 98** (Zwischenstand `1468541`, ohne `[skip ci]`) scheiterte an der Typprüfung (F-26); der Code danach ist in Lauf 99 geprüft.

## Nächster Schritt

Nach Freigabe: Phase 7 (Subnetze und Tags; Abschnitt 25).
