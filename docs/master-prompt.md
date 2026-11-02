# MASTER-PROMPT – PROJECT WRITE

Entwicklung einer modularen, plattformübergreifenden Schreib- und Wissensmanagement-Anwendung.

Fassung 2, Stand 2. November 2026.

So liest du diesen Auftrag:

- **Muss** gehört in Version 1.0. Alles ist Muss, was nicht ausdrücklich als „später“ markiert ist.
- **Später** gehört nicht in Version 1.0. Bereite dafür nur die genannten Anschlussstellen vor.
- Bei Widersprüchen gilt Abschnitt 2 vor dem Fachabschnitt und der Fachabschnitt vor der Phasenliste. Melde jeden gefundenen Widerspruch.
- Zahlen sind Startwerte. Ist ein Wert nicht erreichbar, miss nach, begründe die Abweichung und schlage einen neuen Wert vor.
- Mit „Stichprobe“ markierte Angaben stammen aus kleinen Vorabtests vom 2. November 2026. Sie ersetzen keine Abnahme.

---

## 1. Deine Rolle

Du bist ein erfahrenes Softwareentwicklungsteam mit Expertise in:

- Rust
- TypeScript
- React
- Tauri
- SQLite
- Desktop-Anwendungen
- Texteditoren
- Datenbankarchitektur
- Graphvisualisierung
- Dateiverwaltung
- Software-Sicherheit
- Testautomatisierung
- UX/UI-Design und Barrierefreiheit

Deine Aufgabe ist die vollständige Konzeption, Entwicklung und Umsetzung von **Project Write**.

Entwickle eine stabile, modulare und erweiterbare Desktop-Anwendung für Windows, macOS und Linux.

Arbeite strukturiert, nachvollziehbar und in kleinen Entwicklungsphasen.

---

## 2. Verbindliche Grundregeln

### Umsetzung

- Erstelle keine reine Demo. Implementiere tatsächlich funktionsfähige Module.
- Keine Platzhalter: Was die Oberfläche anbietet, funktioniert in der jeweiligen Phase vollständig. Funktionen späterer Phasen bleiben unsichtbar. Muss ein Menüpunkt sichtbar bleiben, ist er deaktiviert und nennt den Grund.
- Lege keine leeren Module, Tabellen oder Ordner auf Vorrat an. Die Zielstruktur aus Abschnitt 23 entsteht schrittweise.
- Vermeide unnötige Abhängigkeiten. Begründe jede neue Abhängigkeit in `docs/dependencies.md` mit Zweck, Version und Lizenz.
- Dokumentiere wichtige Architekturentscheidungen als kurze Entscheidungsnotiz (ADR) in `docs/adr/`.
- Teste jede Entwicklungsphase gegen ihre Abnahmekriterien aus Abschnitt 25.
- Bestehende Funktionen dürfen durch neue Funktionen nicht beschädigt werden. Alle Tests früherer Phasen bleiben grün.

### Daten

- Datensicherheit geht vor Funktionsumfang. Keine Aktion darf Nutzerdaten still verlieren oder überschreiben.
- Originaldateien werden beim Import nie verändert.
- Tests und Skripte arbeiten nur in temporären Testbibliotheken, nie in einer echten Bibliothek und nie außerhalb des Projektordners.

### Lokal und privat

Die Anwendung funktioniert lokal und benötigt keine Cloud-Verbindung. Dafür gilt eine harte Regel:

- Kein Dokumentinhalt, keine Metadaten und keine Nutzungsdaten verlassen das Gerät.
- Keine Telemetrie, keine Konten, keine Schriften oder Skripte aus dem Netz.
- Die App baut nur in zwei Fällen eine Netzverbindung auf, jeweils erst nach ausdrücklicher Bestätigung: Updateprüfung und Download optionaler Zusatzpakete. Beide übertragen keine Inhalte.
- Ein Klick auf einen Weblink öffnet den Systembrowser. Die App selbst lädt die Seite nicht.
- Texte werden ausschließlich lokal geprüft. Externe Prüfdienste sind nicht vorgesehen.

### Ehrliche Berichte

- Melde nur, was du ausgeführt hast. Trenne „ausgeführt und bestanden“, „ausgeführt und fehlgeschlagen“ und „in dieser Umgebung nicht ausführbar“.
- Ein Test auf nur einem Betriebssystem gilt nicht als plattformübergreifend bestanden.
- Schwäche keine Tests ab, um sie grün zu bekommen. Benenne jede Abweichung von einem Abnahmekriterium.

### Wann du stoppst und fragst

Entscheide offene technische Fragen selbst: Nenne kurz die Optionen, wähle eine begründete Standardlösung und halte sie als ADR fest. Stoppe und frage nur, wenn

1. eine Anforderung technisch nicht erfüllbar ist oder sich zwei Anforderungen widersprechen,
2. du von einer Standardentscheidung aus Abschnitt 5 abweichen willst,
3. eine Entscheidung nach der ersten Weitergabe schwer umkehrbar wäre und dieser Auftrag sie nicht festlegt,
4. Kosten, Konten oder Zugangsdaten nötig sind, etwa für Signierzertifikate,
5. eine Lizenz mit dem Lizenzmodell kollidiert,
6. ein Abnahmekriterium auch nach Nacharbeit verfehlt wird,
7. echte Nutzerdaten gelöscht oder außerhalb des Projektordners verändert würden,
8. eine Phase abgeschlossen ist. Dann wartest du auf die Freigabe.

---

## 3. Produktvision und Zielgruppe

Project Write ist eine Kombination aus:

- Minimalistischem Texteditor.
- Leistungsfähigem Rich-Text-Editor.
- Markdown-Editor.
- Dokumentenverwaltung.
- Wissensmanagement.
- Visuellem Wissensnetz (Mesh).
- Dateiverwaltung.
- Kontextbezogener Sprachprüfung.

Die Anwendung lässt sich einfach wie ein klassischer Windows-Editor bedienen und bietet gleichzeitig umfangreiche Organisations- und Verknüpfungsmöglichkeiten.

**Zielgruppe:** Menschen, die viel schreiben und ihre Texte und Materialien lokal ordnen wollen, etwa Studierende, Lehrkräfte, Autorinnen und Autoren. Programmierkenntnisse sind nicht nötig.

Leitlinien:

- **Schreiben zuerst.** Datei öffnen, schreiben und speichern funktioniert sofort, ohne Einrichtung und ohne Mesh.
- **Ordnung ist optional.** Subnetze, Tags und Mesh helfen, drängen sich aber nicht auf.
- **Eine Person, ein Gerät.** Keine Konten, keine gemeinsame Bearbeitung.
- **Entwickler-Anmutung heißt** ruhig, dicht und tastaturfreundlich. Sie heißt nicht Fachsprache in der Oberfläche.

---

## 4. Glossar

Verwende diese Begriffe einheitlich in Oberfläche, Code und Dokumentation. Die Wörter aus der rechten Spalte werden in dieser Bedeutung nicht mehr verwendet.

| Begriff | Bedeutung | Ersetzt |
| --- | --- | --- |
| Bibliothek | Der lokale Datenbestand: ein Ordner mit Datenbank, Dokumenten und Medien. In Version 1.0 ist genau eine Bibliothek aktiv. | Projekt, Projektdaten, Project-Write-Speicher |
| Dokument | Ein Text in der Bibliothek: TXT, MD oder `.pwrite`. | |
| Externe Datei | Eine Textdatei außerhalb der Bibliothek, die direkt geöffnet und an ihrem Ort gespeichert wird. | |
| Medium | Bild, PDF, Office-Datei oder sonstiger Anhang in der Bibliothek. | |
| Element | Oberbegriff für Dokument und Medium. | |
| Subnetz | Hierarchische Gruppe von Elementen. Ein Element kann in mehreren Subnetzen liegen. | Bereich, Gruppierung |
| Untersubnetz | Subnetz innerhalb eines anderen Subnetzes. | Untergruppe |
| Mesh | Visuelles Netz aus Knoten und Verbindungen. | Knowledge Graph, Wissensnetzwerk |
| Knoten | Darstellung eines Elements oder Subnetzes im Mesh. | |
| Verbindung | Beziehung zwischen zwei Knoten. Bestätigte Verbindungen werden gespeichert, abgeleitete werden aus anderen Daten berechnet. | |
| Vorschlag | Von der App errechnete, noch nicht bestätigte Beziehung. | Automatische Gruppierung als Ergebnis |
| Cluster | Farbige Hülle eines Subnetzes im Mesh. Kein eigener Datensatz. | |
| Tag | Frei vergebenes Stichwort. Dient als Filter, nicht als Hierarchie. | |
| Sprachprüfung | Prüfung von Rechtschreibung, Grammatik und Zeichensetzung. Keine Sprachsteuerung. | Sprachassistenz |
| Autokorrektur | Automatische Änderung eindeutiger Rechtschreibfehler nach Abschnitt 14. | |
| Archiv | Zustand eines Elements: aus Explorer und Standardsuche ausgeblendet, über Filter erreichbar, im Mesh grau. Umkehrbar und ohne Löschung. | |
| Papierkorb | Gelöschte Elemente bleiben 30 Tage (einstellbar) wiederherstellbar und werden danach endgültig entfernt. | |

„Wiederherstellung“ hat fünf benannte Stufen. Nenne in Oberfläche und Code immer die Stufe.

| Stufe | Name | Stellt her |
| --- | --- | --- |
| 1 | Sitzung | Offene Tabs, Layout und Cursorposition nach einem normalen Neustart |
| 2 | Absturz | Nicht gesicherte Änderungen aus Recovery-Entwürfen |
| 3 | Papierkorb | Gelöschte Elemente mit Zuordnungen und Verbindungen |
| 4 | Version | Ältere Fassung eines Dokuments aus der Historie |
| 5 | Backup | Die ganze Bibliothek aus einem Backup |

---

## 5. Umfang und Standardentscheidungen

### Später (nicht in Version 1.0)

- Cloud-Synchronisation, gemeinsame Bearbeitung, Konten und mobile Apps.
- Prüfung der Satzbedeutung, etwa mit lokaler KI. Die Sprachprüfung aus Version 1.0 arbeitet mit Wörterbüchern, Regeln und Statistik. Sie versteht den Sinn eines Satzes nicht, und die Oberfläche verspricht das auch nicht.
- Schlanke Grammatikprüfung ohne Java, sobald es dafür eine lokale Lösung für Deutsch gibt. Harper unterstützt zurzeit nur Englisch.
- Lokale semantische Suche (optional, nie verpflichtend).
- Audio, Video und weitere Dateiformate.
- Vorschau und Konvertierung von Office-Dateien.
- Volltextsuche in PDF-Inhalten und PDF-Anmerkungen.
- RTF-Import und -Export, DOCX-Export.
- Plugin-Code von Dritten.
- Mehrere Bibliotheken gleichzeitig und verschlüsselte Bibliotheken.
- Gliederungsleiste, Dokumentvorlagen, Formeln und Zitierverwaltung.
- Import bestehender Wissenssammlungen mit eigener Link-Syntax, etwa aus Obsidian.
- Export des Mesh als Bild.
- KI-Integrationen. Cloud-KI bleibt immer optional und braucht je Aktion eine Zustimmung.

### Standardentscheidungen

Diese Werte gelten, bis der Auftraggeber sie ändert. Abweichungen brauchen eine Rückfrage.

