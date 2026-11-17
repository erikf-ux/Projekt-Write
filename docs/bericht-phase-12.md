# Bericht Phase 12 – Sprachprüfung

Stand: 17. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig).

- **Entwicklungsumgebung:** Cloud-Container, Ubuntu 24.04, 4 vCPU, keine GPU (Xvfb, WebKitGTK).
- **CI:** GitHub Actions `windows-latest` und `ubuntu-24.04`; macOS ist zurückgestellt.

Phase 12 ist umgesetzt:

- Rechtschreibprüfung beim Tippen mit mitgelieferten Wörterbüchern für Deutsch und Englisch;
- einfache Regeln;
- eine Autokorrektur nur für eindeutige Fehler;
- eigene Wörterbücher;
- Sprachprofile, Sprachabschnitte und Spracherkennung je Absatz;
- LanguageTool als Zusatzpaket.

Bei LanguageTool hat der Auftraggeber Variante 2 gewählt: Die App nutzt eine vorhandene Installation und lädt nichts herunter. Alle Abnahmekriterien sind durch Tests belegt. Nicht getestet ist LanguageTool selbst; dazu unten mehr.

## Was wurde implementiert?

Die Entscheidungen stehen in ADR 0023 bis 0026.

- **Vergleich zuerst (ADR 0023):** `spellbook` gegen Hunspell-Bindings, mit einem deutschen Korpus aus 1.198 richtigen Wörtern (darunter Komposita) und 212 Fehlern. Gewählt ist `spellbook` (reines Rust, gleiche Ergebnisse, kein C-Build). Alle Prüfwerke liegen hinter der Schnittstelle `LanguageEngine`.
- **Wörterbücher (ADR 0024):**
  - Sprachen: `de-DE`, `de-AT`, `de-CH`, `en-US`, `en-GB`.
  - Auslieferung: als getrennte Datenpakete mit Lizenztext und Prüfsummen, als Tauri-Ressource.
  - Quelltext: Der Quelltext von igerman98 liegt für die GPL mit bei.
  - Weitere Sprachen: Hunspell-Wörterbücher lassen sich als Sprache hinzufügen; erkannt werden UTF-8, ISO 8859-1 und ISO 8859-15.
- **Prüfung je Absatz in Rust:**
  - Rechtschreibung mit Vorschlägen, sortiert nach Abschnitt 14 (höchstens fünf);
  - Regeln: doppeltes Wort, doppeltes Leerzeichen, Großschreibung am Satzanfang;
  - Satzsegmentierung (58 Satzende-Fälle).
  - Code und Links schickt der Editor maskiert, sie werden nicht geprüft.
- **Editor:**
  - **Darstellung:** rote, wellige und blaue, gestrichelte Unterstreichung; im Druck unsichtbar.
  - **Neuprüfung:** nur geänderte Absätze, in Stapeln außerhalb der Eingabe, mit Zwischenspeicher.
  - **Kontextmenü:** Rechtsklick, Kontextmenütaste oder `Shift + F10`; mit Vorschlägen, „Zum Wörterbuch hinzufügen“ und „Ignorieren“.
  - **Statusleiste:** zeigt die Sprache; ein Befehl schaltet die Prüfung aus und ein.
- **Autokorrektur:**
  - **Wann:** nur, wenn alle sieben Bedingungen aus Abschnitt 14 erfüllt sind.
  - **Kandidaten:** werden deterministisch selbst erzeugt (ADR 0025, Punkt 4).
  - **Rückgängig:** Jede Korrektur ist ein eigener Undo-Schritt. Wird sie direkt mit `Strg + Z` zurückgenommen, gilt sie als abgelehnt und kommt nicht wieder.
  - **Start:** Sie ist aus, bis der Nutzer sie einschaltet. Die Frage steht auf dem Startbildschirm (D5) und in den Einstellungen.
