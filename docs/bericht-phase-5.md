# Bericht Phase 5 – Dokumente und externe Dateien

Stand: 4. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig). Entwicklungsumgebung: Cloud-Container, Ubuntu 24.04, 4 vCPU, 15 GB RAM, keine GPU (Xvfb, Software-Rendering), WebKitGTK, Rust 1.99.1, Node.js 22. CI: GitHub Actions `windows-latest` und `ubuntu-24.04`; macOS ist auf Wunsch des Auftraggebers zurückgestellt. Entwickelt und geprüft wurde lokal unter Linux; **CI-Lauf 97** hat danach alles unter Windows und Linux bestätigt (siehe „CI“).

## Was wurde implementiert?

- **Dokumentverwaltung** (Abschnitt 13): Erstellen in drei Formaten, Öffnen, Bearbeiten, Umbenennen, **Kopieren** („… (Kopie)“, neue ID, gleiches Format und gleicher Inhalt, ohne Historie), **Favoriten**, **Zuletzt verwendet**. Der Explorer ist ein Baum (ARIA `tree`, virtualisiert, eine Tab-Stelle) mit „Favoriten“, „Zuletzt verwendet“ (Bibliotheksdokumente und externe Dateien, neueste zuerst) und „Alle Dokumente“. Migration 0004 (`favorited_at`, `last_opened_at`).
- **`.pwrite`, Version 1** (Abschnitt 10, `docs/pwrite-format.md`): Hülle mit `format`, `formatVersion`, `id`, `title`, Zeiten, `language`, `tags`, `content` (Editor-Dokument), `media`. Rust behält unbekannte Felder, auch in Einträgen von `media`; eine Datei mit höherer `formatVersion` öffnet schreibgeschützt mit Hinweis und wird nie geschrieben. Neues Rich-Text-Dokument über die Befehlspalette.
- **Externe Dateien** (Abschnitt 9, ADR 0015): `Strg + O` öffnet den Systemdialog in Rust; das Frontend bekommt nur eine Sitzungs-ID, Dateiname und Ordner als Anzeigetext.
  - **Kodierung ohne neue Abhängigkeit** (`services/encoding.rs`): UTF-8 mit und ohne BOM, UTF-16 LE/BE mit und ohne BOM, Windows-1252. Zeilenenden LF oder CRLF nach Mehrheit; gemischte Zeilenenden werden genannt und beim ersten Speichern einheitlich. Binärdateien öffnen nicht. Kodierung und Zeilenenden lassen sich in der Statusleiste umstellen; kann Windows-1252 ein Zeichen nicht darstellen, speichert Rust nichts, und die App bietet „Als UTF-8 speichern“.
  - **Unveränderte Dateien werden nie geschrieben**, auch nicht mit `Strg + S`. Gespeichert wird atomar im Ordner der Datei; die Rechte der alten Datei bleiben (Unix), Verknüpfungen werden aufgelöst, sodass die Zieldatei ersetzt wird.
  - **Änderungen von außen:** Rust merkt sich Größe, Änderungszeit und SHA-256; das Frontend fragt alle 2 s und beim Fokus des Fensters nach, Rust prüft vor jedem Speichern erneut. Die Rückfrage bietet „Neu laden“, „Meine Fassung behalten“ und „Als Kopie speichern …“; bei einer verschwundenen Datei „Speichern unter …“, „Weiter bearbeiten“, „Tab schließen“. Nur berührte Dateien (gleicher Inhalt) gelten nicht als geändert.
  - **Speichern** mit `Strg + S`, „Speichern unter …“ mit `Strg + Umschalt + S`, Autosave optional (Einstellungen › Allgemein, Standard aus). Änderungspunkt im Tab, Ordner als Tooltip. Schließen und Beenden mit Änderungen fragen „Speichern, Verwerfen, Abbrechen“. Schreibgeschützte Dateien: Hinweis und „Speichern unter …“. Ab 10 MB fragt die App vor dem Öffnen.
  - **In Bibliothek aufnehmen:** Kopie über den Import (UTF-8, LF; `.pwrite` mit neuer Hülle), Duplikat-Rückfrage, das Original bleibt; der Tab zeigt danach das Bibliotheksdokument.
  - **Zuletzt verwendet** in `settings.json` (`recentFiles`, höchstens 20, Pfade bleiben in Rust); ein verschwundener Eintrag ist markiert und lässt sich entfernen.
