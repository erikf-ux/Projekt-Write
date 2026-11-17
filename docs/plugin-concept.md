# Konzept: Erweiterungen (Plugins) – Berechtigungen und Sandbox

Stand: Phase 13, 17. November 2026. Bezug: Abschnitt 22 und 20 des Auftrags, ADR 0027.

Dieses Papier beschreibt, wie Project Write später Erweiterungen von Dritten aufnehmen könnte. **In Version 1.0 ist nichts davon ausführbar.** Die App enthält nur eingebaute Erweiterungen. Sie hat keinen Weg, Code von außen zu laden (ADR 0027, Punkt 3).

## 1. Stand in Version 1.0

- **Register:** Es gibt interne Register für Editorfunktionen, Dateiformate, Prüfwerke, Exporter und Mesh-Renderer. Die eigenen Module tragen sich dort ein und werden über die Register gefunden. So entsteht schon jetzt die Schnittstelle, an die später Erweiterungen andocken.
- **Versiegelt:** Die Register sind nach dem Start versiegelt (Frontend) oder unveränderlich (Rust).
- **Keine Ladewege:** Es gibt keinen Plugin-Ordner und keinen Befehl, der Code liest.
- **Schutzschichten:**
  - Die CSP erlaubt nur Skripte der App selbst.
  - ESLint verbietet `eval`, `new Function` und `import()` mit offenem Ziel.
  - Ein E2E-Test belegt, dass fremder Code nicht läuft.
- **Sichtbar:** Die Einstellungen zeigen unter „Erweitert“ die eingebauten Erweiterungen, nur lesend.

## 2. Ziele und Grundsätze für später

1. **Der Nutzer behält die Kontrolle.** Keine Erweiterung läuft ohne seine ausdrückliche Zustimmung. Jede lässt sich abschalten und entfernen, ohne Reste.
2. **Geringste Rechte.** Eine Erweiterung bekommt nur die Berechtigungen, die sie im Manifest nennt und die der Nutzer bestätigt hat. Ohne Berechtigung sieht sie nichts.
3. **Durchsetzung in Rust, nicht im Frontend.** Jede Berechtigung prüft der Rust-Kern bei jedem Aufruf. Prüfungen in der Oberfläche sind nur Komfort.
4. **Kein Netz ohne Erlaubnis.** Die harte Regel aus Abschnitt 2 gilt auch für Erweiterungen. Netzzugriff ist eine eigene Berechtigung mit fester Liste von Hosts und eigener Bestätigung.
5. **Daten bleiben heil.** Eine Erweiterung kann die Bibliothek nicht beschädigen. Schreibzugriffe laufen über dieselben Dienste wie die eigenen Module: atomares Schreiben, Versionen, Papierkorb, Index.
6. **Fehler bleiben eingeschlossen.** Absturz, Endlosschleife oder zu hoher Speicherbedarf einer Erweiterung beenden nur die Erweiterung, nie die App.

## 3. Bedrohungsmodell

| Gefahr                 | Beispiel                                        | Gegenmaßnahme                                                                               |
| ---------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Daten abfließen lassen | Erweiterung liest alle Dokumente und sendet sie | kein Netz ohne Berechtigung; Lesen nur im erlaubten Umfang; Sandbox ohne Systemzugriff      |
| Daten zerstören        | Erweiterung überschreibt Dokumente              | Schreiben nur über Dienste mit Versionen und Papierkorb; eigene Berechtigung                |
| Code im Hauptprozess   | native Bibliothek, `eval` im Haupt-Webview      | nur WebAssembly oder isolierter Rahmen; kein natives Laden; CSP bleibt streng               |
| Oberfläche fälschen    | Dialog, der wie die App nach Daten fragt        | Erweiterungen zeichnen nur in gekennzeichneten Bereichen; Systemdialoge nur über die App    |
| Dauerhaftigkeit        | Erweiterung trägt sich selbst neu ein           | Installation nur über den Installationsdialog; Register bleiben für alles andere versiegelt |
| Lieferkette            | manipuliertes Paket, Update mit neuen Rechten   | Signatur, Prüfsummen, erneute Zustimmung bei neuen Berechtigungen                           |
| Ressourcen             | Endlosschleife im Prüfwerk                      | Zeit- und Speichergrenzen, Abbruch, Abschalten                                              |

## 4. Erweiterungspunkte

Die Register aus ADR 0027 sind die einzigen Andockstellen:

| Register               | Was eine Erweiterung beiträgt                                                                             | Wo sie läuft                                                  |
| ---------------------- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| Dateiformate           | Lesen und Schreiben eines weiteren Formats, etwa AsciiDoc, als Umwandlung von und nach dem Dokumentmodell | WebAssembly in Rust                                           |
| Exporter               | weiteres Exportformat, etwa EPUB oder DOCX; schreibt nur in eine vom Nutzer gewählte Datei                | WebAssembly in Rust                                           |
| Prüfwerke              | weitere Sprache oder Prüfung über die Schnittstelle `LanguageEngine`                                      | WebAssembly in Rust                                           |
| Editorfunktionen       | Knoten, Marken, Befehle im Editor                                                                         | isolierter Rahmen; das Schema bleibt in der App (siehe 5.2)   |
| Mesh-Renderer          | andere Darstellung des Netzes über die Schnittstelle `renderer.ts`                                        | isolierter Rahmen                                             |
| später: KI-Integration | Vorschläge zu Text oder Verbindungen                                                                      | WebAssembly oder isolierter Rahmen, Netz nur mit Berechtigung |

## 5. Sandbox

### 5.1 Rechnende Erweiterungen: WebAssembly in Rust

- **Laufzeit:** Dateiformate, Exporter und Prüfwerke laufen als WebAssembly-Modul im Rust-Kern, etwa mit Wasmtime.
- **Kein Systemzugriff:** Das Modul bekommt kein WASI-Dateisystem, kein Netz, keine Uhr mit hoher Auflösung und keine Umgebungsvariablen. Daten reichen nur die Host-Funktionen herein und heraus, die zu den bestätigten Berechtigungen passen.
- **Grenzen:** Speicher je Instanz (etwa 64 MB), Rechenzeit über „Fuel“ oder Epochen-Unterbrechung, Größe der Ein- und Ausgabe.
- **Ergebnisse:** Was ein Modul liefert, prüft die App wie jede andere Eingabe (Abschnitt 20). Ein Dateiformat liefert ein Dokument im Schema der App, das gegen das Schema geprüft wird; ein Exporter liefert Bytes, die nur in die vom Nutzer gewählte Datei geschrieben werden.

### 5.2 Erweiterungen der Oberfläche: isolierter Rahmen

- **Eigener Rahmen:** Mesh-Renderer und Oberflächenteile laufen in einem eigenen `<iframe sandbox="allow-scripts">` mit eigener Herkunft und eigener CSP. Dafür wäre eine ADR zur Lockerung von `frame-src` nötig, die Abschnitt 20 verlangt.
- **Kein Zugriff auf die App:** Der Rahmen hat keinen Zugriff auf `__TAURI_INTERNALS__`, auf das DOM der App oder auf ihre Speicher. Er spricht nur über `postMessage` mit einem schmalen, versionierten Protokoll. Die App prüft jede Nachricht gegen ein Schema und gegen die Berechtigungen.
- **Editorfunktionen:** Sie sind heikler, weil Tiptap im Haupt-Webview läuft. Vorgeschlagen ist, dass eine Erweiterung nur deklarativ Knoten und Marken beschreibt (Name, Attribute mit Typen, Darstellung als Vorlage ohne Skript). Befehle laufen als WebAssembly, das eine Transaktion vorschlägt; die App prüft und wendet sie an. Fremdes JavaScript im Haupt-Webview bleibt ausgeschlossen.

### 5.3 Keine nativen Erweiterungen

Native Bibliotheken (`.dll`, `.so`, `.dylib`) werden nie geladen. Sie wären nicht einzugrenzen.

## 6. Berechtigungen

Eine Erweiterung nennt ihre Berechtigungen im Manifest. Bei der Installation zeigt die App sie in Klartext, der Nutzer stimmt zu oder bricht ab. Neue Berechtigungen in einem Update verlangen eine neue Zustimmung. Jede Berechtigung lässt sich später einzeln entziehen; die Erweiterung muss damit umgehen.

| Berechtigung             | Bedeutung                                                                    |
| ------------------------ | ---------------------------------------------------------------------------- |
| `document.read.current`  | das gerade bearbeitete Dokument lesen                                        |
| `document.write.current` | das gerade bearbeitete Dokument ändern (als Transaktion, mit Rückgängig)     |
| `library.read`           | alle Dokumente und Metadaten der Bibliothek lesen                            |
| `library.write`          | Dokumente anlegen und ändern (mit Versionen, Papierkorb)                     |
| `media.read`             | Medien der Bibliothek lesen                                                  |
| `file.write.chosen`      | in eine vom Nutzer gewählte Datei schreiben (Export)                         |
| `file.read.chosen`       | eine vom Nutzer gewählte Datei lesen (Import)                                |
| `settings.own`           | eigene Einstellungen speichern (getrennter Bereich)                          |
| `ui.command`             | Befehle in Palette und Menü anbieten                                         |
| `ui.panel`               | einen eigenen Bereich in der Seitenleiste zeigen                             |
| `network:<host>`         | Verbindungen nur zu diesem Host; jeder Host einzeln, mit eigener Bestätigung |

