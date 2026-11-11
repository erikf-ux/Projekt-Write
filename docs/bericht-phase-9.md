# Bericht Phase 9 – Suche

Stand: 11. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig). Entwicklungsumgebung: Cloud-Container, Ubuntu 24.04, 4 vCPU, 15 GB RAM, keine GPU (Xvfb, Software-Rendering), WebKitGTK, Rust 1.99.1, Node.js 22. CI: GitHub Actions `windows-latest` und `ubuntu-24.04`; macOS ist auf Wunsch des Auftraggebers zurückgestellt. Den CI-Lauf nennt der Abschnitt „CI“.

Phase 9 begann auf „Bearbeite schon mal einen Teil von Phase 9“ mit dem Rust-Teil; nach der Frage zu F-46 ging es auf „Fahre fort“ weiter bis zum Abschluss.

**Die vier Abnahmekriterien sind erfüllt**, jeweils in Rust-Tests und im echten Programm (E2E): Die ersten Treffer erscheinen mit der Referenzbibliothek nach 122–180 ms (Ziel 300 ms); `Muller`, `Mueller`, `Strasse`, `haus`, `C++` und `"` verhalten sich wie verlangt; archivierte Elemente erscheinen nur über den Filter; der Index stimmt nach Bearbeiten, Löschen und Wiederherstellen.

**Zwei Punkte solltest du kennen** (Einzelheiten unter „Gefundene Widersprüche und Abweichungen“):

1. **Prüfung beim Start (F-46):** Du hattest keinen der drei Wege gewählt; ich habe meinen Vorschlag umgesetzt, weil er sich jederzeit zurücknehmen lässt. Lokal prüft das Öffnen jede Tabelle einzeln, die ganze Datenbank 10 s später im Hintergrund (Punkt 1).
2. **Ein einzelnes Zeichen** (`e`) sucht nur noch das ganze Wort, nicht mehr jeden Wortanfang (Punkt 3).

## Was wurde implementiert?

Entscheidungen in ADR 0020 (15 Punkte).

- **Suchindex** (Migration 0009): `search_entries` ordnet jede Zeile einem Dokument, Medium oder Subnetz zu; `search_words` und `search_parts` stehen wörtlich wie in Abschnitt 19. Drei Ansichten beschreiben, was der Index je Element hält; Trigger halten Titel, Tags, Subnetze, Metadaten und die Beschreibung von Medien und Subnetzen in der Transaktion jeder Änderung aktuell (auch bei Struktur-Undo, Tag-Zusammenführung und Import).
- **Text der Dokumente**: Rust schreibt ihn in derselben Transaktion wie den neuen Inhalt, die Links und die Medienverwendungen; Markdown ohne die IDs interner Links und Medien, `.pwrite` als Text mit einer Zeile je Block. Eine Prüfsumme hält fest, welcher Inhalt indexiert ist; das Öffnen der Bibliothek trägt fehlenden Text nach (nach der Migration einmalig).
- **Anfragen** baut Rust: jedes Wort gequotet, Schreibvarianten (`ß`/`ss`, `ä`/`ae`, `ö`/`oe`, `ü`/`ue`), Wörter und Wortanfänge vor Teilwörtern, `bm25()` mit Titel 10, Tags 5, Subnetze 3, Metadaten 2, Inhalt 1. Wörter mit Zeichen (`C++`) müssen wörtlich vorkommen.
- **Filter**: Typ, Tags (alle), Subnetz mit Untersubnetzen, Änderungsdatum, nur Favoriten, Archiv einbeziehen, Papierkorb einbeziehen.
- **Ergebnis**: Art, ID, Titel, Zustand und ein Ausschnitt in Stücken mit Markierung; kein Pfad.
- **Befehle** `search_query` und `search_rebuild`, geprüft wie alle Befehle.
- **Oberfläche**: Seitenleiste „Suche“ neben dem Explorer (Activity Bar, `Ctrl + Shift + F`, Menü „Bearbeiten“ › „Global suchen“); Feld mit kurzer Pause nach dem Tippen, Filter, Trefferliste mit markiertem Wort, Pfeiltasten, Bild auf/ab, Enter. Ein Dokument öffnet an der Fundstelle (Suchleiste mit dem Wort, erster Treffer ausgewählt), ein Medium in seiner Ansicht, Subnetze und Elemente im Papierkorb im Explorer.
- **„Suchindex neu aufbauen“** in Einstellungen › Bibliothek und in der Befehlspalette.
- **Prüfung der Datenbank** beim Öffnen und im Hintergrund (F-46, ADR 0020 Punkt 14).

