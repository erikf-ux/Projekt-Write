# ADR 0014 – Editor: Schema, Markdown-Adapter und Verlustschutz

- Status: angenommen
- Datum: 2026-11-03
- Phase: 4

## Kontext

Abschnitt 7 legt Tiptap 3.x fest; `@tiptap/markdown` „läuft deshalb hinter einer eigenen Adapter-Schicht“. Abschnitt 10 verlangt prüfbar verlustfreies Markdown: mindestens 50 Fälle in `tests/fixtures/markdown/`, abschließend dokumentierte Normalisierungen, kein Vergleich nur über gerendertes HTML, Lösungen für die Verlustfälle der Stichprobe, eine erlaubte HTML-Teilmenge, einen Verlustschutz vor dem Speichern und eine Round-Trip-Probe beim Öffnen. Abschnitt 12 beschreibt den hybriden Editor.

Die Stichprobe mit Tiptap 3.32.2 ergab sechs Verlustfälle. Die eigene Prüfung fand weitere, zum Teil schwerere: Tiptaps Leser für nummerierte Listen nimmt auch `a.`, `iv.` und `Dr.` als Nummer, sodass der Absatz „Dr. Müller kommt.“ zu einer Liste wird. Ein Listenpunkt, der mit einer Überschrift beginnt, wird zum Absatz. Text nach einem HTML-Block in einem Listenpunkt landet als ungültiger loser Text im Dokument. marked macht in Listenpunkten Tabulatoren zu Leerzeichen, auch in Code. Mehrere Leerzeilen werden zu leeren Absätzen. Inline-Code schließt jede andere Formatierung aus, sodass `**`code`**` nicht ins Schema passt. Und `&` wird beim Speichern zu `&amp;`, jedes `_` und `[` bekommt einen Backslash.

## Entscheidungen

