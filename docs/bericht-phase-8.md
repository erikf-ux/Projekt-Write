# Bericht Phase 8 – Medien

Stand: 10. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig). Entwicklungsumgebung: Cloud-Container, Ubuntu 24.04, 4 vCPU, 15 GB RAM, keine GPU (Xvfb, Software-Rendering), WebKitGTK, Rust 1.99.1, Node.js 22. CI: GitHub Actions `windows-latest` und `ubuntu-24.04`; macOS ist auf Wunsch des Auftraggebers zurückgestellt. Entwickelt und geprüft wurde lokal unter Linux; den CI-Lauf nennt der Abschnitt „CI“.

**Die vier Abnahmekriterien sind erfüllt**, jeweils in Rust-Tests und im echten Programm (E2E): Originale bleiben bytegleich, ein abgebrochener Import hinterlässt weder Dateien noch Zeilen, ein SVG mit Skript und eine Test-PDF nach dem Muster von CVE-2024-4367 führen nichts aus.

**Drei Punkte solltest du kennen** (Einzelheiten unter „Gefundene Widersprüche und Abweichungen“):

1. Beim **Einfügen eines Bildes in den Text** fragt die App bei einem Duplikat nicht, sondern verwendet das vorhandene Medium (Punkt 3).
2. Die **CSP** erlaubt jetzt `connect-src 'self'`, weil PDF.js Schriftdaten aus dem eigenen Build lädt; `isEvalSupported` gibt es in PDF.js 6.5 nicht mehr (Punkte 1 und 2).
3. Die **Neubewertung des Isolation Pattern** war vor Phase 8 fällig; ich habe sie erst während der Phase festgehalten. Ergebnis: bleibt aus (Punkt 6).

## Was wurde implementiert?

Entscheidungen in ADR 0019 (17 Punkte und Nachträge).

- **Medien in der Bibliothek** (Abschnitte 9 und 17): Bilder (PNG, JPEG, GIF, WebP, SVG), PDF und Office-Dateien als Anhang (DOCX, XLSX, PPTX), in `media/images/`, `media/pdf/`, `media/attachments/` mit UUID-Namen; Migration 0008 (`media`, `document_media`, `media_tags`, `subnet_memberships` mit `media_id` nach dem geprüften Muster aus Abschnitt 18 neu aufgebaut).
- **Typprüfung über den Inhalt** (Abschnitt 20): Signaturen und Wurzelelement (SVG), Office über `[Content_Types].xml`. Eine Endung, die nicht zum Inhalt passt, Makro-Dateien (Endung, Inhaltstyp oder `vbaProject.bin`), Dateien außerhalb der Positivliste und Bilder über der Pixelgrenze (Einstellung `media.maxMegapixels`, Start 100) werden mit Grund abgelehnt. Grenzwerte: 1 GiB je Datei, 100 MiB beim Ablegen und Einfügen, SVG 10 MiB.
- **Import in zwei Schritten** (Abschnitt 9): Vorbereiten kopiert jede Datei nur lesend nach `tmp/import-<id>/`, berechnet SHA-256 und prüft den Typ; der Dialog zeigt Fortschritt, die übernommenen Dateien, die übersprungenen mit Grund und fragt bei **Duplikaten** (vorhandenes verwenden, trotzdem importieren, auslassen – je Datei oder für alle). Abschließen schreibt erst ein Journal, dann in einer Transaktion Zeilen und Umbenennungen; bei einem Fehler wandern die Dateien zurück. Bricht der Prozess mittendrin ab, räumt das nächste Öffnen der Bibliothek anhand des Journals auf.
- **Ordnerimport**: rekursiv, ohne symbolische Links und versteckte Einträge; auf Wunsch wird die Ordnerstruktur zu Subnetzen (unter einem gewählten Subnetz oder oben), gleichnamige werden wiederverwendet. Textdokumente im Ordner werden übersprungen (Grund: „Textdokument – über Ablegen importieren“).
- **Ablegen auf das Fenster**: Textdokumente wie bisher als Dokument, alle anderen Dateien in den Medienimport; über einer Schreibfläche fügt der Editor sie ein.
- **Galerie** (virtualisiert, Vorschaubilder, Filter nach Art und Titel, Tastatur: Pfeile, Bild auf/ab, Pos1/Ende, Enter) und **Medienansicht** als eigene Tabs: Bild eingepasst oder in Originalgröße, Details, Titel und Beschreibung, „Verwendet in“, Favorit, Verschieben, Tags, Archiv, Papierkorb; Anhänge öffnen als schreibgeschützte Kopie mit dem Standardprogramm.
- **Vorschaubilder**: Rust verkleinert Bilder auf höchstens 320 px (PNG mit Transparenz, sonst JPEG); PDF-Vorschauen zeichnet PDF.js nacheinander, sobald die Galerie sie braucht, Rust prüft das PNG. SVG zeigt die Galerie über den Handler selbst.
- **PDF-Ansicht mit PDF.js 6.5.438**, erst beim ersten PDF geladen: Seiten, Zoom, Textebene zum Markieren und Kopieren, eigene Linkebene (interne Links springen, Weblinks erst nach Rückfrage und über die Linkprüfung von Rust). Ohne Viewer, ohne PDF-Skripte, ohne WebAssembly, mit lokalen Schriften und CMaps.
- **Medien-Handler `pwmedia://`** (Abschnitt 17): löst IDs über die Datenbank auf, liefert nur Bilder und PDFs, jede Antwort mit `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox` und `nosniff`. Das Frontend sieht keinen Pfad; das Asset-Protokoll bleibt aus.
- **Struktur für Medien**: Zuordnungen mit primärem Subnetz, Verschieben, Tags, Favoriten, Archiv, Papierkorb und Struktur-Undo gelten für Dokumente und Medien; der Explorer zeigt Medien in ihren Subnetzen und unter „Medien“. Ein Medium, das ein Dokument verwendet, bleibt im Papierkorb, bis es niemand mehr verwendet.
- **Editor** (Abschnitt 12): Bilder der Bibliothek erscheinen über den Handler mit Unterschrift (Bildtitel) und Anzeigegröße (`<img … width="50%">`), Bilder mit Webadresse bleiben Hinweise, ein fehlendes Medium zeigt „Medium fehlt“. „Bild einfügen …“ und „PDF-Verweis einfügen …“ (Menü „Format“, Befehlspalette) wählen aus der Bibliothek oder importieren neu; ein PDF-Verweis zeigt auf eine Seite und öffnet sie mit `Strg + Klick`. Eingefügte und abgelegte Bilddateien werden importiert und eingefügt. „Bild bearbeiten …“ (Doppelklick oder Befehl) ändert Alternativtext, Unterschrift und Größe.
- **Medienverknüpfungen**: `document_media` entsteht beim Speichern aus dem Inhalt (Bilder mit Seite 0, PDF-Verweise mit ihrer Seite); `.pwrite` führt die Liste `media` in der Hülle.
- **Referenzbibliothek** (Abschnitt 6): jetzt mit 5.000 Medien – 3.593 Bilder in allen fünf Formaten, 1.020 PDFs, 387 DOCX –, 1.193 Verwendungen in Dokumenten, 5.685 Zuordnungen und 5.022 Medien-Tags; über den echten Import erzeugt, reproduzierbar, im Fingerabdruck.

