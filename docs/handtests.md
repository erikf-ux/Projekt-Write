# Handtests – Project Write

Freigegeben vom Auftraggeber am 19. November 2026 (ADR 0028, Nachtrag).

Diese Tests brauchen ein echtes Gerät und eine Person davor. CI und der Entwicklungscontainer können sie nicht ausführen:

- Der eingebettete WebDriver sendet nur synthetische Tasten und Ereignisse (ADR 0003).
- Native Dialoge, SmartScreen und Screenreader lassen sich dort nicht bedienen.
- Die Runner haben keine GPU.

Jeder Test nennt, was zu tun ist, und was als bestanden gilt. Ergebnisse bitte in die Tabelle am Ende eintragen. Ein Befund bekommt eine F-Nummer in `docs/status.md`.

## Vorbereitung

- **Installer:** aus dem letzten grünen CI-Lauf, Artefakte `installers-Windows` und `installers-Linux` (sieben Tage aufbewahrt). Alternativ selbst bauen: `npx tauri build --bundles nsis,msi` unter Windows, `npx tauri build --bundles deb,appimage` unter Linux.
- **Testbibliothek:** Eine Bibliothek mit ein paar Dokumenten, Bildern und einer PDF genügt. Für die Messungen die Referenzbibliothek: `node scripts/reference-library.mjs` (10.000 Dokumente, 5.000 Medien).
- **Vorher sichern:** Wer eine echte Bibliothek hat, legt vorher ein Backup an (Befehl „Backup erstellen“ in der Befehlspalette).

## 1. Installation unter Windows (echter Rechner)

1. Den NSIS-Installer `Project Write_<Version>_x64-setup.exe` aus dem Browser herunterladen und starten.
   - **Erwartet:** SmartScreen warnt („Der Computer wurde durch Windows geschützt“). Über „Weitere Informationen“ → „Trotzdem ausführen“ geht es weiter.
   - Mit Smart App Control (Windows 11) blockiert Windows die Installation ganz; das ist bekannt und dokumentiert (`docs/installation.md`). Bitte notieren, ob Smart App Control aktiv war.
2. Installieren. Die Lizenzseite zeigt die Wahl zwischen MIT und Apache-2.0.
3. Über das Startmenü starten, Bibliothek wählen, ein Dokument schreiben, schließen und wieder öffnen.
4. **Dateizuordnung:**
   - Eine `.pwrite`-Datei im Explorer doppelklicken: Sie öffnet sich in Project Write, auch wenn die App schon läuft.
   - Bei einer `.md`-Datei „Öffnen mit“ → Project Write wählen.
5. **Ordnerdialog:** Beim ersten Start „Ordner wählen“ öffnet den echten Windows-Dialog; die gewählte Bibliothek wird angelegt.
6. **Drucken:** Datei › Drucken öffnet den Systemdialog; „Microsoft Print to PDF“ ergibt eine PDF mit Bild und Seitenumbrüchen.
7. Über „Apps & Features“ deinstallieren.
   - **Erwartet:** Programm und Startmenüeintrag sind weg; der Bibliotheksordner ist unverändert da.
8. Dasselbe mit dem MSI-Paket (`msiexec` oder Doppelklick), Schritte 2, 3 und 7.

**Bestanden:** alle Schritte wie erwartet, die Bibliothek ist nach der Deinstallation vollständig.

## 2. Installation unter Linux (echter Desktop)

1. `.deb` installieren: `sudo apt install "./Project Write_<Version>_amd64.deb"`.
   - **Erwartet:** apt holt WebKitGTK und GTK selbst.
2. Aus dem Anwendungsmenü starten. Dann wie Windows 1.3 bis 1.6; Drucken über den GTK-Dialog, „In Datei drucken“.
3. **Dateizuordnung:** `.pwrite` doppelklicken und „Öffnen mit“ im Dateimanager.
4. **Ablegen aus dem Dateimanager:** ein Bild in die Schreibfläche ziehen und eine Datei in den Explorer ziehen. Außerdem ein echtes Bildschirmfoto einfügen (`Strg + V`).
5. Deinstallieren mit `sudo apt purge project-write`.
   - **Erwartet:** Die Bibliothek bleibt.
6. **AppImage:** `chmod +x` setzen, starten, schließen, die Datei löschen. Unter Ubuntu 24.04 ist `libfuse2t64` nötig.

## 3. Tastatur (Windows und Linux)

Ohne Maus, nur mit `Tab`, `Umschalt + Tab`, Pfeiltasten, `Enter`, `Escape`, `Shift + F10` und der Kontextmenütaste:

