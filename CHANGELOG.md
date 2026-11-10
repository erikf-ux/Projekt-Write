# Changelog

Alle wesentlichen Änderungen an Project Write. Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionen nach [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Phase 7 – Subnetze und Tags

#### Hinzugefügt

- Subnetze als Baum mit Name, Beschreibung und Farbe (fünf Cluster-Farben des Designs oder eine eigene Farbe mit mindestens 3:1 Kontrast in beiden Designs). Zyklen lehnen Rust und ein Datenbank-Trigger ab; Geschwister heißen nie gleich.
- Mehrfachzuordnung: Ein Dokument kann in mehreren Subnetzen liegen und bleibt ein Datensatz und eine Datei; genau ein Subnetz ist primär. „Verschieben nach …“ wechselt das primäre Subnetz, „Zuordnen zu …“ fügt eines hinzu, „Aus „X“ entfernen“ und „Als primär festlegen“ im Kontextmenü; dazu Drag & Drop im Explorer (mit `Strg` zuordnen).
- Ein Subnetz löschen löscht kein Dokument: Untersubnetze rücken eine Ebene hoch oder werden mitgelöscht; Dokumente ohne Subnetz stehen unter „Nicht zugeordnet“.
- Explorer: Favoriten, Zuletzt verwendet, die Subnetze als Baum, Nicht zugeordnet, Archiv, Papierkorb (statt „Alle Dokumente“); weitere Zuordnungen sind markiert; der Weg zum aktiven Dokument klappt auf.
- Breadcrumbs über der Schreibfläche mit dem Weg des primären Subnetzes, weiteren Subnetzen und den Tags; ein Klick zeigt das Subnetz im Explorer.
- Tags: vergeben (ein neuer Name legt ein Tag an), entfernen, umbenennen, zusammenführen, löschen; Groß- und Kleinschreibung zählen nicht. `.pwrite` trägt die Tags in der Hülle; ein importiertes `.pwrite` bringt sie mit.
- Filter im Explorer nach Tags und Titel.
- Struktur-Undo: Jede Strukturaktion (auch Löschen und Archivieren) lässt sich mit `Strg + Z` im Explorer, in der Befehlspalette oder über „Rückgängig“ im Hinweis zurücknehmen und wiederholen.
- Kopien behalten Subnetze und Tags.
- Beispiel-Subnetze „Projekte“, „Wissen“, „Ideen“ als Wahl beim ersten Start.
- Referenzbibliothek mit 500 Subnetzen, 1.000 Tags und Zuordnungen; E2E-Messung des Explorers damit.
- Migration 0007 (`subnets`, `subnet_memberships`, `tags`, `document_tags`).

#### Geändert

- „Alle Dokumente“ im Explorer entfällt; der Subnetz-Baum und „Nicht zugeordnet“ zeigen zusammen jedes Dokument.

#### Sicherheit

- Gebündeltes SQLite 3.54.1 statt 3.51.3 (RUSTSEC-2026-0459, CVE-2026-119375: alter Speicher bei einer präparierten `-wal`-Datei). Bis SQLx die neue Version zulässt, über eine gepatchte Kopie von `sqlx-sqlite` (ADR 0018).

### Phase 6 – Wiederherstellung

#### Hinzugefügt

- Stufe 1, Sitzung: Offene Tabs, aktiver Tab, Ansicht und Auswahl jedes Tabs werden 2 s nach einer Änderung und beim Beenden gesichert und beim nächsten Start mit derselben Bibliothek wieder geöffnet; gelöschte Dokumente und verschwundene Dateien fallen weg.
- Stufe 2, Absturz: Höchstens 1 s nach einer ungespeicherten Eingabe liegt der Text als Entwurf in `recovery/` der Bibliothek, auch während weiter getippt wird und auch für externe Dateien. Nach einem Absturz bietet der nächste Start „Wiederherstellen“, „Verwerfen“ oder „Später“ an; der jetzige Stand eines Dokuments wird vorher als Version gesichert.
- Stufe 3, Papierkorb: „Löschen“ (`Entf` im Explorer, Kontextmenü, Befehl) verschiebt ein Dokument in den Papierkorb; Datei, Versionen und Verbindungen bleiben, „Wiederherstellen“ bringt alles zurück. „Endgültig löschen“ und „Papierkorb leeren“ fragen nach; nach 30 Tagen (einstellbar) entfernt die App Dokumente im Papierkorb beim Öffnen der Bibliothek.
- Archiv: Archivierte Dokumente stehen nur in der Gruppe „Archiv“, nicht in Favoriten, Zuletzt verwendet, Alle Dokumente und der Linkauswahl.
- Stufe 4, Dokumenthistorie: Versionen beim Bearbeiten (alle 10 Minuten), beim Schließen, vor dem Umwandeln, vor jeder Wiederherstellung und benannt („Version speichern …“), komprimiert und ohne Doppel. Aufbewahrung: alles der letzten 24 Stunden, danach eine Version je Tag bis 30 Tage und je Woche bis 365 Tage; benannte Versionen bleiben. Werte in den Bibliothekseinstellungen.
- Dialog „Versionen“: zwei Stände nebeneinander mit markierten Zeilen und Zählung; „Diese Version wiederherstellen“ sichert den aktuellen Stand zuerst.
- Migration 0006 (`document_history`).
- E2E-Tests: harter Abbruch beim Tippen (höchstens 2 s Verlust, Angebot beim nächsten Start), Papierkorb mit Verbindungen, Historie, Sitzung über einen Neustart.

### Phase 5 – Dokumente und externe Dateien

#### Hinzugefügt

- Rich-Text-Dokumente im eigenen Format `.pwrite` (Version 1, `docs/pwrite-format.md`): unbekannte Felder bleiben beim Speichern erhalten, Dateien einer neueren Formatversion öffnen schreibgeschützt mit Hinweis.
- Explorer als Baum mit „Favoriten“, „Zuletzt verwendet“ (auch externe Dateien) und „Alle Dokumente“; Dokumente kopieren („… (Kopie)“), als Favorit markieren.
- Externe Dateien öffnen (`Strg + O`), bearbeiten und speichern (`Strg + S`, „Speichern unter …“ mit `Strg + Umschalt + S`): Kodierung (UTF-8 mit und ohne BOM, UTF-16 LE/BE, Windows-1252) und Zeilenenden (LF, CRLF) bleiben erhalten und lassen sich in der Statusleiste umstellen; eine unveränderte Datei wird nie geschrieben. Änderungspunkt im Tab, Rückfrage beim Schließen und Beenden („Speichern, Verwerfen, Abbrechen“), Rückfrage ab 10 MB, Binärdateien werden abgelehnt, schreibgeschützte oder verschwundene Dateien bieten „Speichern unter“. Autosave für externe Dateien ist einstellbar (Standard aus).
- Änderungen von außen: Die App vergleicht offene Dateien alle 2 s und beim Fokus mit der Platte und fragt „Neu laden“, „Meine Fassung behalten“ oder „Als Kopie speichern“.
- „In Bibliothek aufnehmen“ legt eine Kopie einer externen Datei in der Bibliothek an; der Tab zeigt danach das Bibliotheksdokument.
- Umwandeln zwischen TXT, Markdown und `.pwrite` mit Warnung vor Verlusten; die Verlustwarnung beim Speichern von Markdown bietet „Als `.pwrite` speichern“.
- Dateizuordnung für `.pwrite`; eine Datei als Startargument oder bei einem zweiten Start öffnet in der laufenden App, eine Datei aus dem Dokumentordner der Bibliothek als Bibliotheksdokument.
- Interne Links: `[[` öffnet eine Dokumentauswahl, der Link zeigt auf `pwrite://<id>` und überlebt das Umbenennen; `Strg + Klick` öffnet das Dokument. Die Seitenleiste „Verbindungen“ zeigt die Rückverweise.
- Migrationen 0004 (Favoriten, zuletzt geöffnet) und 0005 (`document_links`).

#### Geändert

- Der Import per Drag & Drop erkennt dieselben Kodierungen wie externe Dateien und nimmt `.pwrite` an.

### Phase 4 – Editor

#### Hinzugefügt

- Schreibfläche mit Tiptap 3: Rich Text für Markdown (Überschriften, Fett, Kursiv, Unterstrichen, Durchgestrichen, Inline-Code, Links, Listen, Nummerierungen, Aufgabenlisten, Zitate, Codeblöcke, Trennlinien, Tabellen) mit Markdown-Kürzeln beim Tippen; Reintext für `.txt`; Quelltextansicht für Markdown (`Strg + /`). Bilder erscheinen als Hinweis mit Adresse und werden nie geladen.
- Markdown-Adapter mit lesbarem Escaping, erlaubter HTML-Teilmenge, YAML-Kopf, Fußnoten und Rohblöcken; 69 Round-Trip-Fälle in `tests/fixtures/markdown/`, 15 abschließend dokumentierte Normalisierungen (ADR 0014).
- Verlustschutz: Was der Editor nicht verlustfrei halten kann, öffnet in der Quelltextansicht mit Begründung; vor dem Speichern prüft der Editor geänderte Blöcke und warnt mit „Trotzdem speichern“ und „Abbrechen“.
- Schreibstatistik in der Statusleiste und als Dialog: Wörter nach Unicode-Regeln, Zeichen, Zeichen ohne Leerzeichen, Absätze, Lesezeit (Wörter pro Minute einstellbar); für die Markierung. Gezählt wird nach dem ersten Bild, damit lange Dokumente schneller erscheinen.
- Suchen und Ersetzen im Dokument (`Strg + F`, `Strg + H`, `F3`, `Umschalt + F3`): Groß-/Kleinschreibung, ganze Wörter, reguläre Ausdrücke (RE2), Treffer einzeln oder alle ersetzen.
- Einfügen: HTML durch das Editor-Schema, Markdown-Text als Markdown, `Strg + Umschalt + V` als reiner Text.
- Menüs Bearbeiten und Format, Formatierungsbefehle mit Kürzeln (`Strg + B/I/U/E/K` …), Link-Dialog (nur `http`, `https`, `mailto` und relative Adressen).
- Weblinks öffnen mit `Strg + Klick` im Systembrowser: Rust prüft die Adresse (`link_open`) und übergibt sie dem System als ein Argument, nie durch eine Shell. Beim Tippen werden nur Adressen mit `http://`, `https://`, `mailto:` oder `www.` und E-Mail-Adressen von selbst zu Links; ein fehlendes Leerzeichen nach einem Punkt („im.Am“) bleibt Text.
- E2E-Tests des Editors (`tests/e2e/specs/editor.e2e.ts`): getippte Webadressen werden Links und als Markdown gespeichert, `Strg + Klick` erreicht `link_open` (ein einfacher Klick nicht), `Strg + B` formatiert und speichert.
- Leistungsmessung im echten Programm (`tests/e2e/specs/performance.e2e.ts`): Ein Manuskript mit 50.000 Wörtern öffnet unter Linux in höchstens 469 ms, 95 % der Anschläge erscheinen nach höchstens 38 ms (Rich Text) bzw. 37 ms (Quelltext).

#### Geändert

- Die Schreibfläche der Phase 3 (Textfeld) ist durch Tiptap ersetzt; Rückgängig und Wiederholen sind Befehle des Editors.
- Escape beendet den Fokusmodus auch, wenn der Fokus in der Schreibfläche liegt.

#### Behoben

- Unter Linux (WebKitGTK) dauerte das Öffnen langer Markdown-Dokumente quadratisch lange (50.000 Wörter: 16 s): JavaScriptCore kompiliert einige Ausdrücke von marked nicht, und drei Tokenizer durchsuchten den ganzen Resttext. Der Adapter liest jetzt in linearer Zeit, mit denselben Tokens.
- Die Wortzählung war unter Linux niedriger als unter Windows (Zahlen zählten nicht als Wörter); sie ist jetzt auf allen Systemen gleich.

### Phase 3 – Oberfläche

#### Hinzugefügt

- Hauptlayout nach Abschnitt 11: natives Menü (Datei, Ansicht), Toolbar, Activity Bar (Explorer, Schreibfläche), Seitenleiste mit Explorer (Breite ändern, ein- und ausblenden), Dokument-Tabs, Statusleiste.
- Befehlsregister: Menü, Toolbar, Befehlspalette (`Strg + Umschalt + P`, nur funktionierende Befehle), Tastenkürzel und Kontextmenüs führen dieselben Befehle aus.
- Konfigurierbare Tastenkürzel mit Konflikterkennung: belegte Kürzel werden vor dem Zuweisen gemeldet, Konflikte in `settings.json` in Statusleiste und Einstellungen angezeigt; ein Kürzel mit Konflikt führt nichts aus.
- Explorer mit virtualisierter Liste (Tastatur, Umbenennen mit `F2`, Kontextmenü); Tabs mit Tastatur, Kontextmenü, Verschieben per Tastatur und Drag & Drop.
- Schreibfläche der Phase 3 für Text und Markdown-Quelltext mit Autosave nach Abschnitt 9 und Zeilenbreite höchstens 72 Zeichen (abschaltbar).
- Drag & Drop: abgelegte Text- und Markdown-Dateien werden importiert (Positivliste, Inhaltsprüfung, UTF-8, Duplikat-Rückfrage, Originalname gespeichert).
- Designs: hell, dunkel, wie das System; importierbare Design-Dateien mit Kontrastprüfung in Rust.
- Erster Start: Bibliotheksort und Sprache, danach ein leeres Dokument. Sprache, Design, Zoom, Seitenleiste, Zeilenbreite, Tastenkürzel und Fensterzustand in `settings.json`.
- Fokusmodus (`F11`: Vollbild, nur die Schreibfläche), Oberflächenzoom (50–200 %).
- Beenden speichert zuerst alles; scheitert das, entscheidet der Nutzer.
- Migration 0003 (`documents.original_name`); Befehle für Oberfläche, Dokumente und Designs mit Tests für gültige und ungültige Eingaben.

#### Geändert

- Die Sprache bestimmt jetzt Rust (Einstellung oder Systemsprache), für Menü und Webview gleich.
- Die Bibliotheksansicht liegt nach dem ersten Start in den Einstellungen.
- E2E: Der Rauchtest von Phase 1 läuft zuerst (Startbildschirm); Pixel-Hilfen in `tests/e2e/pixels.ts`.

### Phase 2 – Speicherkern

#### Hinzugefügt

- Bibliothek nach Abschnitt 9: anlegen (Standard `Dokumente/ProjectWrite`, nur in leeren Ordnern, sonst im Unterordner `ProjectWrite`), öffnen, schließen, umziehen (kopieren, prüfen, umschalten; der alte Ordner bleibt und ist als umgezogen markiert).
- `library.json` mit ID und Formatversion; neuere Formate werden abgelehnt, unbekannte Felder bleiben erhalten.
- `library.lock` mit Betriebssystem-Dateisperre: eine zweite Instanz kann dieselbe Bibliothek nicht öffnen; nach einem Absturz bleibt keine veraltete Sperre. `tauri-plugin-single-instance`: ein zweiter Programmstart übergibt an die laufende Instanz.
- Erkennung von Sync-Ordnern (Cloud Files API unter Windows, Anbieter-Konfiguration, Ordnernamen) und Netzlaufwerken; Warnung mit Bestätigung, dort Rollback-Journal statt WAL.
- Atomares Speichern (Temporärdatei, `fsync`, Umbenennen; unter Windows `MoveFileExW` mit Write-Through).
- Pfadprüfung: kein `..`, keine absoluten Pfade, keine reservierten Namen, keine symbolischen Links oder Junctions innerhalb der Bibliothek.
- Beim Öffnen: `tmp/` leeren, `quick_check`, neuere Schemata ablehnen, Sicherung (`VACUUM INTO` + `integrity_check`) vor jeder Migration, Dateien ohne Datenbankeintrag melden statt löschen.
- Migration 0002: Tabelle `documents`. Vollständiges geplantes Schema in `docs/database.md`.
- Dokument-Repository: anlegen, lesen, speichern (unveränderter Inhalt wird nicht neu geschrieben), umbenennen, in den Papierkorb legen.
- App-Einstellungen in `settings.json` im Konfigurationsordner des Systems (Bibliothekspfad); Bibliothekseinstellungen typisiert in `application_settings`.
- Lokaler, rotierender Log ohne Dokumentinhalte.
- Neun Bibliotheksbefehle; Ordnerwahl im nativen Dialog aus Rust, das Frontend erhält nur Tokens und Anzeigetexte.
- Bibliotheksansicht auf dem Startbildschirm (Deutsch und Englisch): anlegen, öffnen, umziehen mit Fortschritt und Abbruch, Warnungen, Hinweise aus dem letzten Öffnen.
- Skript für die reproduzierbare Referenzbibliothek (`scripts/reference-library.mjs`, 10.000 Dokumente in Phase 2).
- Tests: 100 harte Prozessabbrüche während des Speicherns, zweiter Prozess, Pfadangriffe mit echten Links, Migrationen ab jeder früheren Version, E2E für Anlegen, Umzug, Neustart und zweite Instanz.

#### Geändert

- Befehlsanfragen müssen JSON-Objekte sein (vorher nahm serde für Strukturen auch Arrays an).
- Rust ≥ 1.89; CI ohne macOS (Entscheidung des Auftraggebers), Linux-E2E in einer D-Bus-Sitzung.
- Debug-Builds: Abhängigkeiten ohne Debug-Informationen, SHA-256 und SQLite optimiert.

### Phase 1 – Grundgerüst

#### Hinzugefügt

- Tauri-2-Anwendung mit React 19, TypeScript 6 (strict), Vite 8, Tailwind CSS 4 und Zustand.
- Startbildschirm im Project-Write-Design mit Systemprüfung (Datenspeicher, Datenbankschema, Datenverknüpfungen, Volltextsuche), Dark und Light passend zum System, Deutsch und Englisch.
- Design-Tokens aus Abschnitt 11 als einzige Farbquelle, Kontrastprüfung aller verwendeten Paare. Themes werden über das Attribut `data-theme` und daraus erzeugte CSS-Regeln umgeschaltet.
- Sichere Kommunikationsschnittstelle: `Input<T>` mit Pflicht-Validierung, einheitliches Fehlerformat `{ code, messageKey, details? }`, aus Rust erzeugte TypeScript-Typen (`ts-rs`), App-Manifest und eine Capability mit genau den eigenen Befehlen.
- Befehl `system_check`.
- SQLite-Anbindung über SQLx 0.9 (mitgebaut): Schreibverbindung und Lese-Pool, `foreign_keys`, `synchronous=FULL`, `busy_timeout`, WAL oder Rollback-Journal, Migrationen mit Ablehnung neuerer Schemata.
- Migration 0001: Tabelle `application_settings`.
- FTS5-Probe mit den Tabellendefinitionen aus Abschnitt 19 und Quoting von Suchbegriffen.
- Strikte CSP nach Abschnitt 20, `withGlobalTauri: false`, `freezePrototype: true`, Asset-Protokoll aus, `dragDropEnabled: false`.
- Tests: Rust-Unit- und Integrationstests, Vitest, WebdriverIO-Rauchtest mit eingebettetem WebDriver (nur Debug-Build, Feature `e2e`).
- CI auf Windows, macOS und Linux; Prüfung des Release-Builds auf Testplugins; `cargo audit`, `cargo deny`, `npm audit`.
- Dokumentation: Architektur, Datenbank, Abhängigkeiten, ADRs 0001–0005, Lizenzliste.
- Windows: Common-Controls-v6-Manifest wird auch in Test-Executables gelinkt.
- App-Icon (Schreibmaschine) nach Vorlage des Auftraggebers, größte Größe 256 × 256 px; Windows-Logo aus der Vorlage entfernt. Neu erzeugen mit `node scripts/app-icons.mjs`.
- Bundle-Kennung `app.projectwrite.desktop` bestätigt.

#### Behoben

- E2E-Farbmessungen unter macOS und Windows lieferten veraltete Werte, weil die CI-Fenster nicht von selbst rendern. Die Tests messen Farben jetzt an Screenshot-Pixeln (F-1, F-2 in `docs/status.md`).
- `cargo test --manifest-path src-tauri/Cargo.toml` schrieb die TypeScript-Typen nach `src-tauri/bindings/` statt nach `src/types/generated/`; die CI-Prüfung auf aktuelle Typen war dadurch wirkungslos (F-3 in `docs/status.md`).
