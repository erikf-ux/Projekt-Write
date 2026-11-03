# Bericht Phase 3 – Oberfläche

Stand: 3. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig). Entwicklungsumgebung: Cloud-Container, Ubuntu 24.04, Rust 1.99.1, Node.js 22, Xvfb (kein Referenzgerät nach Abschnitt 6). CI: GitHub Actions `windows-latest` und `ubuntu-24.04`; macOS ist auf Wunsch des Auftraggebers zurückgestellt.

## Was wurde implementiert?

- **Hauptlayout** (Abschnitt 11): natives Menü (Datei, Ansicht), Toolbar, Activity Bar, Seitenleiste mit Explorer, Dokument-Tabs, Schreibfläche und Statusleiste. Die Maße folgen den Startwerten des Designs (Activity Bar 48 px, Seitenleiste 260 px, Toolbar und Tabs 36 px, Statusleiste 24 px).
- **Befehlsregister:** Menü, Toolbar, Activity Bar, Befehlspalette, Tastenkürzel und Kontextmenüs führen dieselben 25 Befehle aus. Die Palette (`Strg + Umschalt + P`) zeigt nur, was gerade funktioniert; nicht verfügbare Menüeinträge sind deaktiviert.
- **Tastenkürzel:** Standardkürzel nach Abschnitt 11, soweit ihre Funktionen in Phase 3 existieren. Jedes Kürzel ist in den Einstellungen änderbar, entfernbar und zurücksetzbar. Ein belegtes Kürzel wird vor dem Zuweisen gemeldet. Konflikte aus `settings.json` erscheinen in der Statusleiste und in den Einstellungen; ein Kürzel mit Konflikt führt nichts aus und meldet sich beim Drücken. Kombinationen der Texteingabe sind reserviert. Browser-Funktionen des Webviews wie Neu laden und Drucken sind unterdrückt.
- **Explorer:** virtualisierte Liste der Bibliotheksdokumente. Pfeiltasten, Enter öffnet, `F2` benennt in der Zeile um, Kontextmenü mit Maus, Kontextmenütaste oder `Shift + F10`.
- **Tabs:** mehrere Dokumente, Tastatur (Pfeile, Entf), Kontextmenü, Mittelklick, Verschieben per Tastatur und Drag & Drop, Fehlermarke bei gescheitertem Speichern.
- **Schreibfläche der Phase 3:** Text und Markdown-Quelltext in einem Textfeld je Tab. Der Undo-Verlauf bleibt, solange der Tab offen ist. Zeilen höchstens 72 Zeichen (abschaltbar). Autosave nach Abschnitt 9 mit „Gespeichert“, „Speichert …“ oder „Fehler beim Speichern“. Tiptap ersetzt das Textfeld in Phase 4.
- **Drag & Drop:** Abgelegte `.txt`- und `.md`-Dateien werden importiert. Rust prüft Endung und Inhalt (UTF-8 ohne Steuerzeichen, BOM entfernt, LF). Bei gleichem Inhalt fragt die App; der Originalname wird gespeichert (Migration 0003). Ordner werden abgelehnt.
- **Designs:** wie das System, hell, dunkel. Importierte Design-Dateien prüft Rust samt aller 16 Kontrastpaare (≥ 4,5:1), entfernen ist möglich.
- **Seitenleiste:** Breite per Maus oder Pfeiltasten, ein- und ausblenden.
- **Fokusmodus** (`F11`): Vollbild ohne Menüleiste, nur die Schreibfläche; `F11` oder Esc beendet ihn. **Oberflächenzoom** 50–200 %.
- **Erster Start:** Bibliotheksort und Sprache, danach öffnet sich ein leeres Dokument.
- **Sprachdateien** Deutsch und Englisch; Rust bestimmt die Sprache für Menü und Webview (Einstellung oder Systemsprache).
- **App-Einstellungen:** Sprache, Design, Zoom, Seitenleiste, Zeilenbreite, Tastenkürzel und Fensterzustand in `settings.json`. Jeder Wert wird einzeln geprüft; ein ungültiger Wert macht die Datei nicht unbrauchbar.
- **Beenden:** Erst wird alles gespeichert. Scheitert das, entscheidet der Nutzer: weiterarbeiten oder trotzdem beenden.
- **Befehle:** 16 neue (Oberfläche, Ereigniskanal, Menü, Fokusmodus, Schließen, Dokumente, Import, Designs), jeweils mit Tests für gültige und ungültige Eingaben; 12 neue Fehlercodes mit deutschen und englischen Meldungen.