- **Import per Drag & Drop** erkennt dieselben Kodierungen und nimmt `.pwrite` an.
- **Umwandeln mit Verlustschutz** (Abschnitt 10): TXT wird zu Absätzen, Markdown und `.pwrite` gehen über das Editor-Dokument ineinander über, Markdown zu TXT behält den Markdown-Text. Vorher nennt die App, was verloren ginge (was Markdown nicht halten kann, jede Formatierung, der YAML-Kopf, unbekanntes Markdown). Die ID bleibt; Rust schreibt die neue Datei, ändert dann Zeile und Links in einer Transaktion und löscht zuletzt die alte Datei. Die Verlustwarnung beim Speichern von Markdown bietet jetzt **„Als `.pwrite` speichern“**.
- **Dateizuordnung** (Abschnitt 9): `.pwrite` über `bundle.fileAssociations` (MIME `application/x-project-write`); die Linux-Pakete bringen den shared-mime-info-Typ und eine Desktop-Vorlage mit `%F` mit (ohne `%F` übergäben Dateimanager die Datei nicht). Startargumente und ein zweiter Start (`tauri-plugin-single-instance`) landen in einer Warteschlange in Rust, bis die Oberfläche sie abholt; das Fenster kommt nach vorn. Eine Datei aus dem Dokumentordner der offenen Bibliothek öffnet als Bibliotheksdokument.
- **Interne Links und Rückverweise** (Abschnitt 12): `[[` öffnet eine Dokumentauswahl; der Link zeigt auf `pwrite://<id>` mit dem Titel als Text und überlebt so das Umbenennen (Markdown: `[Titel](pwrite://<id>)`). `Strg + Klick` öffnet das Dokument in der App. Rust leitet `document_links` (Migration 0005) bei jedem Speichern, Anlegen, Import, Kopieren und Umwandeln aus dem Inhalt ab, in derselben Transaktion wie die Metadaten. Die Seitenleiste „Verbindungen“ zeigt die Rückverweise.

## Welche Dateien wurden erstellt oder verändert?

Rust, neu: `services/{encoding,external,opening,pwrite}.rs`, `commands/external.rs`, `migrations/0004_favorites_and_recent.sql`, `migrations/0005_document_links.sql`, `linux/{project-write.desktop,project-write-mime.xml}`. Geändert: `services/{documents,import,dialog}.rs`, `services/settings/{app,ui}.rs` (`recentFiles`, `autosaveExternal`), `services/ui/{mod,menu}.rs` (Ereignis `openRequested`, Menü „Datei öffnen …“, „Speichern unter …“), `database/documents.rs`, `filesystem/atomic.rs` (Ersetzen mit Rechten der alten Datei), `commands/{documents,mod}.rs`, `state.rs`, `lib.rs`, `error.rs` (5 Fehlercodes), `build.rs`, `capabilities/main-window.json`, `tauri.conf.json`, `tests/{ipc,migrations}.rs`. 19 neue Befehle.

Frontend, neu: `components/prompt/{promptStore.ts,PromptDialog.tsx}`, `modules/documents/{OutsideChangeDialog,Connections}.tsx`, `modules/editor/{convert.ts,InternalLinkPicker.tsx}`, `modules/editor/markdown/lossKeys.ts`; Tests `modules/documents/{externalFiles.test.ts,Explorer.test.tsx}`, `modules/editor/{convert,internalLinks,pwrite}.test.ts`. Geändert: `modules/documents/{workspaceStore.ts,Explorer.tsx}`, `modules/editor/{session.ts,DocumentEditor.tsx,LinkDialog.tsx,schema/extensions.ts}`, `components/{layout/StatusBar.tsx,tabs/TabBar.tsx}`, `app/{App,Workbench}.tsx`, `app/{commands,lifecycle}.ts`, `modules/commands/{types,keymap}.ts`, `modules/settings/GeneralSettings.tsx`, `services/ipc.ts`, `i18n/{de,en}.json`, erzeugte Typen.

E2E: neu `tests/e2e/specs/{external,documents}.e2e.ts`; geändert `sandbox.ts`, `wdio.conf.ts` (`PW_E2E_PICK_FILES`), `specs/workbench.e2e.ts`. Doku: ADR 0015 (mit Nachträgen), `docs/pwrite-format.md`, `docs/{architecture,database,dependencies,status}.md`, dieser Bericht, `CHANGELOG.md`, `README.md`. **Keine neue Abhängigkeit** (geprüft und verworfen: `encoding_rs`, `chardetng`, `notify`, `url`/`percent-encoding`; `docs/dependencies.md`).

