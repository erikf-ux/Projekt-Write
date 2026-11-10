# ADR 0019 – Medien: Import, Typprüfung, Medien-Handler, Vorschaubilder, PDF und Verweise

- Status: angenommen
- Datum: 10. November 2026
- Phase: 8

## Kontext

Phase 8 umfasst laut Abschnitt 25:

- Bilderimport und Import ganzer Ordner mit SHA-256-Duplikaterkennung;
- Medienverwaltung, Galerie und Vorschaubilder;
- PDF-Anzeige und Office-Import;
- den Medien-Handler;
- Medienverknüpfungen, Bilder und PDF-Verweise im Editor.

Abnahme:

- Originaldateien bleiben bytegleich, belegt durch Prüfsummen vor und nach dem Import.
- Ein abgebrochener Import hinterlässt weder Dateien noch Datenbankeinträge.
- Ein SVG mit Skript führt nichts aus. Eine Test-PDF nach dem Muster von CVE-2024-4367 führt nichts aus.

Regeln stehen in den Abschnitten 9 (Speicherstruktur, Import in die Bibliothek), 17 (Medienverwaltung, Medien sicher anzeigen) und 20 (Positivliste, CSP, Pfade). Dazu kommen Abschnitt 10 (`.pwrite` referenziert Medien), Abschnitt 12 (Bilder und PDF-Verweise im Editor, eingefügte Bilder werden importiert), Abschnitt 13 (Explorer-Eintrag „Medien“) und Abschnitt 18 (geprüftes Muster für Zuordnungen mit `media_id`).

## Entscheidungen

1. **Umfang.**
   - Bilder: PNG, JPEG (`.jpg`, `.jpeg`), WebP, GIF, SVG.
   - PDF.
   - Office als Anhang: DOCX, XLSX, PPTX.

   Verschoben: „PDF im Mesh darstellen“ in Phase 11 (das Mesh entsteht dort), Mediennamen in der Suche in Phase 9, `.pwritepack` in Phase 10. Audio und Video kommen später (Abschnitt 17).

2. **Migration 0008.**
   - **`media`**: wie im Entwurf in `docs/database.md`, mit drei Abweichungen:
     - `title` (Anzeigename, umbenennbar) und `description` (0–2000 Zeichen) statt `caption`; Bildunterschriften gehören zur einzelnen Verwendung im Dokument (Punkt 10);
     - dazu `last_opened_at` für „Zuletzt verwendet“;
     - `page_count` setzt die PDF-Anzeige, sobald ein PDF zum ersten Mal geöffnet ist.
   - **`document_media`** und **`media_tags`** entstehen wie geplant.
   - **`subnet_memberships`** wird nach dem geprüften Muster aus Abschnitt 18 neu aufgebaut: neue Tabelle, Daten kopieren, alte entfernen, umbenennen; `created_at` bleibt.
3. **Typprüfung über den Inhalt, Positivliste** (Abschnitt 20). Erkannt wird an den ersten Bytes:
   - PNG: Signatur; JPEG: `FF D8 FF`; GIF: `GIF87a`/`GIF89a`; WebP: `RIFF…WEBP`;
   - PDF: `%PDF-` in den ersten 1024 Bytes;
   - SVG: UTF-8-Text, dessen Wurzelelement nach XML-Deklaration, Kommentaren und DOCTYPE `<svg` ist;
   - Office: ZIP mit `[Content_Types].xml`, darin der Hauptinhaltstyp von DOCX, XLSX oder PPTX.

   Die Endung muss zum erkannten Typ passen, sonst lehnt Rust ab. Makro-Dateien lehnt Rust ab: die Endungen `.docm`, `.xlsm`, `.pptm`, ein Hauptinhaltstyp mit `macroEnabled` oder ein Teil `vbaProject.bin`. ZIP-Grenzen: höchstens 10.000 Einträge; `[Content_Types].xml` höchstens 1 MiB entpackt. Alles andere wird mit Grund abgelehnt.

