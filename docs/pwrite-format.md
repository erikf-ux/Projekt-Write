# Dateiformat `.pwrite`, Version 1

Das Rich-Text-Format von Project Write (Abschnitt 10). Eine `.pwrite`-Datei ist UTF-8-JSON ohne BOM. Medien werden referenziert, nicht eingebettet.

```json
{
  "format": "pwrite",
  "formatVersion": 1,
  "id": "0b8f8f4e-6d0e-4f3e-9a3c-2a1d3c4b5e6f",
  "title": "Kapitel 1",
  "createdAt": "2026-11-04T10:00:00Z",
  "updatedAt": "2026-11-04T10:05:00Z",
  "language": "de-DE",
  "tags": [],
  "content": {
    "type": "doc",
    "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Hallo" }] }]
  },
  "media": []
}
```

## Hülle

| Feld            | Typ                | Regeln                                                                 |
| --------------- | ------------------ | ---------------------------------------------------------------------- |
| `format`        | Text               | genau `pwrite`                                                         |
| `formatVersion` | ganze Zahl         | `1`; höher: Datei öffnet schreibgeschützt mit Hinweis                  |
| `id`            | Text               | UUID in Kleinbuchstaben; in der Bibliothek die ID des Dokuments        |
| `title`         | Text               | 1–300 Zeichen                                                          |
| `createdAt`     | Text               | ISO 8601 UTC                                                           |
| `updatedAt`     | Text               | ISO 8601 UTC                                                           |
| `language`      | Text               | BCP-47-Sprachkennung, etwa `de-DE`                                     |
| `tags`          | Liste von Texten   | ab Phase 7; bis dahin leer                                             |
| `content`       | Objekt             | Dokument im Schema unten                                               |
| `media`         | Liste von Objekten | `id`, `sha256`, `originalName`, `mimeType`; ab Phase 8, bis dahin leer |

Unbekannte Felder bleiben beim Laden und Speichern erhalten, auf oberster Ebene und in Einträgen von `media`. In der Bibliothek sind Titel und Zeitstempel der Datenbank maßgeblich; Project Write schreibt sie beim Speichern in die Hülle.

## Schema von `content`, Version 1

Knoten haben `type`, wo nötig `attrs` und `content`; Text-Knoten haben `text` und optional `marks`. Fehlt ein Attribut, gilt der Standardwert.

| Knoten                     | Inhalt                           | Attribute (Standard)                                                                                                       |
| -------------------------- | -------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `doc`                      | Block+                           | –                                                                                                                          |
| `paragraph`                | Inline\*                         | –                                                                                                                          |
| `heading`                  | Inline\*                         | `level` 1–6 (1)                                                                                                            |
| `blockquote`               | Block+                           | –                                                                                                                          |
| `bulletList`               | `listItem`+                      | –                                                                                                                          |
| `orderedList`              | `listItem`+                      | `start` (1), `type` (`null`; `a`, `A`, `i`, `I` für Buchstaben und römische Zahlen)                                        |
| `listItem`                 | `paragraph` Block\*              | –                                                                                                                          |
| `taskList`                 | `taskItem`+                      | –                                                                                                                          |
| `taskItem`                 | `paragraph` Block\*              | `checked` (`false`)                                                                                                        |
| `codeBlock`                | Text\* ohne Formatierung         | `language` (`null`)                                                                                                        |
| `horizontalRule`           | –                                | –                                                                                                                          |
| `table`                    | `tableRow`+                      | –                                                                                                                          |
| `tableRow`                 | (`tableHeader` \| `tableCell`)\* | –                                                                                                                          |
| `tableHeader`, `tableCell` | Block+                           | `colspan` (1), `rowspan` (1), `colwidth` (`null` oder Liste von Pixelbreiten), `align` (`null`, `left`, `center`, `right`) |
| `image`                    | – (inline)                       | `src` (`""`), `alt` (`""`), `title`, `width`, `height`, `href`, `linkTitle` (je `null`)                                    |
| `rawBlock`                 | –                                | `raw` (`""`), `kind` (`html` oder `footnote`)                                                                              |
| `rawInline`                | – (inline)                       | `raw` (`""`), `kind` (`html`, `footnote` oder `autolink`)                                                                  |
| `hardBreak`                | – (inline)                       | –                                                                                                                          |
| `text`                     | –                                | `text`, `marks`                                                                                                            |

Marks: `bold`, `italic`, `underline`, `strike`, `code`, `link` mit `href` (`http`, `https`, `mailto`, relative Adressen und `pwrite://<id>` für interne Links) und `title`. Die Attribute `target`, `rel` und `class` des Links schreibt der Editor mit; sie haben keine Bedeutung.

Bilder werden nie geladen (Abschnitt 12); sie erscheinen als Hinweis mit Adresse, bis die Medien kommen (Phase 8).

## Versionen

Jede Änderung an Hülle oder Schema erhöht `formatVersion` und bringt eine Migration mit (Abschnitt 10). Eine Datei mit unbekannter höherer Version öffnet schreibgeschützt; Project Write schreibt sie nie.
