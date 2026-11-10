# Bericht Phase 7 – Subnetze und Tags

Stand: 10. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig). Entwicklungsumgebung: Cloud-Container, Ubuntu 24.04, 4 vCPU, 15 GB RAM, keine GPU (Xvfb, Software-Rendering), WebKitGTK, Rust 1.99.1, Node.js 22. CI: GitHub Actions `windows-latest` und `ubuntu-24.04`; macOS ist auf Wunsch des Auftraggebers zurückgestellt. Entwickelt und geprüft wurde lokal unter Linux; den CI-Lauf nennt der Abschnitt „CI“.

**Zur Messung „flüssig“** (Abschnitt „Explorer-Messung“): Auf der Container-Maschine vor der Pause verfehlte das Scrollen das Ziel. Auf der jetzigen Maschine erreichen nach den Optimierungen alle Werte ihr Ziel, in drei Läufen. Das Ergebnis hängt also deutlich an der Maschine; die Bestätigung auf dem Referenzgerät steht aus.

## Was wurde implementiert?

Entscheidungen in ADR 0017 (18 Punkte und Nachträge).

- **Hierarchische Struktur** (Abschnitt 16): Subnetze mit Name, Beschreibung, Farbe, übergeordnetem Subnetz, Zeitstempeln (Migration 0007). Geschwister heißen nie gleich (ohne Rücksicht auf Groß- und Kleinschreibung). **Zyklen** lehnt Rust ab (`SUBNET_CYCLE`), zusätzlich ein Trigger in SQLite, der die Vorfahren rekursiv prüft.
- **Mehrfachzuordnung mit primärem Subnetz** (Abschnitte 16 und 18, geprüftes Muster): Ein Dokument liegt in beliebig vielen Subnetzen und bleibt eine Zeile in `documents` und eine Datei. Die erste Zuordnung ist primär; entfällt sie, rückt die älteste verbleibende nach. **Verschieben** (Abschnitt 13) wechselt das primäre Subnetz, die anderen Zuordnungen bleiben; „Zuordnen zu …“, „Aus „X“ entfernen“, „Als primär festlegen“, „Aus allen Subnetzen nehmen“ (auf „Nicht zugeordnet“ ziehen). Die Datei bleibt immer, wo sie ist.
- **Subnetz löschen** löscht nie ein Element: Untersubnetze rücken eine Ebene hoch (ein dort vergebener Name bekommt „ (2)“) oder werden auf Wunsch mitgelöscht; die Abfrage nennt die Zahl der Zuordnungen, die entfallen.
- **Explorer** (Abschnitt 13): Favoriten, Zuletzt verwendet, die Subnetze als Baum (Untersubnetze vor Dokumenten, nach Namen), Nicht zugeordnet, Archiv, Papierkorb. Ein Dokument steht in jedem seiner Subnetze; eine weitere Zuordnung trägt ein Symbol mit Text für Screenreader. Der Weg zum aktiven Dokument klappt auf. Virtualisiert, eine Tab-Stelle, Pfeiltasten, `F2`, `Entf`, Kontextmenü-Taste.
- **Navigation über Breadcrumbs**: über der Schreibfläche der Weg des primären Subnetzes bis zum Dokument, die weiteren Subnetze und die Tags; ein Klick zeigt das Subnetz im Explorer.
- **Farbverwaltung**: die fünf Cluster-Farben des Designs (als Token gespeichert, sie wechseln mit dem Design) oder eine eigene Farbe, die gegen Seitenleiste und Schreibfläche beider Designs mindestens 3:1 erreicht (Abschnitt 6, grafische Elemente); der Dialog nennt den Kontrast. Untersubnetze erben die Farbe, neue auf oberster Ebene bekommen reihum die nächste. Die Farbe erscheint als Punkt mit Rand, der Name steht immer daneben.
- **Tags** (Abschnitt 13): vergeben über den Namen (ein neuer legt ein Tag an), entfernen, umbenennen (trifft der Name ein anderes Tag, fragt die App nach dem Zusammenführen), zusammenführen, löschen. `Äpfel` und `äpfel` sind dasselbe Tag. `.pwrite` schreibt die Tags beim Speichern in die Hülle; ein importiertes `.pwrite` bringt seine Tags mit.
- **Filter** im Explorer nach Tags (alle gewählten) und Titel; Subnetze ohne Treffer verschwinden, die mit Treffern klappen auf.
- **Verschieben ohne Maus und mit Maus**: Kontextmenü und Befehlspalette („Verschieben nach …“, „Zuordnen zu …“ mit einer durchsuchbaren Auswahl der Wege), Drag & Drop im Explorer (mit `Strg` zuordnen statt verschieben; Subnetz auf Subnetz macht es zum Untersubnetz).
- **Struktur-Undo** (Abschnitt 8: „Strukturaktionen … haben ein eigenes Undo“): Jede Strukturaktion, auch Löschen, Wiederherstellen und Archivieren, ist ein Schritt mit dem Zustand vorher und nachher. `Strg + Z`/`Strg + Y` im Explorer, Befehlspalette, und nach jedem Schritt ein Hinweis mit „Rückgängig“. Rust prüft vor dem Zurückschreiben, dass nichts dazwischen kam (`STRUCTURE_CHANGED`). Höchstens 100 Schritte, nur im Speicher.
- **Kopieren** übernimmt Subnetze (mit demselben primären) und Tags, nicht die Historie (Abschnitt 13).
- **Erster Start**: Kästchen „Beispiel-Subnetze anlegen (Projekte, Wissen, Ideen)“, nicht vorausgewählt.
- **Referenzbibliothek** (Abschnitt 6): jetzt mit 500 Subnetzen in bis zu vier Ebenen, 1.000 Tags, 16.870 Zuordnungen, 19.698 Dokument-Tags; 1.541 Dokumente sind nicht zugeordnet. Reproduzierbar, im Fingerabdruck.