## Welche Dateien wurden erstellt oder verändert?

Rust, neu: `services/search/{mod,query,text}.rs`, `database/search.rs`, `commands/search.rs`, `migrations/0009_search.sql`, `tests/search.rs`. Geändert: `database/documents.rs` (Text in der Transaktion des Inhalts), `services/documents.rs` (`References` mit Text), `database/diagnostics.rs` (Prüfung je Tabelle), `services/library/mod.rs` (Prüfung beim Öffnen und im Hintergrund, Nachtragen), `services/session.rs`, `services/ui/menu.rs` („Global suchen“), `commands/mod.rs`, `build.rs`, `capabilities/main-window.json`, `tests/{database,ipc,migrations}.rs`.

Frontend, neu: `modules/search/` (`searchStore.ts`, `SearchPanel.tsx`, `openHit.ts`, `rebuild.ts`, Test `search.test.tsx`). Geändert: `app/{Workbench,App}.tsx`, `app/commands.ts`, `modules/commands/{types,keymap}.ts`, `modules/settings/{uiStore.ts,LibraryPreferences.tsx}`, `modules/editor/session.ts` (ersten Treffer zeigen), `services/ipc.ts`, `i18n/{de,en}.json`, `types/generated/`.

E2E: neu `specs/search.e2e.ts`; geändert `wdio.conf.ts`, `specs/{reference,workbench}.e2e.ts`. Doku: ADR 0020, `docs/{architecture,database,status}.md`, dieser Bericht, `CHANGELOG.md`, `README.md`.

Keine neuen Abhängigkeiten.

## Welche Tests wurden durchgeführt?

Umfang: **428 Rust-Tests** (280 Unit, 148 Integration; Phase 8: 397), **717 Vitest-Tests** in 40 Dateien (708 in 39), **71 WebdriverIO-Tests** in 17 Dateien (64 in 16).

Lokal (Linux-Container, Xvfb, D-Bus-Sitzung), alle ausgeführt und bestanden:

| Prüfung                                                                                                         | Ergebnis                                                                                                          |
| --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Prettier, ESLint (`--max-warnings 0`), TypeScript (App und Node-Teil), Vitest                                   | bestanden                                                                                                         |
| rustfmt, Clippy `-D warnings` (Linux, Linux + `e2e`, Windows-Ziel)                                              | bestanden                                                                                                         |
| Rust-Tests, darunter die Migrationen ab jeder früheren Version (0 bis 8 → 9) und das Befüllen bestehender Daten | bestanden                                                                                                         |
| Erzeugte TypeScript-Typen neu geschrieben                                                                       | bestanden                                                                                                         |
| Messung in Rust mit der vollen Referenzbibliothek (16 Anfragen)                                                 | langsamste 65 ms                                                                                                  |
| E2E-Gesamtlauf                                                                                                  | bestanden: 17 von 17 Dateien; erste Treffer mit der Referenzbibliothek 132–181 ms, Explorer steht nach 760–896 ms |

Was die Tests zeigen, im Einzelnen:

