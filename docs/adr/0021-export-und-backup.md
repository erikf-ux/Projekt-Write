# ADR 0021 – Export, Drucken, Backup und Wiederherstellung (Stufe 5)

- Status: vorgeschlagen (Phase 10 in Arbeit)
- Datum: 11. November 2026
- Phase: 10

## Kontext

Phase 10 umfasst laut Abschnitt 25: Export als TXT, Markdown und PDF, Drucken, Paketexport und Gesamtexport; Backup, automatische Backups und „Backup prüfen“; Wiederherstellung Stufe 5. Abnahme: PDF-Export und Drucken auf allen drei Betriebssystemen getestet; ein Backup während laufender Eingaben ist konsistent; ein Abbruch während des Backups hinterlässt keine fertig aussehende Datei; nach der Wiederherstellung stimmen alle Prüfsummen; ein verändertes oder beschädigtes Backup wird abgelehnt; das Backup enthält keine früheren Backups.

Regeln stehen in Abschnitt 21 (Export, Backup in vier Schritten, Ausschlussliste, Ziel, Automatik, „Backup prüfen“, Wiederherstellung), Abschnitt 10 (`.pwritepack`, Verlustschutz), Abschnitt 20 (ZIP: Pfade prüfen, entpackte Größe begrenzen, Prüfsummen vergleichen), Abschnitt 18 (`integrity_check` nach Backup und Wiederherstellung) und Abschnitt 4 (Stufe 5 „Backup“).

Die Umwandlungen zwischen den Formaten liegen im Frontend (Editor-Schema und Markdown-Adapter, ADR 0014/0015); sie brauchen keinen offenen Editor.

## Entscheidungen

1. **Aufgabenteilung.** Das Frontend wandelt Inhalte um (`.pwrite` → Markdown oder Text, Markdown → `.pwrite`) und meldet Verluste. Rust wählt Ziele über den Systemdialog, schreibt jede Datei atomar, schreibt Links um, liest und schreibt ZIP-Dateien und prüft sie. Das Frontend sieht keinen Pfad (ADR 0008).

2. **TXT-Export** eines Dokuments: Text ohne Formatierung; enthält das Dokument Formatierungen, warnt der Verlustschutz wie beim Umwandeln (Abschnitt 10) und exportiert erst nach Bestätigung. UTF-8 mit LF.

3. **Markdown-Export** eines Dokuments: Die Datei heißt wie der Titel; Medien, die das Dokument verwendet, landen bitgleich in einem Unterordner `<Titel>-Medien/` neben der Datei, mit ihrem Originalnamen (doppelte Namen nummeriert); `pwmedia://<id>` wird zum relativen Pfad dorthin. Interne Links (`pwrite://<id>`) zeigen beim Einzelexport auf nichts Mitexportiertes: Sie werden zu ihrem Linktext. `.pwrite`-Dokumente wandelt das Frontend um; Verluste zeigt der Verlustschutz (ADR 0014).

4. **Gesamtexport** der Bibliothek als Ordner `Project Write Export <Datum>/` (Abschnitt 21: „damit die Daten auch ohne die App nutzbar bleiben“): Jedes Dokument außerhalb des Papierkorbs als Markdown (TXT bleibt TXT) im Ordnerpfad seines primären Subnetzes, nicht zugeordnete in der obersten Ebene, archivierte in `Archiv/`; Medien in `Medien/` mit Originalnamen; interne Links und Medien als relative Pfade. Dateinamen aus Titeln, für alle drei Systeme bereinigt (verbotene Zeichen, reservierte Namen, Länge), Doppel nummeriert. Dazu `index.md` mit allen Dokumenten nach Subnetz. Ein Gesamtexport schreibt in einen neuen Ordner unter temporärem Namen und benennt ihn erst am Ende um; ein Abbruch hinterlässt höchstens den temporären Ordner.

5. **`.pwritepack`** (Abschnitt 10): ZIP mit `document.pwrite`, `manifest.json` und `media/<id>.<ext>`. Das Manifest nennt Format (`pwritepack`), Formatversion 1, App-Version, Titel und je Medium ID, Pfad, Größe, SHA-256, Originalname und Typ. Markdown- und TXT-Dokumente wandelt das Frontend vorher in `.pwrite` um (mit Verlustschutz). **Import:** ZIP-Prüfung (Punkt 9), Prüfsummen, jedes Medium durch die Typprüfung des Medienimports (ADR 0019); ein Medium mit gleicher Prüfsumme in der Bibliothek wird verwendet, sonst importiert; die Medien-IDs im Dokument werden auf die Bibliothek umgeschrieben; das Dokument bekommt immer eine neue ID. Alles in den zwei Schritten des Medienimports; ein Fehler hinterlässt nichts.

6. **Drucken und PDF** (Abschnitt 21): über Druck-CSS und `window.print`, also den Systemdruckdialog. „Als PDF exportieren“ öffnet denselben Dialog mit dem Hinweis, dort „Als PDF speichern“ bzw. den PDF-Drucker zu wählen. Gedruckt wird das aktive Dokument: nur die Schreibfläche, ohne Oberfläche, mit Seitenrändern, Titel, Seitenumbrüchen vor Überschriften nur wo nötig, Bilder in Seitenbreite, Links mit Adresse, Quelltextansicht als Rich Text. Kein Export ohne Dialog (unter WebKitGTK ein offener Fehler).