## Welche Dateien wurden erstellt oder verändert?

Rust, neu: `services/structure.rs` (Dienst und Undo-Journal), `database/structure.rs`, `commands/structure.rs`, `migrations/0007_subnets_and_tags.sql`, `tests/structure.rs`. Geändert: `services/library/mod.rs` (Journal je geöffneter Bibliothek), `services/session.rs` (`with_structure`), `services/documents.rs` (Tags in der `.pwrite`-Hülle), `commands/{documents,external,mod}.rs` (Löschen, Archivieren, Kopieren und Import über die Struktur), `error.rs` (6 Fehlercodes), `build.rs`, `capabilities/main-window.json`, `examples/reference_library/{generate,main}.rs`, `tests/{ipc,migrations,reference_library}.rs`. **19 neue Befehle.**

Frontend, neu: `modules/structure/` (`structureStore.ts`, `tree.ts`, `dialogStore.ts`, `SubnetDialog.tsx`, `SubnetPicker.tsx`, `TagsDialog.tsx`, `TagManager.tsx`, `Breadcrumbs.tsx`, `sampleSubnets.ts`, Test `structure.test.tsx`), `modules/documents/{explorerRows.ts,explorerStore.ts,explorerRows.test.ts}`. Geändert: `modules/documents/{Explorer.tsx,workspaceStore.ts,importFiles.ts,Explorer.test.tsx}`, `app/{Workbench.tsx,commands.ts}`, `modules/commands/types.ts`, `components/NoticeBar.tsx` (Schaltfläche im Hinweis), `modules/settings/uiStore.ts`, `modules/library/LibraryPanel.tsx`, `services/ipc.ts`, `i18n/{de,en}.json`, erzeugte Typen.

E2E: neu `specs/{structure,reference-prepare,reference}.e2e.ts`; geändert `specs/{workbench,documents}.e2e.ts`, `session.ts`, `sandbox.ts`, `wdio.conf.ts`, `.gitignore` (`tests/e2e/.cache/`). Doku: ADR 0017, `docs/{architecture,database,status}.md`, dieser Bericht, `CHANGELOG.md`, `README.md`. **Keine neue Abhängigkeit.**

## Welche Tests wurden durchgeführt?

Umfang: **348 Rust-Tests** (226 Unit, 122 Integration; Phase 6: 313), **695 Vitest-Tests** in 37 Dateien (682), **56 WebdriverIO-Tests** in 15 Dateien (48).

