# ADR 0027 – Erweiterungen: interne Register, kein fremder Code, Einstellungen übertragen

- Status: angenommen
- Datum: 17. November 2026
- Phase: 13

## Kontext

Abschnitt 22 verlangt für Version 1.0 eine vorbereitete Plugin-Architektur:

- interne Register für Editorfunktionen, Dateiformate, Prüfwerke, Exporter und Mesh-Renderer, die die eigenen Module tatsächlich nutzen;
- ein Konzeptpapier `docs/plugin-concept.md` zu Berechtigungsmodell und Sandbox;
- kein Code von Dritten.

Phase 13 nennt zusätzlich „Erweiterte Einstellungen“. Ihre Abnahme verlangt:

- Die eigenen Exporter und Dateiformate nutzen die Register.
- Fremder Code lässt sich nicht laden, belegt durch einen Test.
- Einstellungen lassen sich exportieren, importieren und zurücksetzen.

## Entscheidung

### 1. Register in Rust

- **Ort:** `src-tauri/src/services/registry.rs` mit einem allgemeinen `Registry<T>`.
- **Aufbau:** Ein Register entsteht genau einmal beim Start, aus einer Funktion je Art, die die eingebauten Einträge aufzählt. Danach ist es unveränderlich: Es gibt keine Methode, die später etwas hinzufügt oder ersetzt.
- **IDs:** Jeder Eintrag hat eine ID der Form `[a-z][a-z0-9.-]*`, eindeutig je Register. Eine doppelte ID ist ein Programmierfehler und bricht den Aufbau ab; ein Test belegt das.
- **Arten in Rust:**
  - **Dateiformate:** `txt`, `md`, `pwrite`, je mit Dateiendung, Unterordner in der Bibliothek, Art (Text oder Rich Text) und Lese- und Schreibweg.
  - **Exporter:** TXT, Markdown (mit Medien), Paket `.pwritepack`, Bibliothek als Markdown-Ordner.
  - **Prüfwerke:** Rechtschreibung (`spellbook`), Regeln, LanguageTool (Zusatzpaket).
- **Nutzung:** Der Code wählt Format, Exporter und Prüfwerk über das Register (Nachschlagen per ID oder Dateiendung) statt über fest verdrahtete Fallunterscheidungen.
- **Auskunft:** Der Befehl `extensions_list` nennt die Einträge aller Register in Rust, für die Oberfläche (Punkt 5).

#### Umsetzung in Rust (Nachtrag)

Zwei Präzisierungen, die sich bei der Umsetzung ergeben haben:

- **Ein Exemplar je Prozess:** Die Register stehen in `Extensions` (`services/extensions.rs`). Sie entstehen einmal je Prozess beim ersten Zugriff (`extensions::shared()`), spätestens in `AppState::initialize`; `AppState` hält dasselbe Exemplar. Dienste ohne `AppState` (Dokumente, Suche, Backup) schlagen Formate so ebenfalls im Register nach.
- **Art des Dateiformats:** Sie hat drei Werte: reiner Text, Markdown und Rich Text. Markdown braucht eigene Wege für Verweise, Medien und Suchtext. Der Lese- und Schreibweg folgt aus der Art: Rich Text geht über die `.pwrite`-Hülle, Text über UTF-8.

### 2. Register im Frontend

- **Ort:** `src/core/registry.ts` mit `createRegistry<T>(kind)`. Methoden: `register(entry)`, `get(id)`, `require(id)`, `list()`, `seal()`.
- **Aufbau:** Alle eingebauten Einträge stehen an einer Stelle, in `src/extensions/builtin.ts`. `main.tsx` ruft sie vor dem ersten Rendern auf und versiegelt danach alle Register. Nach `seal()` wirft `register` einen Fehler.
- **Arten im Frontend:**
  - **Editorfunktionen:** Gruppen von Tiptap-Erweiterungen je Schema (Markdown, Rich Text), etwa Bilder, Links, Sprachabschnitte, Sprachprüfung.
  - **Dateiformate:** Lesen und Schreiben im Editor für `txt`, `md` und `pwrite`.
  - **Exporter:** die Aktionen für TXT, Markdown, Paket, Drucken und PDF; Befehle und Menüpunkte entstehen aus dem Register.
  - **Mesh-Renderer:** React Flow. `MeshPanel` nimmt den Renderer aus dem Register.
