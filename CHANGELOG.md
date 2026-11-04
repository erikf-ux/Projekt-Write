# Changelog

Alle wesentlichen Änderungen an Project Write. Format nach [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), Versionen nach [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Phase 4 – Editor

#### Hinzugefügt

- Schreibfläche mit Tiptap 3: Rich Text für Markdown (Überschriften, Fett, Kursiv, Unterstrichen, Durchgestrichen, Inline-Code, Links, Listen, Nummerierungen, Aufgabenlisten, Zitate, Codeblöcke, Trennlinien, Tabellen) mit Markdown-Kürzeln beim Tippen; Reintext für `.txt`; Quelltextansicht für Markdown (`Strg + /`). Bilder erscheinen als Hinweis mit Adresse und werden nie geladen.
- Markdown-Adapter mit lesbarem Escaping, erlaubter HTML-Teilmenge, YAML-Kopf, Fußnoten und Rohblöcken; 69 Round-Trip-Fälle in `tests/fixtures/markdown/`, 15 abschließend dokumentierte Normalisierungen (ADR 0014).
- Verlustschutz: Was der Editor nicht verlustfrei halten kann, öffnet in der Quelltextansicht mit Begründung; vor dem Speichern prüft der Editor geänderte Blöcke und warnt mit „Trotzdem speichern“ und „Abbrechen“.
- Schreibstatistik in der Statusleiste und als Dialog: Wörter nach Unicode-Regeln, Zeichen, Zeichen ohne Leerzeichen, Absätze, Lesezeit (Wörter pro Minute einstellbar); für die Markierung.
- Suchen und Ersetzen im Dokument (`Strg + F`, `Strg + H`, `F3`, `Umschalt + F3`): Groß-/Kleinschreibung, ganze Wörter, reguläre Ausdrücke (RE2), Treffer einzeln oder alle ersetzen.
- Einfügen: HTML durch das Editor-Schema, Markdown-Text als Markdown, `Strg + Umschalt + V` als reiner Text.
- Menüs Bearbeiten und Format, Formatierungsbefehle mit Kürzeln (`Strg + B/I/U/E/K` …), Link-Dialog (nur `http`, `https`, `mailto` und relative Adressen).
- Weblinks öffnen mit `Strg + Klick` im Systembrowser: Rust prüft die Adresse (`link_open`) und übergibt sie dem System als ein Argument, nie durch eine Shell. Beim Tippen werden nur Adressen mit `http://`, `https://`, `mailto:` oder `www.` und E-Mail-Adressen von selbst zu Links; ein fehlendes Leerzeichen nach einem Punkt („im.Am“) bleibt Text.
- Leistungsmessung im echten Programm (`tests/e2e/specs/performance.e2e.ts`): Ein Manuskript mit 50.000 Wörtern öffnet unter Linux in höchstens 463 ms, 95 % der Anschläge erscheinen nach höchstens 38 ms (Rich Text) bzw. 37 ms (Quelltext).

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