4. **Grenzwerte.**
   - Eine Datei aus dem Dateidialog oder einem Ordner: höchstens 1 GiB.
   - Abgelegte oder eingefügte Medien kommen als Rohdaten über IPC: höchstens 100 MiB.
   - SVG: höchstens 10 MiB.
   - Bilder: höchstens `media.maxMegapixels` Megapixel (Bibliothekseinstellung, Startwert 100, 1–1000). Geprüft wird aus dem Dateikopf, bevor das Bild aufgenommen wird; dieselbe Grenze gilt beim Dekodieren für Vorschaubilder.
5. **Importablauf in zwei Schritten** (Abschnitt 9):
   - **Vorbereiten.** Rust öffnet jede Datei nur lesend und folgt keinem symbolischen Link. Es kopiert sie nach `tmp/import-<id>/`, berechnet dabei SHA-256 und prüft den Typ. Bei Duplikaten (gleiche Prüfsumme wie ein vorhandenes, nicht endgültig gelöschtes Medium) fragt die Oberfläche: vorhandenes verwenden oder trotzdem importieren, je Datei oder für alle.
   - **Abschließen.** Rust schreibt zuerst ein Journal `tmp/import-<id>.json` mit den Zielnamen. Dann folgt eine Transaktion: Zeilen schreiben, Dateien mit UUID-Namen nach `media/<art>/` verschieben, bestätigen. Schlägt etwas fehl, wandern die verschobenen Dateien zurück und der Batch wird entfernt.
   - **Abbrechen** entfernt den Batch.
   - **Absturz.** Bricht der Prozess zwischen Verschieben und Bestätigen ab, findet das nächste Öffnen der Bibliothek das Journal. Es löscht die Dateien, zu denen es keine Zeile gibt, und erst danach `tmp/`. So hinterlässt ein abgebrochener Import weder Dateien noch Zeilen.
   - **Originale** werden nie geöffnet, um zu schreiben. Ein Test vergleicht ihre Prüfsummen vor und nach dem Import.
6. **Ordnerimport.**
   - Rekursiv; symbolische Links und versteckte Einträge werden übersprungen.
   - Dateien außerhalb der Positivliste werden mit Grund als „übersprungen“ gelistet.
   - Der gewählte Ordner wird als `import_folder` gespeichert, der Weg darunter als Teil davon.
   - Auf Wunsch entsteht aus der Ordnerstruktur eine Subnetz-Struktur unter einem gewählten Subnetz oder auf oberster Ebene. Ein gleichnamiges Subnetz auf derselben Ebene wird wiederverwendet; das Medium wird dem Subnetz seines Ordners primär zugeordnet. Alles in derselben Transaktion.
   - Der Ordnerimport nimmt **Medien** auf. Textdokumente im Ordner werden gelistet und übersprungen; sie kommen wie bisher über Ablegen in die Bibliothek (Abweichung, im Bericht).
7. **Vorschaubilder.**
   - Rust (`image`, nur PNG, JPEG, GIF und WebP) verkleinert auf höchstens 320 px Kantenlänge: als PNG, wenn das Bild Transparenz hat, sonst als JPEG (Qualität 80). Ablage: `thumbnails/images/<id>.png|jpg`.
   - Erzeugt werden sie im Hintergrund nach dem Import und bei Bedarf durch den Handler, wenn eins fehlt. Misslingt das, zeigt die Galerie ein Symbol.
   - SVG bekommt kein Rasterbild; die Galerie zeigt die Datei selbst über den Handler.
   - PDF: Das Frontend zeichnet Seite 1 mit PDF.js und schickt das PNG an Rust (`media_store_thumbnail`). Rust prüft Signatur, Größe (≤ 512 px) und Dekodierbarkeit und legt es in `thumbnails/pdf/<id>.png` ab.
   - Vorschaubilder sind abgeleitet; sie stehen nicht in der Datenbank und gehen mit dem Medium.