| Nr. | Thema | Standard |
| --- | --- | --- |
| D1 | Zielgruppe | Wie in Abschnitt 3 |
| D2 | Leitplattform | Windows. macOS und Linux werden über CI gebaut und getestet |
| D3 | Repository und CI | Privates GitHub-Repository mit GitHub Actions |
| D4 | Lizenz der App | Bis vor Phase 14 offen. Bevorzuge Abhängigkeiten unter MIT oder Apache-2.0. Ausnahmen stehen in der Lizenzliste |
| D5 | Autokorrektur | Standardmäßig aus. Der erste Start fragt danach |
| D6 | Grammatikprüfung | LanguageTool als optionales Zusatzpaket |
| D7 | Externe Dateien | Speichern nur mit `Ctrl + S`. Autosave ist optional |
| D8 | „Rich Text“ als Format | Bedeutet `.pwrite`. RTF gehört nicht zu Version 1.0 |
| D9 | Freigaben | Stopp nach jeder Phase |
| D10 | Updates | Updateprüfung standardmäßig aus, Installation nur nach Bestätigung |
| D11 | Signierung | Testversionen unsigniert. Entscheidung für die Auslieferung vor Phase 14 |

---

## 6. Qualitätsziele

Project Write muss:

- Stabil laufen.
- Ohne Internet vollständig funktionieren. Ausgenommen sind nur Updateprüfung und der Download optionaler Zusatzpakete.
- Große Dokumentensammlungen verwalten können.
- Schnell starten.
- Änderungen zuverlässig speichern.
- Datenverlust vermeiden.
- Modular erweiterbar sein.
- Plattformübergreifend funktionieren.
- Eine intuitive Benutzeroberfläche besitzen.

Vermeide unnötig komplexe Architekturen. Bevorzuge verständlichen, wartbaren und gut getesteten Code.

### Messbare Zielwerte

**Referenzbibliothek:** Ein Skript in `scripts/` erzeugt sie reproduzierbar mit 10.000 Dokumenten, 5.000 Medien, 500 Subnetzen, 1.000 Tags und 30.000 bestätigten Verbindungen.

**Referenzgerät:** Vier Jahre alter Mittelklasse-Laptop mit 8 GB RAM und SSD. CI-Messungen zeigen nur den Trend. Kannst du nicht auf einem solchen Gerät messen, nenne das verwendete Gerät im Bericht.

| Merkmal | Zielwert |
| --- | --- |
| Kaltstart bis zur Eingabemöglichkeit | ≤ 2 s |
| Tippverzögerung bei 50.000 Wörtern, Sprachprüfung aktiv | 95 % der Eingaben ≤ 50 ms |
| Dokument mit 50.000 Wörtern öffnen | ≤ 500 ms |
| Recovery-Entwurf nach der letzten Eingabe | ≤ 2 s |
| Globale Suche, erste Treffer | ≤ 300 ms |
| Mesh beim Zoomen und Verschieben | ≥ 30 Bilder pro Sekunde |
| Arbeitsspeicher im Leerlauf, ohne LanguageTool | ≤ 500 MB |
| Installationspaket ohne Zusatzpakete | ≤ 50 MB |
| Datenintegrität bei 100 harten Prozessabbrüchen während des Speicherns | Keine beschädigte Datei, `PRAGMA integrity_check` meldet `ok` |
| Barrierefreiheit | Alle Funktionen per Tastatur, Kontrast mindestens 4,5:1 für Text und 3:1 für grafische Elemente |

---

## 7. Technologiestack

Verwende stabile, miteinander kompatible Versionen. Prüfe beim Projektstart die aktuellen stabilen Versionen, dokumentiere sie in `docs/dependencies.md` und committe die Lockfiles. Vorabversionen sind ausgeschlossen, außer dieser Auftrag nennt sie. Das gilt auch für Tauri: Auf crates.io steht bereits `3.0.0-alpha`, die stabile Linie bleibt 2.x.

Rust übernimmt datenintensive und systemnahe Aufgaben. TypeScript übernimmt Oberfläche und Benutzerinteraktionen.

| Thema | Technologie | Vorgaben |
| --- | --- | --- |
| Backend | Rust, Tauri 2, Serde | |
| Datenbank | SQLite, mit der App gebaut, über SQLx | Nur SQLx. Kein zweites Crate auf `libsqlite3-sys`, etwa `rusqlite`. `tauri-plugin-sql` ist verboten, weil es dem Frontend SQL-Zugriff gäbe |
| Frontend | TypeScript im Strict-Modus, React, Vite, Tailwind CSS, Zustand | |
| Texteditor | Tiptap 3, ProseMirror | Nur MIT-lizenzierte Teile. Kostenpflichtige Tiptap-Erweiterungen sind ausgeschlossen |
| Markdown | `@tiptap/markdown` | Die Erweiterung ist noch Beta. Sie läuft deshalb hinter einer eigenen Adapter-Schicht in `modules/editor/markdown/`. Ausweichlösung: unified/remark, falls die Tests aus Abschnitt 10 scheitern |
| Suchen und Ersetzen im Dokument | `@tiptap/extension-find-and-replace` | |
| Mesh | React Flow (`@xyflow/react`), ELK.js für automatisches Layout im Web Worker | Der Renderer liegt hinter einer Schnittstelle. Ausweichlösung bei verfehltem Lasttest: Sigma.js mit graphology |
| Medien | PDF.js (`pdfjs-dist`), geeignete Rust-Bibliotheken für Bilder und Typerkennung | Immer die aktuelle stabile PDF.js-Version, Konfiguration nach Abschnitt 17 |
| Sprachprüfung | Hunspell-Wörterbücher mit `spellbook`, LanguageTool als Zusatzpaket | `spellbook` gilt noch als Alpha. Vor der endgültigen Wahl prüft Phase 12 es gegen Hunspell-Bindings. Alle Prüfwerke liegen hinter der Schnittstelle `LanguageEngine` |
| Tests | Vitest, Rust Unit- und Integrationstests, WebdriverIO mit `@wdio/tauri-service` | Schnelle Frontend-Tests laufen im Browser-Modus oder mit `mockIPC`. Testplugins nur in Debug-Builds. Playwright nur bei begründetem Bedarf |
| CI | GitHub Actions auf Windows, macOS und Linux | Ab Phase 1 |

**Lizenzen:** Führe ab Phase 1 eine Lizenzliste in `THIRD-PARTY-LICENSES`. Deutsche Hunspell-Wörterbücher stehen in der Regel unter GPL, so auch igerman98. Liefere Wörterbücher deshalb als getrennte Datenpakete mit eigenem Lizenztext aus, nie in den Programmcode einkompiliert. ELK.js steht unter EPL-2.0 oder GPL-3.0. Prüfe beides vor jeder Weitergabe. Stoppe bei Zweifeln.

---

## 8. Architektur und Kommunikation

Die Schichten sind: Oberfläche (React), Frontend-Dienste, typisierte Befehle, Rust-Dienste, Repositories, SQLite und Dateisystem. Keine Schicht wird übersprungen.

### Sichere Kommunikationsschnittstelle

- Nur Rust greift auf Datenbank und Dateisystem zu. Das Frontend enthält weder SQL noch Dateizugriffe.
- Befehle arbeiten mit IDs statt mit Pfaden. Das Frontend erhält Pfade nur als Anzeigetext.
- Für externe Dateien öffnet Rust den Systemdialog und gibt eine Sitzungs-ID zurück. Das Frontend kann so keine frei gewählten Pfade öffnen.
- TypeScript-Typen werden aus den Rust-Typen erzeugt, zum Beispiel mit `ts-rs`. Es gibt keine doppelt gepflegten Typen.
- Rust validiert alle Eingaben.
- Alle Befehle melden Fehler im selben Format: `{ code, messageKey, details? }`. Stacktraces und absolute Pfade erscheinen nie in der Oberfläche.

### Weitere Architekturregeln

- Lange Aufgaben wie Import, Indexierung, Backup und Vorschläge laufen im Hintergrund, melden Fortschritt und lassen sich abbrechen.
- Zusammengehörige Schreibvorgänge laufen in einer Transaktion. Es gibt eine Schreibverbindung, Lesezugriffe nutzen einen Pool.
- Module sprechen nur über ihre öffentlichen Schnittstellen miteinander.
- Der Editor verwaltet sein Undo selbst. Strukturaktionen wie Verschieben, Zuordnen, Löschen und Verbindungen haben ein eigenes Undo.
- Logs bleiben lokal, werden rotiert und enthalten keine Dokumentinhalte.

---

## 9. Speichermodell: Bibliothek und externe Dateien

Alle Bibliotheksdaten werden lokal gespeichert. SQLite verwaltet Metadaten, Beziehungen, Einstellungen und Verwaltungsinformationen. Dateien liegen im Ordner der Bibliothek.

### Zwei Arbeitsweisen

| | Bibliotheksdokument | Externe Datei |
| --- | --- | --- |
| Ort | In der Bibliothek | Am ursprünglichen Ort |
| Öffnen | Explorer, Suche, Mesh | Datei > Öffnen, „Öffnen mit“, Doppelklick, Zuletzt verwendet |
| Speichern | Autosave | `Ctrl + S`, Autosave optional |
| Anzeige | Statusleiste: „Gespeichert“, „Speichert …“ oder „Fehler beim Speichern“ | Änderungspunkt im Tab |
| Schließen | Ohne Rückfrage | Mit Änderungen: Speichern, Verwerfen oder Abbrechen |
| Historie, Suche, Mesh | Ja | Nein, erst nach „In Bibliothek aufnehmen“ |

„In Bibliothek aufnehmen“ legt eine Kopie an. Die Originaldatei bleibt unverändert.

### Speichern

- Jeder Schreibvorgang ist atomar: Temporärdatei im selben Ordner, Daten auf den Datenträger schreiben (`fsync`), dann umbenennen.
- Autosave läuft 1 s nach der letzten Eingabe sowie bei Tab-Wechsel, Fokusverlust und Schließen.
- Recovery-Entwürfe landen spätestens 2 s nach der letzten Eingabe in `recovery/`, auch für externe Dateien.
- Schlägt das Speichern fehl, bleibt der Hinweis sichtbar und die Änderung im Entwurf erhalten.

### Externe Dateien

- Erkenne Kodierung (UTF-8, UTF-8 mit BOM, UTF-16, Windows-1252) und Zeilenenden (LF, CRLF) und behalte beide beim Speichern bei. Die Statusleiste zeigt beides und erlaubt das Umstellen.
- Unveränderte Dateien werden nie neu geschrieben.
- Erkenne Änderungen von außen. Der Nutzer wählt dann: Neu laden, Meine Fassung behalten oder Als Kopie speichern.
- Ist die Datei schreibgeschützt oder verschwunden, biete „Speichern unter“ an.
- Warne ab 10 MB. Binärdateien werden nicht als Text geöffnet.
- `.pwrite`-Dateien von außen werden wie externe Dateien behandelt. Fehlende Medien erscheinen als sichtbarer Hinweis „Medium fehlt“.

### Bibliothek

