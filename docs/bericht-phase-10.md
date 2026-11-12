# Bericht Phase 10 – Export und Backup

Stand: 12. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig). Entwicklungsumgebung: Cloud-Container, Ubuntu 24.04, 4 vCPU, 15 GB RAM, keine GPU (Xvfb, Software-Rendering), WebKitGTK, Rust 1.99.1, Node.js 22. CI: GitHub Actions `windows-latest` und `ubuntu-24.04`; macOS ist auf Wunsch des Auftraggebers zurückgestellt. Den CI-Lauf nennt der Abschnitt „CI“.

**Fünf der sechs Abnahmekriterien sind erfüllt**, in Rust-Tests und im echten Programm: Ein Backup während laufender Eingaben ist konsistent; ein Abbruch während des Backups hinterlässt keine fertig aussehende Datei; nach der Wiederherstellung stimmen alle Prüfsummen; ein verändertes oder beschädigtes Backup wird abgelehnt; das Backup enthält keine früheren Backups.

**Das sechste ist nur teilweise erfüllt** – „PDF-Export und Drucken sind auf allen drei Betriebssystemen getestet“: Geprüft sind unter Windows und Linux die Druckkopie, das Druck-CSS und der Aufruf des Systemdrucks. Den Druckdialog selbst kann der eingebettete WebDriver nicht bedienen (ADR 0003), und macOS ist zurückgestellt. Das echte Drucken und „Als PDF speichern“ braucht einen Handtest je System (Punkt 1 unter „Gefundene Widersprüche und Abweichungen“).

## Was wurde implementiert?

Entscheidungen in ADR 0021 (11 Punkte und Nachträge).

- **Backup `.pwbackup`** in den vier Schritten aus Abschnitt 21: unter der Schreibsperre ein fester Stand (Datenbank mit `VACUUM INTO`, Dokumente und Medien als harte Links, wo nicht möglich als Kopie), `integrity_check` der Kopie, SHA-256 jeder Datei und `manifest.json` (App-, Schema-, Formatversion), ZIP unter `<Name>.pwbackup.part`, vollständig zurückgelesen und geprüft, erst dann umbenannt. Aufgenommen wird eine Positivliste aus der Datenbank: `library.json`, Datenbank, die Dateien der Zeilen in `documents` und `media` – nichts aus `backups/`, `tmp/`, `recovery/`, `thumbnails/`, `exports/`, keine Sperre, kein `-wal`/`-shm`.
- **Ziel und Automatik:** Ziel frei wählbar (Systemdialog), Standard `backups/` der Bibliothek; Warnung, wenn es auf demselben Datenträger liegt. Täglich beim Start ein Backup (30 s nach dem Öffnen, im Hintergrund, wenn das neueste älter als 24 h ist), 7 bleiben; beides einstellbar. Reste abgebrochener Backups räumt der Start weg.
- **Vor jeder Datenbankmigration** ein vollständiges Backup statt der bisherigen Datenbankkopie (Abschnitt 21); scheitert es, wird nicht migriert.
- **„Backup prüfen“** liest alles und verändert nichts: Namen nach Positivliste, keine Links, Verzeichnisse, Verschlüsselung oder Doppel, Größen beim Lesen gezählt (nie den Angaben im ZIP geglaubt), jede Datei im Manifest und umgekehrt, Prüfsummen, `integrity_check` der Datenbank, Format- und Schemaversion.
- **Wiederherstellung Stufe 5:** Backup und Ordner wählen; Entpacken in einen versteckten Ordner daneben, dieselben Prüfungen, alle Dateien erneut von der Platte gelesen, `integrity_check`, dann Umbenennen in `ProjectWrite-Wiederherstellung-<Zeit>` und Öffnen. Nie über eine bestehende Bibliothek; die bisherige bleibt.
- **Export eines Dokuments** als TXT (mit Verlustschutz-Warnung, wenn Formatierung verloren geht) und als Markdown: Medien bitgleich in `<Name>-Medien/` daneben, Links als relative, prozentkodierte Pfade, interne Links werden zu ihrem Text.
- **Gesamtexport** („Bibliothek exportieren …“): ein Ordner mit allen Dokumenten außerhalb des Papierkorbs als Markdown bzw. TXT im Ordnerpfad ihres primären Subnetzes, Archiv in `Archiv/`, Medien in `Medien/`, interne Links als relative Pfade, `index.md`. Dateinamen gültig unter Windows, macOS und Linux. Geschrieben in einen versteckten Ordner, erst am Ende umbenannt; ein Abbruch hinterlässt nichts.
- **Pakete `.pwritepack`:** Dokument (als `.pwrite`), Manifest und Medien. Der Import prüft wie ein Backup, schickt jedes Medium durch die Typprüfung des Medienimports und übernimmt erst dann; vorhandene Medien (gleiche Prüfsumme) werden verwendet, das Dokument bekommt eine neue ID.
- **Drucken und PDF** (`Ctrl + P`, „Als PDF exportieren …“): die sichtbare Schreibfläche mit dem Titel als Kopie in einem eigenen Container, nur dieser im Druck (Druck-CSS mit Seitenrändern, Seitenumbrüchen, Links mit Adresse), dann `window.print` – der Systemdruckdialog.
- **Oberfläche:** Befehle in Befehlspalette und Menü „Datei“ (Drucken, Als PDF exportieren, Backup erstellen), Einstellungen › Bibliothek › Backup mit Ziel, Warnung, Automatik, Anzahl, letztem Backup, Fortschritt und Abbrechen, Prüfen, Wiederherstellen.

