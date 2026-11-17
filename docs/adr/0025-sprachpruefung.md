# ADR 0025 – Sprachprüfung: Ablauf, eigene Wörterbücher, Autokorrektur, Sprachen

- Status: angenommen
- Datum: 17. November 2026
- Phase: 12

## Kontext

Die Abschnitte 7 und 14 verlangen:

- eine Rechtschreibprüfung beim Tippen;
- einfache Regeln (doppeltes Wort, doppeltes Leerzeichen, Großschreibung am Satzanfang);
- eigene Wörterbücher;
- eine Autokorrektur, die nur in eindeutigen Fällen eingreift;
- eine Spracherkennung je Absatz;
- eine manuelle Sprache je Dokument und je Abschnitt.

Die Tipplatenz darf 50 ms (95 %) nicht überschreiten (Abschnitt 4). Prüfwerk und Wörterbücher legen ADR 0023 und ADR 0024 fest.

## Entscheidung

### 1. Ablauf der Prüfung

- **Wo geprüft wird:** Die Prüfung läuft in Rust, je Absatz (Befehl `language_check`).
- **Was der Editor schickt:** den Text des Absatzes. Inline-Code und Links ersetzt er durch U+FFFC, ein Zeichen je UTF-16-Einheit. So bleiben die Offsets gleich und diese Teile werden nicht geprüft.
- **Zwischenspeicher:** Das Ergebnis wird im Editor gespeichert, mit dem Schlüssel Dokument, Epoche, Sprache, geerbte Sprache und Text.
- **Was die Epoche erhöht:**
  - ein neues Wort im eigenen Wörterbuch;
  - eine Änderung der Einstellungen;
  - ein Wechsel des Sprachprofils;
  - der Start oder das Ende von LanguageTool.
- **Wann geprüft wird:** Nach jeder Eingabe werden nur geänderte Absätze in Stapeln neu geprüft, außerhalb der Eingabe.
- **Messung:**
  - Tipplatenz 95 % bei 34 ms;
  - Öffnen eines Dokuments mit 50.000 Wörtern in 268 bis 298 ms.

### 2. Fundstellen und Darstellung

- **Arten:** `spelling`, `grammar`, `punctuation`, `context`.
- **Darstellung:**
  - Rechtschreibung und Kontext: rot und wellig;
  - Grammatik und Zeichensetzung: blau und gestrichelt;
  - im Druck und im PDF-Export nicht sichtbar.
- **Kontextmenü:**
  - höchstens fünf Vorschläge, in der Reihenfolge aus Abschnitt 14;
  - „Zum Wörterbuch hinzufügen“ mit einem Klick;
  - „Ignorieren“ für die Sitzung;
  - zu öffnen mit Rechtsklick, der Kontextmenütaste oder `Shift + F10`.

### 3. Eigene Wörterbücher

- **Ort und Geltung:**
  - Sie gehören zur Bibliothek (Migration 0011) und gelten für alle Sprachen.
  - Sie werden einem Dokument, einem Subnetz oder der ganzen Bibliothek zugeordnet.
  - Ein Subnetz vererbt seine Wörterbücher an alle Unter-Subnetze (rekursive Abfrage).
- **Wortkategorien:** Wort, Fachbegriff, Name, Abkürzung. Die Kategorie dient der Ordnung in der Verwaltung; geprüft wird für alle gleich, wie bei Hunspell: Ein kleingeschriebenes Wort gilt auch großgeschrieben am Satzanfang, jedes Wort auch in Großbuchstaben.
- **Import und Export:** als Textdatei, ein Wort je Zeile.

### 4. Autokorrektur

- **Wann sie korrigiert:** nur, wenn die sieben Bedingungen aus Abschnitt 14 erfüllt sind.
- **Abweichung bei Bedingung 3 („genau ein Kandidat mit Abstand 1“):** `spellbook` liefert seine Vorschläge nicht in fester Reihenfolge und nicht vollständig. Deshalb erzeugt die App alle Kandidaten selbst und prüft jeden gegen das Wörterbuch. Kandidaten sind:
  - Löschen, Vertauschen, Ersetzen und Einfügen über das Alphabet der Affixdatei;
  - ausgeschriebene Umlaute und ß (`ae` → `ä`, `ss` → `ß`).
- **Abstand der Umlaute:** Ein ausgeschriebener Umlaut zählt als ein Schritt. Ohne diese Regel korrigierte das Korpus fünf Wörter falsch; mit ihr korrigiert es keines falsch.
- **Ablehnen:** Macht der Nutzer eine Korrektur sofort mit Rückgängig ungeschehen, gilt sie als abgelehnt und wird für diese Bibliothek nicht wiederholt. Die Einstellungen zeigen die Liste; einzelne Einträge lassen sich löschen.
- **Start:** Die Autokorrektur ist aus, bis der Nutzer sie auf dem Startbildschirm oder in den Einstellungen einschaltet (D5).

### 5. Sprachen

- **Wahl je Absatz**, in dieser Reihenfolge:
  1. markierter Abschnitt;
  2. Erkennung;
  3. die Sprache des vorigen Absatzes, nur bei kurzen Absätzen;
  4. Sprachprofil des Dokuments;
  5. Standardsprache aus den Einstellungen.
- **Erkennung:** `whatlang` 0.18.0. Ein Absatz braucht mindestens 40 Buchstaben und eine Sicherheit von mindestens 0,6. `is_reliable` von `whatlang` lehnte zu viele englische Absätze ab. Erkannt wird nur eine Sprache, für die ein Wörterbuch da ist.
- **Sprachprofil des Dokuments:** eine oder mehrere Sprachen, außerdem „Autokorrektur hier aus“.
  - Die erste Sprache ist die Hauptsprache.
  - Ein Wort in einer der weiteren Sprachen gilt als richtig.
  - Externe Dateien haben kein Profil; für sie gilt die Standardsprache.
- **Markierter Abschnitt:** Mark `lang` im Editor.
  - Markdown: `<span lang="xx">…</span>`.
  - `.pwrite`: dieselbe Mark; dafür steigt die Formatversion auf 2.
  - Eine Datei der Version 1 öffnet unverändert. Gespeichert wird immer die aktuelle Version.

## Alternativen

- **Prüfung im Frontend (WebAssembly):** Sie hätte Wörterbuch und Prüfwerk doppelt geladen und die Wörterbuchverwaltung nicht in Rust gehalten. Verworfen.
- **Vorschläge von `spellbook` für die Autokorrektur:** Sie sind nicht reproduzierbar; „Fesnter“ wurde einmal blockiert und einmal korrigiert. Verworfen.
- **Erkennung mit `lingua`:** genauer bei kurzen Texten, aber über 100 MB Modelle. Verworfen.
- **Mehrsprachigkeit nur über Profile, ohne markierte Abschnitte:** Zitate in einer anderen Sprache wären dann immer falsch markiert worden. Verworfen.

## Folgen

- Neue Abhängigkeit: `whatlang` 0.18.0 (MIT), Begründung in `docs/dependencies.md`.
- Migration 0011 mit fünf Tabellen (`docs/database.md`).
- `.pwrite` Formatversion 2 (`docs/pwrite-format.md`).
- Abnahme mit dem Korpus:
  - 0 von 1.198 richtigen Wörtern werden markiert;
  - 209 von 212 Fehlern werden gefunden;
  - die Autokorrektur korrigiert 106 Fehler und keinen falsch.
