# Bericht Phase 14 – Qualitätssicherung und Auslieferung

Stand: 19. November 2026. Zweig `claude/project-write-phase-1-6sh7n2` (die Sitzung erlaubt nur diesen Zweig).

- **Entwicklungsumgebung:** Cloud-Container, Ubuntu 24.04, 4 vCPU, keine GPU (Xvfb, WebKitGTK).
- **CI:** GitHub Actions `windows-latest` und `ubuntu-24.04`, ohne GPU.
- **Referenzgerät** aus Abschnitt 6: nicht verfügbar.

Entscheidungen des Auftraggebers vor Beginn (ADR 0028):

- Lizenz MIT OR Apache-2.0 (D4);
- Installer vorerst unsigniert (D11);
- keine Update-Funktion;
- macOS weiter zurückgestellt.

Phase 14 ist umgesetzt:

- unsignierte Installer für Windows (NSIS, MSI) und Linux (`.deb`, AppImage) mit Test von Installation und Deinstallation in CI;
- ein Netzwerktest für den Normalbetrieb;
- die Sicherheitsprüfung mit Auswertung der Audits;
- eine Prüfung der Barrierefreiheit;
- ein Bericht über alle Zielwerte aus Abschnitt 6;
- die vollständigen Lizenztexte aller Fremdkomponenten.

**Arbeitsweise:** Wie in Phase 13 liefen drei Agenten gleichzeitig: Installer, Netzwerktest und Sicherheit, Barrierefreiheit und Leistung. Ich habe die Zweige zusammengeführt, die vollständigen Lizenztexte ergänzt, die Prüfungen ausgeführt und einen unsicheren Test robust gemacht. Nach einem Neustart des Containers liefen zwei Agenten mit ihrem bisherigen Kontext weiter; verloren ging nichts.

## Was wurde implementiert?

- **Lizenz (ADR 0028):** `LICENSE-MIT`, `LICENSE-APACHE` und `license` in `Cargo.toml` und `package.json`. Rechteinhaber ist „The Project Write authors“, bis der Auftraggeber einen Namen nennt.
- **Vollständige Lizenztexte:** `THIRD-PARTY-NOTICES` (854 KB) enthält jede Lizenzdatei der 441 Rust-Crates und 86 npm-Pakete, die ausgeliefert werden.
  - Jeder Text steht einmal, mit allen Komponenten, die ihn tragen.
  - Für 36 Crates ohne eigene Lizenzdatei steht der Standardtext ihrer Lizenz mit den Autoren aus den Paketangaben.
  - Erzeugt mit `THIRD-PARTY-LICENSES`; CI prüft, dass beide aktuell sind. Die Installer enthalten beide Dateien.
- **Installer** (`tauri.conf.json`, `scripts/installer-test.mjs`, `docs/installation.md`):
  - **Windows:** NSIS (für den Nutzer, deutsch oder englisch) und MSI.
  - **Linux:** `.deb` und AppImage.
  - **Lizenzseite:** zeigt die Wahl MIT oder Apache-2.0 und beide Texte.
  - **CI-Job `package`:** baut die Pakete und prüft die Größe.
  - **CI-Job `install-test`** auf einem frischen Runner:
    1. Leerer Zustand ist vorausgesetzt; bei vorhandener Bibliothek bricht das Skript ab.
    2. Stille Installation.
    3. Dateien werden byte-genau geprüft (Programm, Wörterbücher, Lizenzen); Testplugins dürfen nicht enthalten sein.
    4. Probestart.
    5. Eine Platzhalter-Bibliothek wird angelegt.
    6. Stille Deinstallation.
    7. Programmdateien müssen entfernt sein.
    8. Die Bibliothek muss byte-genau erhalten sein.
- **Netzwerktest** (`scripts/network-trace.bt`, `network-record-linux.sh`, `network-check.mjs`, `docs/sicherheit.md` §5):
  - **Linux:** `bpftrace` zeichnet im Kernel jeden `connect`, `sendto`, `sendmsg` und `sendmmsg` der App und aller Kindprozesse auf, während die ganze E2E-Suite läuft.
  - **Erlaubt** sind nur lokale Sockets, Netlink und Loopback. DNS und mDNS (auch über 127.0.0.53) sind verboten.
  - **Windows:** eine Stichprobe einmal je Sekunde über alle Prozesse der App; der Schritt scheitert bei jeder Verbindung nach außen und jeder DNS-Anfrage.