Es gibt keine Berechtigung für beliebige Pfade, Prozesse, Systembefehle oder andere Erweiterungen. Berechtigungen gelten je Erweiterung und werden nicht weitergegeben.

## 7. Paket und Manifest

- **Paket:** eine Datei `.pwplugin`, ein ZIP-Archiv mit denselben Prüfungen wie `.pwritepack`: Pfade, entpackte Größe, Prüfsummen.
- **Manifest** `manifest.json`:
  - `id` (umgekehrte Domain), `name`, `version`, `publisher`;
  - `minAppVersion`;
  - `registers` (welche Andockstellen);
  - `permissions`;
  - `entry` (WebAssembly-Modul oder Rahmenseite);
  - `sha256` aller Dateien.
- **Signatur:**
  - Jedes Paket ist mit einem Schlüssel des Herausgebers signiert (Ed25519).
  - Die App kennt eine Liste vertrauenswürdiger Schlüssel.
  - Unsignierte Pakete lassen sich nur in einem ausdrücklich eingeschalteten Entwicklermodus laden, mit dauerhaftem Hinweis.
  - Woher die Schlüssel kommen und wer prüft, ist offen (verbunden mit D11).
- **Ablage:** im Konfigurationsordner unter `plugins/<id>/<version>/`. Eigene Daten einer Erweiterung liegen getrennt unter `plugin-data/<id>/` und werden beim Entfernen gelöscht.

## 8. Lebenszyklus

1. **Installieren:** Datei wählen, Signatur und Manifest prüfen, Berechtigungen zeigen, Zustimmung; danach ist die Erweiterung installiert, aber aus.
2. **Einschalten:** Die App lädt das Modul oder den Rahmen beim nächsten Start der betroffenen Funktion, nicht im laufenden Register. Die Register werden dafür beim Start aus eingebauten und eingeschalteten Erweiterungen aufgebaut und dann versiegelt, wie heute.
3. **Abgesicherter Start:** Mit einem Startschalter, und nach zwei Abstürzen in Folge automatisch, startet die App ohne Erweiterungen.
4. **Fehler:** Überschreitet eine Erweiterung ihre Grenzen oder stürzt sie ab, schaltet die App sie ab, meldet das und protokolliert den Grund.
5. **Aktualisieren:** wie Installieren; bei neuen Berechtigungen erneute Zustimmung. Updates kommen nie automatisch.
6. **Entfernen:** löscht Paket und eigene Daten. Dokumente, die Inhalte der Erweiterung enthalten (etwa eigene Knoten), bleiben erhalten; die App zeigt solche Knoten als geschützte, unveränderte Blöcke, wie heute rohes HTML.

## 9. Was vor einer Umsetzung zu entscheiden ist

- **Laufzeit:** Wahl der WebAssembly-Laufzeit, Lizenz, Größe des Installationspakets (Ziel ≤ 50 MB) und Startzeit.
- **CSP:** ADR zur Lockerung von `frame-src` für den isolierten Rahmen (Abschnitt 20).
- **Signaturen:** Infrastruktur und Verwaltung der vertrauenswürdigen Schlüssel.
- **Dokumentformat:** Wie fremde Knoten in `.pwrite` und Markdown gespeichert werden, ohne dass Dokumente ohne die Erweiterung unlesbar werden.
- **KI-Integrationen:** zusätzlich Datenschutzhinweise, Auswahl des Textausschnitts, Protokoll der gesendeten Daten.

## 10. Prüfungen, die später dazukommen

- Eine Erweiterung ohne Berechtigung `library.read` erhält bei jedem Lesebefehl einen Fehler (Rust-Test).
- Eine Endlosschleife im Modul wird nach der Zeitgrenze abgebrochen, die App bleibt bedienbar.
- Ein manipuliertes oder unsigniertes Paket wird abgelehnt.
- Der Rahmen erreicht weder `__TAURI_INTERNALS__` noch das DOM der App (E2E).
- Normalbetrieb mit eingeschalteten Erweiterungen ohne Netzberechtigung baut keine Verbindung auf (wie der Test aus Abschnitt 20).