- **Schreibvarianten** (Rust, E2E): `Muller`, `Mueller`, `MÜLLER` und `müll` finden `Müller`; `Strasse` findet `Hauptstraße` als Teilwort und `Hauptstrasse` als Wort; `haus` findet `Häuser` (Wortanfang, zuerst) und `Krankenhaus` (Teilwort); der Ausschnitt markiert das Wort.
- **Zeichen** (Rust, E2E): 22 Eingaben mit Zeichen der FTS5-Syntax (`"`, `*`, `(`, `NEAR(`, `title:x`, `^`, …) ohne Fehler; `C++` findet nur das Dokument mit `C++`, nicht jedes mit einem `C`.
- **Archiv und Papierkorb** (Rust, E2E): nur über den Filter; der Treffer trägt das Zeichen „Im Archiv“ bzw. „Im Papierkorb“.
- **Index nach Änderungen** (Rust, E2E): Speichern (altes Wort weg, neues da), Umbenennen, Tags vergeben, umbenennen, entziehen, Subnetz zuordnen und umbenennen, Medium umbenennen und beschreiben, Papierkorb und zurück (auch über das Struktur-Undo), endgültig löschen, Subnetz löschen – danach jeweils gleich viele Einträge und FTS-Zeilen wie Elemente. Im echten Programm: bearbeitet, gelöscht, über die Suche im Papierkorb gefunden, im Explorer wiederhergestellt, wieder gefunden.
- **Nachtragen und Neuaufbau** (Rust, E2E): Text ohne Index wird beim Öffnen nachgetragen; der Neuaufbau liefert dieselben Treffer; ein beschädigter Index wird bei der Prüfung erkannt und neu aufgebaut.
- **Filter** (Rust, Vitest): Tags (alle), Subnetz mit Untersubnetzen, Favoriten, Datum, Anzahl, Typ.
- **Oberfläche** (Vitest, E2E): Pause nach dem Tippen, verworfene veraltete Antworten, leeres Feld fragt nichts, Tastatur, Öffnen an der Fundstelle mit Suchleiste, Fehlermeldung, Rückkehr zum Explorer.
- **IPC-Vertrag**: gültige Anfragen, 13 ungültige (fremde Felder, Pfade statt IDs, falsche Zeiten, Grenzen) im einheitlichen Fehlerformat; ein Treffer enthält genau die vereinbarten Felder.

## Abnahmekriterien Phase 9

| Kriterium                                                                                                           | Stand                                                                                                                                                             |
| ------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Erste Treffer in der Referenzbibliothek innerhalb des Zielwerts (300 ms)                                            | **erfüllt**: im echten Programm vom Tippen bis zu den Treffern auf dem Bildschirm 122–180 ms einschließlich 60 ms Pause (10 Anfragen, Container); CI siehe unten. |
| `Muller`/`Mueller` → `Müller`, `Strasse` → `Straße`, `haus` → `Krankenhaus` und `Häuser`, `C++` und `"` ohne Fehler | **erfüllt** (Rust und E2E).                                                                                                                                       |
| Archivierte Elemente erscheinen nur über den Filter                                                                 | **erfüllt** (Rust und E2E; ebenso der Papierkorb).                                                                                                                |
| Der Index stimmt nach Bearbeiten, Löschen und Wiederherstellen                                                      | **erfüllt** (Rust mit Zählung aller Zeilen; E2E im echten Programm).                                                                                              |

## Messungen

Container, Debug-Build (SQLite optimiert übersetzt), volle Referenzbibliothek (10.000 Dokumente mit 81 MB Text, 5.000 Medien, 500 Subnetze, 1.000 Tags):

| Wert                                   | vorher                                                                        | nachher     |
| -------------------------------------- | ----------------------------------------------------------------------------- | ----------- |
| Öffnen der Bibliothek (Rust)           | 4,8 s (F-46)                                                                  | 0,64 s      |
| Langsamste von 16 Suchen in Rust       | 162 ms (`e`)                                                                  | 65 ms       |
| Erste Treffer im Programm, 10 Anfragen | 150–350 ms (`e` 350, `Krankenhaus` 303)                                       | 122–180 ms  |
| Datenbank                              | 480 MB (Index: Teilwörter 187 MB, Wörter 62 MB, gespeicherter Text 2 × 90 MB) | unverändert |
| Explorer steht nach dem Öffnen (E2E)   | –                                                                             | 700–747 ms  |

