# Project Write

Lokale, plattformübergreifende Schreib- und Wissensmanagement-Anwendung für Windows, macOS und Linux. Keine Cloud, keine Konten, keine Telemetrie.

> Stand: **Phase 6 – Wiederherstellung**. Der vollständige Auftrag steht in [`docs/master-prompt.md`](docs/master-prompt.md), der aktuelle Stand in [`docs/status.md`](docs/status.md).

## Voraussetzungen

- Rust (stable, ≥ 1.89), Node.js 22
- Linux: `libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev` (für E2E zusätzlich `xvfb` und `dbus`)
- Windows: WebView2 (in Windows 10/11 enthalten), Visual Studio Build Tools
- macOS: Xcode Command Line Tools

## Befehle

| Zweck                                          | Befehl                                                                                                                                                   |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Abhängigkeiten installieren                    | `npm ci`                                                                                                                                                 |
| App im Entwicklungsmodus                       | `npm run tauri dev`                                                                                                                                      |
| Alle Frontend-Prüfungen                        | `npm run check` (Prettier, ESLint, TypeScript, Vitest)                                                                                                   |
| Rust-Tests (erzeugt auch die TypeScript-Typen) | `cargo test --manifest-path src-tauri/Cargo.toml`                                                                                                        |
| Debug-Build für E2E                            | `npm run build:e2e`                                                                                                                                      |
| E2E-Tests                                      | `npm run test:e2e` (Linux ohne Bildschirm: `dbus-run-session -- xvfb-run -a npm run test:e2e`)                                                           |
| Echten Ordnerdialog prüfen (manuell)           | `npx wdio run tests/e2e/manual/wdio.manual.conf.ts` (unter Linux mit `xvfb-run`; speichert ein Bildschirmfoto)                                           |
| Referenzbibliothek erzeugen                    | `node scripts/reference-library.mjs` (Standard: 10.000 Dokumente in `src-tauri/target/reference-library`; `--seed`, `--documents`, `--force`, `--check`) |
| Lizenzliste aktualisieren                      | `node scripts/third-party-licenses.mjs`                                                                                                                  |
| Release-Binary                                 | `npx tauri build --no-bundle`                                                                                                                            |
| App-Icons neu erzeugen                         | `node scripts/app-icons.mjs` (Quelle `src-tauri/icons/app-icon.png`, 256 × 256 px)                                                                       |

## Dokumentation

- [Architektur](docs/architecture.md)
- [Datenbank](docs/database.md)
- [Abhängigkeiten](docs/dependencies.md)
- [Architekturentscheidungen](docs/adr/README.md)
- [Changelog](CHANGELOG.md)
- [Lizenzen Dritter](THIRD-PARTY-LICENSES)

## Die Bibliothek

Project Write speichert Dokumente in einer **Bibliothek**: einem Ordner mit Datenbank und Dateien (Standard `Dokumente/ProjectWrite`). Eine Bibliothek ist immer nur in einer Instanz geöffnet. Liegt sie in einem Sync-Ordner (OneDrive, Dropbox, iCloud Drive, Google Drive) oder auf einem Netzlaufwerk, warnt die App und arbeitet dort im Rollback-Modus der Datenbank; empfohlen ist ein lokaler Ort. Ein Umzug kopiert, prüft und schaltet erst dann um; der alte Ordner bleibt, bis Sie ihn entfernen.

## Bedienung

Beim ersten Start wählen Sie den Ort der Bibliothek und die Sprache; danach öffnet sich ein leeres Dokument. Dokumente der Bibliothek speichern von selbst (1 s nach der letzten Eingabe, beim Tab-Wechsel und beim Schließen); die Statusleiste zeigt „Gespeichert“. Text- und Markdown-Dateien, die Sie auf das Fenster ziehen, werden in die Bibliothek importiert; das Original bleibt unverändert.

Markdown-Dokumente bearbeiten Sie als Rich Text; Markdown-Kürzel wie `# ` oder `**fett**` wirken beim Tippen, `Strg + /` zeigt den Markdown-Quelltext. Gespeichert wird immer Markdown. Was Markdown nicht verlustfrei halten kann, meldet der Editor vor dem Speichern; ein Dokument, das er nicht verlustfrei darstellen kann, öffnet in der Quelltextansicht und nennt den Grund. Die Statusleiste zeigt die Wörter und die Lesezeit, ein Klick darauf die ganze Statistik. Bilder erscheinen als Hinweis mit Adresse und werden nie geladen. Weblinks öffnen Sie mit `Strg + Klick` im Systembrowser.

