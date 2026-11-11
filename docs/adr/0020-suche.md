# ADR 0020 – Suche: Index, Anfragen, Filter und Aktualität

- Status: angenommen
- Datum: 11. November 2026
- Phase: 9

## Kontext

Phase 9 umfasst laut Abschnitt 25: globale Suche, SQLite FTS5 mit Wort- und Teilwortindex, Schreibvarianten, Filter, Ergebnisnavigation. Abnahme: erste Treffer in der Referenzbibliothek innerhalb des Zielwerts (Abschnitt 6: 300 ms); `Muller` und `Mueller` finden `Müller`, `Strasse` findet `Straße`, `haus` findet `Krankenhaus` und `Häuser`, `C++` und ein einzelnes `"` lösen keinen Fehler aus; archivierte Elemente erscheinen nur über den Filter; der Index stimmt nach Bearbeiten, Löschen und Wiederherstellen.

Abschnitt 19 legt die beiden FTS5-Tabellen fest (`search_words` mit `unicode61 remove_diacritics 2` und `prefix = '2 3'`, `search_parts` mit `trigram remove_diacritics 1`), dazu das Quoten jedes Worts, die Schreibvarianten, die Reihenfolge (Wörter vor Teilwörtern) und die Startgewichte von `bm25()`. Der Index wird in derselben Transaktion wie die Daten aktualisiert; die Einstellungen bieten „Suchindex neu aufbauen“. Phase 1 hat geprüft, dass FTS5 in der mitgebauten SQLite aktiv ist und beide Definitionen funktionieren.

## Entscheidungen

1. **Schema (Migration 0009).** `search_entries` ordnet jeder FTS-Zeile genau ein Element zu: ein Dokument, ein Medium oder ein Subnetz (`CHECK` auf genau eine der drei Spalten, je eindeutig, `ON DELETE CASCADE`). Ihre `id` ist die `rowid` in `search_words` und `search_parts`. Die beiden FTS5-Tabellen stehen wörtlich wie in Abschnitt 19 (ein Test vergleicht sie mit den Definitionen der Phase-1-Probe). Normale FTS5-Tabellen mit eigenem Inhalt, weil Textausschnitte (`snippet()`) den Text brauchen; der Text der Dokumente steht dadurch zweimal in der Datenbank (Wort- und Teilwortindex). Gemessen wird das mit der Referenzbibliothek (siehe Folgen).

2. **Was im Index steht** (Abschnitt 19: Titel, Inhalte, Tags, Mediennamen, Subnetze, Metadaten):

   | Spalte    | Dokument                      | Medium                          | Subnetz                 |
   | --------- | ----------------------------- | ------------------------------- | ----------------------- |
   | `title`   | Titel                         | Titel                           | Name                    |
   | `body`    | Text aus der Datei (Punkt 4)  | Beschreibung                    | Beschreibung            |
   | `tags`    | Namen der Tags                | Namen der Tags                  | –                       |
   | `subnets` | Namen der Subnetze            | Namen der Subnetze              | Name des übergeordneten |
   | `meta`    | Dateiname beim Import, Format | Dateiname, Importordner, Endung | –                       |

   `search_parts` hält Titel und Inhalt. Subnetze sind eigene Treffer, weil Abschnitt 19 sie als Suchbereich nennt; ihr Name steht zusätzlich bei jedem Element darin. Text in PDFs und Office-Dateien wird nicht indexiert (Abschnitt 17 verlangt es nicht; eine spätere Erweiterung).

