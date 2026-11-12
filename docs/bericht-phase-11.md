# Bericht Phase 11 – Mesh

Stand: 12. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig). Entwicklungsumgebung: Cloud-Container, Ubuntu 24.04, 4 vCPU, keine GPU (Xvfb, Software-Rendering), WebKitGTK. CI: GitHub Actions `windows-latest` und `ubuntu-24.04`, beide ohne GPU; macOS ist zurückgestellt.

ZUSAMMENFASSUNG

## Was wurde implementiert?

Entscheidungen in ADR 0022.

- **Lasttest zuerst:** Das Mesh zeichnet in der Referenzbibliothek 1.000 Knoten und 531 Kanten. Gemessen werden je 120 Bilder Zoomen und Verschieben, dazu zum Vergleich Verschieben ohne Kanten und ohne Knoten. Die Messwerte stehen unter „Lasttest“.
- **Daten (Migration 0010):** Es gibt einen Knoten je Dokument, Medium und Subnetz; Trigger legen ihn an, die Migration trägt bestehende nach. Bestätigte Verbindungen sind sortiert und je Paar und Art eindeutig. Vorschläge liegen getrennt davon mit Status. Layouts und Positionen werden je Ansicht gespeichert.
- **Ansichten in Rust:**
  - **Gesamtnetz:** die obersten Subnetze als geschlossene Cluster, dazu „Nicht zugeordnet“.
  - **Subnetzansicht** mit Breadcrumb.
  - **Fokusansicht:** eine oder zwei Stufen.
  - **Verdichtung:** Verbindungen zu nicht gezeichneten Elementen wandern zum Cluster und werden summiert.
  - **Grenze:** höchstens 1.000 Knoten, Eltern vor Kindern.
- **Mesh-Tab** (`Strg + Umschalt + M`, Aktivitätsleiste) mit React Flow hinter einer Renderer-Schnittstelle:
  - **Knoten:** farbige Hüllen, Symbole je Art, Medien orange, Archiviertes grau.
  - **Kanten:** Verbindung durchgezogen, angenommener Vorschlag gestrichelt, Verweis mit Pfeil, weiteres Subnetz gepunktet.
  - **Ansicht:** Minimap und Detailstufe.
- **Bedienung nach Abschnitt 15:**
  - **Maus:** Klick, `Umschalt + Klick`, Doppelklick, Rechtsklick (Kontextmenü).
  - **Ziehen:** Verschieben fixiert den Knoten. Ablegen auf einem Subnetz ordnet zu, auch aus dem Explorer, mit Rückgängig.
  - **Verbinden und Entfernen:** Griff zum Verbinden, `Entf` für bestätigte Verbindungen.
  - **Abgeleitete Kanten** führen über das Kontextmenü zu ihrer Quelle.
  - **Tags** heben Elemente hervor statt Kanten zu bilden.
- **Tastatur:** Pfeiltasten, Enter, Leertaste, `F`, `C`, `E`, `Entf`, `Shift + F10`, Escape, `+`, `-`, `0`, `L`, `Strg + Z`. Eine Live-Region nennt die Auswahl.
- **Automatisches Layout:** ELK.js im Web Worker. Fixierte Knoten bleiben stehen. „Alles neu anordnen“ bewegt auch sie. Beides lässt sich rückgängig machen.
- **Vorschläge:**
  - **Merkmale:** Tags, Titel-Trigramme, TF-IDF mit Kosinus, Links, Importordner und Subnetze, gewichtet über die Teile, die für das Paar vorliegen.
  - **Schwelle:** 0,35 zu Beginn, einstellbar.
  - **Lauf:** im Hintergrund, abbrechbar; nur Paare, die etwas teilen.
  - **Entscheidung:** Annehmen, Ablehnen (das Paar kommt nicht wieder) oder Später. Jeder Vorschlag nennt seine Gründe.

## Welche Dateien wurden erstellt oder verändert?

**Rust, neu:**

- `migrations/0010_mesh.sql`
- `database/mesh.rs`
- `services/mesh/{mod,graph,suggestions}.rs`
- `commands/mesh.rs`
- `tests/mesh.rs`

**Rust, geändert:**