Neben Text (`.txt`) und Markdown (`.md`) gibt es Rich-Text-Dokumente im Format `.pwrite`; Dokumente lassen sich kopieren, als Favorit markieren und zwischen den Formaten umwandeln (die App nennt vorher, was verloren ginge). `[[` verlinkt ein anderes Dokument der Bibliothek; die Seitenleiste „Verbindungen“ zeigt, welche Dokumente auf das aktuelle verweisen.

Nichts geht verloren, ohne dass Sie es wollen (vier Stufen): Beim nächsten Start sind die Tabs der letzten Sitzung wieder offen, mit Ansicht und Auswahl. Nach einem Absturz bietet die App an, was seit dem letzten Speichern getippt wurde (es fehlen höchstens die letzten ein bis zwei Sekunden). Gelöschte Dokumente liegen im Papierkorb des Explorers, bis Sie ihn leeren oder 30 Tage um sind; „Wiederherstellen“ bringt sie mit allen Verbindungen zurück. Ältere Stände eines Dokuments zeigt „Versionen …“ in der Befehlspalette, neben dem aktuellen Stand; „Version speichern …“ legt einen benannten Stand an, der bleibt. Vor jeder Wiederherstellung sichert die App den aktuellen Stand als Version. Dokumente, die Sie gerade nicht brauchen, legen Sie ins Archiv. Wie lange Papierkorb und Versionen aufbewahrt werden, stellen Sie unter Einstellungen › Bibliothek ein.

Dateien außerhalb der Bibliothek öffnen Sie mit `Strg + O` (oder per Doppelklick auf eine `.pwrite`-Datei). Kodierung und Zeilenenden bleiben, wie sie sind; gespeichert wird mit `Strg + S` (automatisch nur, wenn Sie es unter Einstellungen › Allgemein einschalten). Ändert ein anderes Programm die Datei, fragt die App, welche Fassung gelten soll.

| Aktion                                 | Kürzel (macOS: `Cmd` statt `Strg`)             |
| -------------------------------------- | ---------------------------------------------- |
| Befehlspalette                         | `Strg + Umschalt + P`                          |
| Neues Dokument, Speichern              | `Strg + N`, `Strg + S`                         |
| Datei öffnen, Speichern unter          | `Strg + O`, `Strg + Umschalt + S`              |
| Dokument verlinken                     | `[[` tippen                                    |
| Tab schließen, nächster/vorheriger     | `Strg + W`, `Strg + Bild ab/auf`               |
| Dokument umbenennen                    | `F2`                                           |
| Dokument in den Papierkorb             | `Entf` im Explorer                             |
| Explorer, Seitenleiste ein/aus         | `Strg + Umschalt + E`, `Strg + Umschalt + B`   |
| Fokusmodus                             | `F11` (Esc beendet ihn)                        |
| Quelltextansicht ein/aus               | `Strg + /`                                     |
| Suchen, Ersetzen                       | `Strg + F`, `Strg + H`                         |
| Nächster, vorheriger Treffer           | `F3`, `Umschalt + F3`                          |
| Fett, Kursiv, Unterstrichen, Code      | `Strg + B`, `Strg + I`, `Strg + U`, `Strg + E` |
| Link setzen                            | `Strg + K`                                     |
| Rückgängig, Wiederholen                | `Strg + Z`, `Strg + Y`                         |
| Als reinen Text einfügen               | `Strg + Umschalt + V`                          |
| Vergrößern, Verkleinern, Originalgröße | `Strg + +`, `Strg + -`, `Strg + 0`             |
| Einstellungen                          | `Strg + ,`                                     |

Alle Kürzel außer denen der Texteingabe (Rückgängig, Wiederholen, Einfügen) lassen sich unter Einstellungen › Tastenkürzel ändern; Konflikte werden gemeldet. Eigene Designs (JSON-Dateien mit allen Farben, Kontrast mindestens 4,5:1) importieren Sie unter Einstellungen › Allgemein.

## Datenschutz

Project Write arbeitet vollständig lokal. Die Bibliothek ist in Version 1.0 nicht verschlüsselt; empfohlen wird die Datenträgerverschlüsselung des Betriebssystems (BitLocker, FileVault, LUKS).