8. **Medien-Handler** (Abschnitt 17).
   - Eigenes Protokoll `pwmedia`: `pwmedia://localhost/<id>` und `…/<id>/thumbnail`; unter Windows `http://pwmedia.localhost/…`, wie es WebView2 verlangt. Das Asset-Protokoll bleibt aus.
   - Rust löst die ID über die Datenbank der geöffneten Bibliothek zu `media/<art>/<id>.<ext>` auf: über `SafeRoot`, ohne symbolische Links, nur reguläre Dateien.
   - Ausgeliefert werden nur Bilder und PDFs. Anhänge liefert der Handler nicht aus, denn Project Write liest nichts aus ihnen.
   - Kopfzeilen jeder Antwort:
     - `Content-Type` aus der Typprüfung;
     - `X-Content-Type-Options: nosniff`;
     - `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox`;
     - `Cache-Control: no-cache`;
     - `Access-Control-Allow-Origin` (das Frontend lädt PDFs mit `fetch`).
   - SVG erscheint nur als `<img>`-Quelle, nie im DOM; zusammen mit der Sandbox-CSP führt es nichts aus.
   - In Dokumenten steht immer `pwmedia://<id>`; die Oberfläche wandelt das mit `convertFileSrc` in die Adresse der Plattform. Das Frontend sieht keine Pfade.
9. **PDF-Anzeige mit PDF.js 6.5.438** (aktuelle stabile Version, Apache-2.0), ohne die Viewer-Komponente:
   - Aufruf: `getDocument({ data, useWasm: false, enableXfa: false, … })`.
   - Worker, Standardschriften (`standard_fonts/`), CMaps und die JavaScript-Ersatzdecoder für JPX und JBIG2 liegen lokal im Build.
   - WebAssembly ist aus, weil die CSP `'wasm-unsafe-eval'` nicht erlaubt. Ohne Ersatz bleibt nur QCMS, also ICC-Farbprofile; die Farben stimmen dann nicht ganz.
   - PDF-eigene Skripte laufen nie: `pdf.sandbox.mjs` und die Viewer-Komponente werden nicht geladen.
   - `isEvalSupported: false` aus Abschnitt 17 gibt es in 6.5 nicht mehr. Der Build enthält weder `eval(` noch `new Function`; die CSP verbietet `unsafe-eval` zusätzlich. Abweichung, im Bericht.
   - Funktionen: Seitenwechsel, Zoom, Textebene zum Markieren und Kopieren, eine eigene Linkebene. Weblinks öffnen erst nach Bestätigung, über die Linkprüfung von Rust (`http`, `https`, `mailto`); interne Links springen zur Seite.
   - **CSP-Lockerung:** `connect-src` bekommt `'self'`, denn PDF.js lädt Schriftdaten und CMaps aus dem eigenen Build per `fetch`. Erlaubt sind damit nur die eigenen Dateien der App.
10. **Editor** (Abschnitt 12):
    - **Bilder:** `![Alternativtext](pwmedia://<id> "Bildunterschrift")`. Die Größe steht in der erlaubten HTML-Teilmenge, `<img src="pwmedia://…" width="50%">`; sie ändert nur die Anzeige. Die Anzeige läuft über den Handler, mit der Unterschrift darunter.
    - **Bilder mit Webadresse** bleiben Hinweise (Phase 4). Fehlt ein Medium, erscheint „Medium fehlt“.
    - **Einfügen:** über „Bild einfügen …“ (Auswahl aus der Bibliothek oder Import); abgelegte und eingefügte Bilddateien werden importiert und eingefügt.
    - **PDF-Verweise:** ein Link `[Text](pwmedia://<id>#page=12)`. `Strg + Klick` öffnet das PDF auf der Seite. „PDF-Verweis einfügen …“ wählt PDF und Seite.
    - **`.pwrite`** speichert dasselbe in Bildknoten und Linkmarken. Beim Speichern schreibt Rust die Liste `media` der Hülle (ID, SHA-256, Originalname, Typ).