- **Eigene Wörterbücher (Migration 0011):**
  - je Bibliothek, mit Kategorie;
  - zugeordnet zu Dokumenten, Subnetzen (mit allen Unter-Subnetzen) oder der ganzen Bibliothek;
  - Import und Export als Textdatei;
  - Verwaltung unter Einstellungen › Sprache.
- **Sprachen:**
  - **Sprachprofil je Dokument:** eine oder mehrere Sprachen, Autokorrektur aus.
  - **Markierte Abschnitte:** Mark `lang`, in Markdown `<span lang>`, `.pwrite` Formatversion 2.
  - **Erkennung je Absatz:** mit `whatlang`, ab 40 Buchstaben und Sicherheit 0,6. Kurze Absätze erben die Sprache des vorigen.
- **LanguageTool (ADR 0026):**
  - **Einrichten:** Der Nutzer wählt den Ordner einer Installation. Die App startet den Server mit Java 17 auf einem freien Port auf `127.0.0.1` und prüft je Absatz zusätzlich auf Grammatik, Zeichensetzung und Kontext.
  - **Rechtschreibung:** bleibt beim eigenen Prüfwerk.
  - **Anzeige:** Zustand in Einstellungen und Statusleiste. Das Kontextmenü zeigt die Erklärung von LanguageTool.
  - **Fehler:** Fehlt Java oder endet der Server, prüft die App ohne ihn weiter.

## Welche Dateien wurden erstellt oder verändert?

**Rust, neu:**

- `src-tauri/src/services/language/`:
  - `mod.rs` (Dienst, Prüfung);
  - `engine.rs` (Schnittstelle `LanguageEngine`, eigene Wörter);
  - `spelling.rs` (`spellbook`, Vorschläge, Kandidaten);
  - `dictionaries.rs` (Wörterbuchablage, Import);
  - `tokens.rs`, `rules.rs`, `sentences.rs`;
  - `autocorrect.rs` (sieben Bedingungen);
  - `own.rs` (eigene Wörterbücher, Ablehnungen, Profile);
  - `detect.rs` (Erkennung);
  - `languagetool.rs` (Zusatzpaket).
- `src-tauri/src/commands/language.rs` (27 Befehle), `src-tauri/src/database/language.rs`, `src-tauri/migrations/0011_language.sql`.
- `src-tauri/dictionaries/`: fünf Sprachen, Lizenzen, `SHA256SUMS`, Quellen von igerman98.
- Tests: `tests/language.rs` (Korpus, Autokorrektur, Prüfsummen, Laden) und `tests/language_data.rs`; Daten in `tests/data/language/`.

**Rust, geändert:** `commands/mod.rs`, `state.rs`, `lib.rs` (Ressourcenordner, Server beim Beenden stoppen), `error.rs` (neue Fehlercodes), `services/settings/ui.rs` (`languageCheck`), `services/pwrite.rs` (Formatversion 2), `build.rs`, `tauri.conf.json` (Ressourcen), `Cargo.toml` (`spellbook`, `whatlang`), Tests in `ipc.rs` und `migrations.rs`.

**Frontend, neu:**

- `src/modules/language-check/`:
  - `plugin.ts` (Prüfung, Unterstreichung, Autokorrektur);
  - `extract.ts`;
  - `languageStore.ts`, `languageToolStore.ts`;
  - `CheckMenu.tsx`, `LanguageSettings.tsx`, `DocumentLanguageDialog.tsx`;
  - `plugin.test.ts`.
- `src/modules/editor/schema/lang.ts`.

**Frontend, geändert:** `Workbench.tsx`, `StartScreen.tsx`, `commands.ts`, `StatusBar.tsx`, `DocumentEditor.tsx`, `session.ts`, `editor.css`, `extensions.ts`, `raw.ts`, `exportActions.ts`, `SettingsDialog.tsx`, `uiStore.ts`, `ipc.ts`, `de.json`, `en.json`, erzeugte Typen.

**E2E:** `tests/e2e/specs/language.e2e.ts` (7 Tests), `session.ts`, `wdio.conf.ts`. **Fixture:** `tests/fixtures/markdown/70-sprachabschnitte.md`.