## Welche Dateien wurden erstellt oder verändert?

Rust, neu: `services/media/{mod,detect,import,thumbnails,handler}.rs`, `database/media.rs`, `commands/media.rs`, `migrations/0008_media.sql`, `tests/media.rs`, `examples/reference_library/media_files.rs`. Geändert: `services/structure.rs`, `database/structure.rs`, `commands/structure.rs` (Elemente statt Dokumente), `services/documents.rs`, `database/documents.rs` (`document_media`, Hülle), `services/library/mod.rs` (Journale beim Öffnen), `services/session.rs`, `services/settings/library.rs` (`media.maxMegapixels`), `services/dialog.rs` (Dateiauswahl), `services/ui/menu.rs` (zwei Menüeinträge), `state.rs`, `lib.rs` (Protokoll `pwmedia`, Testdialoge), `error.rs` (6 Fehlercodes), `build.rs`, `capabilities/main-window.json`, `tauri.conf.json` (CSP), `examples/reference_library/{generate,main}.rs`, `tests/{ipc,migrations,security_config,structure,reference_library}.rs`, `Cargo.toml` (auch Optimierung der Bild-Codecs in Debug-Builds).

Frontend, neu: `modules/media/` (`mediaStore.ts`, `mediaUrl.ts`, `pdf.ts`, `PdfView.tsx`, `pdf.css`, `MediaView.tsx`, `Gallery.tsx`, `MediaImportDialog.tsx`, `MediaPanels.tsx`, Test `mediaStore.test.ts`), `modules/editor/{MediaPicker.tsx,ImageDialog.tsx,media.test.ts}`. Geändert: `modules/editor/{schema/image.ts,schema/extensions.ts,session.ts,DocumentEditor.tsx,LinkDialog.tsx,editor.css}`, `modules/documents/{workspaceStore.ts,explorerRows.ts,Explorer.tsx,DropZone.tsx,importFiles.ts}` samt Tests, `modules/structure/structureStore.ts`, `app/{Workbench.tsx,App.tsx,commands.ts,formatCommands.ts}`, `modules/commands/types.ts`, `services/ipc.ts` (Rohdaten-Befehl), `i18n/{de,en}.json`, `vite.config.ts` (PDF.js-Dateien in den Build), `types/generated/`.