## Welche Tests wurden durchgeführt?

Umfang: **271 Rust-Tests** (174 Unit, 97 Integration; Phase 4: 220), **664 Vitest-Tests** in 30 Dateien (632), **42 WebdriverIO-Tests** in 8 Dateien (34).

Lokal (Linux-Container, Xvfb, D-Bus-Sitzung), alle ausgeführt und bestanden:

| Prüfung                                                                                                                                                                                          | Ergebnis                                                                                                                                |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| Prettier, ESLint (`--max-warnings 0`), TypeScript (App und Node-Teil), Vitest (664)                                                                                                              | bestanden                                                                                                                               |
| rustfmt, Clippy `-D warnings` (Linux, Linux + `e2e`, Windows-Ziel `x86_64-pc-windows-gnu`)                                                                                                       | bestanden                                                                                                                               |
| Rust-Tests (271), darunter die 100 harten Abbrüche und die Migrationen ab jeder früheren Version (0 bis 4 → 5)                                                                                   | bestanden                                                                                                                               |
| Erzeugte TypeScript-Typen neu geschrieben und unverändert; Lizenzliste unverändert                                                                                                               | bestanden                                                                                                                               |
| Referenzbibliothek zweimal gebaut, gleicher Fingerabdruck (10.000 Dokumente)                                                                                                                     | bestanden                                                                                                                               |
| E2E (42): Rauchtest (7), Bibliothek (4), Neustart (3), Arbeitsfläche (12), Editor (3), Leistung (5), externe Dateien (6), Dokumente (2)                                                          | bestanden, mehrere vollständige Läufe                                                                                                   |
| Debug-`.deb` gebaut: Desktopdatei `Exec=project-write %F`, `MimeType=application/x-project-write`, MIME-XML im Paket; lokale MIME-Datenbank erkennt `*.pwrite` als `application/x-project-write` | bestanden                                                                                                                               |
| Release-Build ohne Test-Plugins (`check-release-no-test-plugins.sh`)                                                                                                                             | bestanden (`tauri build --no-bundle`, Release-Profil)                                                                                   |
| `npm audit --omit=dev` (ausgelieferte Pakete); `npm audit` (mit Entwicklungswerkzeugen, nur berichtet)                                                                                           | 0 Funde; 34 Funde (20 mittel, 14 hoch, Stand Phase 2: 40), alle in WebdriverIO, Mocha und ESLint wie in `docs/dependencies.md` bewertet |

Was die Tests zeigen, im Einzelnen:

- **Kodierungen** (Rust): jede Kodierung mit LF und CRLF dekodiert und bytegleich zurückgeschrieben; Windows-1252 (0x80–0x9F); gemischte Zeilenenden; Binärdateien; nicht darstellbare Zeichen. Im echten Programm (E2E): fünf Dateien (UTF-8 LF, UTF-8 BOM CRLF, UTF-16 LE CRLF, UTF-16 BE LF, Windows-1252 CRLF) öffnen mit der richtigen Anzeige, werden bearbeitet und mit `Strg + S` in ihrer Kodierung und mit ihren Zeilenenden gespeichert; vorher ist nichts geschrieben.
- **Bytegleich ohne Änderung:** Rust (Speichern ohne Änderung schreibt nichts, Änderungszeit bleibt), Store (kein Aufruf) und E2E (UTF-16 BE ohne BOM, `Strg + S`, Schließen: Bytes und Änderungszeit gleich, keine Rückfrage).
- **Änderungen von außen:** Rust (berührt ≠ geändert, Überschreiben nur mit Zustimmung, verschwundene Datei), Store (Abfrage, „Meine Fassung behalten“, Neu laden startet den Editor neu), E2E (Dialog erscheint nach einer Änderung auf der Platte, „Neu laden“ zeigt den neuen Text, „Meine Fassung behalten“ schreibt die eigene Fassung).
- **Schließen mit Änderungen:** Store und E2E („Abbrechen“ lässt den Tab offen, „Verwerfen“ schließt, die Datei bleibt).
- **`.pwrite`:** Round-Trip im Editor (auch was Markdown nicht halten kann), unbekannte Felder nach dem Speichern erhalten (Rust, IPC, extern), neuere Formatversion schreibgeschützt und nie geschrieben (Rust, IPC, extern).
- **Dateizuordnung:** Rust (Argumente zu Pfaden, `file://`, Warteschlange), IPC (Bibliotheksdatei → Bibliotheksdokument, externe Datei, fehlende Datei, falscher Typ), E2E (ein zweiter Start der App mit einer Datei beendet sich mit 0, und die laufende App öffnet die Datei).
- **Umwandeln:** Vitest (alle Richtungen, Warnungen), IPC (ID bleibt, alte Datei weg, Hülle), E2E (Markdown → `.pwrite` ohne Rückfrage, `.pwrite` → Text mit Warnung).
- **Interne Links:** Rust (Links in Markdown und `.pwrite` finden), IPC (Rückverweise nach Speichern, Kopieren, Entfernen, Umwandeln), Vitest (`[[` fragt, nicht in Code; Link wird als Markdown geschrieben), E2E (`[[` → Auswahl → gespeichert als `[Linkziel](pwrite://<id>)`, `Strg + Klick` öffnet das Ziel, „Verbindungen“ zeigt die Quelle).
- **IPC-Vertrag:** jeder neue Befehl mit gültigen und ungültigen Anfragen (falsche IDs, fremde Felder, Pfade statt IDs, ungültige Kodierungen), einheitliches Fehlerformat.

