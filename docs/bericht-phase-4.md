# Bericht Phase 4 – Editor

Stand: 4. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig). Entwicklungsumgebung: Cloud-Container, Ubuntu 24.04, 4 vCPU Intel Xeon 2,1 GHz, 15 GB RAM, keine GPU (Xvfb, Software-Rendering), WebKitGTK 2.52.6, Rust 1.99.1, Node.js 22. Das ist nicht das Referenzgerät aus Abschnitt 6. CI: GitHub Actions `windows-latest` und `ubuntu-24.04`; macOS ist auf Wunsch des Auftraggebers zurückgestellt.

## Was wurde implementiert?

- **Schreibfläche mit Tiptap 3.32.2** (Abschnitt 12) in den vorhandenen Tabs, eine Sitzung je Tab: Verlauf, Auswahl und Suche bleiben, solange der Tab offen ist.
- **Rich Text für Markdown:** Überschriften 1–6, fett, kursiv, unterstrichen, durchgestrichen, Inline-Code, Links, Listen, Nummerierungen, Aufgabenlisten, Zitate, Tabellen, Codeblöcke, Trennlinien. Markdown-Kürzel wie `## ` oder `**fett**` wirken beim Tippen. Bilder bleiben beim Laden und Speichern erhalten und erscheinen als Hinweis mit Adresse; sie werden nie geladen (Bilder einfügen und PDF-Verweise: Phase 8, laut Phasenplan „zunächst ohne Bilder und PDF-Verweise“).
- **Reintext und Quelltextansicht** im Minimal-Schema: Leerzeichen und Tabulatoren bleiben exakt erhalten, die Quelltextansicht ändert nur, was getippt wird. `.txt` hat nur diese Ansicht, `.md` wechselt mit `Strg + /`.
- **Markdown-Adapter** (ADR 0014): YAML-Kopf wird abgetrennt und unverändert wieder eingesetzt; Fußnoten, HTML-Blöcke und unbekanntes Inline-HTML bleiben als nicht ausgeführte Rohblöcke; Bild im Link mit eigener Regel; Bildgröße über `<img>`; erlaubte HTML-Teilmenge `<u>`, `<br>`, `<img>`; lesbares Escaping (`Müller & Söhne`, `snake_case`, `[[Wiki-Link]]` bleiben stehen).
- **Verlustschutz:** Die Round-Trip-Probe beim Öffnen öffnet ein Dokument, das der Editor nicht verlustfrei halten kann, in der Quelltextansicht und nennt den Grund. Vor dem Speichern prüft die Sitzung jeden geänderten Block; was Markdown nicht schreiben kann (verbundene Zellen, mehrere Blöcke in einer Zelle, Tabellen ohne Kopfzeile, Ausrichtung je Zelle, Nummerierung mit Buchstaben, Betonung direkt an einem Satzzeichen, Roh-HTML am Zeilenanfang …), nennt eine Warnung mit „Trotzdem speichern“ und „Abbrechen“. Autosave überschreibt so nichts still.
- **Laden und Speichern über den Speicherkern** (Abschnitt 9): Autosave 1 s nach der letzten Eingabe und beim Verlassen der Schreibfläche, Zustand in der Statusleiste.
- **Schreibstatistik:** Wörter nach Unicode-Regeln, Zeichen, Zeichen ohne Leerzeichen, Absätze, Lesezeit (abschaltbar, 50–1000 Wörter pro Minute, Standard 200); bei einer Markierung für die Markierung. Statusleiste und Dialog.
- **Suchen und Ersetzen** (`Strg + F`, `Strg + H`, `F3`, `Umschalt + F3`): Treffernavigation, Groß-/Kleinschreibung, ganze Wörter, reguläre Ausdrücke (RE2, lineare Laufzeit), einzeln oder alle ersetzen, in allen Ansichten.
- **Einfügen:** HTML durchläuft das Schema (Farben und Schriften entfallen, Bilder werden Hinweise, Links nur `http`, `https`, `mailto`); Text, der wie Markdown aussieht, wird als Markdown gelesen; `Strg + Umschalt + V` fügt reinen Text ein.
- **Menüs Bearbeiten und Format** (natives Menü, 34 Menübefehle), Formatierungsbefehle in Palette und Kürzeln (`Strg + B/I/U/E/K`), Link-Dialog.
- **Weblinks:** `Strg + Klick` öffnet über den neuen Befehl `link_open` im Systembrowser. Rust prüft die Adresse (`http`, `https`, `mailto`, ohne Leer- und Steuerzeichen, höchstens 2048 Zeichen) und übergibt sie als ein Argument, nie durch eine Shell (`ShellExecuteW`, `xdg-open`). Ein einfacher Klick öffnet nichts. Von selbst werden nur Adressen mit `http://`, `https://`, `mailto:`, `www.` und E-Mail-Adressen zu Links.
- **Leistung:** Lesen von Markdown in linearer Zeit, auch in JavaScriptCore (siehe unten); Messung im echten Programm.