1. **Schema = was Markdown halten kann** (`modules/editor/schema/`): Absätze, Überschriften 1–6, fett, kursiv, durchgestrichen, unterstrichen (`<u>`), Inline-Code, Links, Listen, Nummerierungen, Aufgabenlisten, Zitate, Codeblöcke, Trennlinien, Tabellen, Bilder sowie Rohblöcke und Roh-Inline für HTML und Fußnoten. Formatierungen ohne Markdown-Form (Farben, Hervorhebung) gibt es im Schema nicht; was über Einfügen hereinkommt, durchläuft dieses Schema.
2. **Ein Adapter** (`modules/editor/markdown/`) ist der einzige Weg zwischen Markdown und Editor. Beim Lesen trennt er den YAML-Kopf ab, macht CRLF zu LF, schützt Tabulatoren vor marked und macht einen Absatz aus nur `&nbsp;` zum leeren Absatz. Beim Schreiben streicht er leere Absätze am Rand, wechselt die Marker benachbarter Listen (`-`/`*`, `.`/`)`), schreibt Nummern mit Ziffern, jeden Code-Abschnitt für sich und Autolinks roh. Den Kopf setzt er unverändert wieder ein.
3. **Lesbares Escaping** (`escape.ts`): Ein Zeichen bekommt nur dort einen Backslash, wo es sonst die Bedeutung änderte. `[[Wiki-Link]]`, `snake_case`, `Müller & Söhne`, `3 * 4` bleiben stehen. Am Zeilenanfang eines Absatzes werden Blockanfänge maskiert (`\#`, `\-`, `1\.`, `\>`, Setext-Linie, Tabellen-Trennzeile, `\[ ]`).
4. **Markdown-Regeln** (`markdownRules.ts`) ersetzen die Ausgabe von Dokument, Absatz, Überschrift, hartem Umbruch, Code, Codeblock, Link, Listen und Listenpunkten. Listenpunkte und nummerierte Listen werden so gelesen, wie CommonMark sie liest. Drei interne Methoden des Markdown-Managers werden ersetzt: `encodeTextForMarkdown`, `parseTokens` (keine leeren Absätze aus Leerzeilen) und `parseFallbackToken` (meldet Tokens ohne Erweiterung). Alle Eingriffe sind an Tiptap 3.32.2 gebunden; die Tests unten schlagen an, wenn ein Update das Verhalten ändert.
5. **Erlaubte HTML-Teilmenge:** ein Paar `<u>…</u>`, `<br>` und `<img>` mit genau `src` und höchstens `alt`, `title`, `width`, `height`. Alles andere bleibt als Rohblock oder Roh-Inline erhalten und wird als Text gezeigt, nie als HTML. Bilder werden nie geladen, sie erscheinen als Hinweis mit Adresse (Abschnitt 12). Ein Bild ohne Größe wird `![Alt](Adresse "Titel")`, eines mit Größe `<img …>`, ein Bild im Link `[![…](…)](…)`.
6. **Normalisierungen**, abschließend: die 15 Einträge in `tests/fixtures/markdown/normalizations.json`. Ihre ausführbare Fassung ist `canonicalBlocks` in `probe.ts`.

   | Kennung        | Was sich ändert                                                                                                                                          |
   | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
   | `betonung`     | `_x_` wird `*x*`, `__x__` wird `**x**`                                                                                                                   |
   | `ueberschrift` | Setext wird ATX, schließende `#` entfallen, mehrzeilige Setext-Überschriften werden eine Zeile                                                           |
   | `liste`        | Marker `-` und `1.` (andere nur zur Trennung benachbarter Listen), fortlaufende Nummern, `[X]` wird `[x]`, Einrückung, keine Leerzeilen zwischen Punkten |
   | `trennlinie`   | `***`, `___`, `- - -` werden `---` (am Dateianfang ohne YAML-Kopf `***`)                                                                                 |
   | `codeblock`    | Eingerückt oder `~~~` wird ` ``` `, der Zaun ist länger als jede Backtick-Folge im Code                                                                  |
   | `inline-code`  | Backtick-Zahl und Füllleerzeichen; Leerraum am Rand außerhalb; Zeilenumbruch wird Leerzeichen                                                            |
   | `tabelle`      | Spalten neu ausgerichtet, Leerraum in Zellen wird ein Leerzeichen                                                                                        |
   | `link`         | Referenzlinks werden Inline-Links, nackte Adressen `<Adresse>`, `www.…` wird `[www.…](http://www.…)`, Ziele mit Klammern in `<…>`                        |
   | `umbruch`      | `\` und `<br>` werden zwei Leerzeichen (in Überschriften und bei Umbrüchen hintereinander `<br>`), Umbrüche am Absatzrand entfallen                      |
   | `zeichen`      | Zeichenreferenzen werden Zeichen, außer `&nbsp;`; U+00A0 wird `&nbsp;`                                                                                   |
   | `escape`       | Backslashes kommen hinzu oder entfallen, wo die Bedeutung gleich bleibt; in Linktexten `&#91;`/`&#93;`                                                   |
   | `leerzeilen`   | Eine Leerzeile zwischen Blöcken, ein Zeilenumbruch am Ende, leere Absätze am Rand entfallen, CRLF wird LF                                                |
   | `leerraum`     | Leerraum am Rand von Absätzen, Überschriften und Zellen und um Zeilenumbrüche entfällt; am Rand einer Formatierung steht er außerhalb                    |
   | `zitat`        | `>` vor jeder Zeile, verschachtelte Zitate durch `>` getrennt                                                                                            |
   | `html`         | `<img>` ohne Größe wird Markdown-Bild; Attribute in fester Reihenfolge mit `"`                                                                           |

7. **Round-Trip-Test** (`roundtrip.test.ts`, 69 Fälle): Lesen → Tiptap-Editor → Speichern. Der Text muss byte-genau der Eingabe oder ihrer `.expected.md` gleichen, die nur genannte Normalisierungen enthalten darf. Die gespeicherte Datei muss im Editor dasselbe Dokument ergeben (Vergleich des Dokuments, nicht des HTML), der YAML-Kopf muss byte-genau zurückkommen, die Probe darf nichts finden, und erneutes Speichern ändert nichts. Dazu kommt ein Stabilitätstest mit 300 erzeugten Dokumenten aus festem Startwert (`stability.test.ts`): Jedes Dokument ohne Verlust muss gleich zurückkommen.
8. **Probe beim Öffnen** (`probeMarkdown`): Tokens ohne Platz im Schema, ProseMirror-Prüfung des Dokuments, Vergleich nach dem Muster von Punkt 7, YAML-Kopf. Findet sie etwas, öffnet das Dokument in der Quelltextansicht und nennt den Grund. So überschreibt auch Autosave nichts still.
9. **Verlustschutz beim Speichern:** `findLosses` nennt, was der Editor halten kann, Markdown aber nicht schreiben: verbundene Zellen, mehrere Blöcke oder Nicht-Absätze in einer Zelle, Tabelle ohne Kopfzeile oder mit Kopfzellen darunter, Ausrichtung je Zelle, feste Spaltenbreiten, Nummerierung mit Buchstaben, Fett oder Kursiv, das an einem Satzzeichen direkt vor einem Buchstaben endet oder beginnt (`**Hinweis:**Text`), Inline-Code mit Backticks in Fett, Kursiv oder einem Link, Roh-HTML am Zeilenanfang. Dazu kommt beim Speichern je geändertem Block die Round-Trip-Prüfung als Netz für alles, was keine eigene Regel hat. Die Warnung nennt die Inhalte und bietet „Trotzdem speichern“ und „Abbrechen“; „Als `.pwrite` speichern“ folgt mit dem Format (Phase 5).

## Folgen

- Bekannte Grenzen, alle durch Probe oder Verlustschutz abgefangen statt still verändert: marked liest manche dicht gepackten `*`-Folgen anders als CommonMark (im Stabilitätslauf über 3000 Dokumente 5 Fälle); `$…$` ist für Markdown Text, ein `_` darin bekommt einen Backslash, wo es Betonung beginnen könnte; ein Fußnotenverweis direkt vor `(` wird als `[^1]\(` geschrieben.
- Ein Tiptap-Update verlangt, die ersetzten Methoden und Regeln zu prüfen; Round-Trip- und Stabilitätstest sind dafür die Messlatte.
- `.pwrite` (Phase 5) hat diese Grenzen nicht. Der Verlustschutz bietet dann den Wechsel an.
