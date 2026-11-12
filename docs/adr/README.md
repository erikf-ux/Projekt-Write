# Architekturentscheidungen (ADR)

Kurze Entscheidungsnotizen nach dem Muster Kontext → Optionen → Entscheidung → Folgen.
Eine ADR wird nicht nachträglich umgeschrieben. Ändert sich eine Entscheidung, entsteht eine neue ADR, die die alte ablöst.

| Nr.                                               | Titel                                                                                        | Status                                        |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------- | --------------------------------------------- |
| [0001](0001-versionen-und-kompatibilitaet.md)     | Versionen und Kompatibilität des Technologiestacks                                           | angenommen                                    |
| [0002](0002-ipc-vertrag.md)                       | IPC-Vertrag: validierte Eingaben, einheitliches Fehlerformat, erzeugte Typen                 | angenommen                                    |
| [0003](0003-e2e-tests-ohne-global-tauri.md)       | E2E-Tests mit eingebettetem WebDriver, ohne `withGlobalTauri`                                | angenommen                                    |
| [0004](0004-datenbank-vor-der-bibliothek.md)      | Datenbank vor der Bibliothek: In-Memory in Phase 1, erste Migration                          | angenommen, Laufzeit-Teil abgelöst durch 0006 |
| [0005](0005-isolation-pattern.md)                 | Isolation Pattern                                                                            | angenommen                                    |
| [0006](0006-bibliothek-sperre-und-speicherort.md) | Bibliothek: Sperre, Speicherort und Rollback-Modus                                           | angenommen                                    |
| [0007](0007-atomares-speichern.md)                | Atomares Speichern und Dokument-Repository                                                   | angenommen                                    |
| [0008](0008-ordnerwahl-in-rust.md)                | Ordnerwahl in Rust, Tokens statt Pfaden                                                      | angenommen                                    |
| [0009](0009-einstellungen-und-logging.md)         | App- und Bibliothekseinstellungen, Logging                                                   | angenommen                                    |
| [0010](0010-referenzbibliothek.md)                | Referenzbibliothek                                                                           | angenommen                                    |
| [0011](0011-umfang-der-oberflaeche-in-phase-3.md) | Umfang der Oberfläche in Phase 3                                                             | angenommen                                    |
| [0012](0012-befehle-tastenkuerzel-und-menue.md)   | Befehle, Tastenkürzel, natives Menü                                                          | angenommen                                    |
| [0013](0013-importierbare-designs.md)             | Importierbare Designs und Oberflächeneinstellungen                                           | angenommen                                    |
| [0014](0014-editor-und-markdown-adapter.md)       | Editor: Schema, Markdown-Adapter und Verlustschutz                                           | angenommen                                    |
| [0015](0015-dokumente-und-externe-dateien.md)     | Dokumente und externe Dateien                                                                | angenommen                                    |
| [0016](0016-wiederherstellung.md)                 | Wiederherstellung, Papierkorb, Archiv und Historie                                           | angenommen                                    |
| [0017](0017-subnetze-und-tags.md)                 | Subnetze, Zuordnungen, Tags und Struktur-Undo                                                | angenommen                                    |
| [0018](0018-sqlite-sicherheitsupdate.md)          | Sicherheitsupdate von SQLite über eine gepatchte Kopie von `sqlx-sqlite`                     | angenommen (vorübergehend)                    |
| [0019](0019-medien.md)                            | Medien: Import, Typprüfung, Medien-Handler, Vorschaubilder, PDF und Verweise                 | angenommen                                    |
| [0020](0020-suche.md)                             | Suche: Index, Anfragen, Filter und Aktualität                                                | angenommen                                    |
| [0021](0021-export-und-backup.md)                 | Export, Drucken, Backup und Wiederherstellung (Stufe 5)                                      | angenommen                                    |
| [0022](0022-mesh.md)                              | Mesh: Renderer nach Lasttest, Ansichten, Verbindungen, Layout und Vorschläge                 | angenommen                                    |
| [0023](0023-rechtschreibpruefwerk.md)             | Prüfwerk für die Rechtschreibung: `spellbook` statt Hunspell-Bindings (Vergleich mit Korpus) | angenommen                                    |