- **Prüfwerke:** Sie liegen nur in Rust; das Frontend kennt sie über `extensions_list`.

#### Umsetzung im Frontend (Nachtrag)

- **Schemata:** Die Editorfunktionen sind nach den Schemata `markdown` und `plain` gegliedert. `.pwrite` nutzt das Markdown-Schema; ein eigenes Rich-Text-Schema gibt es nicht. Ein Test belegt, dass die Editoren dieselben Tiptap-Erweiterungen in derselben Reihenfolge erhalten wie vorher.
- **Befehle:** Die Exportbefehle entstehen aus dem Register, mit unveränderten IDs, Titeln und Tastenkürzeln. Deshalb registriert `main.tsx` die Befehle erst nach dem Versiegeln (vorher geschah das beim Laden von `App.tsx`). Das native Menü bleibt in Rust definiert.
- **Exporter:** Der Export der ganzen Bibliothek steht auch im Frontend-Register.

### 3. Kein fremder Code

Es gibt keinen Weg, Code von außen in die App zu bringen.

- **Register:** Sie nehmen nach dem Start nichts mehr an (Frontend: `seal()`; Rust: unveränderlich).
- **Kein Ladeweg:** Es gibt keinen Plugin-Ordner und keinen Befehl, der Code liest. Das Frontend lädt nichts dynamisch.
  - ESLint verbietet `eval`, `new Function`, `setTimeout` mit Text und `import()` mit einem nicht festen Ziel.
  - Die CSP erlaubt Skripte nur aus `'self'`: kein `'unsafe-eval'`, kein `'unsafe-inline'`, keine `data:`- und `blob:`-Skripte.
  - Rust lädt keine dynamischen Bibliotheken; geprüft wird das über den Abhängigkeitsbaum.
- **Test:** `tests/e2e/specs/extensions.e2e.ts` belegt im echten Programm, dass all das scheitert:
  - `eval` und `new Function`;
  - ein Inline-`<script>`;
  - ein Skript aus `data:`, aus `blob:` und von einer fremden Adresse;
  - `import()` einer fremden Adresse;
  - `register` nach dem Start.

  Dazu kommen Unit-Tests der Register in Vitest und Rust.

- **Abweichung beim Test „`register` nach dem Start“:** Das E2E-Programm nutzt dasselbe Frontend wie die Auslieferung; nur Rust hat die Eigenschaft `e2e`. Ein Testzugang zu den Registern würde also mit ausgeliefert. Deshalb prüft Vitest das Versiegeln mit derselben Startfolge wie `main.tsx`. Der E2E-Test belegt, dass kein Objekt auf `window` ein Register anbietet.
- **Abhängigkeitsbaum:** `dlopen2` kommt unter Linux über `tao` (Tauri) hinein. Es lädt nur die feste Systembibliothek `libunity` für Fortschritt und Zähler in der Taskleiste; die App nutzt das nicht, und die Capability erlaubt keine `core:window`-Befehle. `libloading`, `libffi`, Skript- und WebAssembly-Laufzeiten kommen nicht vor. Das Laden von SQLite-Erweiterungen ist nicht eingeschaltet.

### 4. Einstellungen exportieren, importieren, zurücksetzen

- **Datei:** `.pwsettings`, JSON in UTF-8:

  ```json
  { "format": "projectwrite-settings", "formatVersion": 1, "exportedAt": "…", "app": { … }, "library": { … } }
  ```

  `library` fehlt, wenn keine Bibliothek offen ist.