- `services/library/mod.rs`, `services/session.rs`
- `services/settings/library.rs` (`mesh.suggestionThreshold`)
- `error.rs` (6 Fehlercodes)
- `commands/mod.rs`, `build.rs` (auch `rerun-if-changed` für Migrationen)
- `capabilities/main-window.json`
- `tests/migrations.rs`

**Frontend, neu:**

- `modules/mesh/{MeshPanel,MeshCanvas,SuggestionsPanel}.tsx`
- `modules/mesh/{meshStore,suggestionStore,layout,renderer,autoLayout,elkWorker}.ts`
- `modules/mesh/mesh.test.ts`

**Frontend, geändert:**

- `app/{Workbench.tsx,commands.ts}`
- `modules/commands/{types,keymap}.ts`
- `modules/documents/workspaceStore.ts`, `modules/media/MediaPanels.tsx`
- `modules/settings/ShortcutSettings.tsx` (F-54)
- `services/ipc.ts`, `i18n/{de,en}.json`, `types/generated/`
- `vite.config.ts` (d3-color unter dem eingefrorenen Prototyp)

**E2E:**

- neu: `specs/mesh.e2e.ts`, `measurements.ts`
- geändert: `specs/{reference,workbench,export-backup}.e2e.ts`, `wdio.conf.ts`

**Doku:**

- ADR 0022, `docs/{architecture,database,dependencies,status}.md`, dieser Bericht, `CHANGELOG.md`, `README.md`
- `THIRD-PARTY-LICENSES`

Neue Abhängigkeiten:

- `@xyflow/react` 12.12.0 (MIT) mit d3-Teilen (ISC, BSD-3-Clause) und einer eigenen Kopie von `zustand` 4;
- `elkjs` 0.12.1 („EPL-2.0 OR GPL-3.0-or-later“, gewählt EPL-2.0).

## Welche Tests wurden durchgeführt?

TESTZAHLEN

Lokal (Linux-Container, Xvfb), ausgeführt und bestanden: Prettier, ESLint, TypeScript, Vitest, rustfmt, Clippy (Linux, Linux + `e2e`, Windows-Ziel), alle Rust-Tests, erzeugte Typen, Lizenzliste, E2E-Gesamtlauf mit 19 Dateien.

- **Datenvergleich nach einem Vorschlagslauf** (Rust): Verglichen werden alle Zeilen von `mesh_connections`, `document_links`, `document_media`, `subnet_memberships` und `document_tags` vor und nach dem Lauf. Sie sind gleich; geprüft wird das auch nach einer Ablehnung, einer Annahme und einem zweiten Lauf.
- **Vorschläge nur in `mesh_suggestions`, abgelehnte nie wieder** (Rust, E2E): Der zweite Lauf erzeugt nichts Neues. Das abgelehnte Paar bleibt `rejected`. Im Programm wird ein abgelehnter Vorschlag nach einem neuen Lauf nicht mehr angezeigt. Angenommen entsteht genau eine Verbindung `similarity`.
- **Abgeleitetes nicht in `mesh_connections`** (Rust): Links, Medien im Text und Zuordnungen erscheinen in den Ansichten, `mesh_connections` hält nur die von Hand gezogene Verbindung.
- **Tastatur** (E2E): Das Mesh öffnet mit `Strg + Umschalt + M`. Pfeiltasten wählen Knoten, Enter zeigt ein Subnetz, Escape geht zurück. Per Tastatur zwei Dokumente verbinden, mit `E` und `Entf` die Verbindung entfernen und mit `Strg + Z` zurückholen. `Shift + F10` öffnet das Kontextmenü.
- **Layout** (E2E): ELK.js läuft im Worker des echten Webviews und ordnet an. Ein fixierter Knoten bleibt, wo er war.
- **Ansichten und Verdichtung** (Rust): geschlossene Cluster mit Elementzahl und summierten Kanten, Inhalte in der Hülle, Subnetzansicht nur mit Innerem, Fokusansicht mit einer und zwei Stufen, Grenze von 1.000 Knoten mit Eltern vor Kindern, Positionen je Ansicht.
- **Tags** (E2E): Ein gewählter Tag macht die übrigen Elemente blass; gleiche Tags bilden keine Kante.

## Lasttest

LASTTEST

## Abnahmekriterien Phase 11

ABNAHME

## Was konnte nicht getestet werden und warum?