## Welche Dateien wurden erstellt oder verändert?

Frontend, neu: `modules/editor/` mit `session.ts`, `DocumentEditor.tsx`, `editor.css`, `StatisticsDialog.tsx`, `LinkDialog.tsx`, `paste.ts`, `statistics.ts`, `statisticsStore.ts`, `find/{findStore.ts,FindBar.tsx}`, `markdown/{adapter,escape,manager,probe,losses}.ts`, `schema/{extensions,markdownRules,raw,image,attributes,tokenizers}.ts`, `plain/schema.ts`; `app/formatCommands.ts`; `test/plainMarked.ts`; Tests `markdown/{roundtrip,stability,losses,manager}.test.ts`, `session.test.ts`, `DocumentEditor.test.tsx`, `statistics.test.ts`, `find/FindBar.test.tsx`, `paste.test.ts`, `app/formatCommands.test.ts`. Entfernt: `modules/editor/PlainTextEditor.tsx` (Textfeld der Phase 3). Geändert: `app/{App.tsx,Workbench.tsx,commands.ts,focus.ts}`, `components/layout/StatusBar.tsx`, `modules/commands/{types,keymap}.ts`, `modules/documents/workspaceStore.ts`, `modules/settings/{GeneralSettings,ShortcutSettings}.tsx`, `services/ipc.ts`, `i18n/{de,en}.json`, `test/setup.ts`, erzeugte Typen.

Rust, neu: `services/links.rs`, `commands/links.rs`. Geändert: `services/settings/ui.rs` (Lesezeit), `services/ui/menu.rs` (Menüs Bearbeiten, Format), `filesystem/windows.rs` (`ShellExecuteW`), `state.rs`, `lib.rs`, `error.rs` (2 Fehlercodes), `commands/mod.rs`, `build.rs`, `capabilities/main-window.json`, `Cargo.toml` (Feature `Win32_UI_Shell`), `tests/ipc.rs`.

Testdaten: 69 Fälle in `tests/fixtures/markdown/` mit 30 `.expected.md` und `normalizations.json`. E2E: neu `tests/e2e/specs/{editor,performance}.e2e.ts`, `tests/e2e/longText.ts`; geändert `wdio.conf.ts`, `sandbox.ts`, `specs/workbench.e2e.ts`. Abhängigkeiten: Tiptap-Pakete 3.32.2, `marked` 17.0.6 direkt (war schon im Baum). Doku: ADR 0014, `docs/architecture.md`, `docs/dependencies.md`, `docs/status.md`, dieser Bericht, `CHANGELOG.md`, `README.md`.

## Welche Tests wurden durchgeführt?

Umfang: **220 Rust-Tests** (132 Unit, 88 Integration; dazu 2 Hilfsprozesse), **632 Vitest-Tests** in 25 Dateien, **34 WebdriverIO-Tests** in 6 Dateien (Phase 3: 213, 156, 26).

Lokal (Linux-Container, Xvfb, D-Bus-Sitzung), alle ausgeführt und bestanden:

| Prüfung                                                                                                                                    | Ergebnis  |
| ------------------------------------------------------------------------------------------------------------------------------------------ | --------- |
| Prettier, ESLint, TypeScript, Vitest (632)                                                                                                 | bestanden |
| rustfmt, Clippy `-D warnings` (Linux, Linux + `e2e`, Windows-Ziel `x86_64-pc-windows-gnu`)                                                 | bestanden |
| Rust-Tests (220), darunter die 100 harten Abbrüche                                                                                         | bestanden |
| Round-Trip-Test: 69 Fälle; Stabilitätstest: 300 erzeugte Dokumente                                                                         | bestanden |
| Vergleich der Tokens mit und ohne Schnellprüfungen: alle Fixtures, 300 Dokumente, 37 Grenzfälle, Trennlinien-Regel an 20.000 Zufallstexten | bestanden |
| E2E (34): Rauchtest (7), Bibliothek (4), Neustart (3), Arbeitsfläche (12), Editor (3), Leistung (5)                                        | bestanden |

## Leistung (Abschnitt 6)