Die Referenzbibliothek baut sich mit dem Index in 333 s statt rund 250 s auf.

## Was konnte nicht getestet werden und warum?

- **Windows:** läuft in CI (Rust, Vitest, E2E mit WebView2). Lokal nur Linux.
- **Referenzgerät:** nicht verfügbar; die Werte stammen aus dem Container und aus CI.
- **Hintergrundprüfung bei echter Beschädigung der Datei** (nicht nur des Index): ein Test dafür bräuchte eine gezielt beschädigte Seite; geprüft sind der Erfolg, der beschädigte Index und das Abbrechen beim Schließen.
- **Native Menüklicks** auf „Global suchen“: in CI nicht bedienbar; der Weg Menü → Befehl ist getestet.

## Offene Fehler

Keine bekannten.

## Behobene Fehler

F-46 bis F-50 in `docs/status.md`:

- **F-46** (Widerspruch, entschieden): `quick_check` beim Start brauchte mit dem Suchindex 4,1 s.
- **F-47** (Fehler der Suche, von einem Rust-Test gefunden): `C++` fand jeden Text mit einem einzelnen `C`, weil der Wortindex die Zeichen verliert.
- **F-48** (Leistung, von der E2E-Messung gefunden): Ein einzelnes Zeichen als Wortanfang brauchte im Programm 350 ms; jetzt ganzes Wort, und 50 statt 100 Treffer.
- **F-50** (Fehler eines Tests, CI-Lauf 108 unter Linux): Der Kürzel-Konflikttest lud die Seite neu, bevor die App die Tabs gespeichert hatte (2 s nach einer Änderung; beim Beenden sofort); dann kam die leere Sitzung des vorigen Starts zurück. Der Test wartet jetzt, bis Rust die offenen Tabs hält. Wahrscheinlich war das auch die Ursache von F-44 unter Windows in Phase 8.
- **F-49** (Fehler der Tests): Ein Phase-1-Test legte die Suchtabellen selbst an, die es seit Migration 0009 gibt; die Workbench-Prüfung erwartete die alte Activity Bar; der E2E-Test tippte ein Suchwort, das schon im Feld stand (keine neue Suche); zwei Vitest-Erwartungen nannten deutsche Texte, die Tests laufen auf Englisch.

## Gefundene Widersprüche und Abweichungen