## Welche Dateien wurden erstellt oder verändert?

Rust, neu: `services/backup/{mod,archive,control}.rs`, `services/export/{mod,names,pack}.rs`, `commands/{backup,export}.rs`, `tests/{backup,export}.rs`. Geändert: `services/library/mod.rs` (Backup vor Migration, Export-Register), `services/session.rs` (Ziel, Übersicht, Erstellen, Abbrechen, Automatik, Wiederherstellung öffnen), `services/settings/{app,library}.rs` (`backupTargets`, `backup.automatic`, `backup.keep`), `services/media/detect.rs`, `services/ui/menu.rs`, `error.rs` (5 Fehlercodes), `state.rs`, `lib.rs` (eigene Dialoge in E2E-Builds), `commands/mod.rs`, `build.rs`, `capabilities/main-window.json`, `tests/{ipc,library}.rs`.

Frontend, neu: `modules/export/{exportActions.ts,export.test.ts}`, `modules/backup/{backupStore.ts,BackupPreferences.tsx}`. Geändert: `app/{commands.ts,App.tsx,commands.test.ts}`, `modules/commands/{types,keymap}.ts`, `modules/settings/LibraryPreferences.tsx`, `modules/documents/workspaceStore.ts`, `services/ipc.ts`, `index.css` (Druck-CSS), `i18n/{de,en}.json`, `types/generated/`.

E2E: neu `specs/export-backup.e2e.ts`; geändert `sandbox.ts`, `wdio.conf.ts`. Doku: ADR 0021, `docs/{architecture,database,status}.md`, dieser Bericht, `CHANGELOG.md`, `README.md`.

Keine neuen Abhängigkeiten: `zip` (seit Phase 8) liest und schreibt die Archive.

## Welche Tests wurden durchgeführt?

Umfang: 466 Rust-Tests (5 nur von Hand, wie bisher), 723 Vitest-Tests in 41 Dateien, 18 E2E-Dateien (neu `export-backup.e2e.ts` mit 6 Tests).

Lokal (Linux-Container, Xvfb, D-Bus-Sitzung), alle ausgeführt und bestanden:

| Prüfung                                                                        | Ergebnis                                                                      |
| ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------- |
| Prettier, ESLint (`--max-warnings 0`), TypeScript (App und Node-Teil), Vitest  | bestanden                                                                     |
| rustfmt, Clippy `-D warnings` (Linux, Linux + `e2e`, Windows-Ziel)             | bestanden                                                                     |
| Rust-Tests, darunter 25 harte Abbrüche eines Kindprozesses während des Backups | bestanden                                                                     |
| Erzeugte TypeScript-Typen neu geschrieben                                      | bestanden                                                                     |
| E2E-Gesamtlauf                                                                 | erster Lauf 17 von 18 Dateien (siehe „Offene Fehler“), zweiter Lauf 18 von 18 |

Was die Tests zeigen, im Einzelnen:

- **Konsistent während laufender Eingaben** (Rust, E2E): Ein Task speichert 20 Dokumente in einer Schleife, während fünf Backups entstehen; jedes wiederhergestellte Backup hat zu jeder Zeile genau die Datei mit Größe und Prüfsumme. Im echten Programm speichert die Seite ein Dokument in einer Schleife, während „Backup erstellen“ läuft; nach der Wiederherstellung ist das Dokument eine ganze Fassung.
- **Abbruch ohne fertig aussehende Datei** (Rust): 25 harte Abbrüche eines Kindprozesses, der Backups in Folge schreibt; 16 trafen ein laufendes Backup. Danach ist jede Datei mit der Endung `.pwbackup` ein vollständiges, geprüftes Backup, Reste tragen `.part`; die Bibliothek ist unversehrt. Abbrechen über die Oberfläche hinterlässt nichts.
- **Alle Prüfsummen nach der Wiederherstellung** (Rust, E2E): Dateien unter `documents/` und `media/` mit gleichen Prüfsummen, gleiche Zeilen, `integrity_check`, die Suche findet wieder.
- **Verändert oder beschädigt** (Rust, E2E): geändertes Dokument, geänderte Prüfsumme im Manifest, zusätzliche und fehlende Datei, Namen wie `../boese.txt`, `/etc/passwd`, `C:/x`, `backups/alt.pwbackup`, neuere Formatversion, beschädigte Datenbank mit passender Prüfsumme, größer als angegeben, abgeschnittenes Archiv – alles abgelehnt, beim Wiederherstellen ohne Rest. Im Programm: ein halbiertes Backup meldet „nicht in Ordnung“.
- **Keine früheren Backups** (Rust): Zwei Backups nacheinander in `backups/`; das zweite enthält nur `manifest.json`, `library.json`, die Datenbank und die Dateien der Zeilen – keine Backups, keine Entwürfe, Exporte oder Dateien ohne Zeile.
- **Export** (Rust, E2E): Links eines Gesamtexports führen zu ihren Dateien (auch über Unterordner hinweg), `index.md` vollständig, Archiv und Papierkorb richtig, Dateinamen bereinigt, Abbruch ohne Rest; TXT mit Warnung, Markdown mit Medienordner.
- **Pakete** (Rust, E2E): hin und zurück, zweiter Import verwendet das vorhandene Medium, fünf veränderte Pakete und ein als Bild getarntes Programm abgelehnt, ohne Zeilen oder Dateien zu hinterlassen.
- **Drucken** (Vitest, E2E): die Druckkopie hat Titel und Text und keine bearbeitbaren Elemente, der Fenstertitel ist der Dokumenttitel, nach `afterprint` ist alles wie vorher; die Druckregeln für `#pw-print` sind geladen.
- **IPC-Vertrag:** jeder neue Befehl mit gültigen und ungültigen Anfragen, ein Ziel in der Bibliothek abgelehnt.

## Abnahmekriterien Phase 10

| Kriterium                                                                 | Stand                                                                                                                                                               |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PDF-Export und Drucken auf allen drei Betriebssystemen getestet           | **teilweise**: Druckkopie, Druck-CSS und Aufruf des Systemdrucks unter Windows und Linux (E2E); der Systemdialog selbst ist ein Handtest, macOS ist zurückgestellt. |
| Ein Backup während laufender Eingaben ist konsistent                      | **erfüllt** (Rust: 20 Dokumente, 5 Backups; E2E: Speichern in einer Schleife).                                                                                      |
| Ein Abbruch während des Backups hinterlässt keine fertig aussehende Datei | **erfüllt** (Rust: 25 harte Abbrüche; Abbrechen in Rust).                                                                                                           |
| Nach der Wiederherstellung stimmen alle Prüfsummen                        | **erfüllt** (Rust und E2E).                                                                                                                                         |
| Ein verändertes oder beschädigtes Backup wird abgelehnt                   | **erfüllt** (Rust: 13 Fälle; E2E).                                                                                                                                  |
| Das Backup enthält keine früheren Backups                                 | **erfüllt** (Rust).                                                                                                                                                 |

## Was konnte nicht getestet werden und warum?

- **Der Systemdruckdialog und „Als PDF speichern“:** ein natives Fenster, das der eingebettete WebDriver nicht bedient (ADR 0003). Ein Handtest unter Windows und Linux steht aus.
- **macOS:** zurückgestellt.
- **Windows:** läuft in CI (Rust, Vitest, E2E mit WebView2). Lokal nur Linux.
- **Echte Sync-Ordner und Netzlaufwerke als Backup-Ziel:** keine im Container; harte Links fallen dort auf Kopien zurück (in Rust geprüft ist nur der Rückfallweg im Code, nicht ein echtes Netzlaufwerk).
- **Die Automatik im echten Programm:** in E2E abgeschaltet, damit sie keine Messung stört; geprüft sind in Rust die Regel „älter als 24 h“ und das Erstellen und Ausdünnen.

## Offene Fehler

Keine bekannten. Eine Beobachtung: Im ersten lokalen E2E-Gesamtlauf lag die Galerie-Messung aus Phase 8 (zweiter Durchgang, 95 % der Bildwechsel) bei 64 ms, Grenze 50 ms. Im zweiten Lauf waren es 42 ms; in CI bisher immer darunter. Phase 10 ändert an der Galerie nichts, und die Backup-Automatik ist in E2E abgeschaltet; ich halte es für Schwankung des Containers (Software-Rendering, 4 vCPU). Tritt es wieder auf, gehe ich ihm nach.

