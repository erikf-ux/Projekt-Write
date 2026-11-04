# ADR 0015 – Dokumente und externe Dateien

- Status: angenommen
- Datum: 2026-11-04
- Phase: 5

## Kontext

Phase 5 umfasst laut Abschnitt 25: Erstellen, Öffnen, Bearbeiten, Umbenennen, Kopieren, Favoriten, Zuletzt verwendet, `.pwrite`, TXT und MD als Dateiformate, Autosave, externe Dateien mit Kodierung, Zeilenenden und Änderungen von außen, Dateizuordnung auf drei Wegen, interne Links und Rückverweise sowie Umwandeln mit Verlustschutz. Die Regeln stehen in den Abschnitten 8 (Pfade nur in Rust), 9 (externe Dateien, Dateizuordnung), 10 (`.pwrite`, Verlustschutz), 12 (Links) und 13 (Dokumentverwaltung).

## Entscheidungen

1. **Umfang nach Abschnitt 25.** Löschen, Papierkorb und Archiv folgen in Phase 6, Verschieben und Tags in Phase 7, der Ordnerimport (`import_folder`) mit den Medien in Phase 8. ADR 0011 nannte für den Papierkorb Phase 5; der Phasenplan geht vor. Die Version „vor jeder Umwandlung“ (Abschnitt 13) entsteht mit der Dokumenthistorie in Phase 6.
2. **`.pwrite`, Version 1** (`docs/pwrite-format.md`): die Hülle aus Abschnitt 10, `content` ist ein Dokument im Schema des Editors. Rust liest und schreibt die Hülle und behält unbekannte Felder, auch in Einträgen von `media`. Das Frontend prüft `content` gegen das Schema. Eine Datei mit höherer `formatVersion` öffnet schreibgeschützt mit Hinweis. Bibliotheksdokumente liegen in `documents/richtext/`. `.pwrite` hat Rich Text ohne Quelltextansicht (Abschnitt 12) und braucht keinen Verlustschutz: Es hält alles, was der Editor hält. `language` ist beim Anlegen die Sprache der Oberfläche, `tags` bleibt bis Phase 7 leer, `media` bis Phase 8.
3. **Externe Dateien unter einem Token.** Rust zeigt den Systemdialog (`rfd`, Filter `.txt`, `.md`, `.markdown`, `.pwrite`) und verwaltet jede geöffnete Datei unter einer zufälligen ID. Das Frontend sieht Name und Ordner nur als Anzeigetext (Abschnitt 8). E2E-Builds beantworten den Dialog aus `PW_E2E_PICK_FILES`, wie den Ordnerdialog (ADR 0008).
4. **Kodierung ohne neue Abhängigkeit** (`services/encoding.rs`): BOM für UTF-8 und UTF-16 LE/BE; ohne BOM gültiges UTF-8; UTF-16 ohne BOM am Muster der Nullbytes; sonst Windows-1252 über eine Tabelle für 0x80–0x9F, der Rest entspricht Latin-1. NUL und Steuerzeichen außer Tab, LF, CR und FF machen eine Datei zur Binärdatei; sie öffnet nicht als Text. `encoding_rs` wäre die Alternative; für vier Kodierungen genügen gut getestete 150 Zeilen.
5. **Zeilenenden** LF oder CRLF nach Mehrheit. Beim Speichern schreibt Rust dieselbe Kodierung (mit BOM, wenn die Datei eins hatte) und dieselben Zeilenenden; die Statusleiste zeigt beides und stellt um. Mischformen werden beim ersten Speichern einheitlich, der Hinweis sagt es. Kann Windows-1252 ein Zeichen nicht darstellen, speichert Rust nichts und bietet UTF-8 an.
6. **Unveränderte Dateien werden nie geschrieben.** Ein Tab, dessen Text dem geladenen gleicht, speichert nicht, auch nicht mit `Strg + S`. Gespeichert wird atomar im Ordner der Datei (ADR 0007); die Rechte der alten Datei gehen auf die neue über.
7. **Änderungen von außen:** Rust merkt sich beim Laden und Speichern Größe, Änderungszeit und SHA-256. Das Frontend fragt alle 2 s und beim Fokus des Fensters nach, Rust prüft vor jedem Speichern erneut. Bei einer Abweichung fragt die App: Neu laden, Meine Fassung behalten oder Als Kopie speichern. Ein Dateiwächter (`notify`) wäre eine Abhängigkeit mit drei Plattform-Backends; die Abfrage ist überall gleich und testbar.
8. **Speichern externer Dateien** mit `Strg + S`, Autosave optional (Einstellung, Standard aus). Der Tab zeigt einen Änderungspunkt. Schließen und Beenden mit Änderungen fragen: Speichern, Verwerfen, Abbrechen. Ist die Datei schreibgeschützt oder verschwunden, bietet die App „Speichern unter“ an. Ab 10 MB fragt sie vor dem Öffnen.
9. **In Bibliothek aufnehmen** legt über den Import eine Kopie an (UTF-8, LF; `.pwrite` unverändert), das Original bleibt unverändert. Der Tab zeigt danach das Bibliotheksdokument.
10. **Zuletzt verwendet:** Bibliotheksdokumente über `documents.last_opened_at`, externe Dateien in `settings.json` (`recentFiles`, höchstens 20). Die Pfade bleiben in Rust, das Frontend bekommt Einträge mit ID und Anzeigetext. Ein verschwundener Eintrag meldet sich beim Öffnen und lässt sich entfernen.
11. **Favoriten** über `documents.favorited_at`. Der Explorer zeigt Favoriten, Zuletzt verwendet und alle Dokumente; Subnetze kommen mit Phase 7.
12. **Kopieren:** neue ID, Titel „… (Kopie)“, gleiches Format und gleicher Inhalt; Tags und Subnetze kopiert ab Phase 7, die Historie nie (Abschnitt 13).
13. **Umwandeln** über den Editor: TXT wird zu Absätzen, MD und `.pwrite` gehen ineinander über, MD zu TXT behält den Markdown-Text. Verluste nennt die Warnung vor dem Umwandeln: `.pwrite` zu MD über `findLosses` (ADR 0014), Rich Text zu TXT verliert jede Formatierung. Die Warnung beim Speichern von MD bietet jetzt „Als `.pwrite` speichern“. Die ID bleibt; Rust schreibt die neue Datei, ändert dann die Zeile und löscht zuletzt die alte Datei.
14. **Interne Links:** `[[` öffnet eine Dokumentauswahl. Der Link zeigt auf `pwrite://<id>` und überlebt so das Umbenennen; in Markdown steht `[Titel](pwrite://<id>)`. `Strg + Klick` öffnet das Dokument in der App. Rust leitet `document_links` beim Speichern aus dem Inhalt ab, in derselben Transaktion wie die Metadaten. Die Seitenleiste „Verbindungen“ zeigt die Rückverweise.
15. **Dateizuordnung:** `.pwrite` über `bundle.fileAssociations`, `.md` und `.txt` nicht von selbst (das Angebot gehört zum Installer, Phase 14). Startargumente (Windows, Linux) und ein zweiter Start (`tauri-plugin-single-instance`) landen in einer Warteschlange in Rust, bis die Oberfläche sie abholt; das Fenster kommt nach vorn. `RunEvent::Opened` (macOS) ruht mit macOS. Eine Datei aus dem Dokumentordner der offenen Bibliothek öffnet als Bibliotheksdokument.
16. **Import per Drag & Drop** erkennt dieselben Kodierungen (ADR 0011, Punkt 7) und speichert UTF-8 mit LF.

## Folgen

- Externe Dateien haben keine Historie, keine Suche und kein Mesh, bis sie in die Bibliothek aufgenommen sind (Abschnitt 9). Recovery-Entwürfe auch für externe Dateien kommen mit Phase 6.
- Ein Doppelklick lässt sich in CI nicht auslösen: Getestet werden Startargumente und zweiter Start, also das, was das System bei einem Doppelklick tut.
- `docs/database.md`: Migration 0004 legt `favorited_at`, `last_opened_at` und `document_links` an.