- **Sicherheitsprüfung** `docs/sicherheit.md`: Abschnitt 20 Punkt für Punkt mit Belegen; Auswertung von `cargo audit`, `cargo deny` und `npm audit`; Neubewertung des Isolation Pattern (ADR 0005).
- **Barrierefreiheit** (`tests/e2e/a11y.ts`, `specs/accessibility.e2e.ts`):
  - Jeder Befehl steht in der Befehlspalette (80 von 97, die übrigen 17 mit Begründung ausgeblendet).
  - Elf Dialoge werden per Tastatur geöffnet, mit Fokusfalle, Escape und Rückgabe des Fokus.
  - Alle Einstellungsbereiche, Fokuswechsel per Tastatur und Kontextmenüs.
  - ARIA-Regeln und gezeichneter Kontrast in beiden Designs.
  - **Sechs Befunde behoben**, darunter der Kontrast von Checkboxen im hellen Design (1,76:1 → ≥ 4,8:1) und die Fokusrückgabe von Palette und Dialogen.
- **Neue Messungen:** Kaltstart bis zur Eingabemöglichkeit und Arbeitsspeicher im Leerlauf, unter Linux und Windows. Bericht `docs/leistung-und-barrierefreiheit.md`.

## Welche Tests wurden durchgeführt?

| Art                         | Anzahl                            | Ergebnis                                                                                          |
| --------------------------- | --------------------------------- | ------------------------------------------------------------------------------------------------- |
| Rust (Unit, Integration)    | 660 (655 ausgeführt, 5 ignoriert) | bestanden                                                                                         |
| Vitest                      | 772 in 47 Dateien                 | bestanden                                                                                         |
| Auswertung Netzwerktest     | 16 (node:test)                    | bestanden                                                                                         |
| E2E (WebdriverIO)           | 23 Spec-Dateien                   | bestanden (lokal Linux; CI-Lauf 127 Linux und Windows)                                            |
| Netzwerktest                | ganze E2E-Suite                   | keine Verbindung (lokal und CI Linux; CI Windows: keine Verbindung nach außen, keine DNS-Anfrage) |
| Installation/Deinstallation | NSIS, MSI, `.deb`, AppImage       | bestanden (CI-Lauf 127 auf frischen Runnern; `.deb` und AppImage auch lokal)                      |

Lokal ausgeführt und bestanden:

- Prettier, ESLint, TypeScript, Vitest;
- rustfmt, Clippy (ohne und mit `e2e`), alle Rust-Tests;
- erzeugte Typen, Lizenzliste und Lizenztexte unverändert, Lizenzseite der Installer aktuell;
- E2E-Gesamtlauf mit Netzwerk-Aufzeichnung, nach F-59 zweimal in Folge grün (23 von 23 Dateien, keine Verbindung).

## Zielwerte aus Abschnitt 6

Werte aus dem letzten lokalen Lauf (Container, Debug-Build mit E2E-Unterstützung) und aus CI-Lauf 127. CI-Werte zeigen nach Abschnitt 6 nur den Trend; Einzelheiten in `docs/leistung-und-barrierefreiheit.md`.

