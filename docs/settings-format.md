# Dateiformat `.pwsettings`, Version 1

Einstellungen von Project Write zum Mitnehmen auf einen anderen Rechner oder als eigene Sicherung (ADR 0027, Punkt 4). Eine `.pwsettings`-Datei ist UTF-8-JSON ohne BOM, höchstens 1 MB groß.

```json
{
  "format": "projectwrite-settings",
  "formatVersion": 1,
  "exportedAt": "2026-11-17T12:00:00.000Z",
  "app": {
    "autosaveExternal": false,
    "keybindings": { "palette.show": ["Ctrl+K"] },
    "language": "de",
    "languageCheck": {
      "autocorrect": false,
      "defaultLanguage": null,
      "detect": true,
      "enabled": true,
      "rules": true
    },
    "limitLineWidth": true,
    "readingTime": { "visible": false, "wordsPerMinute": 200 },
    "sidebar": { "visible": true, "width": 260 },
    "theme": "dark",
    "zoom": 1.25
  },
  "library": {
    "backup.automatic": true,
    "backup.keep": 7,
    "history.dailyDays": 30,
    "history.keepAllHours": 24,
    "history.weeklyDays": 365,
    "media.maxMegapixels": 100,
    "mesh.suggestionThreshold": 0.35,
    "trash.retentionDays": 30
  }
}
```

## Hülle

| Feld            | Typ        | Regeln                                                                           |
| --------------- | ---------- | -------------------------------------------------------------------------------- |
| `format`        | Text       | genau `projectwrite-settings`; sonst wird die Datei abgelehnt                    |
| `formatVersion` | ganze Zahl | `1`; eine höhere Version wird abgelehnt (`SETTINGS_FILE_NEWER`)                  |
| `exportedAt`    | Text       | ISO 8601 UTC; nur zur Anzeige in der Vorschau, ein ungültiger Wert wird überhört |
| `app`           | Objekt     | Einstellungen der Oberfläche, siehe unten; darf fehlen                           |
| `library`       | Objekt     | Einstellungen der offenen Bibliothek, siehe unten; fehlt, wenn keine offen war   |

Andere Felder auf oberster Ebene werden übergangen und in der Vorschau genannt.

## `app`: Oberfläche

Die Werte aus `settings.json`, die die Oberfläche ausmachen. Jeder Wert wird beim Import mit denselben Prüfungen geprüft wie beim Ändern in den Einstellungen (`UiSettings::validate` in `src-tauri/src/services/settings/ui.rs`).

| Schlüssel          | Inhalt                                                                                                   |
| ------------------ | -------------------------------------------------------------------------------------------------------- |
| `language`         | `system`, `de` oder `en`                                                                                 |
| `theme`            | `system`, `light`, `dark` oder `custom:<id>` eines importierten Designs                                  |
| `zoom`             | 0,5 bis 2                                                                                                |
| `sidebar`          | `visible` (ja/nein), `width` (160 bis 640)                                                               |
| `limitLineWidth`   | ja/nein                                                                                                  |
| `readingTime`      | `visible` (ja/nein), `wordsPerMinute` (50 bis 1000)                                                      |
| `autosaveExternal` | ja/nein                                                                                                  |
| `keybindings`      | abweichende Tastenkürzel: Befehls-ID → Liste von Kürzeln in kanonischer Form                             |
| `languageCheck`    | `enabled`, `rules`, `autocorrect`, `detect` (je ja/nein), `defaultLanguage` (BCP-47-Kennung oder `null`) |

Ein Objekt wie `sidebar` muss vollständig sein; unbekannte Felder darin machen den Wert ungültig. Fehlt ein Schlüssel ganz, bleibt die bisherige Einstellung.

Ein importiertes Design ist eine Datei und wird nicht mitgenommen, nur seine Wahl. Fehlt das Design auf dem anderen Rechner, gilt wie sonst „Wie das System“.

## `library`: Bibliothek

Die Schlüssel aus `application_settings` der offenen Bibliothek, mit dem Wert, der gerade gilt (auch dem Standardwert, wenn keiner gespeichert ist). Geprüft wird jeder mit der Prüfung seines `LibrarySetting` (`src-tauri/src/services/settings/library.rs`).