Lokal (Linux-Container, Xvfb, D-Bus-Sitzung), alle ausgeführt und bestanden:

| Prüfung                                                                                                                                 | Ergebnis                                                                                                                                              |
| --------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Prettier, ESLint (`--max-warnings 0`), TypeScript (App und Node-Teil), Vitest (695)                                                     | bestanden                                                                                                                                             |
| rustfmt, Clippy `-D warnings` (Linux, Linux + `e2e`, Windows-Ziel `x86_64-pc-windows-gnu`)                                              | bestanden                                                                                                                                             |
| Rust-Tests (348), darunter die 100 harten Abbrüche und die Migrationen ab jeder früheren Version (0 bis 6 → 7)                          | bestanden                                                                                                                                             |
| Erzeugte TypeScript-Typen neu geschrieben und unverändert; Lizenzliste unverändert                                                      | bestanden                                                                                                                                             |
| Referenzbibliothek zweimal gebaut, gleicher Fingerabdruck (10.000 Dokumente, 500 Subnetze, 1.000 Tags)                                  | bestanden (je Lauf 86–138 s, vorher 31 s: jede der rund 37.000 Strukturaktionen ist eine eigene Transaktion mit `fsync`, derselbe Weg wie in der App) |
| E2E (56) in 15 Dateien, darunter Struktur (4) und Explorer-Messung (3)                                                                  | bestanden; mehrere vollständige Läufe                                                                                                                 |
| Release-Build ohne Test-Plugins; `npm audit --omit=dev` (ausgelieferte Pakete); `npm audit` (mit Entwicklungswerkzeugen, nur berichtet) | bestanden; 0 Funde; 34 Funde (20 mittel, 14 hoch), unverändert, bewertet in `docs/dependencies.md`                                                    |

Was die Tests zeigen, im Einzelnen:

- **Zyklen** (Rust, IPC, E2E): Ein Subnetz unter sich selbst, sein Kind oder Enkelkind wird abgelehnt, von Rust und – mit direktem SQL – vom Trigger; der Baum bleibt unverändert. Im echten Programm: Ein Subnetz auf sein eigenes Untersubnetz gezogen zeigt „Ein Subnetz kann nicht in sich selbst …“, die Ebenen bleiben.
- **Ein Datensatz, eine Datei** (Rust, IPC, E2E): nach zwei Zuordnungen eine Zeile in `documents` und eine Datei im Ordner; im Explorer zwei Zeilen mit derselben ID, die weitere markiert, die Breadcrumbs nennen beide Subnetze. Eine zweite Zuordnung zum selben Subnetz ändert nichts.
- **Löschen ohne Elementverlust** (Rust, IPC, E2E): Untersubnetze rücken hoch (mit „ (2)“ bei vergebenem Namen) oder gehen mit, kein Dokument verschwindet, die primäre Zuordnung rückt nach; im echten Programm danach `Strg + Z` – Subnetz, Ebene und beide Zuordnungen sind zurück.
- **Verschieben** (Rust): primär wechselt, andere bleiben; Verschieben in ein Subnetz, in dem das Dokument schon liegt, macht es primär; Entfernen der primären lässt die älteste nachrücken.
- **Tags** (Rust, IPC, Vitest, E2E): eindeutig ohne Groß- und Kleinschreibung, Umbenennen auf einen vergebenen Namen bietet das Zusammenführen an, Zusammenführen ohne Doppel, Löschen ohne Dokumentverlust; `.pwrite` trägt die Tags, ein Import übernimmt sie. Im echten Programm: Tag vergeben, in den Breadcrumbs sichtbar, Filter zeigt nur das Dokument.
- **Struktur-Undo** (Rust): eine Folge von 16 Schritten – angelegt, verschoben mit vertauschten Eltern, umbenannt, zugeordnet, Tags umbenannt und zusammengeführt, archiviert, gelöscht, Subnetz gelöscht – vollständig zurück und wieder vor, jeweils zustandsgleich; halber Weg vor und zurück; eine Änderung am Journal vorbei wird erkannt und abgelehnt; ein endgültig gelöschtes Dokument lässt sich nicht zurückholen.
- **Farben** (Rust, Vitest): Cluster-Farben reihum und vererbt; eine eigene Farbe unter 3:1 wird abgelehnt, der Dialog nennt den Kontrast.
- **Explorer-Zeilen** (Vitest): Baum, Mehrfachzuordnung, Filter; 10.000 Dokumente und 500 Subnetze ganz aufgeklappt (19.086 Zeilen) in 50 ms gebaut, der Filter in 9 ms.
- **IPC-Vertrag**: jeder neue Befehl mit gültigen und ungültigen Anfragen (fremde Felder, Pfade statt IDs, ungültige Farben und Namen), einheitliches Fehlerformat.