E2E: neu `fixtures.ts` (PNG, ZIP/DOCX, PDF, CVE-Test-PDF, SVG mit Skript – ohne weitere Abhängigkeiten), `specs/media.e2e.ts`; geändert `sandbox.ts`, `wdio.conf.ts`, `specs/{workbench,reference-prepare,reference}.e2e.ts`. Doku: ADR 0019 (Nachträge), ADR 0005 (Neubewertung), `docs/{architecture,database,dependencies,status}.md`, dieser Bericht, `CHANGELOG.md`, `README.md`, `THIRD-PARTY-LICENSES`.

**Neue Abhängigkeiten** (begründet in ADR 0019 und `docs/dependencies.md`):

- `image` 0.25.10 (MIT OR Apache-2.0): Maße, Dekodieren mit Grenzwerten, Vorschaubilder; nur PNG, JPEG, GIF, WebP, ohne `rayon`, reines Rust.
- `zip` 9.5.0 (MIT): Office-Prüfung; Phase 10 braucht es für `.pwritepack` und `.pwbackup`.
- `pdfjs-dist` 6.5.438 (Apache-2.0): PDF-Ansicht, wie Abschnitt 7 sie vorsieht.

Die Lizenzliste wächst um 17 Crates (darunter `moxcms` und `pxfm` unter BSD-3-Clause OR Apache-2.0, beide in `deny.toml` erlaubt) und 3 npm-Pakete.

## Welche Tests wurden durchgeführt?

Umfang: **397 Rust-Tests** (261 Unit, 136 Integration; Phase 7: 348), **708 Vitest-Tests** in 39 Dateien (695 in 37), **E2E_GESAMT WebdriverIO-Tests** in 16 Dateien (56 in 15).

Lokal (Linux-Container, Xvfb, D-Bus-Sitzung), alle ausgeführt und bestanden:

| Prüfung                                                                                                                                                                                                            | Ergebnis                                                                                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| Prettier, ESLint (`--max-warnings 0`), TypeScript (App und Node-Teil), Vitest (708)                                                                                                                                | bestanden                                                                                                                                         |
| rustfmt, Clippy `-D warnings` (Linux, Linux + `e2e`)                                                                                                                                                               | bestanden                                                                                                                                         |
| Rust-Tests (397), darunter 100 harte Abbrüche beim Speichern, 40 harte Abbrüche beim Import und die Migrationen ab jeder früheren Version (0 bis 7 → 8)                                                            | bestanden                                                                                                                                         |
| Erzeugte TypeScript-Typen neu geschrieben und unverändert; Lizenzliste neu erzeugt                                                                                                                                 | bestanden                                                                                                                                         |
| Referenzbibliothek reproduzierbar: zweimal voll gebaut (10.000 Dokumente, 5.000 Medien, noch nur PNG und JPEG); in der endgültigen Fassung mit allen fünf Bildformaten zweimal mit 1.000 Dokumenten und 500 Medien | bestanden, jeweils gleicher Fingerabdruck (voll je Lauf rund 250 s, davon 105 s für die Medien); die volle Größe der endgültigen Fassung prüft CI |
| E2E (E2E_GESAMT) in 16 Dateien, darunter Medien (7) und Galerie-Messung (1)                                                                                                                                        | E2E_ERGEBNIS                                                                                                                                      |
| Build ohne `eval(` und `Function`-Konstruktor (Suche im fertigen Frontend)                                                                                                                                         | bestanden; die einzigen Treffer sind ein Methodenname in `re2js` und `FunctionBasedShading` in PDF.js                                             |

`cargo audit` und `cargo deny` laufen in CI (siehe „CI“); lokal waren sie diesmal nicht installiert.

Was die Tests zeigen, im Einzelnen:

- **Bytegleich** (Rust, E2E): Prüfsummen und Änderungszeiten aller Originale vor und nach dem Import, für Einzeldateien und einen Ordner mit Unterordnern; jede Kopie in der Bibliothek gleicht ihrem Original. Im echten Programm: Ordner „Reise“ mit fünf Medien in drei Ebenen, danach dieselben Prüfsummen und Zeiten.
- **Abbruch ohne Rest** (Rust, E2E): Abbrechen nach dem Vorbereiten, ein eingeschleuster Fehler beim Abschließen (alle Dateien wandern zurück), ein unvollständiger Import beim Öffnen der Bibliothek und **40 harte Abbrüche** eines Kindprozesses mitten im Abschließen – jedes Mal keine Datei in `media/`, keine Zeile, nichts in `tmp/`. Im echten Programm: Ordnerimport bis zum Dialog, die Kopien liegen in `tmp/`, Abbrechen – danach weder Dateien noch Kacheln.
- **SVG mit Skript** (Rust, E2E): Ein SVG mit `onload`, `<script>`, `<foreignObject>` mit Skript und `onerror` sowie einem `javascript:`-Link. Im echten Programm in der Galerie und in der Medienansicht gezeigt: kein Marker gesetzt, Fenstertitel unverändert, das SVG ist nur Bildquelle. Rust prüft die Kopfzeilen des Handlers.
- **CVE-2024-4367** (E2E): eine Test-PDF mit einer Type1-Schrift, deren `FontMatrix` Code einschleust (Marker und Fenstertitel). Im echten Programm geöffnet: Die Seite erscheint, nichts davon läuft.
- **Typprüfung** (Rust, E2E): falsche Endung, Makro-DOCX (`.docm`), eine „PNG“ mit Programmkopf, Markdown im Ordner, versteckte Dateien, Bilder über der Pixelgrenze – alle mit ihrem Grund abgelehnt.
- **Duplikate** (Rust, E2E): gleiche Prüfsumme bietet das vorhandene Medium an; „vorhandenes verwenden“ legt nichts an.
- **Ordner als Subnetze** (Rust, E2E): „Reise › Tag 1 › Abend“ entsteht, jedes Medium liegt primär im Subnetz seines Ordners.
- **Papierkorb** (Rust): Ein verwendetes Medium lässt sich nicht endgültig löschen, die Aufbewahrungsfrist überspringt es.
- **PDF-Ansicht** (E2E): drei Seiten, Textebene, interner Link springt auf Seite 3, Blättern, Seitenfeld, ein Weblink fragt und öffnet nach „Abbrechen“ nichts.
- **Editor** (Vitest, E2E): Bild und PDF-Verweis aus der Auswahl eingefügt, über den Handler gezeigt, als `![…](pwmedia://…)` und `[…](pwmedia://…#page=2)` gespeichert; `Strg + Klick` öffnet das PDF auf Seite 2; „Verwendet in“ nennt das Dokument. Ein eingefügtes Duplikat verwendet das vorhandene Medium, ein neues Bild wird importiert; Unterschrift und 50 % landen als `<img … title="…" width="50%">` in der Datei. „Medium fehlt“ und Bilder mit Webadresse in Vitest.
- **IPC-Vertrag**: jeder neue Befehl mit gültigen und ungültigen Anfragen (fremde Felder, Pfade statt IDs, falsche Arten), einheitliches Fehlerformat.

## Abnahmekriterien Phase 8

| Kriterium                                                        | Stand                                                                                                                       |
| ---------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Originale bleiben bytegleich (Prüfsummen vorher und nachher)     | **erfüllt** (Rust: Einzeldateien und Ordner; E2E: Ordnerimport im echten Programm).                                         |
| Ein abgebrochener Import hinterlässt weder Dateien noch Zeilen   | **erfüllt** (Rust: Abbrechen, eingeschleuster Fehler, unvollständiger Import, 40 harte Abbrüche; E2E: Abbrechen im Dialog). |
| Ein SVG mit Skript führt nichts aus                              | **erfüllt** (E2E in Galerie und Medienansicht; Rust: Kopfzeilen des Handlers).                                              |
| Eine Test-PDF nach dem Muster von CVE-2024-4367 führt nichts aus | **erfüllt** (E2E, im echten Programm geöffnet).                                                                             |

## Galerie-Messung

GALERIE_MESSUNG

## Was konnte nicht getestet werden und warum?

- **Windows:** läuft in CI (Rust, Vitest, E2E mit WebView2). Lokal nur Linux.
- **Echtes Ablegen aus dem Dateimanager und echtes Einfügen eines Bildschirmfotos:** Der eingebettete WebDriver erzeugt die Ereignisse synthetisch (ADR 0003); getestet sind die Wege im echten Programm mit erzeugten `DataTransfer`-Objekten und die Logik in Vitest.
- **Echter Dateidialog** für Dateien und Ordner: In den E2E-Tests antworten die Dialoge aus `PW_E2E_PICK_MEDIA` (nur im E2E-Build); der Weg dahinter ist derselbe.
- **Mit dem Standardprogramm öffnen:** In CI gibt es kein Standardprogramm; Rust prüft die schreibgeschützte Kopie.
- **Referenzgerät:** nicht verfügbar; die Galerie-Werte stammen aus dem Container und aus CI.