| Schlüssel                  | Inhalt             |
| -------------------------- | ------------------ |
| `trash.retentionDays`      | 1 bis 3650 Tage    |
| `backup.automatic`         | ja/nein            |
| `backup.keep`              | 1 bis 100          |
| `history.keepAllHours`     | 1 bis 8760 Stunden |
| `history.dailyDays`        | 1 bis 3650 Tage    |
| `history.weeklyDays`       | 1 bis 36500 Tage   |
| `mesh.suggestionThreshold` | 0,05 bis 0,95      |
| `media.maxMegapixels`      | 1 bis 1000         |

## Nicht enthalten

Was an diesen Rechner, an den Ort der Bibliothek oder an diese Sitzung gebunden ist, steht nicht in der Datei. Steht es doch darin (von Hand eingetragen), wird es übergangen und in der Vorschau als „an einen Rechner gebunden“ genannt:

- `libraryPath`: Ort der Bibliothek;
- `window`: Größe und Lage des Fensters;
- `session`: die Tabs der letzten Sitzung;
- `recentFiles`: zuletzt verwendete externe Dateien;
- `backupTargets`: Backup-Ziele, Pfade dieses Rechners;
- `languageTool`: Ordner und Schalter des LanguageTool-Zusatzpakets;
- `languageCheck.autocorrectAsked`: ob der erste Start schon nach der Autokorrektur gefragt hat;
- `storage.acceptedLocation` in `library`: die Bestätigung eines Sync-Ordners oder Netzlaufwerks gilt für diesen Ort.

Importierte Designs, eigene und zusätzliche Wörterbücher sind Dateien und werden nicht mitgenommen. Bibliotheksdaten (Dokumente, Struktur, Medien, Versionen) gehören nie zu den Einstellungen.

## Import

1. Rust zeigt den Dialog zum Öffnen und liest die Datei (höchstens 1 MB).
2. Die ganze Datei wird geprüft: Kennung, Version, jeder bekannte Wert. Ein ungültiger Wert lehnt die ganze Datei ab (`SETTINGS_VALUE_INVALID` mit dem Schlüssel); es ändert sich nichts.
3. Unbekannte Schlüssel und solche aus „Nicht enthalten“ werden übergangen und genannt. Werte für die Bibliothek werden übergangen, wenn keine Bibliothek offen ist.
4. Die Vorschau zeigt jede Änderung mit bisherigem und neuem Wert. Die geprüften Werte bleiben in Rust; die Oberfläche bekommt nur ein Token.
5. Bei „Übernehmen“ legt die App zuerst eine Sicherung an (siehe unten), schreibt dann die Bibliothekswerte in einer Transaktion und danach `settings.json`. Scheitert das Schreiben von `settings.json`, werden die Bibliothekswerte zurückgesetzt: Übernommen wird alles oder nichts.
6. Die laufende App übernimmt Design, Sprache, Zoom, Menü, Tastenkürzel und Sprachprüfung sofort.

Wartet eine Vorschau nicht mehr (übernommen, durch eine neue ersetzt, oder inzwischen ist eine andere Bibliothek offen), meldet die App `SETTINGS_IMPORT_EXPIRED`.

## Zurücksetzen

Getrennt für Oberfläche und Bibliothek, jeweils nach Bestätigung:

- **Oberfläche:** alle Schlüssel aus `app` auf die Standardwerte. Alles aus „Nicht enthalten“ bleibt.
- **Bibliothek:** die Zeilen der Schlüssel aus `library` werden entfernt, es gelten die Standardwerte. Andere Zeilen von `application_settings` (etwa `storage.acceptedLocation`) und alle Bibliotheksdaten bleiben.

## Sicherungen

Vor jedem Import und jedem Zurücksetzen schreibt die App den bisherigen Stand als `.pwsettings` in den Ordner `settings-backups/` im Konfigurationsordner, Name `settings-JJJJMMTTTHHMMSSmmmZ.pwsettings` (UTC). Die fünf neuesten bleiben; ältere Dateien mit genau diesem Namensmuster werden entfernt, andere Dateien im Ordner nie. Eine Sicherung lässt sich wie jede andere Datei importieren.

## Versionen

`formatVersion` steigt, wenn sich die Bedeutung eines Schlüssels ändert oder ein Objekt (etwa `sidebar`) neue Pflichtfelder bekommt. Neue Schlüssel allein brauchen keine neue Version: ältere Versionen übergehen sie und nennen sie. Eine Datei mit höherer Version lehnt die App ab, statt sie teilweise zu verstehen.