| Merkmal                                     | Ziel                                          | Container                                                                          | CI Linux                         | CI Windows                   | Stand                                                                                                      |
| ------------------------------------------- | --------------------------------------------- | ---------------------------------------------------------------------------------- | -------------------------------- | ---------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Kaltstart bis zur Eingabe                   | ≤ 2 s                                         | 0,49 s (kleine Bibliothek); 1,80 s (Tabs der Sitzung); 1,72 s (Referenzbibliothek) | 0,89 / 1,98 / 1,89 s             | 0,85 / 2,40 / 3,16 s         | erfüllt mit kleiner Bibliothek; mit Tabs und Referenzbibliothek an der Grenze, in CI unter Windows darüber |
| Tippen bei 50.000 Wörtern, Sprachprüfung an | 95 % ≤ 50 ms                                  | 23 ms                                                                              | 27 ms                            | 18,8 ms                      | erfüllt                                                                                                    |
| 50.000 Wörter öffnen                        | ≤ 500 ms                                      | 187–216 ms                                                                         | 207–233 ms                       | 186–206 ms                   | erfüllt                                                                                                    |
| Recovery-Entwurf                            | ≤ 2 s                                         | 872 ms verloren nach hartem Abbruch                                                | 873 ms                           | 508 ms                       | erfüllt                                                                                                    |
| Suche, erste Treffer                        | ≤ 300 ms                                      | 111–125 ms                                                                         | 107–169 ms                       | 107–170 ms                   | erfüllt                                                                                                    |
| Mesh zoomen und verschieben                 | ≥ 30 Bilder/s                                 | Zoomen 98 ms, Verschieben 43 ms (95 %), ohne GPU                                   | 115 / 54 ms                      | 243,5 / 39,8 ms              | nicht erfüllt ohne GPU (ADR 0022)                                                                          |
| Arbeitsspeicher im Leerlauf                 | ≤ 500 MB                                      | 437 MB (kleine Bibliothek); 522 MB (Referenzbibliothek), jeweils PSS               | 441 / 536 MB (PSS)               | 129 / 175 MB (Private Bytes) | unter Linux mit der Referenzbibliothek knapp verfehlt (Debug-Build); Windows erfüllt                       |
| Installationspaket                          | ≤ 50 MB                                       | `.deb` 10,3 MB; AppImage 88,7 MB                                                   | `.deb` 10,4 MB; AppImage 88,8 MB | NSIS 7,7 MB; MSI 10,1 MB     | `.deb`, NSIS und MSI erfüllt; AppImage nicht (siehe unten)                                                 |
| 100 harte Abbrüche beim Speichern           | keine beschädigte Datei, `integrity_check` ok | 100 von 100, ok                                                                    | in `cargo test`                  | in `cargo test`              | erfüllt                                                                                                    |
| Barrierefreiheit                            | Tastatur; Kontrast 4,5:1 / 3:1                | Text ≥ 5,45:1, Grafik ≥ 4,83:1 (hell)                                              | grün, Kontrast wie lokal         | grün, Kontrast wie lokal     | erfüllt im geprüften Umfang                                                                                |

CI-Spalten: Kaltstart kleine Bibliothek / Tabs der Sitzung / Referenzbibliothek; Mesh Zoomen / Verschieben (95 % der Bilder); Arbeitsspeicher kleine Bibliothek / Referenzbibliothek.

## Abnahmekriterien Phase 14

| Kriterium (Abschnitt 25)                                                                                                 | Stand                                                                                                                  |
| ------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------- |
| Alle Zielwerte aus Abschnitt 6 sind gemessen und in einem Bericht dokumentiert                                           | erfüllt (`docs/leistung-und-barrierefreiheit.md`, lokal und CI-Lauf 127); zwei Werte verfehlt, einer knapp, siehe oben |
| Der Netzwerktest zeigt im Normalbetrieb keine Verbindung                                                                 | erfüllt: Linux lokal und in CI vollständig im Kernel aufgezeichnet; Windows in CI als Stichprobe ohne Befund           |
| Installation und Deinstallation auf einem sauberen System je Betriebssystem getestet, die Bibliothek bleibt unangetastet | erfüllt für Windows (NSIS, MSI) und Linux (`.deb`, AppImage) in CI-Lauf 127; macOS zurückgestellt                      |
| Der Release-Build enthält keine Testplugins                                                                              | erfüllt (Job „Release build contains no test plugins“, dazu Prüfung der installierten Programme)                       |
| `cargo audit`, `cargo deny` und `npm audit` ausgewertet, Lizenzliste vollständig                                         | erfüllt (`docs/sicherheit.md` §6; `THIRD-PARTY-LICENSES` und `THIRD-PARTY-NOTICES`)                                    |

## Was konnte nicht getestet werden und warum?

- **Referenzgerät:** nicht verfügbar; alle Werte aus Container und CI ohne GPU, im Debug-Build.
- **Windows-Netzwerktest:** nur eine Stichprobe je Sekunde. Kürzere Verbindungen und Namensauflösungen innerhalb von WebView2 können fehlen.
- **LanguageTool:** In CI gibt es kein Java; ob ein echtes LanguageTool selbst Verbindungen aufbaut, ist nicht gemessen.
- **Echte Tab-Navigation und Screenreader:** Der WebDriver sendet synthetische Tasten (ADR 0003). Ein Handtest mit Tastatur, NVDA und Orca steht aus.
- **Installation unter Windows auf einem echten Rechner** mit SmartScreen und Smart App Control: Nur der CI-Runner ist geprüft.
- **macOS:** zurückgestellt (ADR 0028).

## Offene Fehler

Keine bekannten.