## Abnahmekriterien Phase 5

| Kriterium                                                                                        | Stand                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| UTF-8, UTF-8 BOM, UTF-16, Windows-1252, LF/CRLF bleiben beim Bearbeiten erhalten                 | **erfüllt** unter Linux und Windows (E2E und Rust in CI-Lauf 97; UTF-16 auch ohne BOM in Rust).                                                                                                                                                                                                                                                                                                                                                          |
| Unveränderte Datei ist byte-identisch                                                            | **erfüllt** (Rust, Store, E2E inkl. Änderungszeit).                                                                                                                                                                                                                                                                                                                                                                                                      |
| Änderungen von außen lösen Dialog aus                                                            | **erfüllt** (E2E: Dialog nach Änderung auf der Platte; Neu laden und Meine Fassung behalten).                                                                                                                                                                                                                                                                                                                                                            |
| Doppelklick, „Öffnen mit“ und zweiter Start öffnen die Datei auf jedem Betriebssystem            | **teilweise prüfbar.** Was das System dabei tut – die App mit der Datei als Argument starten oder, wenn sie läuft, ein zweites Mal –, ist getestet: zweiter Start im E2E-Test unter Linux, Startargumente in Rust- und IPC-Tests. Der Doppelklick selbst braucht eine installierte Zuordnung (Installer, Phase 14); das Linux-Paket ist gebaut und geprüft, aber nicht installiert (Abschnitt 2). Windows: nicht ausgeführt (CI), macOS: zurückgestellt. |
| `.pwrite`-Round-Trip, unbekannte Felder erhalten, neuere Formatversionen öffnen schreibgeschützt | **erfüllt** (Vitest, Rust, IPC, externe Dateien).                                                                                                                                                                                                                                                                                                                                                                                                        |

## Was konnte nicht getestet werden und warum?

- **Windows, Sonderfälle:** Die Rust-, IPC- und E2E-Tests laufen in CI auch unter Windows (Lauf 97 grün, inklusive zweitem Start über die Einzelinstanz). Nicht eigens geprüft: die Rechte (ACL) einer ersetzten Datei – die neue Datei erbt die des Ordners – und Dateien mit Schreibschutz-Attribut.
- **macOS:** zurückgestellt; `RunEvent::Opened` ist nicht umgesetzt (ADR 0015).
- **Doppelklick und „Öffnen mit“ im Dateimanager:** brauchen eine installierte Zuordnung; nicht installiert (Tests nur im Projektordner). Geprüft sind Paketinhalt und MIME-Erkennung mit einer lokalen Datenbank.
- **Der erste Start mit Dateiargument** im echten Programm: Der E2E-Testdienst startet die App für alle Dateien mit denselben Argumenten; ein Dateiargument würde in jeder Datei einen Tab öffnen. Geprüft sind Zerlegen der Argumente, Warteschlange und Abholen (Rust, IPC) und derselbe Weg über den zweiten Start (E2E).
- **Native Datei- und Speicherdialoge:** WebDriver bedient sie nicht; der `e2e`-Build beantwortet sie aus `PW_E2E_PICK_FILES` (ADR 0008). Die Dialoge selbst (Filter, vorgeschlagener Name) sind nicht bedient worden.
- **Schreibgeschützte Dateien im Container:** Die Tests laufen als root, der Schreibschutz greift dort nicht; der Rust-Test erkennt das und prüft dann nichts. Die Logik (Öffnen zum Schreiben schlägt fehl → `FILE_READ_ONLY` → „Speichern unter“) ist im Store getestet.
- **Native Menüklicks** („Datei öffnen …“, „Speichern unter …“): wie bisher nur Menü-IDs und Weiterleitung.