## Behobene Fehler

F-51 (CI-Lauf 110, Windows): Nach der Prüfung eines Backups mit beschädigter Datenbank blieb die Prüfkopie im Arbeitsordner liegen. Die Integritätsprüfung brach mit einem Fehler ab und schloss die Datenbankverbindung nicht selbst; geschlossen wurde sie erst später im Hintergrund, und so lange verweigert Windows das Löschen. Jetzt wird die Verbindung in jedem Fall geschlossen, bevor der Fehler zurückkommt; ebenso beim Vergleich der Datenbankkopie beim Verschieben einer Bibliothek (Phase 2). Unter Linux tritt das nicht auf, weil dort auch offene Dateien gelöscht werden können.

Während der Arbeit fanden die Tests außerdem: einen Typfehler in einer Meldung, eine Bedingung für die sichtbare Schreibfläche, die jsdom nicht auswerten kann, und zwei Tests der Phasen 1 und 2, die von der Datenbankkopie vor einer Migration ausgingen (jetzt ein vollständiges Backup).

## Gefundene Widersprüche und Abweichungen

1. **Drucken und PDF auf allen drei Systemen** (Abschnitt 25): macOS ist zurückgestellt, und den Druckdialog kann niemand im Test bedienen. Ich schlage vor: ein kurzer Handtest je System mit einer Checkliste (drucken, als PDF speichern, Seitenränder, Bilder, Links) – sag mir, ob du ihn selbst machen willst oder ob ich eine Checkliste in `docs/` anlegen soll.
2. **„Während des Backups schreibt Autosave nur nach `recovery/`“** (Abschnitt 21): Gelöst über die Schreibsperre nur für den kurzen Schnappschuss (Datenbankkopie und harte Links); so lange wartet das Speichern, die Recovery-Entwürfe laufen weiter. Das Ergebnis ist dasselbe – kein Verlust, ein fester Stand –, ohne einen zweiten Speicherweg.
3. **Vor jeder Migration ein vollständiges Backup** statt der Datenbankkopie aus Abschnitt 18 (wie `docs/database.md` es für Phase 10 vorsah). Bei großen Bibliotheken dauert das erste Öffnen nach einem Update entsprechend länger.
4. **Pakete:** Medien über 100 MiB werden abgelehnt (Grenze des Medienimports für Dateien, die nicht aus dem Dateidialog kommen).
5. **Wiederherstellung:** Die wiederhergestellte Bibliothek trägt dieselbe Bibliotheks-ID wie die alte. Beide lassen sich öffnen; Einstellungen, die an der ID hängen (Backup-Ziel, Sitzung), gelten für beide.
6. **Menü:** Im nativen Menü stehen Drucken, Als PDF exportieren und Backup erstellen; die übrigen Befehle in Befehlspalette und Einstellungen.
7. **Ein Zweig je Phase** (Abschnitt 24): weiterhin nur der erlaubte Zweig.

## Entscheidungen (ADR)

- 0021 Export, Drucken, Backup und Wiederherstellung (Stufe 5), angenommen, mit Nachträgen.

## CI

- **Lauf 110** (Commit `d6f1e70`, Bericht): Linux vollständig grün (E2E 18 von 18). Windows rot in den Rust-Tests: F-51 (Prüfkopie einer beschädigten Datenbank blieb liegen). Die übrigen Windows-Schritte liefen danach nicht mehr.
- **Lauf 111** (Commit `3ee149d`, Behebung F-51): **grün** auf Windows und Linux – Prettier, ESLint, TypeScript, Vitest, rustfmt, Clippy, Rust-Tests, reproduzierbare Referenzbibliothek, erzeugte Typen, Lizenzliste, E2E 18 von 18 je System; außerdem Abhängigkeitsprüfung und „Release enthält keine Test-Plugins“.
- Messwerte Windows (Lauf 111): Galerie zweiter Durchgang 95 % 16,7 ms (Grenze 50 ms), erster Durchgang 17,3 ms; Suche mit der Referenzbibliothek 96–143 ms (Ziel 300 ms); Explorer bereit nach 749–782 ms; Öffnen von 50.000 Wörtern 166–181 ms. Export und Backup im Programm in 2,7 s (6 Tests).

## Nächster Schritt

Nach Freigabe: Phase 11 (Mesh; Abschnitt 25).