## Behobene Fehler

- **Barrierefreiheit:**
  - Kontrast der Checkboxen und Optionsfelder im hellen Design;
  - Fokusrückgabe der Befehlspalette;
  - Fokusrückgabe von Dialogen mit `autoFocus`;
  - Schließen-Schaltfläche innerhalb eines Tabs;
  - Kontextmenü innerhalb der Tab-Liste;
  - `aria-controls` auf nicht vorhandene Bereiche.
- **Lizenztexte:** Den Installern lag nur die Liste der Fremdkomponenten bei, nicht deren Texte und Copyright-Hinweise. Jetzt liegt `THIRD-PARTY-NOTICES` bei.
- **E2E `media.e2e.ts`:** Der Test eingefügter Bilder wartete nicht auf das Einfügen; das war eine Wettlaufsituation nur im Test.
- **E2E `accessibility.e2e.ts`:** Im ersten Gesamtlauf mit Netzwerk-Aufzeichnung verlor der WebDriver eine getippte Taste (wie F-27), und drei Dateien scheiterten in Folge. Der Test prüft jetzt den getippten Titel und tippt bei Abweichung neu. Danach lief die Suite zweimal mit Aufzeichnung grün.
- **F-59, Befehlspalette verlor den Fokus:** Kam der verspätete Fokus des Editors (Tiptap, nächster Animationsframe) erst nach dem Öffnen der Palette an, lag der Fokus im Text, und `Enter` fügte eine Zeile ein, statt den Befehl auszuführen. Der Barrierefreiheitstest deckte das auf, als er nur noch den Anfang eines Befehlstitels tippte. Die Palette hält den Fokus jetzt wie ein modaler Dialog (wie F-58); ein Vitest-Fall scheitert mit dem alten Code. **Nachtrag (CI-Lauf 125):** Die erste Fassung der Fokusfalle hielt den Fokus auch dann fest, wenn ein Befehl ihn beim Schließen der Palette selbst verschob (etwa ins Umbenennen-Feld des Explorers); sieben Spec-Dateien scheiterten. Die Palette gibt den Fokus jetzt frei, sobald sie einen Befehl ausführt oder schließt; ein zweiter Vitest-Fall scheitert mit dem Stand von Lauf 125.
- **Netzwerk-Aufzeichnung in CI (Lauf 123):** Das Startskript prüfte das Trace-Dateisystem als normaler Nutzer; auf dem GitHub-Runner darf das nur root, der dann versuchte `mount` brach ab. Die Prüfung läuft jetzt mit root-Rechten, ein gescheitertes `mount` bricht nicht mehr ab.
- **Paketquellen in CI (Lauf 126):** Auf einem Linux-Runner fehlte nach `apt-get update` das Ubuntu-Archiv im Paketindex. Alle Linux-Schritte installieren Pakete jetzt über `scripts/apt-install.sh`: Es wiederholt Aktualisierung und Installation, prüft vorher, ob das erste Paket im Index steht, und stellt notfalls die Standardquellen des Runners wieder her.
- **WebView2 unter Windows (Lauf 123):** Verbindungen zu Microsoft und DNS-Anfragen nach `wpad` aus dem Prozessbaum der App. Behoben mit Startschaltern für WebView2 (siehe Abweichung 5).
- **Netzwerktest mit `strace`:** Er bremste die App so stark, dass 12 bis 13 Spec-Dateien scheiterten und das Tippen 516 ms brauchte. Er misst damit keinen Normalbetrieb mehr und ist durch `bpftrace` im Kernel ersetzt.

## Gefundene Widersprüche und Abweichungen