Gemessen in `tests/e2e/specs/performance.e2e.ts` im echten Programm mit einem erzeugten deutschen Manuskript von genau 50.000 Wörtern (338 KB, 1025 Blöcke: Kapitel, Absätze mit Fett und Kursiv, Listen, Zitate). Öffnen: vom Klick im Explorer bis zu dem Bild, das den ganzen, bearbeitbaren Text zeigt. Tippen: vom Tastendruck bis zum gezeichneten Bild mit dem Zeichen, im Tempo eines schnellen Schreibers (etwa 9 Anschläge pro Sekunde, Pausen nach Wörtern und Sätzen), sodass Autosave und Statistik dazwischen laufen. Die Tasten werden in der Seite so ausgelöst, wie der eingebettete WebDriver es tut (ADR 0003).

Linux-Container (WebKitGTK), alle Läufe nach der Korrektur von F-16; Öffnen dreimal nach dem Import und einmal nach dem Speichern, Tippen als 95-%-Wert (Ziel jeweils ≤ 500 ms bzw. ≤ 50 ms):

| Lauf                               | Öffnen nach dem Import | nach dem Speichern | Tippen Rich Text | Tippen Quelltext |
| ---------------------------------- | ---------------------- | ------------------ | ---------------- | ---------------- |
| 1                                  | 416, 317, 309 ms       | 379 ms             | 35 ms            | –                |
| 2                                  | 368, 443, 332 ms       | 463 ms             | 38 ms            | 37 ms            |
| 3                                  | 398, 269, 411 ms       | 360 ms             | 30 ms            | 35 ms            |
| 4                                  | 353, 418, 469 ms       | 303 ms             | 33 ms            | 36 ms            |
| 5                                  | 547, 451, 501 ms       | 421 ms             | 34 ms            | 34 ms            |
| 6 (Statistik nach dem ersten Bild) | 447, 359, 347 ms       | 872 ms¹            | 36 ms            | 40 ms            |
| 7 (Statistik nach dem ersten Bild) | 307, 308, 292 ms       | 345 ms             | 36 ms            | 35 ms            |

¹ Gestört: ESLint und TypeScript liefen in diesem Moment auf denselben 4 vCPUs.

Der Median der Tippverzögerung lag in allen Läufen bei 20–23 ms, der höchste Einzelwert bei 59–110 ms. Seit Lauf 6 zählt die Statistik erst nach dem ersten Bild (150 ms nach dem Öffnen); in Lauf 5 brauchte sie 25–70 ms vor dem ersten Bild.

Vor der Korrektur (F-16) dauerte das Öffnen dort 15,5–17,2 s. Ursache: marked probiert jede Blockregel an jedem Block auf dem ganzen Resttext. JavaScriptCore kompiliert einige dieser Ausdrücke nicht und probiert im Interpreter trotz `^` jede Startposition, dazu teilten drei Tokenizer den Rest in Zeilen. In JavaScriptCore allein gemessen sank das Lesen von 7 s auf 55–130 ms (ADR 0014, Punkt 16). Unter Windows (V8) trat das Problem nie auf.

Einordnung: Die Tippverzögerung erfüllt das Ziel mit Reserve. Das Öffnen-Ziel ist erreicht, aber ohne sichere Reserve: Vor der Verschiebung der Statistik lagen 2 von 20 Messungen darüber (501 und 547 ms), danach keine der ungestörten. Gemessen ohne Sprachprüfung, die erst mit Phase 12 kommt. CI-Werte zeigen nur den Trend (Abschnitt 6); der Test schlägt dort erst beim Zehnfachen der Ziele an. In CI-Lauf 77 öffnete das Manuskript unter Linux in 365–479 ms und unter Windows (WebView2) in 267–341 ms; 95 % der Anschläge lagen unter Linux bei höchstens 36 bzw. 42 ms (Quelltext), unter Windows bei 18 bzw. 14 ms.

## Abnahmekriterien Phase 4

