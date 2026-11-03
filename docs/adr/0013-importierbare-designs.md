# ADR 0013 – Importierbare Designs und Oberflächeneinstellungen

- Status: angenommen
- Datum: 2026-11-03
- Phase: 3

## Kontext

Abschnitt 11: „Anpassbare Themes über Design-Tokens und importierbare Theme-Dateien“, Dark und Light passend zum System, Oberflächenzoom, anpassbare Seitenleisten, Zeilenbreite höchstens 72 Zeichen (abschaltbar). Abschnitt 9: Fensterzustand, Sprache und Tastenkürzel liegen in den App-Einstellungen.

## Entscheidungen

1. **Design-Datei:** JSON mit `formatVersion` 1, `name`, `base` (`dark` oder `light`, bestimmt die Farbgebung der nativen Bedienelemente) und allen zwölf Farb-Tokens aus `tokens.ts` als `#RRGGBB`. Unbekannte Felder werden abgelehnt (keine Schriften, Bilder oder Skripte aus einer Design-Datei).
2. **Prüfung in Rust:** Format, Name (1–60 Zeichen) und die 16 Kontrastpaare des Token-Tests aus Phase 1, jedes mindestens 4,5:1: Text, gedämpfter Text, Akzent, Fehler und Erfolg auf Schreibfläche und Seitenleiste, die Button-Beschriftung auf dem Akzent, die Cluster-Farben auf der Schreibfläche. Die Meldung nennt das Paar und den erreichten Wert.
3. **Speicherort:** eine normalisierte Kopie in `themes/<id>.json` im Konfigurationsordner, höchstens 50. Von Hand verdorbene Dateien werden übersprungen und geloggt, nie gelöscht. Der Import macht das Design sofort aktiv; wird das aktive Design entfernt, gilt wieder „wie das System“.
4. **Auswahl der Datei:** über ein Datei-Eingabefeld des Webviews (nativer Dialog). Nur der Text der Datei geht an Rust, nie ein Pfad. Das Frontend erhält keine Dateirechte.
5. **Anwenden:** Das Frontend erzeugt aus den geprüften Farben eine Regel `[data-theme="custom"]` und prüft das Format vor dem Einsetzen noch einmal.
6. **Oberflächeneinstellungen** stehen flach in `settings.json` neben `libraryPath`: `language`, `theme`, `zoom` (0,5–2), `sidebar` (sichtbar, Breite 160–640 px), `limitLineWidth`, `keybindings` und `window` (nur Rust). Jeder Wert wird beim Laden einzeln geprüft. Ein ungültiger Wert bekommt seinen Standard; die Datei gilt deshalb nicht als beschädigt, und der Bibliothekspfad bleibt erhalten. Unbekannte Felder neuerer Versionen bleiben unverändert.
7. **Sprache:** Rust bestimmt sie einmal für Menü und Webview. Bei „System“ nimmt es unter Windows die Anzeigesprache, sonst die gettext-Variablen (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`). Die Menütexte stammen aus denselben Sprachdateien wie die Oberfläche.
8. **Fenster:** Es startet unsichtbar und wird nach dem Wiederherstellen von Größe, Position und Maximiert-Zustand gezeigt. Eine Position, die auf keinem Bildschirm mehr greifbar ist, wird verworfen.

## Folgen

- Der Zoom wirkt über `WebviewWindow::set_zoom`, nicht über CSS, und gilt damit für die ganze Oberfläche.
- Weitere Tokens späterer Phasen brauchen eine neue `formatVersion` der Design-Datei oder Standardwerte je Basis.