1. **`quick_check` beim Start** (Abschnitt 18) gegen „Kaltstart ≤ 2 s“ (Abschnitt 6), F-46: Seit SQLite 3.44 zerlegt `quick_check` der ganzen Datenbank den Text aller FTS5-Tabellen neu. Umgesetzt wie vorgeschlagen (ADR 0020, Punkt 14): Bibliotheken auf lokalen Datenträgern prüft das Öffnen Tabelle für Tabelle mit allen Indizes (0,19 s), die ganze Datenbank 10 s später im Hintergrund über einen Lesezugang; scheitert die Prüfung, baut die App den Suchindex neu auf und prüft erneut. In Sync-Ordnern und auf Netzlaufwerken (Rollback-Journal) bleibt die ganze Prüfung beim Öffnen, weil ein langes Lesen dort jedes Speichern aufhielte. Was die Einzelprüfung nicht abdeckt: Seiten der Freiliste und Seiten ohne Tabelle; beides prüft der Hintergrund. Ein Ergebnis „fehlgeschlagen“ steht bisher nur im Protokoll; ein Hinweis in der Oberfläche passt zu Backup und Wiederherstellung (Phase 10). Wenn du lieber die ganze Prüfung beim Start willst, ist das eine Zeile.
2. **Speicherbedarf:** Der Index ist rund fünfmal so groß wie der Text (Trigramme, und Abschnitt 19 verlangt den Text in beiden Tabellen). Bei der Referenzbibliothek 480 MB statt rund 50 MB. Kleiner ginge es mit FTS5-Tabellen, die den Text aus einer gemeinsamen Tabelle lesen (`content=`); das ist ein eigener Umbau mit eigenen Tests. Ich schlage vor, das erst anzugehen, wenn die Größe auf dem Referenzgerät stört.
3. **Ein einzelnes Zeichen** (`e`) gilt nur als ganzes Wort, nicht als Wortanfang (F-48): Abschnitt 19 nennt Wortanfänge allgemein; als Wortanfang trifft ein einzelnes Zeichen fast jeden Text, und der Präfixindex (`prefix = '2 3'`) beginnt bei zwei Zeichen.
4. **Wörter mit Zeichen** (`C++`, `E-Mail`) müssen ab drei Zeichen wörtlich in Titel oder Inhalt stehen (F-47). Ein Tag `C++` allein findet sich deshalb über `C++` nicht, über `c` schon.
5. **Was nicht durchsucht wird:** Text in PDFs und Office-Dateien (Abschnitt 17 verlangt ihn nicht; spätere Erweiterung). Markdown steht im Ausschnitt als Quelltext (`**fett**`).
6. **Seitenleiste:** Welche Seitenleiste zuletzt offen war, merkt sich die App nicht über einen Neustart hinaus (die Einstellungen der Oberfläche halten bisher nur Breite und Sichtbarkeit).
7. **Ein Zweig je Phase** (Abschnitt 24): weiterhin nur der erlaubte Zweig.

## Entscheidungen (ADR)

- 0020 Suche: Index, Anfragen, Filter und Aktualität (15 Punkte), angenommen.

## CI

Zwischenstände trugen `[skip ci]` in der Betreffzeile.

- **Lauf 108** (Commit `8368d30`): Audit, Release-Prüfung und Windows vollständig grün (E2E 17 von 17 Dateien). Linux: alle Prüfungen grün bis auf einen E2E-Test, den Kürzel-Konflikttest aus Phase 3 (F-50, ein Fehler des Tests). Die Werte beider Systeme stehen unten.
- **Lauf 109** (Korrektur von F-50): **grün**, alle vier Jobs (Commit `74e0727`): Windows und Linux mit Prettier, ESLint, Typen, Vitest, rustfmt, Clippy, Rust-Tests, reproduzierbarer Referenzbibliothek, aktuellen Typen und Lizenzliste und E2E 17 von 17 Dateien; Audit; Release-Build ohne Test-Plugins.

Werte aus Lauf 108 (volle Referenzbibliothek):

| Wert (Ziel)                                                                  | Linux        | Windows          |
| ---------------------------------------------------------------------------- | ------------ | ---------------- |
| Suche, erste Treffer auf dem Bildschirm, 60 ms Pause eingerechnet (≤ 300 ms) | 97–175 ms    | 108–204 ms       |
| Explorer steht nach dem Öffnen (≤ 1 s)                                       | 476–575 ms   | 977–992 ms       |
| Auf- und Zuklappen, 95 % (≤ 100 ms)                                          | 32 ms        | 47,8 ms          |
| Tag-Filter (≤ 100 ms)                                                        | 48 ms        | 31,0 ms          |
| Scrollen im Explorer, 95 % (≤ 33 ms)                                         | 18–19 ms     | 15,7 ms          |
| Galerie, erster und zweiter Durchgang, 95 %                                  | 39 und 25 ms | 17,3 und 17,3 ms |

Die Windows-Werte des Explorers liegen wie in Phase 8 knapp unter 1 s (Phase 8, Lauf 107: 946–1.085 ms).

## Nächster Schritt

Nach Freigabe: Phase 10 (Export und Backup; Abschnitt 25).