- **Echte Grafikhardware:** Alle Messungen laufen ohne GPU (Container und CI). Auf Rechnern mit GPU-Compositing ist Verschieben nur eine Transformation; dort erwarte ich bessere Werte, gemessen ist das nicht. Eine Messung auf einem Referenzgerät steht aus (wie seit Phase 7 für den Explorer).
- **macOS:** zurückgestellt.
- **Ziehen mit der Maus im Test:** Das Ablegen auf einem Subnetz und das Ziehen von Verbindungsgriffen prüfen weder Unit- noch E2E-Tests mit echten Mausbewegungen. Der WebDriver sendet bei einem Klick nur `click`, keine Folge von `pointerdown`, `pointermove` und `pointerup`. Geprüft sind die Wege dahinter: Zuordnen über die Struktur, Verbinden über den Befehl.
- **Vorschläge mit der Referenzbibliothek:** Die Analyse ist mit kleinen Bibliotheken geprüft (Rust, E2E). Ihre Laufzeit mit 15.000 Elementen ist nicht gemessen.

## Offene Fehler

Keine bekannten. Eine Beobachtung, wie in Phase 10: Die Galerie-Messung aus Phase 8 lag im Container noch einmal knapp über ihrer Grenze (zweiter Durchgang 56 ms bei 95 %, Grenze 50 ms). In zwei Wiederholungen waren es 37 und 43 ms, in CI liegt sie immer darunter. Ich halte es weiter für Schwankung des Containers.

## Behobene Fehler

- **F-54 (CI-Lauf 113, Linux):** Im Test der Tastenkürzel bekam der Rekorder den Fokus nicht; danach scheiterten 13 Spec-Dateien als Folge. Der Rekorder nimmt den Fokus jetzt auch beim Einhängen.
- **Während der Arbeit gefunden:**
  - d3-color warf unter dem eingefrorenen `Object.prototype` der App; behoben beim Bauen, ADR 0022, Punkt 9.
  - Das automatische Anordnen arbeitete mit dem zuletzt geladenen Graphen und hätte einen eben fixierten Knoten versetzt; es lädt jetzt vorher neu.
  - Die Pfeilnavigation sprang nach unten zu einem Knoten fast auf gleicher Höhe; sie sucht jetzt zuerst in einem Kegel von 45°.
  - Ein Verdacht auf einen Konflikt zwischen `Strg + Umschalt + P` und Drucken erwies sich als nicht vorhanden.

## Gefundene Widersprüche und Abweichungen

1. **Gewichtung der Vorschläge:** Abschnitt 15 nennt „eine gewichtete Summe“. Ich teile die Summe durch die Gewichte der Merkmale, die für das Paar vorliegen. Sonst gälte etwa ein Medium ohne Text als unähnlich, und ähnliche Dokumente blieben unter 0,35. Der Nenner ist mindestens 0,5, damit ein ähnlicher Titel allein nichts vorschlägt (ADR 0022, Punkt 8).
2. **„Nicht zugeordnet“ als Cluster:** Elemente ohne Subnetz bilden im Gesamtnetz einen eigenen Cluster, wie im Explorer. Er ist kein Datensatz, und seine Position wird nicht gespeichert.
3. **Rückgängig beim Löschen eines Subnetzes:** Das Rückgängig der Struktur stellt das Subnetz wieder her, aber nicht seine Mesh-Verbindungen und Positionen.
4. **Mesh als Bild exportieren** (Abschnitt 4) gehört nicht zum Umfang von Phase 11 in Abschnitt 25 und fehlt noch.
5. **Beispiel-Subnetze** „Projekte“, „Wissen“ und „Ideen“ in Abschnitt 15 zeigen die Standardfarben. Die App legt sie nicht an.
6. **ELK.js unter EPL-2.0:** gewählt aus „EPL-2.0 OR GPL-3.0-or-later“. Vor Phase 14 mit D4 zu bestätigen (wie die MPL-Crates).
7. **Ein Zweig je Phase** (Abschnitt 24): weiterhin nur der erlaubte Zweig.

## Entscheidungen (ADR)

- 0022 Mesh: Renderer nach Lasttest, Ansichten, Verbindungen, Layout und Vorschläge, angenommen.

## CI

CI_ABSCHNITT

## Nächster Schritt

Nach Freigabe: Phase 12 (Sprachprüfung; Abschnitt 25).