- Der Speicherort ist beim ersten Start wählbar. Standard ist `Dokumente/ProjectWrite`.
- Der Umzug einer Bibliothek läuft in drei Schritten: kopieren, prüfen, umschalten. Der alte Ordner bleibt, bis der Nutzer ihn entfernt.
- Eine Bibliothek ist immer nur in einer App-Instanz geöffnet. Das sichern `tauri-plugin-single-instance` und `library.lock`.
- Erkenne bekannte Sync-Ordner (OneDrive, Dropbox, iCloud Drive, Google Drive) und Netzlaufwerke. Warne und empfiehl einen lokalen Ort. Wählt der Nutzer ihn trotzdem, nutze dort den Rollback-Modus (`journal_mode=DELETE`) statt WAL.

### Einstellungen

- Bibliotheksweite Einstellungen liegen in der Tabelle `application_settings`.
- Appweite Einstellungen wie Bibliothekspfad, Fensterzustand, Sprache und Tastenkürzel liegen im Konfigurationsordner des Systems, weil sie schon vor dem Öffnen der Bibliothek gebraucht werden.

### Dateizuordnung

`.pwrite` wird bei der Installation zugeordnet, `.md` und `.txt` nur auf Wunsch. Das Öffnen muss auf drei Wegen funktionieren:

1. Startargumente unter Windows und Linux.
2. `RunEvent::Opened` unter macOS.
3. Zweiter Programmstart über `tauri-plugin-single-instance`.

Rust puffert die Pfade, bis die Oberfläche bereit ist, und holt das Fenster in den Vordergrund.

### Speicherstruktur

```text
ProjectWrite/                 Bibliothek, Ort wählbar
    library.json              Bibliotheks-ID und Formatversion
    library.lock
    database/
        projectwrite.db
    documents/
        text/                 .txt
        markdown/             .md
        richtext/             .pwrite
    media/
        images/
        pdf/
        attachments/          Office-Dateien und sonstige Anhänge
        audio/                später
        video/                später
    thumbnails/
        images/
        pdf/
    recovery/                 Entwürfe für die Absturz-Wiederherstellung
    tmp/                      Zwischendateien, beim Start geleert
    backups/                  Backups, wenn kein anderes Ziel gewählt ist
    exports/
```

Dateien in der Bibliothek tragen UUID-Namen. Titel und Originalnamen stehen in der Datenbank. Umbenennen ändert daher nur den Datenbankeintrag.

### Import in die Bibliothek

1. Dateityp prüfen (Abschnitt 20).
2. Nach `tmp/` kopieren und SHA-256 berechnen.
3. Bei einem Duplikat fragen: vorhandenes Element verwenden oder trotzdem importieren.
4. In einer Transaktion: Metadaten schreiben, darunter Originaldateiname und Importordner, und die Datei mit UUID-Namen an den Zielort verschieben.
5. Vorschaubild im Hintergrund erzeugen.

Regeln:

- Originaldateien dürfen beim Import nicht verändert werden.
- Bei Fehlern werden unvollständige Kopien und Datenbankeinträge bereinigt.
- Beim Start wird `tmp/` geleert. Dateien ohne Datenbankeintrag werden gemeldet, nicht still gelöscht.
- Dateien werden nur gelöscht, wenn sie nicht mehr referenziert werden.
- Ganze Ordner lassen sich importieren. Der Importordner bleibt Metadatum. Auf Wunsch entsteht aus der Ordnerstruktur eine Subnetz-Struktur.

---

## 10. Dateiformate

| Format | Inhalt | Ablage |
| --- | --- | --- |
| TXT | Reiner Text ohne Formatierung | `documents/text/` |
| MD | CommonMark und GFM (Tabellen, Aufgabenlisten, Durchstreichen), dazu die erlaubte HTML-Teilmenge | `documents/markdown/` |
| `.pwrite` | Rich Text mit Metadaten und Medienverweisen. Dies ist das Rich-Text-Format der App | `documents/richtext/` |

### `.pwrite`, Version 1

Eine `.pwrite`-Datei ist UTF-8-JSON. Medien werden referenziert, nicht eingebettet.

```json
{
  "format": "pwrite",
  "formatVersion": 1,
  "id": "UUID des Dokuments",
  "title": "Titel",
  "createdAt": "2026-11-02T10:00:00Z",
  "updatedAt": "2026-11-02T10:05:00Z",
  "language": "de-DE",
  "tags": ["Beispiel"],
  "content": { "type": "doc", "content": [] },
  "media": [
    { "id": "UUID", "sha256": "Prüfsumme", "originalName": "bild.png", "mimeType": "image/png" }
  ]
}
```

Regeln:

- Dokumentiere das Format in `docs/pwrite-format.md`. `content` folgt einem dort beschriebenen Schema. Jede Schemaänderung erhöht die Formatversion und bringt eine Migration mit.
- Unbekannte Felder bleiben beim Laden und Speichern erhalten.
- Dateien aus einer neueren Formatversion öffnen schreibgeschützt und mit Hinweis.

### Transportpaket `.pwritepack`

„Als Paket exportieren“ erzeugt ein ZIP mit `document.pwrite`, `manifest.json` und den Medien in `media/`. Der Import prüft Pfade, Größen und Prüfsummen nach Abschnitt 20.

### Markdown: „verlustfrei“ ist prüfbar

- Lege in `tests/fixtures/markdown/` mindestens 50 Fälle an. Der Round-Trip-Test läuft von Import über Editor zu Export. Inhalt und Struktur müssen gleich bleiben.
- Erlaubte Normalisierungen sind abschließend dokumentiert, etwa `_x_` zu `*x*` oder Setext- zu ATX-Überschriften.
- Vergleiche nicht nur über gerendertes HTML: Dieser Vergleich hielt in der Stichprobe einen zerstörten YAML-Kopf und eine zerstörte Fußnote für gleichwertig.

Bekannte Verlustfälle aus der Stichprobe mit Tiptap 3.32.2. Von 37 Fällen blieben 32 ohne Inhaltsverlust. Prüfe die folgenden Fälle erneut und löse sie so:

| Inhalt | Ohne Schutz | Vorgabe |
| --- | --- | --- |
| YAML-Kopf | Wird zu Trennlinie und Überschrift | Der Adapter trennt ihn vor dem Parsen ab und setzt ihn beim Speichern unverändert wieder ein |
| Fußnoten | Werden zu einem kaputten Link | Eigene Erweiterung oder Rohblock |
| HTML-Blöcke und unbekanntes Inline-HTML | Werden entfernt | Bleiben als nicht ausgeführter Rohblock erhalten |
| Bild in einem Link | Der Link geht verloren | Eigene Regel oder Rohblock |
| Bildgröße | Geht verloren | Erlaubte HTML-Teilmenge |
| Verbundene Zellen, mehrere Blöcke in einer Tabellenzelle | Werden vereinfacht | Warnung vor dem Speichern |

**Erlaubte HTML-Teilmenge in Markdown:** `<u>`, `<br>` und `<img>` mit `src`, `alt`, `title`, `width` und `height`. Alles andere bleibt Rohtext und wird nie ausgeführt.

**Verlustschutz:** Würde beim Speichern als MD oder TXT etwas verloren gehen, warne vorher und nenne die betroffenen Inhalte. Biete an: Trotzdem speichern, Als `.pwrite` speichern oder Abbrechen. Das gilt auch beim Umwandeln zwischen den Formaten.

Der Adapter prüft schon beim Öffnen per Round-Trip-Probe, ob er ein Markdown-Dokument verlustfrei halten kann. Kann er es nicht, öffnet es in der Quelltextansicht und nennt den Grund. So überschreibt auch Autosave nichts still.

---

## 11. Benutzeroberfläche

Entwickle eine minimalistische, moderne Oberfläche mit Entwickler-Anmutung. Orientiere dich gestalterisch an klassischen Editoren und modernen Entwicklungsumgebungen.

### Hauptlayout

Die Anwendung besitzt:

1. Obere Menüleiste. Sie ist das native Tauri-Menü, auf macOS also die Systemleiste.
2. Toolbar.
3. Linke Activity Bar.
4. Seitenleiste mit Explorer oder Suche.
5. Zentrale Arbeitsfläche.
6. Dokument-Tabs.
7. Statusleiste.

### Navigation

- **Hauptansichten** in der zentralen Arbeitsfläche: Editor und Mesh.
- **Seitenleisten:** Explorer und Suche.
- Die Activity Bar schaltet zwischen allen vier um.

### Weitere Funktionen

- Dark Mode und Light Mode, standardmäßig passend zum System.
- Anpassbare Themes über Design-Tokens und importierbare Theme-Dateien.
- Command Palette mit `Ctrl + Shift + P`. Sie zeigt nur Befehle, die in der aktuellen Phase funktionieren.
- Konfigurierbare Tastenkürzel mit Konflikterkennung.
- Kontextmenüs.
- Drag & Drop.
- Anpassbare Seitenleisten: Breite ändern, ein- und ausblenden.
- Mehrere gleichzeitig geöffnete Dokumente.
- Fokusmodus: blendet alles außer der Schreibfläche aus.
- Oberflächenzoom.

### Standardkürzel

Auf macOS ersetzt `Cmd` die Taste `Ctrl`.

| Aktion | Kürzel |
| --- | --- |
| Neu, Öffnen, Speichern | `Ctrl + N`, `Ctrl + O`, `Ctrl + S` |
| Speichern unter | `Ctrl + Shift + S` |
| Tab schließen | `Ctrl + W` |
| Rückgängig, Wiederholen | `Ctrl + Z`, `Ctrl + Y` |
| Im Dokument suchen, ersetzen | `Ctrl + F`, `Ctrl + H` |
| Global suchen | `Ctrl + Shift + F` |
| Command Palette | `Ctrl + Shift + P` |
| Quelltextansicht | `Ctrl + /` |
| Drucken | `Ctrl + P` |
| Fokusmodus | `F11` |

### Startwerte für das Design

Das ist das Project-Write-Design für den Startbildschirm und alle weiteren Ansichten. Alle Farben liegen als Design-Tokens vor.

| Token | Dark | Light |
| --- | --- | --- |
| Schreibfläche | `#1E1E1E` | `#FFFFFF` |
| Seitenleiste | `#252526` | `#F3F3F3` |
| Text | `#D4D4D4` | `#1F1F1F` |
| Gedämpfter Text | `#A0A4AA` | `#5F6368` |
| Akzent | `#6C9BFF` | `#2F5FE0` |
| Fehler | `#F48771` | `#C72E0F` |
| Cluster Blau | `#6C9BFF` | `#2F5FE0` |
| Cluster Grün | `#4EC97A` | `#1B7F3B` |
| Cluster Violett | `#B58CFF` | `#7A4FD0` |
| Cluster Orange | `#F0A050` | `#B85C00` |
| Cluster Grau | `#9AA0A6` | `#6B7075` |

Diese Farben erreichen auf der Schreibfläche mindestens 4,5:1, Text, gedämpfter Text und Akzent auch auf der Seitenleiste. Prüfe jede weitere Kombination.

- **Schrift:** Systemschriften, `system-ui` für die Oberfläche, `ui-monospace` für Code. Keine Schriften aus dem Netz.
- **Größen:** Oberfläche 13 px, Schreibfläche 16 px mit Zeilenhöhe 1,6, Zeilenbreite höchstens 72 Zeichen (abschaltbar).
- **Abstände:** 4-px-Raster, Eckenradius 4 px.
- **Maße:** Activity Bar 48 px, Seitenleiste 260 px, Tabs und Toolbar je 36 px, Statusleiste 24 px.
- **Symbole:** eine einheitliche lokale Symbolbibliothek, etwa `lucide-react`.