1. **AppImage über 50 MB (88,8 MB in CI, 88,7 MB lokal):** Ein AppImage muss WebKitGTK, JavaScriptCore und ICU mitbringen; das `.deb` und die Windows-Installer nehmen die Laufzeit vom System. CI misst die Größe des AppImage, lässt den Lauf aber nicht daran scheitern. **Entscheidung des Auftraggebers nötig:** Zielwert für das AppImage nicht anwenden oder AppImage weglassen.
2. **Arbeitsspeicher mit der Referenzbibliothek:** 522 bis 535 MB PSS im Debug-Build, Ziel 500 MB. Gut die Hälfte gehört dem Webprozess. Eine Messung mit Release-Build auf dem Referenzgerät steht aus.
3. **Mesh mit 30 Bildern pro Sekunde:** ohne GPU nicht erreicht. Die Entscheidung zum Renderer steht in ADR 0022; die Messung mit GPU steht weiter aus.
4. **Kaltstart:** Mit wiederhergestellten Tabs und mit der Referenzbibliothek liegt er bei 1,7 bis 2,0 s, also an der Grenze. In einem Lauf lag er bei 2,003 s. Im ersten Lauf nach einem Neustart des Containers, also mit leerem Datei-Cache, dauerte der Start mit der Referenzbibliothek einmal 35,8 s; die Wiederholung lag bei 1,9 s. Dasselbe trat lokal im jeweils ersten Lauf nach einem Neubau auf (36,8 s; die Seite begann erst nach 35,5 s zu laden, die Zeit vergeht also vor dem Fenster). Die Referenzbibliothek liegt dabei im lokalen Cache und wird von Lauf zu Lauf weiter benutzt; in CI, wo sie frisch entsteht, lag der Wert unter Linux bei 1,9 bis 2,4 s. Unter Windows lag er in CI-Lauf 127 bei 2,40 s (Tabs) und 3,16 s (Referenzbibliothek); dort begann das Öffnen der Bibliothek erst nach 1,84 s. Die Ursache ist nicht geklärt; die Messung auf dem Referenzgerät mit einem echten Neustart steht aus.
5. **Windows-Netzwerktest:** nur als Stichprobe (siehe oben). In CI-Lauf 123 hielt WebView2 Verbindungen zu zwei Microsoft-Adressen und fragte per DNS nach `wpad`; die App selbst verband nur über Loopback. Seitdem startet WebView2 mit Schaltern ohne Hintergrund-Netzverkehr, Komponenten-Updates und Proxy-Suche (`additionalBrowserArgs`, `docs/sicherheit.md` §5), und der Windows-Schritt scheitert bei jedem Befund.
6. **Barrierefreiheit:** keine neue Abhängigkeit (axe-core). Die Regeln prüft ein eigenes Modul, weil axe-core als eingespritzter Quelltext an der CSP und den eingefrorenen Prototypen scheitern würde.
7. **Ein Zweig je Phase** (Abschnitt 24): Die Agenten arbeiteten auf lokalen Zweigen; gepusht wurde nur der erlaubte Zweig.

## Entscheidungen (ADR)

- 0028 Lizenz MIT OR Apache-2.0, vorerst unsigniert, keine Updates, Windows und Linux (D4, D11); angenommen.
- 0005 Isolation Pattern: Neubewertung vor Phase 14, bleibt aus.

## CI

| Lauf | Ergebnis    | Inhalt                                                                                                                                                                                                                                                                                      |
| ---- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 123  | rot (Linux) | E2E auf beiden Systemen grün, Installer und Installationstests grün; Netzwerk-Aufzeichnung startete auf dem Runner nicht (Prüfung des Trace-Dateisystems ohne root); Windows-Stichprobe fand Verbindungen von WebView2                                                                      |
| 124  | rot (Linux) | Netzwerk-Aufzeichnung lief, keine Verbindung; Windows-Netzwerktest streng und grün; `accessibility.e2e.ts` scheiterte (F-59)                                                                                                                                                                |
| 125  | rot (Linux) | Regression durch die erste Fassung von F-59 (sieben Spec-Dateien); Netzwerktest Linux und Windows grün, Installer grün                                                                                                                                                                      |
| 126  | rot (Linux) | Alles grün bis auf den Installer-Job Linux: `apt-get update` lud nur die Microsoft-Paketquelle, WebKitGTK fehlte im Index (Störung des Runners); Installationstest übersprungen                                                                                                             |
| 127  | grün        | Alle acht Jobs grün: E2E 23/23 unter Linux und Windows, Netzwerktest Linux (nur lokale Sockets) und Windows (keine Verbindung nach außen, keine DNS-Anfrage), Installer und Installationstests auf beiden Systemen; die Paketinstallation im Linux-Prüfjob brauchte 25 min, lief aber durch |

## Nächster Schritt

Abschnitt 25 endet mit Phase 14. CI-Lauf 127 ist vollständig grün. Ich brauche vom Auftraggeber:

1. die Entscheidung zum AppImage;
2. gegebenenfalls einen Namen als Rechteinhaber;
3. die Freigabe der offenen Handtests: Installation unter Windows, Tastatur und Screenreader, Messung auf dem Referenzgerät.
