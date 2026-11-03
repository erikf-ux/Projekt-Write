# ADR 0010 – Referenzbibliothek

- Status: angenommen
- Datum: 2026-11-03
- Phase: 2

## Kontext

Abschnitt 6: „Ein Skript in `scripts/` erzeugt sie reproduzierbar mit 10.000 Dokumenten, 5.000 Medien, 500 Subnetzen, 1.000 Tags und 30.000 bestätigten Verbindungen.“ Phase 2 verlangt das Skript; Medien, Subnetze, Tags und Verbindungen haben aber erst ab den Phasen 7, 8 und 11 Tabellen (Abschnitt 18: „Tabellen entstehen per Migration in der Phase, die sie braucht“).

## Entscheidungen

- `scripts/reference-library.mjs` baut und startet das Rust-Beispiel `src-tauri/examples/reference_library`. Das Beispiel erzeugt die Bibliothek über `Library::create` und das Dokument-Repository – dieselben Wege wie die App, nichts daran vorbei.
- **Reproduzierbar:** IDs (UUID v4 aus Zufallsbytes eines festen Generators), Zeitstempel (fester Start, gesäte Schritte), Titel und Texte stammen aus einem Startwert (`--seed`, Standard 2026). Ein Fingerabdruck (SHA-256 über alle Dateien außer `library.lock` und der Datenbankdatei, dazu alle Zeilen und die Schemaversion) belegt die Gleichheit; `--check` baut zweimal und vergleicht. CI prüft das (Linux volle Größe, Windows 1.000 Dokumente), ein Rust-Test eine kleine Bibliothek.
- **Inhalt Phase 2:** 10.000 Dokumente (Markdown mit Überschriften, Listen, Aufgabenlisten, Zitaten, Codeblöcken, Hervorhebungen; Reintext), darunter eines mit 50.000 Wörtern für die Zielwerte aus Abschnitt 6, deutsche Wörter mit Umlauten, `ß`, Komposita und Abkürzungen (`z. B.`, `Dr.`) für spätere Such- und Sprachprüfungstests. **Abweichung, offen benannt:** Medien, Subnetze, Tags und Verbindungen ergänzt das Skript in den Phasen 7, 8 und 11; `.pwrite`-Dokumente, sobald das Format feststeht (Phase 4/5).
- Ziel nur innerhalb des Projektordners (Abschnitt 2), Standard `src-tauri/target/reference-library`. Ein vorhandener Ordner wird nur mit `--force` ersetzt und nur, wenn `library.json` die Markierung `referenceLibrary` trägt.

## Folgen

- Gemessen (Linux-Container, Debug-Build): 10.000 Dokumente, 81 MB Text, 31,5 s.
- Jede Änderung am Generator ändert den Fingerabdruck; verglichen wird deshalb innerhalb eines Laufs, nicht gegen einen festgeschriebenen Wert.