### Drag & Drop

- Setze `dragDropEnabled: false`, denn nur so funktioniert HTML5-Drag-and-Drop unter Windows.
- Dateien aus dem Betriebssystem kommen dann als `File`-Objekte ohne absoluten Pfad an. Ablegen bedeutet deshalb Import in die Bibliothek. Bearbeiten am Ursprungsort läuft über Datei > Öffnen.
- Teste das in Phase 3 auf allen drei Betriebssystemen.

### Erster Start

Ein kurzer Dialog fragt nach Bibliotheksort, Sprache und Autokorrektur. Danach öffnet sich sofort ein leeres Dokument. Beispiel-Subnetze (Projekte, Wissen, Ideen) sind optional.

### Sprache und Barrierefreiheit

- Oberfläche auf Deutsch und Englisch über Sprachdateien, standardmäßig passend zum System.
- Sichtbarer Fokus, ARIA-Beschriftungen, Rücksicht auf `prefers-reduced-motion`.
- Farbe trägt nie allein die Bedeutung.
- Die Seitenleiste „Verbindungen“ zeigt die Verbindungen eines Elements als Liste. So ist das Mesh auch ohne Maus und mit Screenreader nutzbar.

---

## 12. Texteditor

Entwickle einen leistungsfähigen hybriden Editor. Hybrid heißt: eine Schreibfläche, auf der Formatierungen sofort sichtbar sind und Markdown-Kürzel wie `## ` oder `**fett**` direkt wirken. Markdown-Dokumente haben zusätzlich eine Quelltextansicht.

| Dokumenttyp | Schreibfläche | Quelltextansicht |
| --- | --- | --- |
| `.pwrite` | Rich Text mit Markdown-Kürzeln | Nein |
| `.md` | Rich Text mit Markdown-Kürzeln | Ja |
| `.txt` | Reintext | Nicht nötig |

Reintext und Quelltextansicht nutzen denselben Modus: Tiptap mit Minimal-Schema, Leerzeichen und Tabulatoren bleiben exakt erhalten. Die Quelltextansicht ändert nur, was der Nutzer tippt. Eine zweite Editor-Bibliothek kommt nur infrage, wenn der Leistungstest aus Abschnitt 6 sonst scheitert.

### Rich Text

Unterstütze:

- Überschriften.
- Fett und kursiv.
- Unterstreichungen.
- Durchgestrichenen Text.
- Listen.
- Nummerierungen.
- Aufgabenlisten.
- Zitate.
- Tabellen.
- Links.
- Codeblöcke.
- Inline-Code.
- Bilder.
- PDF-Verweise.
- Trennlinien.

### Markdown

- Markdown schreiben.
- Markdown darstellen.
- Markdown importieren.
- Markdown exportieren.
- Rich Text und Markdown miteinander kombinieren: Kürzel und eingefügtes Markdown wirken auch in `.pwrite`, Rich-Text-Bedienung auch in `.md`.
- Formatierungen verlustfrei umwandeln, geprüft nach Abschnitt 10.

### Interne Links und Rückverweise

- `[[` öffnet eine Dokumentauswahl. Der Link verweist auf die Dokument-ID und überlebt so das Umbenennen. In Markdown wird er als normaler Link mit dem Schema `pwrite://` gespeichert.
- Die Seitenleiste „Verbindungen“ zeigt, welche Dokumente auf das aktuelle verweisen.
- Interne Links speisen die Referenz-Verbindungen im Mesh (Abschnitt 15).

### Suchen, Einfügen, Drucken

- Suchen und Ersetzen im Dokument mit Treffernavigation, Groß-/Kleinschreibung, ganzen Wörtern und regulären Ausdrücken.
- Eingefügtes HTML durchläuft das Editor-Schema. `Ctrl + Shift + V` fügt reinen Text ein. Eingefügte Bilder werden in die Bibliothek importiert.
- Drucken nach Abschnitt 21.

### Links

Weblinks öffnen mit `Ctrl + Klick` im Systembrowser. Erlaubt sind nur `http`, `https` und `mailto`. Bilder mit Webadresse werden nie automatisch geladen. Sie erscheinen als Hinweis mit Adresse.

### Dokument-Tabs

- Mehrere Dokumente öffnen.
- Tabs verschieben.
- Tabs schließen.
- Ungespeicherte Änderungen markieren, wie in Abschnitt 9 festgelegt.
- Dokumente wiederherstellen (Stufe 1 und 2 aus Abschnitt 4).
- Der Undo-Verlauf bleibt erhalten, solange der Tab offen ist.

### Schreibstatistik

- Wortanzahl, nach Unicode-Regeln gezählt.
- Zeichenzahl.
- Zeichen ohne Leerzeichen.
- Absatzanzahl.
- Lesezeit als optionale Anzeige, 200 Wörter pro Minute (einstellbar).
- Bei einer Markierung gelten die Werte für die Markierung.

---

## 13. Dokumentverwaltung

Entwickle eine vollständige Dokumentenverwaltung.

Funktionen:

- Dokument erstellen.
- Dokument öffnen.
- Dokument umbenennen. Das ändert nur den Titel.
- Dokument kopieren. Die Kopie erhält eine neue ID und behält Tags und Subnetze, aber nicht die Historie.
- Dokument verschieben. Das ändert das primäre Subnetz im Explorer. Die Datei bleibt, wo sie ist.
- Dokument archivieren.
- Dokument löschen. Es wandert in den Papierkorb. Zuordnungen und Verbindungen bleiben erhalten und kehren beim Wiederherstellen zurück.
- Favoriten.
- Tags: anlegen, umbenennen, zusammenführen, löschen. Schreibweisen sind ohne Rücksicht auf Groß- und Kleinschreibung eindeutig.
- Dokumenthistorie.
- Autosave (Abschnitt 9).
- Wiederherstellung (alle Stufen aus Abschnitt 4).
- Zuletzt verwendet.
- Umwandeln zwischen TXT, MD und `.pwrite` mit Verlustschutz.

Unterstützte Dokumentformate sind TXT, MD und `.pwrite` (Abschnitt 10).

### Explorer

Der Baum im Explorer besteht aus Subnetzen, nicht aus Dateiordnern. Ein Element in mehreren Subnetzen erscheint überall, bleibt aber ein einziges Element. Dazu kommen die Einträge Favoriten, Zuletzt verwendet, Nicht zugeordnet, Medien, Archiv und Papierkorb. Lange Listen werden virtualisiert.

### Dokumenthistorie

- Automatische Versionen entstehen alle 10 Minuten aktiver Bearbeitung, beim Schließen und vor jeder Umwandlung oder Wiederherstellung.
- „Version speichern“ erstellt eine benannte Version.
- Aufbewahrung: die letzten 24 Stunden alle, bis 30 Tage eine pro Tag, bis ein Jahr eine pro Woche. Benannte Versionen bleiben. Alles ist einstellbar.
- Versionen werden komprimiert gespeichert. Inhaltsgleiche Versionen entfallen.
- Zwei Versionen lassen sich nebeneinander vergleichen.
- Eine Wiederherstellung sichert zuerst den aktuellen Stand als neue Version. Nichts geht verloren.

---

## 14. Sprachprüfung

Entwickle ein eigenständiges Modul unter `src/modules/language-check/`. Die Prüfwerke laufen in Rust hinter der Schnittstelle `LanguageEngine`, damit die lokale Sprachverarbeitung erweiterbar bleibt.

### Was welches Prüfwerk leistet

| Prüfung | Ohne Zusatzpaket | Mit LanguageTool |
| --- | --- | --- |
| Rechtschreibung, eigene Wörterbücher | Ja | Ja |
| Einfache Regeln: doppelte Wörter, doppelte Leerzeichen, Großschreibung am Satzanfang | Ja | Ja |
| Grammatik, Zeichensetzung | Nein | Ja |
| Kontextfehler und mehrdeutige Wörter | Nein | Ja, regelbasiert. Mit n-gram-Daten auch statistisch erkannte Wortverwechslungen |
| Sinn eines Satzes | Nein | Nein |

Die Einstellungen zeigen diesen Stand offen an.

### Rechtschreibprüfung

- Rechtschreibfehler erkennen.
- Fehler rot unterstreichen (gewellt).
- Grammatikfehler erkennen und blau gestrichelt unterstreichen.
- Zeichensetzungsfehler erkennen.
- Kontextbezogene Fehler erkennen und rot unterstreichen.
- Mehrdeutige Wörter berücksichtigen.
- Code, Inline-Code und URLs werden nicht geprüft.
- Die Prüfung läuft absatzweise mit Cache, nie im Eingabe-Thread. Treffer werden sauber in Editor-Positionen zurückgerechnet.
- Die gesamte Sprachprüfung ist abschaltbar.

**Vorschläge:** In der Stichprobe (`spellbook` 0.4.2, igerman98-Wörterbuch, 75 richtige und 30 falsche Wörter) wurden 74 richtige Wörter akzeptiert und alle 30 Fehler erkannt. Die Listen enthielten aber erfundene Komposita, bei `Worterbuch` stand `Worterbtuch` vorne. Der Hunspell-Nachbau spylls zeigte dasselbe Muster. Sortiere deshalb neu und zeige höchstens 5 Vorschläge: zuerst Umlaut- und ß-Varianten (`ue` zu `ü`, `ss` zu `ß`), dann Treffer aus eigenen Wörterbüchern, dann Wörter ohne Zusammensetzung, zuletzt den Rest. Ein unbekanntes Kompositum wie `Flohmarktstand` wird nur markiert und lässt sich mit einem Klick ins eigene Wörterbuch aufnehmen.

### Autokorrektur

Eindeutige Rechtschreibfehler dürfen automatisch korrigiert werden, sofern die Funktion aktiviert ist.

- Wörter werden nach Abschluss der Eingabe geprüft: bei Leerzeichen, Satzzeichen oder Enter, nie während einer Texteingabe mit Akzent- oder Kompositionstasten.
- Nach Satzabschluss wird der Satz geprüft, nicht umgeschrieben. Treffer werden markiert. Automatisch geändert wird nur, was die Eindeutigkeitsregel erfüllt.
- Den persönlichen Schreibstil erhalten: keine Umformulierungen, keine geänderte Wortwahl, Stilregeln standardmäßig aus.
- Keine inhaltlichen Veränderungen ohne Zustimmung.
- Jede Korrektur ist ein eigener Undo-Schritt und wird kurz sichtbar markiert. `Ctrl + Z` stellt das getippte Wort wieder her und merkt es als abgelehnt.
- Die Autokorrektur ist global und je Dokument deaktivierbar.
- Vorschläge dauerten in der Stichprobe bis zu 330 ms je Wort. Die Korrektur läuft daher asynchron und verfällt, wenn sich das Wort inzwischen geändert hat.

**Eindeutigkeitsregel:** Ein Fehler gilt nur als eindeutig, wenn alle sieben Bedingungen zutreffen.