| Kriterium                                                   | Stand                                                                                                                                                                                                                                                                                                                                                                                                   |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mindestens 50 Markdown-Fälle bestehen den Round-Trip-Test   | **erfüllt.** 69 Fälle, von Import über Editor zu Export: Text bytegleich zur Eingabe oder zu ihrer `.expected.md`, die nur Normalisierungen aus der abschließenden Liste (15 Einträge) enthalten darf; Vergleich der Dokumente, nicht des HTML; YAML-Kopf bytegleich; zweites Speichern ändert nichts. Dazu 300 erzeugte Dokumente. Unter Windows und Linux in CI.                                      |
| Alle Verlustfälle aus Abschnitt 10 sind gelöst oder warnen  | **erfüllt.** YAML-Kopf, Fußnoten, HTML-Blöcke und unbekanntes Inline-HTML, Bild im Link, Bildgröße: gelöst und mit eigenen Round-Trip-Fällen belegt. Verbundene Zellen und mehrere Blöcke in einer Zelle: Warnung vor dem Speichern. Dazu eigene Funde (etwa „Dr. Müller kommt.“ als Liste), gelöst oder mit Warnung. „Als `.pwrite` speichern“ fehlt in der Warnung, bis es das Format gibt (Phase 5). |
| Die Tippverzögerung erfüllt den Zielwert bei 50.000 Wörtern | **erfüllt im prüfbaren Umfang:** 95 % der Anschläge ≤ 38 ms (Rich Text) und ≤ 40 ms (Quelltext) im Linux-Container in allen Läufen, unter Windows in CI ≤ 18 ms. Ohne Sprachprüfung (Phase 12), nicht auf dem Referenzgerät.                                                                                                                                                                            |
| Die Statistik stimmt auf deutschen Testtexten               | **erfüllt.** Unit-Tests mit deutschen Texten (Umlaute, ß, „Zitate“, Gedankenstrich, „2,50 €“, „E-Mail“, „z. B.“, Zahlen, kombinierende Akzente, Emoji); im echten Programm zählt die Statusleiste das Manuskript mit genau 50.000 Wörtern, unter Linux und Windows. Dabei gefunden und behoben: WebKit zählte Zahlen nicht als Wörter (F-15).                                                           |

## Was konnte nicht getestet werden und warum?

- **macOS:** zurückgestellt; nicht gebaut, nicht getestet.
- **Referenzgerät:** nicht verfügbar; gemessen im Container (oben) und als Trend in CI.
- **Sprachprüfung beim Tippen:** kommt mit Phase 12; die Tippverzögerung ist ohne sie gemessen.
- **Der Systembrowser selbst:** `xdg-open` und `ShellExecuteW` laufen in keinem Test, CI würde sonst einen Browser starten. Getestet sind die Adressprüfung (Rust), gültige und ungültige Aufrufe von `link_open` (IPC-Tests mit aufzeichnendem Opener) und im echten Programm der Weg `Strg + Klick` → `link_open` → Rust (E2E; der `e2e`-Build zeichnet auf, statt zu öffnen).
- **Native Menüklicks** (Bearbeiten, Format): WebDriver bedient keine nativen Menüs. Getestet sind Menü-IDs, Befehle und Weiterleitung.
- **Echte Tastatur- und IME-Eingabe:** Der eingebettete WebDriver sendet synthetische Ereignisse (`keydown`, dann `execCommand('insertText')`) und setzt als `keyCode` den Zeichencode; `.` (46) liest ProseMirror deshalb als Entf-Taste. Tests, die Punkte tippen, lösen dieselben Ereignisse in der Seite ohne `keyCode` aus (ADR 0003). Eingabemethoden mit Komposition (etwa Akzente per Tottaste) sind nicht geprüft.
- **`Strg + Umschalt + V` und Einfügen aus echten Programmen** (Word, LibreOffice, Browser): Getestet mit synthetischen Zwischenablage-Ereignissen und HTML-Proben; was eine Webview bei diesem Kürzel liefert, ist so nicht prüfbar.

## Offene Fehler

Keine bekannten. Bekannte Grenze (ADR 0014): marked liest manche dicht gepackten `*`-Folgen anders als CommonMark (im Stabilitätslauf über 3000 Dokumente 5 Fälle); Probe und Verlustschutz fangen sie ab, statt still zu ändern.

## Behobene Fehler

F-12 bis F-18 in `docs/status.md`. F-12 (Escape im Fokusmodus), F-14 (Autolink „im.Am“), F-15 (Wortzählung unter Linux) und F-16 (Öffnen 16 s unter Linux) fanden die E2E-Tests im echten Programm; F-13 (Windows-Clippy) und F-17 (Wettlauf beim Neustart der App zwischen zwei E2E-Dateien, nur Test) fand CI; F-18 (Wettlauf im neuen Editor-Test, nur Test) fiel bei einem lokalen Lauf auf. Die Round-Trip-Fehler, die der Stabilitätstest während der Entwicklung fand, wurden jeweils vor dem Commit behoben; die verbleibenden Grenzen beschreibt ADR 0014.

## Gefundene Widersprüche und Abweichungen

