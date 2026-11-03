# ADR 0012 – Befehle, Tastenkürzel, natives Menü

- Status: angenommen
- Datum: 2026-11-03
- Phase: 3

## Kontext

Abschnitt 11 verlangt ein natives Tauri-Menü, eine Command Palette mit ausschließlich funktionierenden Befehlen, konfigurierbare Tastenkürzel mit Konflikterkennung und Kontextmenüs. Jede Funktion soll per Tastatur erreichbar sein, der Fokus sichtbar.

## Entscheidungen

1. **Ein Befehlsregister** (`src/modules/commands/`): Jeder Befehl hat eine ID (`document.new`), einen Titel (Sprachdatei), eine Kategorie, `isAvailable` und `run`. Palette, Kürzel, natives Menü, Toolbar, Activity Bar und Kontextmenüs rufen Befehle nur über ihre ID auf. Die Palette zeigt nur Befehle, deren `isAvailable` jetzt zutrifft. Menüeinträge nicht verfügbarer Befehle sind deaktiviert. Ein Test prüft, dass jeder Menüeintrag ein registrierter Befehl ist, der Compiler, dass die Menü-IDs aus Rust (`MenuCommand`, ts-rs) zu den Befehls-IDs passen.
2. **Tastenkürzel nur im Webview:** Ein Tastatur-Listener am Fenster (Capture-Phase) wertet alle Kürzel aus. Native Menü-Accelerators gibt es nicht. Begründung: Sonst würden Kürzel doppelt ausgelöst (Menü und Webview); nur der Webview-Weg ist konfigurierbar und in CI testbar. Unter Windows steht das Kürzel als Text nach einem Tabulator im Menü, wie Win32-Menüs es zeigen. GTK hat dafür keine Spalte; unter Linux zeigt das Menü nur den Titel, die Palette und die Einstellungen zeigen die Kürzel.
3. **Kanonische Form** `Ctrl+Alt+Shift+Taste`, in Rust und TypeScript gleich geprüft. Buchstaben stehen groß. `Shift` gilt nur bei Buchstaben und benannten Tasten (bei anderen Zeichen steckt es im Zeichen: `Ctrl+?`, `Ctrl+Plus`). Außer F1–F24 braucht jedes Kürzel `Ctrl` oder `Alt`, damit kein Kürzel das Tippen stört. Für die Texteingabe sind Kombinationen reserviert (Kopieren, Einfügen, Rückgängig, Wortsprünge). Unter macOS steht `Ctrl` für `Cmd`.
4. **Konflikte:** Benutzerkürzel ersetzen die Standardkürzel je Befehl; eine leere Liste entfernt alle Kürzel des Befehls. Ein Kürzel, das zwei Befehlen gehört, führt nichts aus und wird gemeldet: als Hinweis beim Drücken, in der Statusleiste und oben in den Kürzel-Einstellungen. Wer ein belegtes Kürzel zuweisen will, sieht vorher, wem es gehört, und kann ersetzen oder abbrechen. Konflikte entstehen so nur durch Handbearbeitung von `settings.json`.
5. **Browser-Funktionen des Webviews:** Tauri 2.12 kann die Browser-Kürzel von WebView2 nicht abschalten (wry kann es, Tauri reicht es nicht durch). Der Listener unterdrückt deshalb Neu laden (`F5`, `Ctrl+R`, das ungespeicherte Eingaben verwerfen würde), Drucken, Suchen, Quelltext und Verlaufsnavigation.
6. **Ereigniskanal statt Event-Rechten:** Menüklicks und Schließanfragen kommen über einen Tauri-Channel (`ui_subscribe`) ins Frontend. Die Capability bleibt bei den App-Befehlen; `core:event` ist nicht freigegeben.
7. **Schließen:** Rust hält das Schließen des Fensters an und fragt das Frontend. Das Frontend speichert und bestätigt mit `ui_close_response`. Ohne zuhörendes Frontend und beim zweiten Klick auf Schließen schließt das Fenster sofort, so ist niemand gefangen.
8. **Kein Bearbeiten-Menü:** Die vordefinierten Einträge (Kopieren, Einfügen …) senden unter Windows Tastendrücke. Unter Linux tun sie nur mit libxdo und nur unter X11 etwas, unter Wayland nie. Ein Menüeintrag ohne Wirkung ist nicht zulässig. Kopieren, Einfügen und Rückgängig funktionieren in der Schreibfläche über die Tastatur. Das Bearbeiten-Menü kommt mit dem Editor (Phase 4), der Undo selbst verwaltet.
9. **Barrierefreiheit:** Toolbar, Activity Bar und Tabs sind zusammengesetzte Widgets mit einem Tab-Stopp und Pfeiltasten (ARIA-Muster). Explorer und Palette nutzen `aria-activedescendant`. Kontextmenüs öffnen auch mit der Kontextmenütaste und `Shift+F10`. Dialoge halten den Fokus und geben ihn zurück; Escape schließt den innersten Dialog. Jeder Bereich hat einen sichtbaren Fokusring in der Akzentfarbe. Die Schreibfläche zeigt ihn als Leiste am Rand.

## Folgen

- Neue Befehle späterer Phasen werden registriert und erscheinen damit automatisch in Palette, Kürzel-Einstellungen und (falls vorgesehen) im Menü.
- Die Kürzel des Editors (Rückgängig, Suchen, Ersetzen, Quelltextansicht, Drucken) kommen mit Phase 4 als Befehle hinzu. Die heute unterdrückten Browser-Kürzel werden dann zu echten Befehlen.
- Echte Tastatureingaben (nicht synthetische) sind in CI nicht prüfbar (ADR 0003). Die E2E-Tests prüfen Struktur, Tastenbedienung über synthetische Ereignisse und den sichtbaren Fokus an Pixeln.