**Doku:**

- ADR 0024, 0025, 0026 (0023 schon vorher);
- `database.md`, `dependencies.md`, `pwrite-format.md`, `status.md`;
- `CHANGELOG.md`, `README.md`, `THIRD-PARTY-LICENSES`, `scripts/third-party-licenses.mjs`.

## Welche Tests wurden durchgeführt?

| Art                      | Anzahl                            | Ergebnis                                                                        |
| ------------------------ | --------------------------------- | ------------------------------------------------------------------------------- |
| Rust (Unit, Integration) | 610 (605 ausgeführt, 5 ignoriert) | bestanden                                                                       |
| Vitest                   | 743 in 43 Dateien                 | bestanden                                                                       |
| E2E (WebdriverIO)        | 20 Spec-Dateien                   | bestanden (lokal, Linux; eine Schwankung unter „Offene Fehler“; CI siehe unten) |

Lokal ausgeführt und bestanden: Prettier, ESLint, TypeScript, Vitest, rustfmt, Clippy (ohne und mit `e2e`), alle Rust-Tests, erzeugte Typen, Lizenzliste, E2E-Gesamtlauf.

- **Korpus** (Rust, `tests/language.rs`): 0 von 1.198 richtigen Wörtern markiert (0 %); 209 von 212 Fehlern erkannt (98,6 %).
- **Autokorrektur gegen den Korpus:** kein richtiges Wort geändert; 106 Fehler korrigiert, keiner falsch.
- **Die sieben Bedingungen:** je ein eigener Test (`autocorrect.rs`, `condition_1_…` bis `condition_7_…`), dazu Bedingung 3 mit Leerzeichen und Bindestrich.
- **Satzende-Testdaten:** 58 Fälle grün (`tests/data/language/satzende.txt`).
- **Undo** (Vitest, E2E): Eine Autokorrektur ist ein eigener Undo-Schritt; eine Ersetzung aus dem Kontextmenü ist eine einzelne Transaktion. Im E2E-Test bringt `Strg + Z` „Zimemr“ zurück, die Ablehnung steht in der Datenbank, und die Korrektur kommt nicht wieder.
- **E2E im echten Programm:**
  - Unterstreichung, Vorschlag aus dem Kontextmenü;
  - Wort zum eigenen Wörterbuch hinzufügen;
  - Code nicht prüfen, doppeltes Leerzeichen und doppeltes Wort blau;
  - Autokorrektur und Ablehnung; Ein- und Ausschalten;
  - englischer Absatz im deutschen Dokument;
  - LanguageTool ohne Installation.
- **LanguageTool** (Rust, mit lokalem Testserver): HTTP-Client, Umwandlung der Antwort (UTF-16 in Byte-Offsets, Rechtschreibung und Stil verworfen), Java-Versionen, Erkennen einer Installation, keine doppelten Fundstellen.
- **Migration 0011** ab jeder früheren Version (`tests/migrations.rs`), Prüfsummen der Wörterbücher, Laden aller fünf Sprachen (20 bis 35 ms je Sprache).

## Leistung

| Messung                                                 | Zielwert | Container            |
| ------------------------------------------------------- | -------- | -------------------- |
| Tippverzögerung mit aktiver Sprachprüfung (95 %)        | ≤ 50 ms  | 36 ms (Median 22 ms) |
| Öffnen eines Dokuments mit 50.000 Wörtern (mit Prüfung) | –        | 268 bis 298 ms       |
| Laden eines Wörterbuchs                                 | –        | 20 bis 35 ms         |

## Abnahmekriterien Phase 12