## Abnahmekriterien Phase 7

| Kriterium                                                         | Stand                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Zyklen werden abgelehnt                                           | **erfüllt** (Rust, Datenbank-Trigger, IPC, E2E).                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Ein Element in zwei Subnetzen bleibt ein Datensatz und eine Datei | **erfüllt für Dokumente** (Rust, IPC, E2E). Medien gibt es erst ab Phase 8; ihre Zuordnungen folgen demselben Muster.                                                                                                                                                                                                                                                                                                                                                                                                   |
| Das Löschen eines Subnetzes löscht kein Element                   | **erfüllt** (Rust, IPC, E2E, mit Struktur-Undo).                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Der Explorer bleibt mit der Referenzbibliothek flüssig            | **erfüllt auf der aktuellen Messmaschine** (drei Läufe), gemessen im echten Programm mit der vollen Referenzbibliothek; Ziele aus ADR 0017, Punkt 17 (der Auftrag nennt keinen Zahlenwert). Baum nach dem Öffnen 649–799 ms (Ziel 1 s), Auf- und Zuklappen 95 % ≤ 49 ms (Ziel 100 ms), Tag-Filter 60–80 ms (Ziel 100 ms), Scrollen 95 % der Bildabstände 22–25 ms (Ziel 33 ms). Auf der vorherigen Container-Maschine verfehlte das Scrollen das Ziel (siehe „Explorer-Messung“); auf dem Referenzgerät nicht gemessen. |

## Explorer-Messung

Gemessen im Debug-Build unter Xvfb mit Software-Rendering (ohne GPU), 4 vCPU, in zwei Container-Maschinen. Die Sitzung lief nach der Pause auf einer neuen Maschine.

| Wert (Ziel)                               | Vorherige Maschine, vor der Optimierung | Vorherige Maschine, optimiert | Jetzige Maschine, optimiert (3 Läufe) |
| ----------------------------------------- | --------------------------------------- | ----------------------------- | ------------------------------------- |
| Baum nach dem Öffnen (≤ 1 s)              | 1.055–1.281 ms                          | 837–1.268 ms                  | 649–799 ms                            |
| Auf- und Zuklappen, 95 % (≤ 100 ms)       | 56 ms                                   | 55–81 ms                      | 49 ms                                 |
| Tag-Filter (≤ 100 ms)                     | 103–144 ms                              | 96–116 ms                     | 60–80 ms                              |
| Scrollen, 95 % der Bildabstände (≤ 33 ms) | 41 ms                                   | 36–47 ms                      | 22–25 ms                              |
| zum Vergleich: reine DOM-Liste, 95 %      | –                                       | 24–27 ms                      | 19–20 ms                              |

Was ich gefunden und geändert habe:

- **Was die Zeit kostet:** React braucht je Scrollschritt 2 ms (95 %: 4 ms). Der Rest sind Stil, Layout und Zeichnen der neu eingehängten Zeilen, und das hängt an der Maschine; die Symbole in den Zeilen spielen dabei keine messbare Rolle.
- **Optimierungen:**
  - Dokumentliste und Struktur parallel laden;
  - einmal sortieren statt bei jedem Filter;
  - Zeilen als memoisierte Komponenten;
  - Scrollen in einer eigenen Komponente, synchron gerendert;
  - `contain: strict` je Zeile.

Der E2E-Test prüft Auf- und Zuklappen gegen das Ziel. Aufbau, Filter und Scrollen prüft er gegen Regressionsgrenzen (1,5 s, 200 ms, 50 ms), weil CI-Maschinen ebenso schwanken. Die Werte schreibt er jedes Mal mit.