## Welche Dateien wurden erstellt oder verändert?

Rust, neu: `services/settings/ui.rs`, `services/locale.rs`, `services/ui/` (`mod.rs`, `menu.rs`, `window.rs`), `services/themes.rs`, `services/import.rs`, `commands/ui.rs`, `commands/documents.rs`, `migrations/0003_document_import.sql`. Geändert: `lib.rs`, `state.rs`, `error.rs`, `commands/mod.rs`, `services/settings/app.rs`, `services/session.rs`, `services/documents.rs`, `database/documents.rs`, `filesystem/windows.rs`, `filesystem/mod.rs`, `build.rs`, `Cargo.toml`, `tauri.conf.json`, `capabilities/main-window.json`, Tests `tests/{ipc,migrations}.rs`.

Frontend, neu: `app/{Workbench.tsx,commands.ts,bindings.ts,lifecycle.ts,focus.ts,ClosePrompt.tsx}`, `components/` (`Dialog`, `ContextMenu`, `NoticeBar`, `errorText`, `layout/{Sidebar,StatusBar}`, `toolbar/IconToolbar`, `navigation/ActivityBar`, `tabs/TabBar`), `modules/commands/` (`types`, `keymap`, `registry`, `shortcuts`, `CommandPalette`), `modules/documents/` (`workspaceStore`, `Explorer`, `importFiles`, `DropZone`), `modules/editor/PlainTextEditor.tsx`, `modules/settings/` (`uiStore`, `SettingsDialog`, `GeneralSettings`, `ShortcutSettings`) samt Tests. Geändert: `app/App.tsx`, `app/StartScreen.tsx`, `main.tsx`, `services/ipc.ts`, `modules/themes/applyTheme.ts`, `i18n/{de,en}.json`, `test/setup.ts`.

E2E: neu `tests/e2e/specs/workbench.e2e.ts`, `tests/e2e/pixels.ts`; geändert `wdio.conf.ts` (Reihenfolge), `session.ts`, `specs/{smoke,library,restart}.e2e.ts`. Skripte: `scripts/reference-library.mjs`. Doku: ADRs 0011–0013, `docs/architecture.md`, `docs/database.md`, `docs/dependencies.md`, `docs/status.md`, dieser Bericht, `CHANGELOG.md`, `README.md`.

## Welche Tests wurden durchgeführt?

Umfang: **213 Rust-Tests** (127 Unit, 86 Integration; dazu 2 Hilfsprozesse), **156 Vitest-Tests**, **26 WebdriverIO-Tests** (Phase 2: 146, 78, 14).

CI-Ergebnis: siehe Abschnitt „CI“ unten.

Lokal (Linux-Container, Xvfb, D-Bus-Sitzung), alle ausgeführt und bestanden:

| Prüfung                                                                                              | Ergebnis                                                                                       |
| ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Prettier, ESLint, TypeScript, Vitest (156)                                                           | bestanden                                                                                      |
| rustfmt, Clippy `-D warnings` (Linux, Linux + `e2e`, Windows-Ziel `x86_64-pc-windows-gnu`)           | bestanden (Windows-Ziel erst nach F-7)                                                         |
| Rust-Tests (213), darunter die 100 harten Abbrüche                                                   | bestanden                                                                                      |
| Referenzbibliothek `--check` (300 Dokumente)                                                         | bestanden                                                                                      |
| E2E (26): Rauchtest Phase 1 (7), Bibliothek (4), Neustart und zweite Instanz (3), Arbeitsfläche (12) | bestanden                                                                                      |
| Sichtprüfung per Bildschirmfoto (hell, dunkel, Palette, Einstellungen)                               | Layout wie geplant; zwei Schönheitsfehler behoben (Fokusleiste mittig, Mac-Symbol der Palette) |

## Abnahmekriterien Phase 3

| Kriterium                                                            | Stand                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Drag & Drop ist auf allen drei Betriebssystemen getestet             | **teilweise.** Unter Windows und Linux in CI (`workbench.e2e.ts`): Import abgelegter Dateien samt Duplikat-Rückfrage und abgelehntem PNG, Tabs verschieben. Die Ereignisse sind synthetische HTML5-Ereignisse mit echten `File`-Objekten, also der Weg, den ein Ablegen aus dem System mit `dragDropEnabled: false` nimmt. Ein echtes Ziehen aus dem Dateimanager ist in CI nicht automatisierbar. **macOS ist nicht getestet** (zurückgestellt).                                                                                                                   |
| Alle Funktionen sind per Tastatur erreichbar, der Fokus ist sichtbar | **erfüllt im prüfbaren Umfang.** E2E: Jeder Knopf ist ein Tab-Stopp oder Teil eines zusammengesetzten Widgets mit genau einem Tab-Stopp. Explorer, Tabs, Palette, Kontextmenü, Einstellungen und Kürzel-Editor werden per Tastatur bedient. Der Fokusring von Toolbar, Activity Bar, Explorer, Trenner, Tab und Schreibfläche ist an Pixeln gemessen; den Startbildschirm deckt der Rauchtest ab. Echte Tab-Tastendrücke kann der eingebettete WebDriver nicht senden (ADR 0003). Das native Menü ist mit Alt/F10 erreichbar (Betriebssystem), in CI nicht prüfbar. |
| Die Command Palette zeigt nur funktionierende Befehle                | **erfüllt.** Unit-Tests der Verfügbarkeit (ohne Bibliothek, ohne Tabs, ohne ungespeicherten Text, bereits gesetzte Einstellungen); E2E: Speichern und Originalgröße fehlen, wenn sie nichts täten. Jeder Menüeintrag ist ein registrierter Befehl.                                                                                                                                                                                                                                                                                                                  |
| Kürzelkonflikte werden gemeldet                                      | **erfüllt.** E2E: Ein belegtes Kürzel wird vor dem Zuweisen gemeldet. Ein Konflikt aus `settings.json` erscheint in Statusleiste und Einstellungen; das Kürzel führt nichts aus und meldet sich beim Drücken. Unit-Tests der Konflikterkennung.                                                                                                                                                                                                                                                                                                                     |
| Lint, Typprüfung und alle Tests in CI auf Windows, macOS und Linux   | siehe CI; macOS zurückgestellt                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Jeder neue Befehl hat Tests für gültige und ungültige Eingaben       | erfüllt (`tests/ipc.rs`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Keine sichtbare Funktion ohne Wirkung; Texte de/en                   | erfüllt; siehe Abweichungen                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |

## Was konnte nicht getestet werden und warum?

- **macOS:** zurückgestellt; nicht gebaut, nicht getestet.
- **Echtes Drag & Drop aus dem Dateimanager** und **echte Tastatureingaben** (Tab-Taste): CI und der eingebettete WebDriver senden nur synthetische Ereignisse. Getestet ist der Code-Weg dahinter.
- **Native Menüklicks:** WebDriver bedient keine nativen Menüs. Getestet sind die Weiterleitung über den Kanal (Rust, echter `Channel`), die Zuordnung der Menü-IDs zu Befehlen (Compiler, Unit-Test) und die Befehle selbst.
- **Vollbild und ausgeblendete Menüleiste im Fokusmodus:** Die Bildschirmfotos zeigen nur den Webview. Getestet ist, dass alles außer der Schreibfläche verschwindet.
- **Echte WebView2-Browser-Kürzel unter Windows** (Neu laden, Drucken): Unterdrückt werden sie über die Ereignisse im Webview, getestet mit synthetischen Tasten; ob WebView2 jede Taste vor der Seite verarbeitet, ist so nicht prüfbar.
- **Messungen auf dem Referenzgerät:** nicht verfügbar.

## Offene Fehler

Keine bekannten.

## Behobene Fehler

F-6 bis F-10 in `docs/status.md`. F-7 (Windows-Build) und F-8/F-9 (Escape im Dialog, unsichtbarer Tab-Fokusring) waren echte Fehler, gefunden von Windows-Clippy und den E2E-Tests vor der Weitergabe. F-6 und F-10 betrafen nur die CI.

## Gefundene Widersprüche und Abweichungen

1. **Phase-3-Umfang und „Funktionen späterer Phasen bleiben unsichtbar“** (Abschnitt 2 gegen Abschnitt 11 und 25). Aufgelöst nach Abschnitt 2 (ADR 0011):
   - Die Activity Bar zeigt Explorer und Schreibfläche, nicht Suche und Mesh.
   - Der erste Start fragt Bibliotheksort und Sprache, nicht die Autokorrektur (D5 gilt für die fertige App unverändert; die Frage kommt mit Phase 12).
   - Beispiel-Subnetze fehlen bis Phase 7.
   - Es gibt kein Löschen, solange es kein Wiederherstellen gibt.
2. **Schreibfläche vor dem Editor:** Tabs, Fokusmodus und Drag & Drop brauchen Dokumente. Phase 3 bringt deshalb eine schlichte Text-Schreibfläche mit Autosave; Phase 4 ersetzt sie durch Tiptap (ADR 0011).
3. **Migration 0003 vorgezogen:** `documents.original_name` war für Phase 5 geplant, der Import per Drag & Drop braucht es jetzt. `import_folder` bleibt in Phase 5, weil abgelegte Dateien keinen Ordner verraten.
4. **Import-Schritt „nach `tmp/` kopieren“** (Abschnitt 9): Die Temporärdatei liegt im Zielordner und wird atomar umbenannt; die Wirkung ist gleich (ADR 0011).
5. **Kein Bearbeiten-Menü:** Die vordefinierten Einträge wirken unter Linux nur mit libxdo unter X11. Kopieren, Einfügen und Rückgängig gehen über die Tastatur (ADR 0012).
6. **Keine nativen Menü-Accelerators:** Kürzel laufen einmal im Webview, konfigurierbar und testbar. Unter Windows steht das Kürzel als Text im Menü, unter Linux nur in Palette und Einstellungen (ADR 0012).
7. **Kodierungen beim Import:** nur UTF-8; UTF-16 und Windows-1252 kommen mit den externen Dateien (Phase 5).
8. **Ein Zweig je Phase** (Abschnitt 24): weiterhin nur der erlaubte Zweig.

## Entscheidungen (ADR)

0011 Umfang der Oberfläche in Phase 3 · 0012 Befehle, Tastenkürzel, natives Menü · 0013 Importierbare Designs und Oberflächeneinstellungen.

## CI

Der CI-Lauf zum letzten Commit läuft noch; das Ergebnis wird hier eingetragen.

## Nächster Schritt

Nach Freigabe: Phase 4 (Editor, Abschnitte 10 und 12): Tiptap mit Rich Text und Markdown-Adapter, Quelltextansicht, Laden und Speichern über den Speicherkern in den vorhandenen Tabs, Schreibstatistik, Suchen und Ersetzen; dazu Bearbeiten-Menü und Editor-Kürzel als Befehle.
