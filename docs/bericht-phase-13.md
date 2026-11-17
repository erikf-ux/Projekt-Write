# Bericht Phase 13 – Erweiterungen

Stand: 17. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig).

- **Entwicklungsumgebung:** Cloud-Container, Ubuntu 24.04, 4 vCPU, keine GPU (Xvfb, WebKitGTK).
- **CI:** GitHub Actions `windows-latest` und `ubuntu-24.04`; macOS ist zurückgestellt.

Phase 13 ist umgesetzt:

- interne Register für Editorfunktionen, Dateiformate, Prüfwerke, Exporter und Mesh-Renderer, die die eigenen Module nutzen;
- ein Test, dass fremder Code nicht läuft;
- Einstellungen exportieren, importieren und zurücksetzen;
- der neue Abschnitt „Erweitert“;
- das Konzeptpapier für spätere Plugins.

Alle drei Abnahmekriterien sind durch Tests belegt.

**Arbeitsweise:** Auf Wunsch des Auftraggebers liefen drei Agenten gleichzeitig, jeder in einem eigenen Git-Worktree: Register in Rust, Register im Frontend, Einstellungen übertragen. Die gemeinsamen Schnittstellen standen vorher in ADR 0027. Ich habe die Zweige zusammengeführt, die Konflikte in den gemeinsamen Befehlslisten gelöst, die Liste der Erweiterungen ergänzt, alle Prüfungen ausgeführt und den E2E-Test für WebKitGTK angepasst.

## Was wurde implementiert?

Die Entscheidungen stehen in ADR 0027.

- **Register in Rust** (`services/registry.rs`, `services/extensions.rs`):
  - **Aufbau:** Ein allgemeines, unveränderliches Register mit geprüften IDs; eine doppelte oder falsch geformte ID bricht den Aufbau ab. Alle Register entstehen einmal je Prozess aus einer Funktion, die die eingebauten Einträge aufzählt.
  - **Dateiformate** (`txt`, `md`, `pwrite`) mit Dateiendung, Ordner, Art, Dialogfiltern und den Wegen für Verweise, Medien und Suchtext. Diese Stellen nutzen das Register: Import, externe Dateien, Suche, Backup, Wartung und die Dialoge.
  - **Exporter** (TXT, Markdown, Paket, Bibliothek). Die Exportbefehle wählen sie über das Register.
  - **Prüfwerke** (Rechtschreibung, Regeln, LanguageTool als Zusatzpaket). `language_check` führt sie aus dem Register aus.
  - **Neuer Befehl** `extensions_list`.
- **Register im Frontend** (`src/core/registry.ts`, `src/extensions/`):
  - **Aufbau:** Register für Editorfunktionen, Dateiformate (Editor), Exporter und Mesh-Renderer. `main.tsx` füllt sie und versiegelt sie vor dem ersten Rendern; danach wirft `register`.
  - **Nutzung:** Der Editor baut seine Erweiterungen aus dem Register, in unveränderter Reihenfolge. Die Sitzung wählt das Format über das Register. Die Exportbefehle entstehen aus dem Register, mit unveränderten IDs und Tastenkürzeln. Das Mesh nimmt seinen Renderer aus dem Register.
- **Kein fremder Code:**
  - Die Register nehmen nach dem Start nichts an.
  - ESLint verbietet `eval`, `new Function`, `setTimeout` mit Text und `import()` mit offenem Ziel.
  - Die CSP ist unverändert streng.
  - Der Abhängigkeitsbaum von Rust ist geprüft (siehe Abweichungen).
- **Einstellungen übertragen:**
  - **Export** als `.pwsettings`.
  - **Import** mit Vorschau der Änderungen und der übergangenen Schlüssel. Übernommen wird alles oder nichts; die geprüften Werte bleiben in Rust.
  - **Zurücksetzen** getrennt für App und Bibliothek.
  - **Sicherung:** Vor jedem Import und Zurücksetzen sichert die App den bisherigen Stand (`settings-backups/`, die fünf neuesten).
  - **Ausgeschlossen:** Pfade und Sitzungsdaten dieses Rechners.
  - **Format:** `docs/settings-format.md`.
- **Einstellungen › Erweitert:**
  - Export, Import und Zurücksetzen;
  - die eingebauten Erweiterungen aller Register, nur lesend;
  - die Speicherorte (Konfiguration, Protokoll, Bibliothek) mit „Ordner öffnen“. Der Befehl nimmt einen Namen, nie einen Pfad.
- **Konzeptpapier** `docs/plugin-concept.md`: Bedrohungsmodell, Andockstellen, Sandbox (WebAssembly in Rust, isolierter Rahmen für Oberflächenteile), Berechtigungen, Paket und Signatur, Lebenszyklus, offene Entscheidungen.