- **Inhalt `app`:** die Einstellungen der Oberfläche: Design, Sprache der Oberfläche, Zoom, Seitenleiste, Lesezeit, Sprachprüfung, Tastenkürzel.
- **Nicht enthalten**, weil an diesen Rechner oder diese Sitzung gebunden:
  - Ort der Bibliothek;
  - Fensterzustand;
  - Sitzung;
  - zuletzt geöffnete Dateien;
  - Ordner von LanguageTool.

  Importierte Designs und Wörterbücher sind Dateien und werden nicht mitgenommen. Die Wahl eines Designs schon; fehlt das Design, gilt wie heute „System“.

- **Inhalt `library`:** die Schlüssel aus `application_settings` der offenen Bibliothek: Papierkorb, Backup, Versionen, Mesh, Medien.
- **Import:**
  - Die ganze Datei wird mit denselben Prüfungen wie beim normalen Ändern geprüft.
  - Vor dem Übernehmen zeigt die App die Änderungen.
  - Übernommen wird alles oder nichts.
  - Eine neuere Formatversion wird abgelehnt; unbekannte Schlüssel werden übergangen und genannt.
- **Zurücksetzen:** getrennt für App und Bibliothek, nach Bestätigung, auf die Standardwerte. Bibliotheksdaten und eigene Wörterbücher bleiben unberührt.
- **Sicherung:** Vor jedem Import und jedem Zurücksetzen legt die App den bisherigen Stand als `.pwsettings` im Konfigurationsordner ab (`settings-backups/`, die fünf neuesten).

#### Umsetzung (Nachtrag)

- **Nicht übertragen** wird außerdem `languageCheck.autocorrectAsked` (ob der erste Start schon nach der Autokorrektur gefragt hat). Es bleibt auch beim Zurücksetzen erhalten. In `library` fehlt `storage.acceptedLocation`, weil es an den Ort gebunden ist.
- **Fehlende Schlüssel** behalten ihren Wert. Verschachtelte Werte wie `sidebar` müssen vollständig sein, sonst ist der Wert ungültig.
- **Übernehmen** nimmt nur ein Kennzeichen aus der Vorschau entgegen; die geprüften Werte bleiben in Rust. Eine neue Vorschau ersetzt die alte. Ist eine andere Bibliothek offen als bei der Vorschau, scheitert das Übernehmen (`SETTINGS_IMPORT_EXPIRED`).
- **Speicherorte:** `system_locations` nennt die Pfade, `system_open_location` öffnet einen Ordner. Der Befehl nimmt einen Namen (`config`, `logs`, `library`), nie einen Pfad.

### 5. Erweiterte Einstellungen

Der neue Abschnitt „Erweitert“ in den Einstellungen enthält:

- Einstellungen exportieren, importieren, zurücksetzen (Punkt 4);
- die eingebauten Erweiterungen je Register, nur lesend, mit Herkunft „eingebaut“;
- die Speicherorte (Konfigurationsordner, Protokoll, Bibliothek) mit „Ordner öffnen“.

### 6. Konzeptpapier

`docs/plugin-concept.md` beschreibt für später Berechtigungen, Sandbox, Signatur, Laden und Entladen. In Version 1.0 ist nichts davon ausführbar.

## Alternativen

- **Register, die man zur Laufzeit füllen kann** (etwa ein Plugin-Ordner): verworfen, Abschnitt 22 schließt Code von Dritten aus.
- **Nur Schnittstellen ohne Register** (wie bisher `renderer.ts`): verworfen, die Abnahme verlangt, dass die eigenen Module die Register nutzen.
- **Einstellungen als Kopie von `settings.json`:** verworfen. Die Datei enthält Pfade und Sitzungsdaten dieses Rechners, und eine Kopie würde ungeprüft übernommen.

## Folgen

- Keine neue Abhängigkeit.
- Neue Befehle:
  - `extensions_list`;
  - `settings_export`, `settings_import_preview`, `settings_import_apply`, `settings_reset`;
  - `system_locations`, `system_open_location`.
- Neue Fehlercodes: `SETTINGS_FILE_INVALID`, `SETTINGS_FILE_NEWER`, `SETTINGS_VALUE_INVALID`, `SETTINGS_IMPORT_EXPIRED`, `FOLDER_OPEN_FAILED`.
- Neue Datei `.pwsettings`; ihr Format beschreibt `docs/settings-format.md`.