1. Durch die ganze Arbeitsfläche wandern: Aktivitätsleiste, Explorer, Tabs, Schreibfläche, Statusleiste.
   - **Erwartet:** Der Fokus ist immer sichtbar und die Reihenfolge logisch; man kommt aus keinem Bereich nicht mehr heraus (außer aus Dialogen, siehe 2).
2. Jeden Dialog öffnen, zum Beispiel Einstellungen, Umbenennen, Papierkorb leeren, Export und Backup.
   - **Erwartet:** `Tab` bleibt im Dialog, `Escape` schließt ihn, danach steht der Fokus wieder dort, wo er vorher war.
3. **Befehlspalette** (`Strg + Umschalt + P`): einen Befehl suchen und mit `Enter` ausführen. Auch Befehle, die den Fokus selbst setzen, etwa „Umbenennen“ (F-59).
4. **Mesh:** mit den Pfeiltasten Knoten wählen, mit `Enter` öffnen, mit `C` verbinden, mit `Entf` entfernen.
5. **Eingabe mit Komposition:** Tottasten (`^` + `e`) und unter Linux eine IME. Außerdem `Strg + Umschalt + V` aus einem echten Programm.

## 4. Screenreader

NVDA unter Windows, Orca unter Linux:

1. Die Befehlspalette ansagen lassen: Titel, die aktive Option beim Blättern und die Zahl der Treffer.
2. Dialoge: Der Titel wird beim Öffnen angesagt; Schaltflächen haben verständliche Namen.
3. Den Explorer-Baum ansagen lassen: Ebene, auf- und zugeklappt, Name.
4. Statusmeldungen wie „Gespeichert“, „Kein Treffer“ oder ein Exportergebnis werden angesagt, ohne den Fokus zu verschieben.
5. Unterstreichungen der Sprachprüfung: Das Kontextmenü mit den Vorschlägen ist per Tastatur erreichbar und wird angesagt.

**Bestanden:** Alles ist ohne Bildschirm bedienbar und verständlich. Bitte jede Stelle notieren, an der der Screenreader schweigt oder Unverständliches sagt.

## 5. Messung auf dem Referenzgerät

Referenzgerät nach Abschnitt 6: ein vier Jahre alter Mittelklasse-Laptop, 8 GB RAM, SSD, mit GPU.

1. **Automatisch** (braucht die Entwicklungsumgebung aus `README.md`):
   1. `npm ci`
   2. `npm run build:e2e`
   3. `npm run test:e2e` (Linux: `dbus-run-session -- xvfb-run -a npm run test:e2e`, besser direkt am Bildschirm ohne `xvfb-run`)

   Die Werte stehen am Ende im Abschnitt „Measurements“: Kaltstart, Arbeitsspeicher, Tippen, Öffnen, Suche, Mesh, Galerie und Explorer. Das ist ein Debug-Build; die Werte sind eher zu hoch.

2. **Von Hand mit dem Release-Build** (installiert aus 1 oder 2):
   - Kaltstart direkt nach einem Neustart des Geräts (leerer Datei-Cache), mit kleiner Bibliothek und mit der Referenzbibliothek. Gemessen wird vom Klick bis zum ersten möglichen Tastendruck; Stoppuhr oder Bildschirmaufnahme genügt.
   - Arbeitsspeicher 10 s nach dem Start ohne Eingabe:
     - Windows: Task-Manager › Details, „Arbeitssatz (privat)“ von `project-write.exe` und allen `msedgewebview2.exe` der App;
     - Linux: zum Beispiel `smem -P 'project-write|WebKit'` (Paket `smem`), Spalte PSS.
   - **Mesh mit 1.000 Knoten:** Zoomen und Verschieben fühlen sich flüssig an. Genaue Bildzeiten liefert die automatische Messung aus Schritt 1.
3. **LanguageTool:** mit einer echten Installation (Java 17, LanguageTool 6.x) einschalten, einen Absatz prüfen und dabei beobachten, ob Verbindungen nach außen entstehen (Windows: `Get-NetTCPConnection`; Linux: `ss -tnp`).

Zielwerte: `docs/leistung-und-barrierefreiheit.md`.

## Protokoll

| Test                    | Gerät, System, Version | Datum | Ergebnis | Befunde |
| ----------------------- | ---------------------- | ----- | -------- | ------- |
| 1. Installation Windows |                        |       |          |         |
| 2. Installation Linux   |                        |       |          |         |
| 3. Tastatur             |                        |       |          |         |
| 4. Screenreader         |                        |       |          |         |
| 5. Referenzgerät        |                        |       |          |         |