3. **Aktualität über Trigger, in derselben Transaktion.** Alles, was die Datenbank selbst weiß, halten Trigger aktuell: Anlegen, Titel, Dateiname und Format von Dokumenten; Titel, Beschreibung und Namen von Medien; Tags vergeben, entziehen, umbenennen, löschen (auch über `ON DELETE CASCADE`); Zuordnungen und Namen von Subnetzen; Löschen eines Eintrags entfernt beide FTS-Zeilen. Drei Ansichten (`search_document_fields`, `search_media_fields`, `search_subnet_fields`) beschreiben die Felder an einer Stelle; Migration, Trigger und Neuaufbau nutzen sie. So kann kein Weg, der die Daten ändert, den Index vergessen: Er läuft in der Transaktion der Änderung, auch beim Struktur-Undo, beim Zusammenführen von Tags und beim Import.

4. **Text der Dokumente.** Die Datei ist die Quelle (Abschnitt 9); Rust schreibt ihren Text in der Transaktion, die den neuen Inhalt festhält (Anlegen, Speichern, Umwandeln, Wiederherstellen einer Version), zusammen mit Links und Medienverwendungen (`References`). `search_entries.indexed_sha256` hält die Prüfsumme des indexierten Inhalts. Was in den Index kommt: TXT unverändert; Markdown als Quelltext ohne die IDs interner Links und Medien (`pwrite://<id>`, `pwmedia://<id>#page=n`), die sonst Teilwörter träfen; `.pwrite` der Text aller Textknoten, Alternativtext und Titel von Bildern, eine Zeile je Block. Steuerzeichen und die beiden Markierungszeichen der Ausschnitte (U+E000, U+E001) werden zu Leerzeichen.

5. **Nachtragen beim Öffnen.** Stimmt `indexed_sha256` nicht mit `documents.content_sha256` überein (nach Migration 0009, nach einem Neuaufbau, falls ein Schreiben scheiterte), liest das Öffnen der Bibliothek die Dateien und trägt den Text nach – in Gruppen zu 100 Dokumenten, unter der Schreibsperre. Eine unlesbare Datei bleibt markiert und wird beim nächsten Öffnen erneut versucht; das Öffnen scheitert daran nicht. Im Normalfall ist die Liste leer, die Prüfung kostet eine Abfrage.

6. **Archiv und Papierkorb ändern den Index nicht.** Die Suche liest den Zustand aus `documents` und `media`. Archivierte und gelöschte Elemente fallen ohne Filter heraus; „Archiv einbeziehen“ und „Papierkorb einbeziehen“ nehmen sie auf. Wiederherstellen braucht deshalb keinen neuen Index. Erst endgültiges Löschen entfernt den Eintrag (`ON DELETE CASCADE`, Trigger).

7. **Anfragen baut Rust.** Die Eingabe wird an Leerraum geteilt, in Kleinbuchstaben gewandelt, Wiederholungen entfallen; höchstens 16 Wörter zu je 100 Zeichen, die ganze Anfrage höchstens 1.000 Zeichen. Ein Wort ohne Buchstaben und Ziffern (`"`, `+`, `-`) wird ignoriert; bleibt nichts, ist das Ergebnis leer – ohne Fehler. Jedes Wort wird gequotet (`"` verdoppelt) und um seine Schreibvarianten erweitert (`ß`/`ss`, `ä`/`ae`, `ö`/`oe`, `ü`/`ue`, in beide Richtungen, je Paar alle Vorkommen zugleich, höchstens acht Varianten je Wort). Alle Wörter müssen zutreffen (`AND`).

8. **Wortindex.** Jede Variante als Wortanfang (`"strasse"*`). Ein Wort, das auf ein Zeichen endet (`C++`), gilt nur als ganzes Wort: `"c++"*` hieße „alles, was mit c beginnt“. Weil der Wortindex Zeichen verliert (`C++` wird `c`), muss ein Wort mit Zeichen ab drei Zeichen zusätzlich wörtlich in Titel oder Inhalt stehen (Abfrage im Teilwortindex); sonst fände `C++` jeden Text mit einem einzelnen `C`. Kürzere Wörter mit Zeichen (`C#`) bleiben bei der losen Bedeutung. Ein einzelnes Zeichen (`e`) gilt ebenfalls nur als ganzes Wort: Als Wortanfang trifft es fast jeden Text (in der App mit der Referenzbibliothek 350 ms), und der Präfixindex aus Abschnitt 19 beginnt bei zwei Zeichen.