## Offene Fehler

Keine bekannten.

## Behobene Fehler

F-19 bis F-22 in `docs/status.md`: F-19 (UTF-16 ohne BOM als UTF-8 gelesen) fand der Unit-Test der Kodierungen; F-20 (Schreibmarke) und F-21 (veraltete Elementreferenz) waren Fehler des neuen E2E-Tests; F-22 (Kürzel-Test der Phase 3 tippte einmal in fünf Läufen zu früh) war ein Wettlauf im Test.

## Gefundene Widersprüche und Abweichungen

1. **Umfang** (ADR 0015, Punkt 1): Löschen, Papierkorb und Archiv folgen in Phase 6, Verschieben und Tags in Phase 7, der Ordnerimport (`import_folder`) in Phase 8 – nach dem Phasenplan in Abschnitt 25. ADR 0011 nannte für den Papierkorb Phase 5; der Phasenplan geht vor. Die Version „vor jeder Umwandlung“ (Abschnitt 13) entsteht mit der Historie in Phase 6.
2. **`document_links` abweichend vom Entwurf** in `docs/database.md`: eigene Migration 0005, kein Fremdschlüssel auf `target_id`, keine Spalte `count` (Begründung dort).
3. **„Als `.pwrite` speichern“** gibt es für Bibliotheksdokumente; externe Markdown-Dateien behalten beim Speichern ihr Format.
4. **Autosave für externe Dateien** ist optional und standardmäßig aus (Abschnitt 9 nennt Autosave für die Bibliothek; für fremde Dateien wäre stilles Schreiben überraschend).
5. **Linux-Desktopdatei:** Tauris Vorlage übergibt keine Dateien (`Exec=` ohne `%F`); eigene Vorlage (ADR 0015, Nachträge).
6. **Ein Zweig je Phase** (Abschnitt 24): weiterhin nur der erlaubte Zweig.

## Entscheidungen (ADR)

0015 Dokumente und externe Dateien (16 Punkte), ergänzt um Nachträge aus der Umsetzung.

## CI

**CI-Lauf 97 ist grün** (von Hand gestartet am 4. November 2026, Commit `7e2cefe`; derselbe Code wie `77f18bf` plus Doku):

| Job                             | Ergebnis                                                                                                                                                                                     |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Windows (`windows-latest`)      | grün: Prettier, ESLint, TypeScript, Vitest, rustfmt, Clippy, Rust-Tests (mit den 100 harten Abbrüchen), Referenzbibliothek zweimal gleich, Typen und Lizenzliste aktuell, E2E alle 8 Dateien |
| Linux (`ubuntu-24.04`)          | grün: dieselben Schritte, E2E alle 8 Dateien                                                                                                                                                 |
| Abhängigkeitsprüfung            | grün: `cargo audit`, `cargo deny`, `npm audit`                                                                                                                                               |
| Release-Build ohne Test-Plugins | grün                                                                                                                                                                                         |

Davor: **Die CI-Läufe 81 bis 96 starteten keinen Job** – wie schon 78 bis 80 (Bericht Phase 4): Alle vier Jobs (Windows, Linux, Abhängigkeitsprüfung, Release-Build) enden nach 2–3 Sekunden ohne Runner, ohne Schritt und ohne Log. Das ist kein Fehler im Code. Wahrscheinlich ist das Kontingent an GitHub-Actions-Minuten des Kontos aufgebraucht; das berührt Kosten und Konto, daher habe ich an CI nichts geändert. Bis zur Klärung prüfe ich lokal alle Schritte des Linux-Jobs (oben) und Clippy für Windows. Lokal ausgeführt: auch der Release-Build ohne Test-Plugins und `npm audit`. Nicht lokal ausführbar: die Windows-Tests sowie `cargo audit` und `cargo deny` (im Container nicht installiert). Neue Abhängigkeiten gibt es in dieser Phase nicht.

Der Auftraggeber hat bestätigt, dass die Minuten verbraucht waren; seit Lauf 97 startet CI wieder.

## Nächster Schritt

Nach Freigabe: Phase 6 (Historie, Papierkorb, Archiv, Recovery; Abschnitt 25).