11. **Medienverknüpfungen.** `document_media` entsteht beim Speichern aus dem Inhalt, wie `document_links`: Bilder mit Seite 0, PDF-Verweise mit ihrer Seite. Die Medienansicht zeigt „Verwendet in“.
12. **Struktur für Medien.** Zuordnungen, primäres Subnetz, Verschieben, Tags und Struktur-Undo gelten für Elemente, also Dokumente und Medien (ADR 0017). Befehle und Journal tragen dazu eine Elementreferenz `{ kind, id }`. Der Explorer zeigt Medien in ihren Subnetzen und unter „Medien“.
13. **Papierkorb, Archiv, Favoriten** wie bei Dokumenten (ADR 0016).
    - „Endgültig löschen“ entfernt Zeile, Datei und Vorschaubild, aber nur, wenn kein Dokument das Medium mehr verwendet (Abschnitt 9: „Dateien werden nur gelöscht, wenn sie nicht mehr referenziert werden“). Sonst bleibt es im Papierkorb, und die Oberfläche nennt den Grund. Die Aufbewahrungsfrist überspringt solche Medien.
    - Ein Medium im Papierkorb wird weiter angezeigt, damit Dokumente nicht leer werden.
14. **Mit dem Standardprogramm öffnen.** Für Anhänge, auf Wunsch auch für Bilder und PDFs: Rust kopiert die Datei schreibgeschützt nach `tmp/open/<id>/<Originalname>` und öffnet sie mit dem System. Änderungen fließen nicht zurück, das sagt die Oberfläche. Die Datei in der Bibliothek bleibt so bytegleich.
15. **Wartung beim Öffnen.**
    - Rust arbeitet zuerst die Import-Journale ab (Punkt 5), dann leert es `tmp/`.
    - Dateien in `media/` ohne Zeile und Zeilen ohne Datei werden im Protokoll gemeldet, nicht still gelöscht.
16. **Referenzbibliothek** (Abschnitt 6): 5.000 Medien – Bilder aller fünf Formate, PDFs und Anhänge – mit Zuordnungen und Tags. Reproduzierbar, im Fingerabdruck.
17. **Abnahmetests:**
    - **Bytegleich:** Prüfsummen und Bytes der Originale vor und nach dem Import, für Einzeldateien und Ordner; die Kopie in der Bibliothek gleicht dem Original.
    - **Abbruch:** Abbrechen nach dem Vorbereiten, ein Fehler während des Abschließens (eingeschleust) und harte Abbrüche während des Abschließens (Kindprozess, wie ADR 0007). Jedes Mal bleiben weder Dateien noch Zeilen.
    - **SVG mit Skript:** `<script>`, `onload` und ein eingebettetes `<foreignObject>` mit Skript. Im echten Programm in Galerie, Vorschau und Dokument geprüft: Es ändert nichts. Die Kopfzeilen prüft ein Rust-Test.
    - **CVE-2024-4367:** eine Test-PDF mit einer Type1-Schrift, deren `FontMatrix` Code einschleust. Im echten Programm geöffnet: Sie führt nichts aus, die Seite wird gezeigt.

## Nachträge aus der Umsetzung