| Kriterium (Abschnitt 25)                                                                                | Stand                                                                                                  |
| ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Höchstens 3 % der richtigen Wörter werden markiert, mindestens 95 % der Fehler werden erkannt           | erfüllt: 0 % markiert, 98,6 % erkannt                                                                  |
| Die Autokorrektur ändert kein richtiges Wort und korrigiert keinen Fehler falsch; je Bedingung ein Test | erfüllt (Korpus; sieben eigene Tests)                                                                  |
| Die Satzende-Testdaten sind grün                                                                        | erfüllt (58 Fälle)                                                                                     |
| Jede Korrektur lässt sich mit einem Undo-Schritt zurücknehmen                                           | erfüllt (Vitest, E2E)                                                                                  |
| Die Tippverzögerung bleibt im Zielwert                                                                  | erfüllt im Container (36 ms bei 95 %, Quelltextansicht 33 ms); Messung auf dem Referenzgerät steht aus |
| Ohne Zusatzpaket funktioniert die App vollständig. LanguageTool bindet nur an `127.0.0.1`               | erfüllt ohne Zusatzpaket (alle Tests ohne LanguageTool); Bindung siehe unten                           |

## Was konnte nicht getestet werden und warum?

- **LanguageTool mit einer echten Installation:**
  - **Warum nicht:** Der Container erreicht languagetool.org nicht, und in CI ist LanguageTool nicht installiert. Geprüft sind alle Teile der App mit einem lokalen Testserver: Erkennen der Installation, Java-Version, Start, HTTP, Umwandlung der Antwort und die Oberfläche ohne Installation. Nicht geprüft ist das Zusammenspiel mit einem echten LanguageTool.
  - **Bindung an `127.0.0.1`:** Die App startet den Server ohne `--public`. Ohne `--public` nimmt LanguageTool nach seiner Dokumentation nur Verbindungen vom eigenen Rechner an. Die App selbst verbindet sich nur mit `127.0.0.1`. Belegt ist das erst mit einer echten Installation.
  - **Nachholen:** als Handtest unter Windows und Linux (`docs/status.md`).
- **Referenzgerät:** Die Tippverzögerung ist im Container gemessen, nicht auf dem Referenzgerät.
- **Echte Eingabe:** Rechtsklick und Kontextmenütaste mit echten Geräten, Eingabe mit Komposition (IME) und Tottasten. Der WebDriver sendet synthetische Ereignisse (ADR 0003).
- **macOS:** zurückgestellt.

## Offene Fehler

Keine bekannten. Beobachtung wie in Phase 10 und 11: Im E2E-Gesamtlauf lag die Galerie-Messung aus Phase 8 im zweiten Durchgang knapp über ihrer Grenze (95 % bei 55 ms, Grenze 50 ms). In drei Wiederholungen der Referenz-Specs waren es 53, 48 und 43 ms; die beiden letzten bestanden. Die Galerie ist in Phase 12 unverändert; die Sprachprüfung arbeitet nur im Editor. Ich halte es weiter für Schwankung des Containers.

## Behobene Fehler

- **F-58 (CI-Lauf 119, Linux):** Im Test der Tastenkürzel verlor der Rekorder den Fokus an das Suchfeld. Danach blieb der Einstellungsdialog offen, und 14 Spec-Dateien scheiterten als Folge.
  - **Ursache:** Tiptap setzt den Fokus in den Editor erst im nächsten Animationsframe. Unter Xvfb kam das Frame aus dem Befehl „Fokusmodus“ des vorigen Tests rund 500 ms später, als der Dialog schon offen war. Die Fokusfalle des Dialogs gab den Fokus dann dem ersten Element statt dem Rekorder.
  - **Nachgestellt:** lokal, mit einem Protokoll der Fokuswechsel samt Stacktraces, etwa in jedem dritten Lauf.
  - **Behebung:** Ein modaler Dialog gibt einen entwichenen Fokus jetzt dem Element zurück, das ihn zuletzt hatte, und nur sonst dem ersten. Das betraf auch Nutzer, nicht nur den Test.
  - **Belegt:** ein neuer Vitest-Fall, der mit dem alten Code scheitert; danach bestand der Ablauf sechsmal in Folge, der E2E-Gesamtlauf mit 20 von 20 Dateien.

Während der Arbeit gefunden und behoben, keiner davon in einem CI-Lauf:

- **Autokorrektur im Korpus:** Fünf Fehler wurden falsch korrigiert, etwa über einen ausgeschriebenen Umlaut. Der ausgeschriebene Umlaut zählt jetzt als ein Schritt.
- **Nicht reproduzierbare Vorschläge von `spellbook`:** „Fesnter“ wurde im Programm einmal korrigiert, einmal nicht. Die Kandidaten für die Autokorrektur erzeugt die App jetzt selbst (ADR 0025, Punkt 4).
- **Erkennung:** `is_reliable` von `whatlang` lehnte englische Absätze ab. Die Erkennung prüft jetzt eine Sicherheit von mindestens 0,6.
- **Doppelte Prüfungen:** Durch das Erben der Sprache wurde jeder Absatz doppelt geprüft. Es erben jetzt nur kurze Absätze.
- **Startbildschirm:** Er wurde mit der Frage zur Autokorrektur zu hoch, und der Kontrasttest scheiterte. Die Frage steht jetzt im Sprachblock.
- **Erwartung im IPC-Test:** Der Test erwartete für neue `.pwrite`-Dateien noch Formatversion 1.

## Gefundene Widersprüche und Abweichungen

1. **LanguageTool als vorhandene Installation statt Download** (Entscheidung des Auftraggebers, Variante 2; ADR 0026). Das Herunterladen nach Bestätigung (Abschnitt 14) lässt sich später ergänzen; Prüfwerk und Oberfläche bleiben dabei gleich.
2. **Autokorrektur, Bedingung 3:** Die Kandidaten kommen nicht von den Vorschlägen des Prüfwerks, sondern aus einer eigenen, vollständigen Erzeugung. Ein ausgeschriebener Umlaut oder ß zählt als ein Schritt (ADR 0025, Punkt 4).
3. **Erkennung:** Abschnitt 14 nennt keine Schwelle. Gewählt sind 40 Buchstaben und eine Sicherheit von 0,6. Kurze Absätze erben die Sprache des vorigen Absatzes.
4. **Wortkategorien** (Wort, Fachbegriff, Name, Abkürzung) ordnen nur die Verwaltung. Geprüft werden alle Wörter gleich, wie bei Hunspell.
5. **„Gesamte Anwendung“:** Eigene Wörterbücher gehören zur Bibliothek. „Gesamte Anwendung“ bedeutet: alle Dokumente dieser Bibliothek. Abgelehnte Korrekturen gelten je Bibliothek und für alle Sprachen.
6. **Lizenzen der Wörterbücher** (vor Phase 14, mit D4):
   - en-GB: Debian nennt die SCOWL-Lizenz, das README die LGPL.
   - GPL-Wörterbücher als getrennte Datenpakete neben der App-Lizenz.
   - `spellbook` steht unter MPL-2.0.
7. **Ein Zweig je Phase** (Abschnitt 24): weiterhin nur der erlaubte Zweig.

## Entscheidungen (ADR)

- 0023 Prüfwerk `spellbook` statt Hunspell-Bindings (Vergleich mit Korpus), angenommen.
- 0024 Wörterbücher: Herkunft, Lizenzen, Auslieferung als Datenpakete, angenommen.
- 0025 Sprachprüfung: Ablauf, eigene Wörterbücher, Autokorrektur, Erkennung und Sprachabschnitte, angenommen.
- 0026 LanguageTool als Zusatzpaket über eine vorhandene Installation, angenommen.

## CI

| Lauf | Ergebnis    | Inhalt                                                             |
| ---- | ----------- | ------------------------------------------------------------------ |
| 118  | grün        | Zwischenstand: Rechtschreibung, Regeln, Autokorrektur, UI          |
| 119  | rot (Linux) | F-58: Rekorder ohne Fokus, 14 Spec-Dateien als Folge; Windows grün |
| 120  | LAUF120     | Abschluss Phase 12 mit F-58                                        |

## Nächster Schritt

Warten auf die Freigabe von Phase 13 (Erweiterungen, Abschnitt 22).
