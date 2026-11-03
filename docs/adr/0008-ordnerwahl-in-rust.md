# ADR 0008 – Ordnerwahl in Rust, Tokens statt Pfaden

- Status: angenommen
- Datum: 2026-11-03
- Phase: 2

## Kontext

Abschnitt 8: „Befehle arbeiten mit IDs statt mit Pfaden. Das Frontend erhält Pfade nur als Anzeigetext. Für externe Dateien öffnet Rust den Systemdialog und gibt eine Sitzungs-ID zurück.“ Phase 2 braucht Ordnerdialoge für Anlegen, Öffnen und Umziehen einer Bibliothek. WebDriver kann native Dialoge nicht bedienen.

## Optionen

1. `tauri-plugin-dialog`: Das Plugin hängt von `tauri-plugin-fs` ab, das Abschnitte 7/20 ausschließen (der Konfigurationstest verbietet es ausdrücklich), und registriert eigene Befehle.
2. **`rfd` 0.17.2 direkt** (MIT): dieselbe Bibliothek, die das Plugin intern nutzt; GTK unter Linux, `IFileOpenDialog` unter Windows, `NSOpenPanel` unter macOS. Keine Befehle, keine Rechte für das Frontend.

## Entscheidung

- Option 2. `services/dialog.rs`: `SystemPicker` erzeugt den Dialog auf dem UI-Thread und wartet auf einem Hilfsthread (wie das Plugin), mit dem Hauptfenster als Elternfenster. Unter Linux bewusst das Feature `gtk3` statt `xdg-portal` (kein zusätzlicher D-Bus-Client nötig, GTK ist ohnehin geladen).
- `library_choose_location` und `library_default_location` liefern ein **Token** (32 Hex-Zeichen, zufällig), den Anzeigepfad, den Inhalt des Ordners (`missing`, `empty`, `library`, `other`) und die Einordnung des Orts. Rust merkt sich höchstens 16 Tokens. Schreibende Befehle (`library_create`, `library_move`) akzeptieren nur Tokens, die für diesen Zweck gewählt wurden; `library_open` jedes Token (ein Ordner, der sich als Bibliothek erweist, kann direkt geöffnet werden). Ein Pfad im Token-Feld ist `INVALID_INPUT`.
- Den Dialogtitel liefert das Frontend in der Sprache der Oberfläche (1–120 Zeichen, ohne Steuerzeichen).
- **Testnaht:** Debug-Builds mit dem Feature `e2e` lesen `PW_E2E_PICK_FOLDERS` (Pfadliste) und `PW_E2E_APP_DIRS` (Konfigurations-, Log- und Dokumentordner in der Sandbox). Release-Builds kennen diese Variablen nicht (`#[cfg(feature = "e2e")]`, das Feature ist im Release per `compile_error!` ausgeschlossen). Rust-Tests nutzen `ScriptedPicker` direkt.
- Der echte Dialog wird manuell geprüft (`tests/e2e/manual/`); unter Linux belegt (GTK-Dialog erscheint).

## Folgen

- Phase 5 nutzt dasselbe Muster für externe Dateien (Datei-Dialog, Sitzungs-ID).
- Unter Windows ist der echte Dialog bis zur ersten manuellen Prüfung auf einem Windows-Gerät nicht belegt (CI kann native Dialoge nicht bedienen).
