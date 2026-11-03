# ADR 0011 – Umfang der Oberfläche in Phase 3

- Status: angenommen
- Datum: 2026-11-03
- Phase: 3

## Kontext

Phase 3 verlangt Hauptlayout, Explorer, Tabs, Drag & Drop, ersten Start und Fokusmodus (Abschnitt 25). Abschnitt 2 verlangt zugleich: „Was die Oberfläche anbietet, funktioniert in der jeweiligen Phase vollständig. Funktionen späterer Phasen bleiben unsichtbar.“ Dokumente bearbeitet erst der Editor (Phase 4), die Dokumentverwaltung folgt in Phase 5, Suche in Phase 9, Mesh in Phase 11, Autokorrektur in Phase 12, Subnetze in Phase 7.

## Entscheidungen

1. **Activity Bar:** Explorer und Schreibfläche. Suche und Mesh erscheinen mit ihren Phasen. Abschnitt 11 („schaltet zwischen allen vier um“) ist damit nach Phase 11 erfüllt.
2. **Erster Start:** Bibliotheksort, dann Sprache (Reihenfolge wie in Abschnitt 11). Die Frage nach der Autokorrektur (D5) kommt mit der Autokorrektur in Phase 12. Für die fertige App gilt D5 unverändert, also aus und beim ersten Start gefragt. Beispiel-Subnetze („optional“) kommen mit den Subnetzen. Nach dem ersten Start öffnet sich ein leeres Dokument, wenn die Bibliothek neu und leer ist.
3. **Schreibfläche:** Tabs, Explorer, Fokusmodus und Drag & Drop brauchen Dokumente. Phase 3 bringt deshalb eine Reintext-Schreibfläche (ein Textfeld je Tab) für Bibliotheksdokumente: TXT als Text, MD als Quelltext. Tiptap ersetzt sie in Phase 4. Das Textfeld bleibt je Tab eingehängt, so lebt der Undo-Verlauf, solange der Tab offen ist (Abschnitt 12). Rechtschreibhinweise des Webviews sind aus, bis die Sprachprüfung kommt (Phase 12).
4. **Speichern:** Bibliotheksdokumente speichern wie in Abschnitt 9 festgelegt: Autosave 1 s nach der letzten Eingabe, beim Tab-Wechsel, bei Fokusverlust und beim Schließen. Die Statusleiste zeigt „Gespeichert“, „Speichert …“ oder „Fehler beim Speichern“. Schlägt das Speichern fehl, bleibt die Änderung im Speicher und der Tab offen. Beim Beenden wird erst gespeichert; scheitert das, entscheidet der Nutzer. Recovery-Entwürfe kommen mit Phase 6.
5. **Dokumentfunktionen in Phase 3:** neu (MD), öffnen, bearbeiten, umbenennen. Kopieren, Papierkorb samt Wiederherstellen, Favoriten, Zuletzt verwendet und externe Dateien folgen in Phase 5. Ohne Wiederherstellen bietet Phase 3 auch kein Löschen an.
6. **Drag & Drop:** Dateien aus dem System werden importiert (Abschnitt 11). Die Positivliste nach Endung umfasst `.txt`, `.md` und `.markdown`. Rust prüft den Inhalt: UTF-8 ohne NUL und Steuerzeichen, ein UTF-8-BOM wird entfernt, die Zeilenenden werden LF. Die Grenze liegt bei 10 MiB. Bei gleichem Inhalt (SHA-256) fragt die App: vorhandenes verwenden, trotzdem importieren oder überspringen. Der Originalname wird gespeichert (Migration 0003, vorgezogen aus Phase 5). Ordner werden abgelehnt; Ordnerimport und Importordner kommen später. Der Webview liefert nur `File`-Objekte ohne Pfad; Rust bekommt den Inhalt (Base64), nie einen Pfad. Tabs lassen sich per Drag & Drop und per Tastatur verschieben.
7. **Weitere Formate beim Import:** UTF-16 und Windows-1252 werden noch abgelehnt (mit Meldung). Die Erkennung von Kodierungen kommt mit den externen Dateien (Phase 5).

## Abweichungen

- Abschnitt 9, Import Schritt 2 („nach `tmp/` kopieren“): Die Temporärdatei liegt im Zielordner und wird atomar umbenannt. Die Wirkung ist gleich: keine halben Dateien, Aufräumen bei Fehlern.
- Kein Bearbeiten-Menü: siehe ADR 0012.

## Folgen

- Phase 4 ersetzt das Textfeld durch Tiptap. Explorer, Tabs, Autosave und Statusleiste bleiben.
- Phase 5 ergänzt die Dokumentfunktionen. Mit dem Papierkorb und seinem Wiederherstellen kommt auch das Löschen in Explorer und Kontextmenü.