## Welche Dateien wurden erstellt oder verändert?

**Rust, neu:**

- `services/registry.rs`, `services/extensions.rs`, `services/formats.rs`;
- `services/export/exporters.rs`, `services/language/engines.rs`;
- `services/settings/transfer.rs`;
- `commands/extensions.rs`, `commands/settings.rs`;
- sieben Berechtigungsdateien.

**Rust, geändert:**

- Dokumente, Import, externe Dateien, Suche, Backup, Wartung, Export, Sprachprüfung;
- `commands/mod.rs`, `commands/system.rs`, `commands/ui.rs`;
- `state.rs`, `lib.rs`, `error.rs` (fünf neue Fehlercodes), `build.rs`, Capability;
- Tests `ipc.rs`, `export.rs`, `language.rs`.

**Frontend, neu:**

- `src/core/registry.ts`, `src/extensions/registries.ts`, `src/extensions/builtin.ts`;
- `src/modules/editor/features.ts`, `src/modules/editor/formats.ts`, `src/modules/export/exporters.ts`;
- `src/modules/settings/AdvancedSettings.tsx`, `src/modules/settings/settingsTransfer.ts`, `src/modules/settings/ExtensionsList.tsx`;
- Tests zu allen neuen Dateien.

**Frontend, geändert:**

- `main.tsx`, `App.tsx`, `commands.ts`;
- `session.ts`, `schema/extensions.ts`, `MeshPanel.tsx`, `renderer.ts`;
- `SettingsDialog.tsx`, `uiStore.ts`, `ipc.ts`, `de.json`, `en.json`;
- `eslint.config.js`, `.prettierignore`, `.gitignore`.

**E2E:** `specs/extensions.e2e.ts` (7 Tests), `specs/settings-transfer.e2e.ts`, `session.ts`, `sandbox.ts`, `wdio.conf.ts`.

**Doku:** ADR 0027, `docs/plugin-concept.md`, `docs/settings-format.md`, `docs/status.md`, `CHANGELOG.md`, `README.md`.

## Welche Tests wurden durchgeführt?

| Art                      | Anzahl                            | Ergebnis                                 |
| ------------------------ | --------------------------------- | ---------------------------------------- |
| Rust (Unit, Integration) | 660 (655 ausgeführt, 5 ignoriert) | bestanden                                |
| Vitest                   | 768 in 47 Dateien                 | bestanden                                |
| E2E (WebdriverIO)        | 22 Spec-Dateien                   | bestanden (lokal, Linux; CI siehe unten) |

Lokal ausgeführt und bestanden:

- Prettier, ESLint, TypeScript, Vitest;
- rustfmt, Clippy (ohne und mit `e2e`), alle Rust-Tests;
- erzeugte Typen und Lizenzliste unverändert;
- 161 Befehle, 161 Berechtigungsdateien, alle in der Capability;
- E2E-Gesamtlauf mit 22 Dateien.

Die Tippverzögerung liegt bei 35 ms (95 %).

- **Register in Rust:** doppelte und falsch geformte IDs brechen ab; die Register enthalten genau die eingebauten IDs. Die Exporttests laufen über das Register, die Dialogfilter sind unverändert. `extensions_list` wird über IPC geprüft.
- **Register im Frontend:**
  - `register` nach dem Versiegeln wirft, mit derselben Startfolge wie `main.tsx`.
  - Editor-Erweiterungen und Schema stimmen in Namen und Reihenfolge mit dem früheren Stand überein.
  - Befehls-IDs, Titel und Reihenfolge der Exportbefehle sind unverändert.
- **Kein fremder Code** (E2E im echten Programm):
  - `eval`, `new Function` und `setTimeout` mit Text scheitern an der CSP.
  - Ein Inline-Skript läuft nicht.
  - Skripte aus `data:`, `blob:` und von fremden Adressen laden nicht; die CSP meldet jeweils einen Verstoß.
  - `import()` von `data:`, `blob:` und fremden Adressen wird abgelehnt, das eigene Modul nicht (Gegenprobe).
  - Die geltende `script-src` enthält weder `unsafe-eval` noch `unsafe-inline`, `data:`, `blob:` oder `*`.
  - Kein Objekt auf `window` bietet ein Register an.
  - Die Liste unter „Erweitert“ zeigt die Einträge beider Seiten, ohne Bedienelemente.
- **Einstellungen:**
  - **Rust:** 18 Unit-Tests: Hin und zurück; falsches Format; neuere Version; ein ungültiger Wert ändert nichts; unbekannte Schlüssel genannt; Sicherungen auf fünf begrenzt; Zurücksetzen; Größengrenze. Dazu 3 IPC-Tests.
  - **Vitest** für die Oberfläche.
  - **E2E:** Export, Zoom ändern, Import mit Vorschau stellt den Zoom wieder her; Zurücksetzen nach Bestätigung; Ordner öffnen.