- **Ablegen auf das Fenster:** Textdokumente (`.txt`, `.md`, `.markdown`, `.pwrite`) gehen wie bisher in den Dokumentimport, jede andere Datei in den Medienimport. Ohne Rückfrage nötig wird sie sofort übernommen; bei einer Ablehnung oder einem Duplikat öffnet der Importdialog, und die Dateien dahinter warten, bis er geschlossen ist. Liegt die Ablage in einer Schreibfläche, nimmt der Editor die Medien selbst.
- **Einfügen und Ablegen in der Schreibfläche:** ohne Dialog. Ein Duplikat verwendet das vorhandene Medium (Abschnitt 9 sieht sonst die Rückfrage vor; beim Einfügen eines Bildes ist „vorhandenes verwenden“ die Antwort, die den Text nicht verändert). Eine abgelehnte Datei nennt ein Hinweis mit Grund. Eingefügte Dateien ohne Namen bekommen „Eingefügtes Bild“ und die Endung ihres Typs. Bilder werden als Bild, PDFs und Anhänge als Link mit ihrem Titel eingefügt.
- **„Bild bearbeiten …“** (Doppelklick auf ein Bild der Bibliothek oder Befehl) ändert Alternativtext, Unterschrift und Anzeigegröße (Original, 25, 50, 75, 100 %). Eine neue Größe ist eine Breite; die Höhe entfällt dann. Als Alternativtext nimmt das Einfügen die Beschreibung des Mediums, sonst seinen Titel. Weil WebKitGTK die Auswahl eines Bildes wieder in einen Cursor daneben verwandelt, gilt auch das Bild direkt hinter (sonst vor) einem leeren Cursor als ausgewählt.
- **Menü:** „Bild einfügen …“ und „PDF-Verweis einfügen …“ stehen im Menü „Format“ hinter „Link …“; ein PDF-Verweis über markiertem Text verlinkt diesen Text.
- **Ansichts-Tabs:** Galerie und Medien öffnen als eigene Tabs neben den Dokumenten. Die Sitzung (Stufe 1) stellt sie nicht wieder her; sie hält weiter nur Dokumente.
- **Explorer:** Medien stehen in ihren Subnetzen und unter „Medien“ (alle). „Nicht zugeordnet“ zeigt weiter nur Dokumente; ein Medium ohne Subnetz findet sich unter „Medien“.
- **Referenzbibliothek:** Die 5.000 Medien entstehen einzeln über Vorbereiten und Abschließen, wie eine eingefügte Datei (72 % Bilder in allen fünf Formaten, 20 % PDFs mit 1 bis 40 Seiten, 8 % DOCX); die Seitenzahl der PDFs setzt der Generator, wie es sonst die PDF-Anzeige beim ersten Öffnen tut. Vorschaubilder von PDFs entstehen erst in der Oberfläche. Etwa jedes achte Markdown-Dokument zeigt ein Bild, jedes zehnte verweist auf eine PDF-Seite; etwa drei von vier Medien liegen in Subnetzen. Medien, Verwendungen, Zuordnungen und Medien-Tags gehen in den Fingerabdruck ein; der Eintrag `referenceLibrary` in `library.json` nennt die Zahl der Medien, damit ein älterer Zwischenspeicher ohne Medien nicht weiterverwendet wird.
- **Debug-Builds:** `image` und seine Codecs werden wie `sha2` und SQLite auch in Debug-Builds optimiert übersetzt (`[profile.dev.package.*]`); Verhalten unverändert, Vorschaubilder in Tests und der Referenzbibliothek entstehen so in einem Bruchteil der Zeit.

## Abhängigkeiten

- `image` 0.25.10 (MIT OR Apache-2.0), nur PNG, JPEG, GIF und WebP, ohne `rayon`: Dekodieren mit Grenzwerten, Verkleinern, Kodieren der Vorschaubilder. Reines Rust; `png` liegt schon im Baum.
- `zip` 9.5.0 (MIT), nur `deflate-flate2` (über das vorhandene `flate2`): Lesen von `[Content_Types].xml` und der Teilenamen für die Office-Prüfung. Phase 10 braucht es für `.pwritepack` und `.pwbackup` ohnehin.
- `pdfjs-dist` 6.5.438 (Apache-2.0). Die optionale Abhängigkeit `@napi-rs/canvas` dient nur Node.js; sie wird nicht ausgeliefert.

## Folgen

- Das Frontend bekommt keine neuen Rechte: Medien laufen über Befehle und den Handler.
- Das Struktur-Undo und die Explorer-Zeilen kennen zwei Elementarten.
- Medien im Papierkorb bleiben sichtbar, solange ein Dokument sie verwendet.