## Offene Fehler

Keine bekannten.

## Behobene Fehler

F-34 bis F-38 in `docs/status.md`:

- **F-34 und F-36 waren Fehler der App**, gefunden von den E2E-Tests unter WebKitGTK: Ein Klick auf eine Galerie-Kachel brachte den Fokus nicht in die Liste; ein Klick wählte ein Bild im Text nicht aus.
- **F-35 und F-37 waren Fehler der Tests:** veraltete Elementverweise unter WebKitGTK und eine Erwartung der Phase 3, die nach der Umstellung des Ablegens nicht mehr stimmte (der offene Dialog riss dann alle folgenden Dateien im selben Prozess mit).
- **F-38** betraf den Generator der Referenzbibliothek (zu große Testbilder).

Außerdem fand der vollständige lokale Lauf vor dem CI-Lauf zwei Dinge, die meine Zwischenprüfungen übersehen hatten: eine nicht formatierte `vite.config.ts` und einen Typfehler im E2E-Test (ich hatte nur den App-Teil der Typprüfung laufen lassen). Beides ist behoben; genau dafür gibt es die Regel aus F-26.

## Gefundene Widersprüche und Abweichungen

1. **`isEvalSupported: false`** (Abschnitt 17) gibt es in PDF.js 6.5 nicht mehr. Der Schutz besteht trotzdem: Die CSP verbietet `unsafe-eval`, und der Build enthält weder `eval` noch den `Function`-Konstruktor (geprüft). Die Test-PDF zu CVE-2024-4367 zielt genau auf diesen Weg und läuft nicht.
2. **CSP-Lockerung `connect-src 'self'`** (ADR 0019, Punkt 9): PDF.js lädt Standardschriften und CMaps per `fetch` aus dem eigenen Build. Erlaubt sind nur die eigenen Dateien der App; Netz gibt es weiter nicht.
3. **Duplikate beim Einfügen in den Text:** Abschnitt 9 verlangt die Frage „vorhandenes Element verwenden oder trotzdem importieren“. Im Importdialog (Galerie, Ordner, Ablegen auf das Fenster) fragt die App. Beim Einfügen oder Ablegen in der Schreibfläche verwendet sie das vorhandene Medium ohne Frage: Das Bild im Text sieht gleich aus, und eine Frage mitten im Schreiben unterbräche mehr, als sie nützt. Wenn du hier auch fragen willst, ist das eine kleine Änderung.
4. **Ordnerimport nimmt nur Medien auf** (ADR 0019, Punkt 6): Textdokumente im Ordner werden mit Grund übersprungen; sie kommen über Ablegen in die Bibliothek.
5. **`media` weicht vom Entwurf ab** (ADR 0019, Punkt 2): `title` und `description` statt `caption` – die Bildunterschrift gehört zur Verwendung im Dokument –, dazu `last_opened_at`.
6. **Isolation Pattern:** Die Neubewertung „vor Phase 8“ (ADR 0005) habe ich erst während der Phase festgehalten. Ergebnis: Es bleibt aus. Technisch hat sich nichts geändert (gleiche Tauri-Version, `frame-src 'none'` blockiert das Iframe), und gegen eine Lücke in PDF.js hülfe es nicht, weil es gültige Befehle durchlässt. Der Schutz liegt in CSP, Build und Rust (Einzelheiten in ADR 0005).
7. **Ansichts-Tabs** (Galerie, Medien) stellt die Sitzung nicht wieder her; sie hält weiter nur Dokumente. **„Nicht zugeordnet“** zeigt weiter nur Dokumente; Medien ohne Subnetz stehen unter „Medien“.
8. **Native Menüleiste:** „Bild einfügen …“ und „PDF-Verweis einfügen …“ stehen im Menü „Format“. Import und Galerie stehen in Befehlspalette und Galerie, noch nicht im nativen Menü.
9. **Ein Zweig je Phase** (Abschnitt 24): weiterhin nur der erlaubte Zweig.

## Entscheidungen (ADR)

- 0019 Medien: Import, Typprüfung, Medien-Handler, Vorschaubilder, PDF und Verweise (17 Punkte), mit Nachträgen aus der Umsetzung.
- 0005 Isolation Pattern: Neubewertung vor Phase 8 nachgetragen, Entscheidung unverändert.

## CI

CI_ABSCHNITT

## Nächster Schritt

Nach Freigabe: Phase 9 (Suche; Abschnitt 25).