## Abnahmekriterien Phase 13

| Kriterium (Abschnitt 25)                                            | Stand                                                                                                   |
| ------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Die eigenen Exporter und Dateiformate nutzen die Register           | erfüllt (Rust und Frontend; Tests prüfen Inhalt und Wege der Register)                                  |
| Fremder Code lässt sich nicht laden, belegt durch einen Test        | erfüllt (`extensions.e2e.ts` im echten Programm; Versiegeln in Vitest; Register in Rust unveränderlich) |
| Einstellungen lassen sich exportieren, importieren und zurücksetzen | erfüllt (Rust, IPC, Vitest, E2E `settings-transfer.e2e.ts`)                                             |

## Was konnte nicht getestet werden und warum?

- **`register` nach dem Start im echten Programm:** Das E2E-Programm nutzt dasselbe Frontend wie die Auslieferung. Ein Testzugang zu den Registern würde also mit ausgeliefert und wäre selbst ein Ladeweg. Belegt ist es deshalb in Vitest, mit derselben Startfolge; der E2E-Test zeigt, dass kein Register von außen erreichbar ist.
- **Echte Dateidialoge** für Export und Import sind in CI nicht bedienbar. Die E2E-Builds beantworten sie über eine Umgebungsvariable, wie bei den anderen Dialogen (ADR 0008).
- **„Ordner öffnen“:** Geprüft ist, dass der Befehl den richtigen Ordner an das System weitergibt. Ob der Dateimanager wirklich aufgeht, ist nicht geprüft.
- **macOS:** zurückgestellt.

## Offene Fehler

Keine bekannten.

## Behobene Fehler

- **E2E-Test „kein fremder Code“ unter WebKitGTK:** Der WebDriver von WebKitGTK meldet einen Fehler, den der Code im Programm selbst abfängt (die Ablehnung durch die CSP, ein abgelehntes `import()`), als gescheiterten Befehl. Der Test wertet diese Meldung jetzt als Ablehnung und liest Ergebnis und CSP-Verstöße in einem zweiten Schritt aus dem Programm. Nachgestellt und geprüft mit einer Probe in WebKitGTK; das Programm selbst war nicht fehlerhaft.
- **Gemeinsamer Build-Cache der Agenten:** Ein Build eines Worktrees schrieb einmal Berechtigungsdateien in den anderen. Nach dem Zusammenführen ist geprüft, dass Befehle, Berechtigungsdateien und Capability genau übereinstimmen.

## Gefundene Widersprüche und Abweichungen

1. **„Erweiterte Einstellungen“** nennt Abschnitt 25 ohne Inhalt. Ich verstehe darunter den Abschnitt „Erweitert“ mit Übertragen der Einstellungen, Speicherorten und der Liste der Erweiterungen (ADR 0027, Punkt 5).
2. **Schemata im Editor:** `.pwrite` nutzt das Markdown-Schema; die Editorfunktionen sind daher nach `markdown` und `plain` gegliedert, nicht nach Rich Text.
3. **Natives Menü:** Es bleibt in Rust definiert. Aus dem Register entstehen Befehlspalette und Tastenkürzel.
4. **Abhängigkeitsbaum:** `dlopen2` kommt unter Linux über Tauri (`tao`) hinein. Es lädt nur die feste Systembibliothek `libunity` für Fortschritt und Zähler in der Taskleiste; die App nutzt das nicht, und die Capability erlaubt keine Fensterbefehle. Weitere Wege, dynamisch Code zu laden, gibt es nicht.
5. **Nicht übertragen** werden außerdem `languageCheck.autocorrectAsked` und `storage.acceptedLocation`, weil sie an den Rechner oder Ort gebunden sind.
6. **Ein Zweig je Phase** (Abschnitt 24): Die Agenten arbeiteten auf eigenen lokalen Zweigen; gepusht wurde nur der erlaubte Zweig.

## Entscheidungen (ADR)

- 0027 Erweiterungen: interne Register, kein fremder Code, Einstellungen übertragen; angenommen, mit Nachträgen aus der Umsetzung.

## CI

| Lauf | Ergebnis | Inhalt   |
| ---- | -------- | -------- |
| 121  | LAUF121  | Phase 13 |

## Nächster Schritt

Warten auf die Freigabe von Phase 14 (Qualitätssicherung und Auslieferung). Vorher sind D4 (Lizenz der App, Wörterbücher, MPL- und EPL-Abhängigkeiten) und D11 (Signierung) zu klären.