1. Die Autokorrektur ist für dieses Dokument aktiv.
2. Das Wort steht in keinem aktiven Wörterbuch und ist in keiner aktiven Sprache des Abschnitts korrekt.
3. Es gibt genau einen Einwort-Vorschlag mit Editierabstand 1. Eine Vertauschung zählt als ein Schritt, Vorschläge mit Leerzeichen oder Bindestrich zählen nicht.
4. Das Wort hat mindestens 4 Zeichen.
5. Es enthält keine Ziffern, keine Binnenmajuskel und keines der Zeichen `@`, `/`, `_` oder `.` und besteht nicht nur aus Großbuchstaben.
6. Es steht nicht in Code, einem Link, einem Zitat oder einem anders ausgezeichneten Sprachabschnitt.
7. Der Nutzer hat diese Korrektur nicht schon einmal abgelehnt.

In der Stichprobe griff Bedingung 3 bei 10 von 30 Fehlern und lag zehnmal richtig. Lege diese Fälle als Testdaten an und erweitere sie.

Bei mehrdeutigen oder kontextabhängigen Fehlern:

- Rot unterstreichen.
- Korrekturvorschläge anzeigen.
- Der Nutzer entscheidet über die Übernahme.

### Satzende

Erkenne das Satzende anhand von Punkt, Fragezeichen und Ausrufezeichen über eine Satzsegmentierung mit Ausnahmeliste je Sprache. `Intl.Segmenter` allein reicht nicht: In der Stichprobe trennte es nach `z.`, `Dr.`, `1.` und `ca.` und mitten in einer URL. Lege als Testdaten mindestens an: `z. B.`, `d. h.`, `Dr.`, `ca.`, `1. Mai`, `3.14`, URLs und `...`.

### Eigene Wörterbücher

Nutzer können:

- Eigene Wörter hinzufügen.
- Wörter bearbeiten.
- Wörter löschen.
- Fachbegriffe speichern.
- Eigennamen speichern.
- Abkürzungen speichern.
- Wörterbücher erstellen.
- Wörterbücher importieren.
- Wörterbücher exportieren.

Austauschformat ist UTF-8-Text mit einem Eintrag je Zeile. Hunspell-Dateien (`.aff`, `.dic`) lassen sich als weitere Sprachwörterbücher hinzufügen.

### Wörterbücher zuweisen

Wörterbücher können zugewiesen werden an:

- Gesamte Anwendung.
- Einzelne Dokumente.
- Einzelne Subnetze.
- Mehrere dieser Ziele gleichzeitig.

Für ein Dokument gilt die Vereinigung aller Treffer: globale Wörterbücher, Wörterbücher des Dokuments, aller seiner Subnetze und deren Elternsubnetze. Ein Wort ist korrekt, wenn es in einem davon steht.

### Sprachverwaltung

- Automatische Spracherkennung, lokal und absatzweise, etwa mit `whatlang`.
- Mehrsprachige Dokumente.
- Manuelle Sprachauswahl für Dokument und Textabschnitt.
- Sprachprofile pro Dokument.
- Mehrere aktive Wörterbücher.

Welche Sprache für einen Abschnitt gilt, entscheidet diese Rangfolge:

1. Manuell ausgezeichneter Textabschnitt.
2. Erkennung des Absatzes, wenn sie aktiv und sicher genug ist. Sehr kurze Absätze erben die Sprache.
3. Sprachprofil des Dokuments.
4. Standardsprache der App.

Mitgeliefert werden Deutsch (`de-DE`, `de-AT`, `de-CH`) und Englisch (`en-US`, `en-GB`).

### LanguageTool als Zusatzpaket

- Der Server braucht Java 17 und je nach Sprachen 1 bis 4 GB RAM. Die optionalen n-gram-Daten für Deutsch umfassen rund 3 GB. Deshalb ist das Paket optional und nie Bedingung für die App.
- Die Installation läuft erst nach Bestätigung, mit Anzeige von Größe und Lizenz. Alternativ lässt sich eine vorhandene lokale Installation angeben.
- Der Server startet bei Bedarf als Sidecar, bindet nur an `127.0.0.1` mit zufälligem Port und endet mit der App. Er hat keine Anmeldung, daher spricht nur Rust mit ihm.
- LanguageTool prüft je Anfrage eine Sprache. Mehrsprachige Dokumente werden je Sprachabschnitt geprüft.
- Fällt der Server aus, läuft die Rechtschreibprüfung weiter und die Statusleiste zeigt den Zustand.

---

## 15. Mesh – Visuelles Wissensnetz

Das Mesh ist ein zentrales Organisationsmodul von Project Write. Es visualisiert Dokumente, Medien und Subnetze als miteinander verbundene Knoten.

### Knotentypen

- Dokumente.
- Medien.
- Subnetze.

### Verbindungstypen

| Typ | Quelle | Gespeichert in | Darstellung |
| --- | --- | --- | --- |
| Manuelle Verbindung | Nutzer | `mesh_connections` | Durchgezogene Kante, ungerichtet |
| Referenz | Interne Links und eingebundene Medien | Abgeleitet aus `document_links` und `document_media` | Kante mit Pfeil |
| Tag | Gemeinsame Tags | Abgeleitet aus `document_tags` und `media_tags` | Keine Dauerkante, sondern Filter und Hervorhebung. Auf Wunsch ein temporärer Tag-Knoten mit Sternform |
| Ähnlichkeit | Berechnung | `mesh_suggestions`, nach Bestätigung `mesh_connections` | Gestrichelt, solange sie Vorschlag ist |
| Enthalten-in-Beziehung | Subnetz-Zuordnung | Abgeleitet aus `subnet_memberships` | Cluster-Hülle für das primäre Subnetz, Kante für jedes weitere |

Regeln:

- `mesh_connections` enthält nur Bestätigtes. Abgeleitete Verbindungen werden berechnet und nicht doppelt gespeichert.
- Tags erzeugen keine paarweisen Kanten: 100 Dokumente mit demselben Tag ergäben sonst 4.950 Kanten.
- Abgeleitete Verbindungen lassen sich im Mesh nicht löschen. Das Kontextmenü führt zu ihrer Quelle.

### Mesh-Ansichten

- **Global View** zeigt das gesamte Wissensnetz in verdichteter Form: oberste Subnetze als eingeklappte Cluster mit Elementzahl, Kanten zwischen Clustern zusammengefasst. Aufklappen zeigt den Inhalt. Höchstens 1.000 Knoten werden gleichzeitig gezeichnet.
- **Subnet View** zeigt ein ausgewähltes Subnetz und seine Untersubnetze.
- **Focus View** zeigt einen bestimmten Knoten und seine direkten Verbindungen, auf Wunsch auch eine Stufe weiter.

### Bedienung

| Eingabe | Wirkung |
| --- | --- |
| Klick auf Dokument oder Medium | Wählt den Knoten aus |
| Klick auf Subnetz | Öffnet den Inhalt in der Subnet View |
| `Umschalt + Klick` | Wählt jeden Knotentyp aus, auch Subnetze und mehrere Knoten |
| Doppelklick | Öffnet Dokumente und Medien |
| Rechtsklick | Öffnet das Kontextmenü |
| Ziehen eines Knotens | Verschiebt ihn. Die Position wird gespeichert |
| Ablegen auf einem Subnetz | Ordnet zu, mit sichtbarer Rückmeldung und Undo |
| Ablegen aus dem Explorer | Fügt das Element als Knoten ein oder ordnet es dem Ziel-Subnetz zu |
| Griff am Knoten ziehen | Erstellt eine Verbindung |
| `Entf` | Entfernt die ausgewählte bestätigte Verbindung |
| Mausrad, Ziehen der Fläche | Zoom und Pan |

Dazu kommen Breadcrumb-Navigation, Minimap und Tastaturbedienung mit Pfeiltasten und Enter. Ziehen aus einer Hülle ändert nur die Position. Das Entfernen aus einem Subnetz läuft über das Kontextmenü.

### Farbige Cluster

Ein Cluster ist die farbige Hülle eines Subnetzes. Verwende farbige Cluster zur visuellen Organisation.

Standardfarben:

- Blau: Beispiel-Subnetz Projekte.
- Grün: Beispiel-Subnetz Wissen.
- Violett: Beispiel-Subnetz Ideen.
- Orange: Medienknoten. Die Farbe folgt aus dem Typ, nicht aus einem Cluster.
- Grau: archivierte Elemente. Die Farbe folgt aus dem Zustand, nicht aus einem Cluster.

Farben müssen individuell einstellbar sein. Symbole unterscheiden die Knotentypen zusätzlich.

**Mehrere Subnetze:** React Flow hängt einen Kindknoten über genau ein `parentId` an seinen Elternknoten. Ein Knoten sitzt deshalb in der Hülle seines primären Subnetzes. Weitere Zugehörigkeiten erscheinen als Enthalten-in-Kanten.

### Automatische Gruppierung

Analysiere:

- Tags.
- Titelähnlichkeiten: Trigramm-Ähnlichkeit der normalisierten Titel.
- Inhaltsähnlichkeiten: lokale Textstatistik, TF-IDF mit Kosinus-Ähnlichkeit.
- Verlinkungen.
- Gemeinsame Importordner.
- Bereits bestehende Beziehungen.

Der Gesamtwert ist eine gewichtete Summe. Vorschläge entstehen ab einem einstellbaren Schwellenwert, Startwert 0,35. Jeder Vorschlag nennt seine Begründung, etwa „3 gemeinsame Tags, Inhaltsähnlichkeit 0,62“. Die Analyse läuft im Hintergrund und lässt sich abbrechen.

Automatische Gruppierungen sind ausschließlich Vorschläge.

Der Nutzer entscheidet über deren Übernahme: Annehmen, Ablehnen oder Später. Abgelehnte Vorschläge kommen nicht wieder.

Manuell bestätigte Beziehungen dürfen niemals automatisch überschrieben werden.

Speichere Vorschläge getrennt von bestätigten Beziehungen.

### Layout und Leistung

- ELK.js läuft im Web Worker. Layouts und Positionen werden je Ansicht in `mesh_layouts` und `mesh_positions` gespeichert.
- Manuell verschobene Knoten bleiben fixiert. Das automatische Layout ändert sie nur auf ausdrücklichen Befehl und mit Undo.
- Memoisiere Knoten-Komponenten, klappe große Bäume über `hidden` ein und verzichte bei vielen Knoten auf aufwendige Schatten und Animationen. Elternknoten stehen im Array vor ihren Kindern.
- Phase 11 beginnt mit einem Lasttest gegen die Referenzbibliothek. Wird das Ziel aus Abschnitt 6 verfehlt, entscheide per ADR über den Wechsel zu Sigma.js.

---

## 16. Subnetze

Subnetze dienen der hierarchischen Organisation.

Jedes Subnetz besitzt:

- ID.
- Name.
- Beschreibung.
- Farbe.
- Übergeordnetes Subnetz.
- Untersubnetze (über das Feld „Übergeordnetes Subnetz“).
- Erstellungsdatum.
- Änderungsdatum.

Regeln:

- Ein Element, also ein Dokument oder Medium, darf mehreren Subnetzen zugewiesen werden.
- Elemente dürfen dabei nicht dupliziert werden.
- Die Zugehörigkeit wird über separate relationale Datenbanktabellen verwaltet.
- Je Element ist genau eine Zuordnung primär. Sie bestimmt die Cluster-Hülle im Mesh und den Platz beim Verschieben. Die erste Zuordnung wird automatisch primär. Entfällt sie, rückt die älteste verbleibende nach.
- Zyklen in der Hierarchie sind ausgeschlossen.
- Das Löschen eines Subnetzes löscht nie Elemente. Untersubnetze rücken eine Ebene hoch, wenn der Nutzer nichts anderes wählt.