7. **Backup `.pwbackup`** in den vier Schritten aus Abschnitt 21:
   1. Offene Änderungen speichern. Während des Backups schreiben Library-Dokumente nur Recovery-Entwürfe (Stufe 2, laufen weiter wie bisher); das Speichern in die Bibliothek wartet und läuft danach. Schreiben bleibt also möglich, nichts geht verloren.
   2. Unter der Schreibsperre der Bibliothek: Datenbank mit `VACUUM INTO` in `tmp/backup-<id>/projectwrite.db`; Dokumente und Medien als harte Links in denselben Ordner (Dateien werden nur atomar ersetzt, ein harter Link ist deshalb ein fester Stand); geht das nicht (FAT, Netzlaufwerk), als Kopie. Dann wird die Sperre frei.
   3. `PRAGMA integrity_check` der Kopie; SHA-256 jeder Datei; `manifest.json` mit Format (`pwbackup`), Formatversion 1, App-Version, Schemaversion, Bibliotheks-ID und -format, Zeit und je Datei Pfad, Größe und SHA-256.
   4. ZIP unter `<Name>.pwbackup.part` im Ziel, mit `fsync`; erneut lesen und alles prüfen wie „Backup prüfen“ (Punkt 9); erst dann in `<Name>.pwbackup` umbenennen. Ein Abbruch hinterlässt höchstens eine `.part`-Datei, die die App beim nächsten Start im Standardziel entfernt.
   - **Ausschlussliste** genau wie Abschnitt 21: `backups/`, `tmp/`, `recovery/`, `thumbnails/`, `exports/`, `library.lock`, `-wal`, `-shm`. Aufgenommen werden nur `library.json`, die Datenbankkopie und die Dateien unter `documents/` und `media/` – über eine Positivliste, nicht über das Durchlaufen des Ordners. Ein Ziel innerhalb der Bibliothek außer `backups/` wird abgelehnt.
   - **Ziel:** frei wählbar (Systemdialog), Standard `backups/` der Bibliothek. Der Pfad ist eine Eigenschaft des Rechners und steht in den App-Einstellungen (`settings.json`, je Bibliotheks-ID), nicht in der Datenbank. Liegt das Ziel auf demselben Datenträger wie die Bibliothek, warnt die App (Abschnitt 21).
   - **Automatik** (Abschnitt 21): standardmäßig täglich beim Start (wenn das letzte gelungene Backup älter als 24 h ist), im Hintergrund nach dem Öffnen; 7 bleiben erhalten, ältere entfernt die App nur, wenn sie zu ihrem Namensmuster gehören. Einstellbar: an/aus, Anzahl. **Vor jeder Datenbankmigration** entsteht ein vollständiges Backup in `backups/pre-migration/` und ersetzt die bisherige Kopie der Datenbank (Abschnitt 18, `docs/database.md`); scheitert es, wird nicht migriert.

8. **Wiederherstellung (Stufe 5):** Backup wählen (Dateidialog), dann einen neuen, leeren Zielordner. Vor jedem Schreiben: Prüfung wie Punkt 9, Schemaversion nicht neuer als die App. Entpacken nach `<Ziel>.restoring-<id>` neben dem Ziel, alle Prüfsummen erneut, `integrity_check` der Datenbank, dann umbenennen und die neue Bibliothek öffnen (Migration, falls älter). Die alte Bibliothek bleibt, bis der Nutzer sie entfernt. Nie über eine bestehende Bibliothek.

9. **„Backup prüfen“ und ZIP-Sicherheit** (Abschnitt 20), für Backups, Pakete und die Wiederherstellung gleich: höchstens 200.000 Einträge; nur relative Pfade aus erlaubten Zeichen, ohne `..`, ohne absolute Pfade, Laufwerksbuchstaben, Backslashes, Steuerzeichen und doppelte Namen (auch nicht bis auf Groß-/Kleinschreibung); keine symbolischen Links; jeder Eintrag in der Positivliste (Punkt 7) bzw. im Paketaufbau; entpackte Größe je Eintrag laut Manifest und insgesamt begrenzt (Verhältnis zur gepackten Größe, absolute Grenze), beim Lesen gezählt statt den Angaben im ZIP zu glauben; jede Datei im Manifest und jede Datei des ZIP im Manifest; Prüfsummen; Datenbank mit `integrity_check`. Prüfen verändert nichts (liest nach `tmp/`, räumt auf).

10. **Befehle und Oberfläche:** „Exportieren als TXT …“, „Exportieren als Markdown …“, „Als PDF exportieren …“, „Drucken …“ (`Ctrl + P`), „Als Paket exportieren …“, „Paket importieren …“, „Bibliothek exportieren …“, „Backup erstellen“, „Backup prüfen …“, „Aus Backup wiederherstellen …“ (Stufe 5) in Menü „Datei“ und Befehlspalette; Einstellungen › Bibliothek: Backup-Ziel, Automatik, Anzahl, letztes Backup. Lange Vorgänge zeigen Fortschritt und lassen sich abbrechen.

11. **Abweichung macOS:** Abschnitt 25 verlangt PDF-Export und Drucken auf allen drei Systemen. macOS ist auf Wunsch des Auftraggebers zurückgestellt (2. November 2026); geprüft werden Windows und Linux. Den Systemdruckdialog kann der eingebettete WebDriver nicht bedienen (ADR 0003): E2E prüft das Druck-CSS (Darstellung im Druckmedium) und dass `window.print` aufgerufen wird; das echte Drucken und Speichern als PDF ist ein Handtest je System.

## Folgen

- Keine neue Abhängigkeit: `zip` (seit Phase 8) liest und schreibt die Archive.
- Backups brauchen kurz Platz in `tmp/` für die Datenbankkopie; Dokumente und Medien nur als harte Links, wo möglich.
- Tests: Rust für Export, Paket, Backup (Konsistenz bei gleichzeitigem Speichern, harte Abbrüche während des Backups, veränderte und beschädigte Backups, Ausschlussliste), Wiederherstellung (alle Prüfsummen); Vitest für Umwandlung und Oberfläche; E2E für Druck-CSS, Export und Backup im echten Programm.