9. **Teilwortindex.** Wörter ab drei Zeichen als Teilwort (`"haus"` findet `Krankenhaus`); kürzere Wörter derselben Anfrage müssen im Wortindex zutreffen. Treffer, die schon der Wortindex lieferte, werden übersprungen.

10. **Reihenfolge und Gewichte.** Zuerst die Treffer des Wortindex, sortiert nach `bm25(search_words, 10, 1, 5, 3, 2)` (Titel, Inhalt, Tags, Subnetze, Metadaten), dann die übrigen Treffer des Teilwortindex nach `bm25(search_parts, 10, 1)`; bei Gleichstand die Eintragsnummer. Höchstens 200 Treffer, ohne Angabe 50.

11. **Filter** (Abschnitt 19): Typ (Dokument, Bild, PDF, Anhang, Subnetz), Tags (alle gewählten müssen zutreffen, wie im Explorer, ADR 0017 Punkt 13), Subnetz (Elemente darin und in allen Untersubnetzen, dazu die Untersubnetze selbst), Datum der letzten Änderung (von einschließlich, bis ausschließlich), nur Favoriten, Archiv einbeziehen, Papierkorb einbeziehen. Alles als Parameter derselben festen SQL-Abfrage (SQLx nimmt nur statisches SQL); `MATCH` bekommt die von Rust gebaute Anfrage als gebundenen Wert.

12. **Ergebnis.** Ein Treffer nennt Art, ID, Titel, Format, letzte Änderung, Archiv, Papierkorb, Favorit, den Ausschnitt und welcher Index ihn fand – keinen Pfad, keinen weiteren Inhalt. Der Ausschnitt kommt von `snippet()` mit den Markierungen U+E000/U+E001; Rust zerlegt ihn in Stücke `{ text, hit }`, das Frontend markiert ohne HTML.

13. **Befehle.** `search_query` (Anfrage und Filter, geprüft wie jeder Befehl: unbekannte Felder, IDs, Zeiten, Grenzen) und `search_rebuild` („Suchindex neu aufbauen“: leert beide FTS-Tabellen und `search_entries`, baut sie aus den Tabellen neu auf und liest die Dateien). Beide über die Bibliothekssitzung; Lesen über die Lesezugänge, Schreiben unter der Schreibsperre der Bibliothek.

14. **Prüfung beim Start** (Abschnitt 18: `PRAGMA quick_check` beim Start). Seit SQLite 3.44 ruft `quick_check` der ganzen Datenbank auch die Integritätsprüfung jeder FTS5-Tabelle auf; sie zerlegt den ganzen Text neu. Mit der Referenzbibliothek: 4,1 s für die ganze Datenbank, 0,19 s für alle Tabellen einzeln; das Öffnen dauerte 4,8 s, Widerspruch zum Kaltstart ≤ 2 s (Abschnitt 6, F-46). Entscheidung (Vorschlag aus dem Bericht, „Fahre fort“):
    - **WAL-Modus** (lokale Datenträger): Beim Öffnen prüft `quick_check` jede Tabelle einzeln mit ihren Indizes (`pragma_quick_check(<Tabelle>)`), die Schattentabellen von FTS5 eingeschlossen; nur die Neuzerlegung entfällt. 10 s nach dem Öffnen prüft ein Hintergrundauftrag die ganze Datenbank über einen Lesezugang; im WAL-Modus hält das niemanden auf. Scheitert die Prüfung, wird der Suchindex neu aufgebaut (er hält nur abgeleitete Daten) und erneut geprüft; scheitert sie dann noch, steht das im Protokoll (`DatabaseCheck::Failed`). Schließen bricht den Auftrag ab.
    - **Rollback-Journal** (Sync-Ordner, Netzlaufwerke): Ein langes Lesen hielte dort jedes Schreiben auf; deshalb bleibt es beim `quick_check` der ganzen Datenbank beim Öffnen.
    - Was die Einzelprüfung nicht abdeckt: Seiten der Freiliste und Seiten, die keiner Tabelle gehören. Beides prüft der Hintergrundauftrag; `integrity_check` nach Backup und Wiederherstellung bleibt (Abschnitt 21).
    - Offen: Die Oberfläche zeigt `Failed` noch nicht an; das Ergebnis steht im Protokoll. Ein Hinweis in der Oberfläche gehört zu „Datenintegrität“ und kommt mit Backup und Wiederherstellung (Phase 10).