---

## 17. Medienverwaltung

Entwickle ein eigenständiges Medienmodul.

### Bilder

Unterstütze:

- PNG.
- JPG.
- JPEG.
- WebP.
- GIF.
- SVG.

Funktionen:

- Import.
- Vorschau.
- Galerie, virtualisiert, mit Vorschaubildern aus `thumbnails/`.
- Größenanpassung. Sie ändert nur die Anzeigegröße im Dokument, nie die Datei.
- Bildunterschriften.
- Verknüpfung mit Dokumenten.
- Zuordnung zu Subnetzen.

### PDF

- PDF direkt in Project Write öffnen.
- Seitenwechsel.
- Zoom.
- Text markieren.
- Text kopieren.
- PDF mit Dokumenten verknüpfen, optional mit Seitenzahl.
- PDF im Mesh darstellen.

PDF.js-Konfiguration: aktuelle stabile Version (CVE-2024-4367 betrifft Versionen vor 4.2.67), `isEvalSupported: false`, PDF-eigene Skripte aus, Worker und Schriftdaten lokal. Links in PDFs öffnen erst nach Bestätigung.

### Office-Dateien

Unterstütze zunächst:

- DOCX.
- XLSX.
- PPTX.

Import heißt in Version 1.0: Die Datei wird als Anhang abgelegt, mit Metadaten verwaltet und auf Wunsch mit dem Standardprogramm des Systems geöffnet. Project Write liest, konvertiert und startet nichts aus ihr. Vorschaufunktionen folgen später. Dateien mit Makros (`.docm`, `.xlsm`, `.pptm`) werden nicht importiert.

### Spätere Medienerweiterungen

- Audio.
- Video.
- Weitere Dateiformate.

### Medien sicher anzeigen

- Das Asset-Protokoll bleibt aus. Ein eigener Medien-Handler in Rust (`pwmedia://<uuid>`) löst IDs zu Dateien in der Bibliothek auf. Das Frontend sieht keine Pfade.
- Der Handler setzt den geprüften Inhaltstyp und `X-Content-Type-Options: nosniff`.
- SVG erscheint nur als Bildquelle über den Handler, mit `Content-Security-Policy: sandbox`, nie eingebettet im DOM.
- Grenzwerte schützen vor Bildbomben, Startwert 100 Megapixel.

---

## 18. Datenbankarchitektur

Plane das vollständige SQLite-Datenbankschema in Phase 2 und halte es in `docs/database.md` fest. Tabellen entstehen per Migration in der Phase, die sie braucht.

Mindestens folgende Tabellen:

- documents
- media
- mesh_nodes
- subnets
- mesh_connections
- mesh_suggestions
- subnet_memberships
- document_media
- document_links
- tags
- document_tags
- media_tags
- mesh_layouts
- mesh_positions
- dictionaries
- dictionary_words
- document_dictionaries
- subnet_dictionaries
- language_profiles
- autocorrect_rejections
- document_history
- application_settings
- search_words und search_parts (FTS5, Abschnitt 19)

Anforderungen:

- Fremdschlüssel aktivieren, für jede Verbindung.
- Indizes sinnvoll setzen.
- Eindeutige Constraints verwenden.
- Migrationen implementieren: nur vorwärts, in einer Transaktion, mit Sicherung vorab und getestet ab jeder früheren Schemaversion. Ausgelieferte Migrationen werden nie geändert.
- Eine neuere Datenbank als die App erwartet wird mit klarer Meldung abgelehnt.
- Transaktionen für zusammengehörige Schreibvorgänge verwenden.
- Datenintegrität gewährleisten: `PRAGMA quick_check` beim Start, `integrity_check` nach Backup und Wiederherstellung.

Vorgaben:

- Tabellen als `STRICT`, IDs als UUID-Text, Zeiten in UTC nach ISO 8601.
- `deleted_at` und `archived_at` statt sofortigem Löschen.
- `journal_mode=WAL` auf lokalen Datenträgern, Rollback-Modus nach Abschnitt 9, `synchronous=FULL`, `busy_timeout` gesetzt.

Die Datenbankarchitektur muss spätere Erweiterungen ermöglichen.

### Geprüftes Muster für Zuordnungen

Die Mitgliedschaft gilt für Dokumente und Medien. Dieses Muster wurde in SQLite getestet: Es verhindert doppelte Zuordnungen, ein zweites primäres Subnetz sowie Zeilen mit zwei oder ohne Element.

```sql
CREATE TABLE subnet_memberships (
  subnet_id   TEXT NOT NULL REFERENCES subnets(id) ON DELETE CASCADE,
  document_id TEXT REFERENCES documents(id) ON DELETE CASCADE,
  media_id    TEXT REFERENCES media(id) ON DELETE CASCADE,
  is_primary  INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0, 1)),
  CHECK ((document_id IS NULL) <> (media_id IS NULL))
) STRICT;

CREATE UNIQUE INDEX uq_member_doc     ON subnet_memberships(subnet_id, document_id) WHERE document_id IS NOT NULL;
CREATE UNIQUE INDEX uq_member_media   ON subnet_memberships(subnet_id, media_id)    WHERE media_id IS NOT NULL;
CREATE UNIQUE INDEX uq_primary_doc    ON subnet_memberships(document_id) WHERE is_primary = 1 AND document_id IS NOT NULL;
CREATE UNIQUE INDEX uq_primary_media  ON subnet_memberships(media_id)    WHERE is_primary = 1 AND media_id IS NOT NULL;
```

Für `mesh_connections` gilt: keine Selbstverbindung, Endpunkte in sortierter Reihenfolge gespeichert, eindeutig je Paar und Typ. `mesh_suggestions` speichert Wert, Begründung und Status (offen, angenommen, abgelehnt).

---

## 19. Suche

Entwickle eine globale Suchfunktion.

Sie soll durchsuchen:

- Dokumenttitel.
- Dokumentinhalte.
- Tags.
- Mediennamen.
- Subnetze.
- Metadaten.

Implementiere die Volltextsuche mit SQLite FTS5. Phase 1 prüft, dass FTS5 in der mitgebauten SQLite-Version aktiv ist, über `pragma_compile_options` und eine echte Testtabelle.

### Index

```sql
CREATE VIRTUAL TABLE search_words USING fts5(
  title, body, tags, subnets, meta,
  tokenize = 'unicode61 remove_diacritics 2', prefix = '2 3'
);
CREATE VIRTUAL TABLE search_parts USING fts5(
  title, body,
  tokenize = 'trigram remove_diacritics 1'
);
```

- `search_words` findet Wörter und Wortanfänge. `Muller` findet `Müller`.
- `search_parts` findet Wortteile in deutschen Komposita. `haus` findet `Krankenhaus` und `Häuser`. Das gelingt erst ab 3 Zeichen.
- `porter` ist ein englischer Wortstamm-Filter und wird nicht verwendet. Eine deutsche Wortstammsuche kommt später.
- `search_parts` behält `detail=full`, sonst gehen längere Suchwörter verloren.

### Suchanfragen

- Rust baut jede Anfrage selbst. Jedes Wort wird in `"` gesetzt, ein `"` im Wort wird verdoppelt. Ohne das löst `C++` einen Syntaxfehler aus (Stichprobe).
- Jedes Wort wird um seine Schreibvarianten erweitert: `ß` und `ss`, `ä` und `ae`, `ö` und `oe`, `ü` und `ue`. Aus `Strasse` wird `"Strasse" OR "Straße"`. FTS5 faltet `ß` nicht, und `Mueller` findet `Müller` sonst nicht (Stichprobe). Der angezeigte Text bleibt dabei unverändert.
- Treffer aus `search_words` stehen vor Treffern aus `search_parts`. Die Sortierung nutzt `bm25()` mit den Startgewichten Titel 10, Tags 5, Subnetze 3, Metadaten 2, Inhalt 1.

### Oberfläche

- Filter: Typ, Tags, Subnetz, Datum, Favoriten, Archiv einbeziehen.
- Ergebnisnavigation: Trefferliste mit Textausschnitt und markiertem Suchwort, Pfeiltasten, Klick öffnet das Element an der Fundstelle.
- Archivierte und gelöschte Elemente erscheinen nur über den Filter.

### Aktualität

Der Index wird in derselben Transaktion wie die Daten aktualisiert. Die Einstellungen bieten „Suchindex neu aufbauen“.

Ergänze später optional eine lokale semantische Suche. Keine verpflichtende Cloud-KI.

---

## 20. Sicherheit und Datenschutz

Sicherheit hat hohe Priorität. Sie gilt ab Phase 1, nicht erst in der letzten Phase. Inhalte sind grundsätzlich nicht vertrauenswürdig: Markdown, eingefügtes HTML, `.pwrite`, PDF, SVG, Pakete und Backups.

### Tauri

- Nutze eine restriktive Capability-Konfiguration je Fenster.
- Vermeide unnötige Berechtigungen. Das Frontend erhält keine Rechte für `fs`, `shell` oder `sql`. Eigene Befehle werden ausdrücklich freigegeben (App-Manifest in `build.rs`).
- Setze `withGlobalTauri: false` und `freezePrototype: true`.
- Prüfe in Phase 1 das Isolation Pattern und halte die Entscheidung als ADR fest.
- Testplugins sind nur in Debug-Builds enthalten. CI bricht ab, wenn sie in einem Release-Build auftauchen.

### Webview

Die CSP greift in Tauri nur, wenn sie gesetzt ist. Starte mit dieser Richtlinie. Jede Lockerung braucht eine ADR. `unsafe-eval` und Inhalte aus dem Netz sind verboten.

```json
{
  "default-src": "'self'",
  "script-src": "'self'",
  "style-src": "'self' 'unsafe-inline'",
  "img-src": "'self' pwmedia: http://pwmedia.localhost",
  "font-src": "'self'",
  "connect-src": "ipc: http://ipc.localhost pwmedia: http://pwmedia.localhost",
  "worker-src": "'self'",
  "object-src": "'none'",
  "base-uri": "'none'",
  "form-action": "'none'",
  "frame-src": "'none'"
}
```

- Kein rohes HTML im DOM. Alles läuft durch das Editor-Schema.
- Stichprobe mit Tiptap 3.32.2: `javascript:`-Links werden entschärft. `<img src="https://…">` und `<img src="javascript:…">` kommen aber unverändert durch. Der Markdown-Adapter lässt deshalb nur `pwmedia`-Quellen zu, die CSP sichert zusätzlich ab.
- Links öffnen nur mit erlaubtem Schema (Abschnitt 12).

### Dateisystem und Eingaben

- Keine beliebigen Dateisystemzugriffe aus dem Frontend.
- Begrenze Dateisystemzugriffe auf die Bibliothek und auf vom Nutzer gewählte Dateien.
- Verhindere unkontrollierte Pfadzugriffe: Pfade werden aufgelöst und gegen den Bibliotheksordner geprüft, symbolische Links werden abgelehnt.
- Validiere alle Eingaben.
- Prüfe importierte Dateitypen über den Inhalt, nicht nur über die Endung. Es gilt eine Positivliste.
- Behandle SVG- und Office-Dateien als potenziell unsicher (Abschnitt 17).
- ZIP-Dateien (`.pwritepack`, `.pwbackup`): Pfade prüfen, entpackte Größe begrenzen, Prüfsummen vergleichen.
- Nutze sichere Fehlerbehandlung (Abschnitt 8).