Mein Vorschlag: einmal auf dem Referenzgerät messen (Release-Build, Hardware-Rendering) und dann die Grenzen im Test auf die Ziele setzen. Wenn du das anders willst – etwa die Ziele schon jetzt in CI erzwingen –, sag es mir.

## Was konnte nicht getestet werden und warum?

- **Windows:** läuft in CI (Rust, Vitest, E2E mit 2.000 Dokumenten für die Explorer-Messung, weil Windows jede Datei synchron schreibt). Lokal nur Clippy für das Windows-Ziel.
- **Echtes Drag & Drop mit der Maus:** Der eingebettete WebDriver erzeugt nur synthetische Ereignisse (ADR 0003); getestet sind die Ereignisfolge im echten Programm und die Logik in Vitest.
- **Screenreader mit dem verschachtelten Baum:** ARIA-Rollen und -Stufen sind gesetzt und in Tests geprüft, nicht mit einem echten Screenreader.
- **Referenzgerät:** nicht verfügbar; alle Leistungswerte stammen aus dem Container (siehe oben).
- **Native Menüleiste:** Die neuen Befehle stehen in Befehlspalette und Kontextmenüs, nicht im nativen Menü.
- **Medien:** kommen in Phase 8.

## Offene Fehler

Keine bekannten. F-28 (Explorer-Messung) ist auf der jetzigen Maschine erfüllt; die Messung auf dem Referenzgerät steht aus.

## Behobene Fehler

F-27 bis F-31 in `docs/status.md`:

- **F-29 war ein Fehler der App:** Ein abgelehnter Zyklus beim Ziehen blieb still. Der E2E-Test hat ihn gefunden.
- **F-27, F-30 und F-31 waren Fehler oder Wackler der Tests:**
  - F-27: Die Kürzelaufnahme blieb hängen und riss die Suite mit.
  - F-30: Wettlauf mit der Sitzungswiederherstellung.
  - F-31: Synthetisches Drag & Drop unter WebKitGTK. Die Abhilfe hilft auch bei echten Drags.

## Gefundene Widersprüche und Abweichungen

1. **Umfang** (ADR 0017, Punkt 1): „Mehrfachzuordnung für Dokumente und Medien“ – Medien entstehen laut Phasenplan erst in Phase 8. Ihre Zuordnungen kommen dort mit dem geplanten Neuaufbau von `subnet_memberships`. Die Filter der globalen Suche folgen in Phase 9.
2. **Schema abweichend vom Entwurf** in `docs/database.md`:
   - kein `sort_key`;
   - kein `archived_at`/`deleted_at` für Subnetze (sie sind keine Elemente);
   - dafür `name_key` mit eindeutigem Index je Ebene;
   - `subnet_memberships.document_id` bis Phase 8 `NOT NULL`.
3. **„Alle Dokumente“ entfällt** im Explorer. Abschnitt 13 nennt Subnetze, Favoriten, Zuletzt verwendet, Nicht zugeordnet, Medien, Archiv und Papierkorb; zusammen zeigen sie jedes Dokument.
4. **Struktur-Undo nur im Speicher:** Nach einem Neustart ist es leer. Dafür fragt das Löschen eines Subnetzes vorher; Stufe 5 (Backup, Phase 10) holt sonst Verlorenes zurück.
5. **„Flüssig“ ohne Zahlenwert im Auftrag:** Die Ziele stammen aus ADR 0017, Punkt 17, angelehnt an das Mesh (30 Bilder pro Sekunde).
6. **Ein Zweig je Phase** (Abschnitt 24): weiterhin nur der erlaubte Zweig.

## Entscheidungen (ADR)

0017 Subnetze, Zuordnungen, Tags und Struktur-Undo (18 Punkte), mit Nachträgen aus der Umsetzung.

## CI

Zwischenstände trugen `[skip ci]`. Den Abschluss prüft ein CI-Lauf unter Windows und Linux; Ergebnis siehe Nachtrag.

## Nächster Schritt

Nach Freigabe: Phase 8 (Medien; Abschnitt 25).