1. **Tippverzögerung „Sprachprüfung aktiv“** (Abschnitt 6) gegen den Phasenplan (Sprachprüfung in Phase 12): gemessen ohne Sprachprüfung; die Messung wird in Phase 12 wiederholt.
2. **Bilder:** Abschnitt 12 nennt Bilder und eingefügte Bilder, der Phasenplan sagt „zunächst ohne Bilder und PDF-Verweise“. Bilder in Markdown bleiben erhalten und erscheinen als Hinweis; Einfügen und Import folgen mit den Medien (Phase 8, ADR 0014).
3. **Verlustschutz ohne „Als `.pwrite` speichern“** und ohne Umwandeln zwischen Formaten: beides kommt mit dem Format in Phase 5.
4. **Interne Links und Rückverweise, Drucken, Wiederherstellen:** Abschnitt 12, aber nicht im Umfang von Phase 4; laut Phasenplan in den Phasen 5, 10 und 6.
5. **Autolink:** Tiptap verlinkt beim Tippen jede Domain mit bekannter Endung. In deutschem Fließtext macht das aus einem vergessenen Leerzeichen nach einem Punkt einen Link (F-14). Automatisch verlinkt werden nur Adressen mit Schema oder `www.` und E-Mail-Adressen; über den Link-Dialog lässt sich jede erlaubte Adresse setzen.
6. **Eigene marked-Instanz** mit Schnellprüfungen und `marked` als direkte Abhängigkeit (ADR 0014, Punkt 16, `docs/dependencies.md`): nötig für das Leistungsziel unter Linux, ohne den Inhalt des Lesens zu ändern. Eine zweite Editor-Bibliothek (Abschnitt 12) war nicht nötig.
7. **Wortzählung ohne `isWordLike`** (Abschnitt 12, „nach Unicode-Regeln“): Wortsegmente nach UAX #29 mit Buchstabe oder Ziffer, weil die Engines `isWordLike` unterschiedlich setzen (F-15).
8. **Ein Zweig je Phase** (Abschnitt 24): weiterhin nur der erlaubte Zweig.

## Entscheidungen (ADR)

0014 Editor: Schema, Markdown-Adapter und Verlustschutz, ergänzt um die Punkte 10–16 (Sitzung je Tab, Speichern, Statistik, Suchen, Einfügen, Weblinks, Lesen in linearer Zeit).

## CI

**Letzter vollständiger Lauf: CI-Lauf 77** (Commit `45b71df`, Code wie `94e01c4`):

| Job                             | Ergebnis                                                                                                                                                                                                 |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Windows (`windows-latest`)      | grün: Prettier, ESLint, TypeScript, Vitest, rustfmt, Clippy, Rust-Tests (mit den 100 harten Abbrüchen), Referenzbibliothek zweimal gleich, Typen und Lizenzliste aktuell, E2E 6 von 6 Dateien (34 Tests) |
| Linux (`ubuntu-24.04`)          | rot nur in einem Schritt: alle Prüfungen und Tests grün, E2E 5 von 6 Dateien; `restart.e2e.ts` bekam keine Sitzung (F-17, Wettlauf im Testablauf, seitdem behoben und lokal dreimal grün)                |
| Abhängigkeitsprüfung            | grün: `cargo audit`, `cargo deny`, `npm audit`                                                                                                                                                           |
| Release-Build ohne Test-Plugins | grün                                                                                                                                                                                                     |

**CI-Lauf 78** (Commit `8b7c5f9`, mit der Korrektur von F-17) **und CI-Lauf 79** (Commit `af8940a`, endgültiger Code) **starteten keinen Job:** Alle vier Jobs endeten nach 2 Sekunden ohne Runner, ohne Schritt und ohne Log; abgerechnet wurden 0 Minuten. Das ist kein Fehler im Code. Wahrscheinlich ist das Kontingent an GitHub-Actions-Minuten des Kontos aufgebraucht (78 Läufe, Windows-Minuten zählen doppelt), möglich ist auch eine Störung bei GitHub. Das berührt Kosten und Konto; ich habe an CI nichts geändert. Die Korrekturen nach Lauf 77 (F-17, Editor-Test, Statistik nach dem ersten Bild) sind deshalb nur lokal geprüft: alle 34 E2E-Tests, Vitest und Clippy grün.

Davor: Die Läufe 72 und 73 scheiterten unter Windows an Clippy (F-13); 74 bis 76 wurden durch neuere Pushes abgebrochen.

## Nächster Schritt

Nach Freigabe: Phase 5 (Dokumente und externe Dateien, Abschnitte 9, 10, 12, 13): Erstellen, Öffnen, Umbenennen, Kopieren, Favoriten und Zuletzt verwendet; `.pwrite`, TXT und MD als Dateiformate; externe Dateien mit Kodierung, Zeilenenden und Änderungen von außen; Dateizuordnung; interne Links und Rückverweise; Umwandeln mit Verlustschutz.