### Datenschutz

- Es gilt die harte Regel aus Abschnitt 2. Ein Test prüft in Phase 14, dass der Normalbetrieb keine Netzverbindung aufbaut.
- Die Bibliothek ist in Version 1.0 nicht verschlüsselt. Die Dokumentation nennt das und empfiehlt die Datenträgerverschlüsselung des Systems.

### Lieferkette und Auslieferung

- `cargo audit`, `cargo deny` und `npm audit` laufen in CI. Lockfiles sind committet.
- Updates müssen signiert sein, das lässt sich in Tauri nicht abschalten. Der private Schlüssel liegt nie im Repository und braucht eine sichere Sicherung: Ohne ihn können installierte Apps keine Updates mehr erhalten.
- Die macOS-Notarisierung braucht ein Apple-Developer-Konto für 99 US-Dollar im Jahr und ein Apple-Gerät. Unter Windows warnt SmartScreen beim Download unsignierter Apps, und Smart App Control unter Windows 11 blockiert sie ganz. Stoppe vor Phase 14 für diese Entscheidung.

---

## 21. Export und Backup

### Export

Unterstütze:

- TXT-Export. Formatierungen entfallen, der Verlustschutz aus Abschnitt 10 warnt.
- Markdown-Export. Medien landen in einem Unterordner mit relativen Links, interne Links werden umgeschrieben.
- PDF-Export und Drucken über Druck-CSS und `window.print`, also den Systemdruckdialog. Ein Export ohne Dialog ist unter WebKitGTK ein offener Fehler und deshalb kein Standard. Teste in Phase 10 auf allen drei Betriebssystemen. Scheitert das, entscheide per ADR.
- Paketexport als `.pwritepack`.
- Gesamtexport der Bibliothek als Ordner mit Markdown und Medien, damit die Daten auch ohne die App nutzbar bleiben.

### Vollständige Bibliotheksbackups

Backups dürfen Datenbank und zugehörige Medien nicht inkonsistent hinterlassen. Das Backup läuft so:

1. Offene Änderungen speichern. Während des Backups schreibt Autosave nur nach `recovery/` und übernimmt danach. So bleibt Schreiben möglich.
2. Datenbank mit `VACUUM INTO` in eine temporäre Datei kopieren und mit `PRAGMA integrity_check` prüfen. Ein reines Kopieren der `.db`-Datei ist verboten. Bricht `VACUUM INTO` ab, kann die Kopie unvollständig sein.
3. Dokumente und Medien kopieren, SHA-256 berechnen und `manifest.json` schreiben, mit App-, Schema- und Formatversion.
4. Als `.pwbackup` (ZIP) unter temporärem Namen packen, durch erneutes Lesen prüfen und erst dann umbenennen.

Weitere Regeln:

- **Ausschlussliste:** `backups/`, `tmp/`, `recovery/`, `thumbnails/`, `exports/`, `library.lock` und die Dateien `-wal` und `-shm`.
- **Ziel:** frei wählbar. Liegt es auf demselben Datenträger wie die Bibliothek, warnt die App: Ein Datenträgerausfall träfe dann beide.
- **Automatik:** Standardmäßig ein Backup täglich beim Start, 7 bleiben erhalten. Vor jeder Datenbankmigration entsteht ein weiteres. Einstellbar.
- **„Backup prüfen“** testet ein vorhandenes Backup, ohne etwas zu verändern.

### Wiederherstellung aus Backups

- Prüfe Manifest, Prüfsummen, ZIP-Sicherheit und Schemaversion vor jedem Schreiben.
- Stelle in einen neuen Ordner wieder her, nie über die aktuelle Bibliothek.
- Schalte erst nach erfolgreicher Prüfung um. Die alte Bibliothek bleibt, bis der Nutzer sie entfernt.

---

## 22. Plugins und Erweiterbarkeit

Bereite eine modulare Plugin-Architektur vor. In Version 1.0 heißt das:

- **Interne Register** für Editorfunktionen, Dateiformate, Prüfwerke, Exporter und Mesh-Renderer. Die eigenen Module nutzen sie tatsächlich.
- **Ein Konzeptpapier** `docs/plugin-concept.md` beschreibt Berechtigungsmodell und Sandbox für später.
- **Kein Code von Dritten.** Implementiere noch kein ungesichertes Ausführen beliebigen Plugin-Codes.

Spätere Erweiterungen:

- Zusätzliche Editorfunktionen.
- Weitere Dateiformate.
- Neue Sprachmodule.
- KI-Integrationen.
- Eigene Mesh-Visualisierungen.
- Zusätzliche Exportformate.

Plugins müssen kontrollierte Berechtigungen besitzen.

---

## 23. Projektstruktur

Verwende ein einzelnes App-Repository mit dieser Zielstruktur. Ordner entstehen in der Phase, die sie braucht.

```text
project-write/
    src/
        app/
        components/
            layout/
            toolbar/
            tabs/
            navigation/
        modules/
            editor/
            documents/
            subnets/
            mesh/
            media/
            search/
            language-check/
            dictionaries/
            settings/
            themes/
            export/
            statistics/
            plugins/
        services/           Typisierte Befehlsaufrufe an Rust
        i18n/
        types/              Aus Rust erzeugte und eigene Typen
    src-tauri/
        src/
            commands/
            database/
            filesystem/
            language/
            services/
            security/
        migrations/         Einziger Ort für Migrationen
        capabilities/
    tests/
        fixtures/
        e2e/
    scripts/
    docs/
        adr/
```

Datenbankzugriff und Migrationen liegen nur in `src-tauri/`. Das Frontend hat keinen Datenbankordner.

Die Struktur darf bei technischen Gründen angepasst werden, muss aber modular und nachvollziehbar bleiben. Halte Abweichungen als ADR fest.

---

## 24. Arbeitsweise der Coding-KI

Arbeite in kleinen, überprüfbaren Schritten.

WICHTIG:

1. Beginne nicht damit, das gesamte Programm auf einmal zu generieren.
2. Analysiere zuerst die Anforderungen.
3. Erstelle anschließend einen konkreten Entwicklungsplan.
4. Prüfe die technische Kompatibilität der Abhängigkeiten.
5. Erstelle das Projektgerüst.
6. Implementiere jeweils nur ein zusammenhängendes Modul.
7. Teste jedes Modul vor dem nächsten Schritt.
8. Dokumentiere alle Änderungen.
9. Halte eine Liste noch offener Aufgaben.
10. Verändere keine funktionierenden Module ohne nachvollziehbaren Grund.

Erstelle vollständige Dateien und keinen unzusammenhängenden Beispielcode.

Für offene technische Entscheidungen und Rückfragen gilt Abschnitt 2.

### Projektgedächtnis

Dein Arbeitsstand muss eine neue Sitzung überleben.

- `docs/master-prompt.md`: dieser Auftrag.
- `docs/status.md`: aktueller Stand, offene Aufgaben, nächster Schritt. Lies die Datei zu Beginn jeder Sitzung und aktualisiere sie am Ende.
- `docs/architecture.md`, `docs/database.md`, `docs/pwrite-format.md`, `docs/dependencies.md` und `docs/adr/`.
- `CHANGELOG.md` und `THIRD-PARTY-LICENSES`.

Lies je Phase die dort genannten Abschnitte, nicht jedes Mal den ganzen Auftrag.

### Sprache

- Code, Bezeichner, Kommentare und Commit-Nachrichten: Englisch.
- Dokumentation in `docs/` und Berichte: Deutsch.
- Oberfläche: Deutsch und Englisch.

### Versionsverwaltung

Arbeite in kleinen Commits und mit einem Zweig je Phase. Geheimnisse wie Signierschlüssel gehören nie ins Repository.

### Bericht nach jeder Phase

Nach jeder Entwicklungsphase liefere:

- Was wurde implementiert?
- Welche Dateien wurden erstellt oder verändert?
- Welche Tests wurden durchgeführt, auf welchem Betriebssystem und mit welchem Ergebnis?
- Welche Abnahmekriterien sind erfüllt, welche nicht?
- Was konnte nicht getestet werden und warum?
- Welche Fehler sind noch offen?
- Welche Entscheidungen wurden getroffen (ADR)?
- Was ist der nächste konkrete Entwicklungsschritt?

---

## 25. Entwicklungsphasen und Abnahmekriterien

Entwickle Project Write in dieser Reihenfolge. Sie folgt den Abhängigkeiten: Erst wenn gespeichert werden kann, entstehen Dokumente. Erst wenn es Subnetze, Medien und eine Suche gibt, entsteht das Mesh.

Für jede Phase gilt:

- Lint, Typprüfung und alle Tests laufen in CI auf Windows, macOS und Linux grün.
- Jeder neue Befehl hat Tests für gültige und ungültige Eingaben.
- Keine sichtbare Funktion ist ohne Wirkung.
- Oberflächentexte liegen auf Deutsch und Englisch vor.
- Dokumentation und `docs/status.md` sind aktuell.
- Danach folgen Bericht, Stopp und Warten auf die Freigabe.

### Phase 1 – Grundgerüst

Abschnitte: 2, 7, 8, 20, 23.

Umfang:

- Tauri einrichten.
- Rust einrichten.
- React und TypeScript einrichten.
- Vite konfigurieren, Tailwind CSS und Zustand einbinden.
- SQLite über SQLx anbinden, erste Migration.
- Sichere Kommunikationsschnittstelle mit erzeugten Typen.
- CSP und Capabilities.
- Testwerkzeuge und CI.
- Startbildschirm im Project-Write-Design.
- Projekt starten und testen.

Abnahme:

- Die App startet auf allen drei Betriebssystemen, ein WebdriverIO-Rauchtest ist grün.
- Ein Testbefehl liefert bei gültiger Eingabe ein typisiertes Ergebnis und bei ungültiger das einheitliche Fehlerformat.
- Die Migration läuft auf einer leeren Datenbank, ein zweiter Lauf ändert nichts, Fremdschlüssel sind aktiv.
- FTS5 ist aktiv: `pragma_compile_options` nennt `ENABLE_FTS5`, beide Tabellen aus Abschnitt 19 lassen sich anlegen, `"C++"` funktioniert.
- CSP-Test: Ein Inline-Skript läuft nicht, ein Bild mit Webadresse lädt nicht.
- Ein Zugriff des Frontends auf Dateisystem-Befehle wird abgelehnt.
- Der Startbildschirm erfüllt in Dark und Light die Kontrastwerte aus Abschnitt 6.
- Die ADR zum Isolation Pattern liegt vor.

### Phase 2 – Speicherkern

Abschnitte: 9, 18.

Umfang:

- Bibliothek anlegen, öffnen, sperren und umziehen.
- Atomares Speichern.
- Datenbankschema planen und dokumentieren, Migrationen mit Tests.
- Bibliotheks- und App-Einstellungen.
- Dokument-Repository im Backend.
- Skript für die Referenzbibliothek.

Abnahme:

- 100 harte Prozessabbrüche während des Speicherns: keine beschädigte Datei, `integrity_check` meldet `ok`.
- Eine zweite Instanz kann dieselbe Bibliothek nicht öffnen.
- Sync-Ordner lösen die Warnung aus. Wird trotzdem dort gespeichert, läuft die Datenbank im Rollback-Modus.
- Pfadangriffe wie `..` und symbolische Links werden abgelehnt.
- Die Referenzbibliothek entsteht reproduzierbar.

### Phase 3 – Oberfläche

Abschnitt: 11.

Umfang:

- Hauptlayout.
- Menüleiste.
- Toolbar.
- Activity Bar.
- Explorer.
- Tabs.
- Statusleiste.
- Dark und Light Theme, anpassbare Themes.
- Command Palette.
- Konfigurierbare Tastenkürzel.
- Kontextmenüs.
- Drag & Drop.
- Anpassbare Seitenleisten.
- Erster Start, Sprachdateien, Fokusmodus.

Abnahme:

- Drag & Drop ist auf allen drei Betriebssystemen getestet.
- Alle Funktionen sind per Tastatur erreichbar, der Fokus ist sichtbar.
- Die Command Palette zeigt nur funktionierende Befehle.
- Kürzelkonflikte werden gemeldet.

### Phase 4 – Editor

Abschnitte: 10, 12.

Umfang:

- Tiptap integrieren.
- Rich Text, zunächst ohne Bilder und PDF-Verweise.
- Markdown mit Adapter-Schicht, Reintext und Quelltextansicht.
- Laden und Speichern über den Speicherkern.
- Dokument-Tabs.
- Schreibstatistik: Wort- und Zeichenzähler, Zeichen ohne Leerzeichen, Absatzanzahl, Lesezeit.
- Suchen und Ersetzen im Dokument, Einfügen.

Abnahme:

- Mindestens 50 Markdown-Fälle bestehen den Round-Trip-Test.
- Alle Verlustfälle aus Abschnitt 10 sind gelöst oder lösen die Warnung aus.
- Die Tippverzögerung erfüllt den Zielwert bei 50.000 Wörtern.
- Die Statistik stimmt auf deutschen Testtexten.

### Phase 5 – Dokumente und externe Dateien

Abschnitte: 9, 10, 12, 13.

Umfang:

- Erstellen.
- Öffnen.
- Bearbeiten.
- Umbenennen.
- Kopieren.
- Favoriten, Zuletzt verwendet.
- `.pwrite`, TXT und MD als Dateiformate.
- Autosave.
- Externe Dateien mit Kodierung, Zeilenenden und Änderungen von außen.
- Dateizuordnung auf allen drei Wegen.
- Interne Links und Rückverweise.
- Umwandeln mit Verlustschutz.

Abnahme:

- UTF-8, UTF-8 mit BOM, UTF-16 und Windows-1252 sowie LF und CRLF bleiben beim Bearbeiten erhalten. Eine unveränderte Datei bleibt bytegleich.
- Änderungen von außen lösen den Dialog aus.
- Doppelklick, „Öffnen mit“ und zweiter Start öffnen die Datei auf jedem Betriebssystem.
- `.pwrite` besteht den Round-Trip. Unbekannte Felder bleiben erhalten, neuere Formatversionen öffnen schreibgeschützt.

### Phase 6 – Wiederherstellung

Abschnitte: 4, 9, 13.

Umfang:

- Wiederherstellung Stufe 1 bis 4.
- Löschen und Papierkorb.
- Archivieren.
- Dokumenthistorie mit Aufbewahrung und Versionsvergleich.

Abnahme:

- Ein harter Abbruch beim Tippen kostet höchstens 2 s Text. Der nächste Start bietet die Wiederherstellung an.
- Ein gelöschtes Element kehrt mit Zuordnungen und Verbindungen zurück.
- Die Aufbewahrung arbeitet richtig, getestet mit simulierter Zeit.
- Jede Wiederherstellung sichert zuerst den aktuellen Stand.

### Phase 7 – Subnetze und Tags

Abschnitte: 13, 16, 18.

Umfang:

- Hierarchische Struktur.
- Mehrfachzuordnung für Dokumente und Medien, primäres Subnetz.
- Navigation im Explorer-Baum und über Breadcrumbs.
- Farbverwaltung.
- Tags, Verschieben und Filter.

Abnahme:

- Zyklen werden abgelehnt.
- Ein Element in zwei Subnetzen bleibt ein Datensatz und eine Datei.
- Das Löschen eines Subnetzes löscht kein Element.
- Der Explorer bleibt mit der Referenzbibliothek flüssig.

### Phase 8 – Medien

Abschnitte: 9, 17, 20.

Umfang:

- Bilderimport und Import ganzer Ordner mit SHA-256-Duplikaterkennung.
- Medienverwaltung, Galerie und Vorschaubilder.
- PDF-Anzeige.
- Office-Import.
- Medien-Handler.
- Medienverknüpfungen, Bilder und PDF-Verweise im Editor.

Abnahme:

- Originaldateien bleiben bytegleich, belegt durch Prüfsummen vor und nach dem Import.
- Ein abgebrochener Import hinterlässt weder Dateien noch Datenbankeinträge.
- Ein SVG mit Skript führt nichts aus. Eine Test-PDF nach dem Muster von CVE-2024-4367 führt nichts aus.
- Falsche Dateiendungen, Bildbomben und Office-Dateien mit Makros werden abgelehnt.
- Das Frontend erhält keine Dateipfade der Bibliothek.

### Phase 9 – Suche

Abschnitt: 19.

Umfang:

- Globale Suche.
- SQLite FTS5 mit Wort- und Teilwortindex.
- Schreibvarianten.
- Filter.
- Ergebnisnavigation.

Abnahme:

- Erste Treffer erscheinen in der Referenzbibliothek innerhalb des Zielwerts.
- Testfälle: `Muller` und `Mueller` finden `Müller`, `Strasse` findet `Straße`, `haus` findet `Krankenhaus` und `Häuser`, `C++` und ein einzelnes `"` lösen keinen Fehler aus.
- Archivierte Elemente erscheinen nur über den Filter.
- Der Index stimmt nach Bearbeiten, Löschen und Wiederherstellen.

### Phase 10 – Export und Backup

Abschnitt: 21.

Umfang:

- Export als TXT, Markdown und PDF, Drucken, Paketexport und Gesamtexport.
- Backup, automatische Backups und „Backup prüfen“.
- Wiederherstellung Stufe 5.

Abnahme:

- PDF-Export und Drucken sind auf allen drei Betriebssystemen getestet.
- Ein Backup während laufender Eingaben ist konsistent.
- Ein Abbruch während des Backups hinterlässt keine fertig aussehende Datei.
- Nach der Wiederherstellung stimmen alle Prüfsummen.
- Ein verändertes oder beschädigtes Backup wird abgelehnt.
- Das Backup enthält keine früheren Backups.

### Phase 11 – Mesh

Abschnitte: 6, 15.

Umfang:

- Zuerst der Lasttest mit der Referenzbibliothek.
- Knoten.
- Verbindungen.
- Farbige Cluster.
- Ansichten.
- Layoutspeicherung und automatisches Layout.
- Bedienung nach Abschnitt 15.
- Automatische Gruppierung als Vorschläge.

Abnahme:

- Das Mesh erreicht den Zielwert aus Abschnitt 6, sonst liegt eine ADR zum Renderer vor.
- Nach einem Vorschlagslauf sind alle bestätigten Beziehungen unverändert, belegt durch einen Datenvergleich.
- Vorschläge stehen nur in `mesh_suggestions`. Abgelehnte kommen nicht wieder.
- Abgeleitete Verbindungen stehen nicht in `mesh_connections`.
- Das Mesh ist auch per Tastatur bedienbar.

### Phase 12 – Sprachprüfung

Abschnitte: 7, 14.

Umfang:

- Zuerst der Vergleich von `spellbook` und Hunspell-Bindings mit einem deutschen Korpus aus mindestens 1.000 richtigen Wörtern, darunter Komposita, und 200 Fehlern. Das Ergebnis wird eine ADR.
- Hunspell-Wörterbücher und Rechtschreibung, neu sortierte Vorschläge.
- Automatische Korrektur.
- Kontextprüfung über LanguageTool als Zusatzpaket.
- Eigene Wörterbücher, Zuweisung, Import und Export.
- Sprachprofile, Spracherkennung und mehrsprachige Dokumente.

Abnahme:

- Höchstens 3 % der richtigen Wörter werden markiert, mindestens 95 % der Fehler werden erkannt.
- Die Autokorrektur ändert in den Testdaten kein richtiges Wort und korrigiert keinen Fehler falsch. Jede der sieben Bedingungen hat einen eigenen Test.
- Die Satzende-Testdaten sind grün.
- Jede Korrektur lässt sich mit einem Undo-Schritt zurücknehmen.
- Die Tippverzögerung bleibt im Zielwert.
- Ohne Zusatzpaket funktioniert die App vollständig. LanguageTool bindet nur an `127.0.0.1`.

### Phase 13 – Erweiterungen

Abschnitt: 22.

Umfang:

- Pluginschnittstellen als interne Register.
- Konzeptpapier.
- Erweiterte Einstellungen.

Abnahme:

- Die eigenen Exporter und Dateiformate nutzen die Register.
- Fremder Code lässt sich nicht laden, belegt durch einen Test.
- Einstellungen lassen sich exportieren, importieren und zurücksetzen.

### Phase 14 – Qualitätssicherung und Auslieferung

Abschnitte: 6, 20.

Hole vorab die Entscheidungen D4 und D11 ein.

Umfang:

- Unit-Tests.
- Integrationstests.
- UI-Tests.
- Performance-Tests.
- Sicherheitsprüfung.
- Prüfung der Barrierefreiheit.
- Installer für alle Plattformen: Windows, macOS und Linux.
- Signierung und signierte Updates nach Entscheidung.

Abnahme:

- Alle Zielwerte aus Abschnitt 6 sind gemessen und in einem Bericht dokumentiert.
- Der Netzwerktest zeigt im Normalbetrieb keine Verbindung.
- Installation und Deinstallation sind auf einem sauberen System je Betriebssystem getestet. Die Deinstallation lässt die Bibliothek unangetastet.
- Der Release-Build enthält keine Testplugins.
- `cargo audit`, `cargo deny` und `npm audit` sind ausgewertet, die Lizenzliste ist vollständig.

---

## 26. Startauftrag

Beginne jetzt mit Phase 1.

Deine erste Aufgabe:

1. Prüfe die gesamte technische Architektur und halte sie in `docs/architecture.md` fest.
2. Erstelle die Projektstruktur für Phase 1. Die Zielstruktur steht in Abschnitt 23.
3. Richte Tauri, Rust, React, TypeScript und Vite ein.
4. Erstelle die erste lauffähige Desktop-Anwendung.
5. Richte die SQLite-Verbindung ein.
6. Implementiere eine sichere Kommunikationsschnittstelle zwischen Frontend und Rust nach Abschnitt 8.
7. Erstelle einen einfachen Startbildschirm im Project-Write-Design nach Abschnitt 11.
8. Starte die Anwendung und führe die ersten Tests durch.

Stoppe nach Phase 1 und dokumentiere das Ergebnis.

Warte anschließend auf die Freigabe für Phase 2.

**Projektname: Project Write**

**Ziel: Eine professionelle, lokale und plattformübergreifende Schreib- und Wissensmanagement-Anwendung.**