15. **Oberfläche** (Abschnitt 11: „Seitenleiste mit Explorer oder Suche“). Die Seitenleiste zeigt Explorer oder Suche; die Activity Bar, `Ctrl + Shift + F` („Global suchen“, auch im Menü „Bearbeiten“) und `Ctrl + Shift + E` schalten um. Der Explorer bleibt unter der Suche eingehängt, damit Baum und Scrollstellung bleiben. Welche Seitenleiste zuletzt offen war, merkt sich die App nicht über einen Neustart hinaus.
    - Die Suche startet 60 ms nach dem letzten Tastendruck; eine Antwort, die eine neuere Anfrage überholt hat, wird verworfen. Ein Filter sucht sofort neu. 50 Treffer je Anfrage.
    - Trefferliste als `listbox`; das Feld behält den Fokus, Pfeiltasten, Bild auf/ab und Enter wirken aus dem Feld und aus der Liste. Jeder Treffer zeigt Art (Symbol und Text für Screenreader), Titel, Favorit/Archiv/Papierkorb und den Ausschnitt mit `<mark>`.
    - **Öffnen an der Fundstelle:** Ein Dokument öffnet im Tab; die Suchleiste des Dokuments sucht das erste markierte Wort des Ausschnitts, der erste Treffer wird ausgewählt und ins Bild gescrollt. Ein Medium öffnet in seiner Ansicht, ein Subnetz wird im Explorer gezeigt, ein Element im Papierkorb in der Gruppe „Papierkorb“ (dort lässt es sich wiederherstellen).
    - Datumsfilter in Ortszeit: „ab“ ab Mitternacht des Tages, „bis“ einschließlich des ganzen Tages.
    - „Suchindex neu aufbauen“ in Einstellungen › Bibliothek und in der Befehlspalette, mit Rückmeldung (Zahl der Dokumente, unlesbare Dateien).

## Folgen

- Kein Pfad zum Index ohne Transaktion: Trigger und `References` decken jeden Schreibweg ab; Tests prüfen Anlegen, Speichern, Umbenennen, Tags, Subnetze, Medien, Papierkorb, Struktur-Undo, endgültiges Löschen und den Neuaufbau, jeweils mit gleicher Zahl von Einträgen und FTS-Zeilen.
- Migration 0009 indexiert alles außer dem Text der Dokumente sofort; den Text trägt das erste Öffnen nach (einmalig, je nach Größe der Bibliothek einige Sekunden).
- Speicherbedarf mit der Referenzbibliothek (81 MB Text): Datenbank 480 MB, davon Teilwortindex 187 MB, Wortindex 62 MB und zweimal rund 90 MB gespeicherter Text. Öffnen 0,64 s; langsamste von 16 Suchen in Rust 65 ms, in der App vom Tippen bis zu den Treffern auf dem Bildschirm 122–180 ms einschließlich der Pause von 60 ms (Container, Debug-Build mit optimiertem SQLite).
- Markdown erscheint im Ausschnitt als Quelltext (`**fett**`). Ein Ausschnitt ohne Markdown-Zeichen wäre eine spätere Verbesserung.
- Eine deutsche Wortstammsuche (Abschnitt 19: später) und eine lokale semantische Suche bleiben offen.
