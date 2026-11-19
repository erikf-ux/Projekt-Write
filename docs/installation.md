# Installation und Deinstallation – Project Write

Stand: 18. November 2026 (Phase 14). Grundlage: ADR 0028 (Lizenz, Signierung, Updates, Plattformen).

Project Write wird für **Windows** (NSIS-Installer und MSI-Paket) und **Linux** (`.deb`-Paket und AppImage) ausgeliefert. macOS ist zurückgestellt. Alle Pakete sind **unsigniert**, und die App hat **keine Update-Funktion**: Eine neue Version installiert man von Hand mit dem neuen Installer über die alte.

## Pakete

| Plattform | Datei                                    | Art                                                          |
| --------- | ---------------------------------------- | ------------------------------------------------------------ |
| Windows   | `Project Write_<Version>_x64-setup.exe`  | NSIS-Installer, installiert nur für den angemeldeten Nutzer  |
| Windows   | `Project Write_<Version>_x64_en-US.msi`  | MSI-Paket, installiert für alle Nutzer (Administratorrechte) |
| Linux     | `Project Write_<Version>_amd64.deb`      | Paket für Debian, Ubuntu und Abkömmlinge                     |
| Linux     | `Project Write_<Version>_amd64.AppImage` | Einzeldatei ohne Installation, für andere Distributionen     |

Jedes Paket enthält das Programm, die mitgelieferten Wörterbücher (`dictionaries/`, je Sprache mit eigener Lizenzdatei; dazu der Quelltext von igerman98 in `dictionaries/sources/`, ADR 0024) sowie die Lizenztexte `LICENSE-MIT`, `LICENSE-APACHE`, die Liste der Fremdkomponenten `THIRD-PARTY-LICENSES` und deren vollständige Lizenztexte und Copyright-Hinweise `THIRD-PARTY-NOTICES`. Die Lizenzseite der Windows-Installer zeigt `src-tauri/installer/license.txt`: die Wahl zwischen MIT und Apache-2.0 und beide Texte unverändert (erzeugt mit `node scripts/installer-license.mjs`, CI prüft, dass die Datei aktuell ist).

Testplugins sind in keinem Paket enthalten (Abschnitt 20): Der Installer-Test sucht im installierten Programm nach dem WebDriver-Plugin und bricht ab, wenn er es findet.

### Paketgrößen

Zielwert aus Abschnitt 6: Installationspaket ohne Zusatzpakete **≤ 50 MB** (gerechnet mit 1 MB = 1.000.000 Byte, die strengere Lesart). CI misst jedes Paket im Job „Installers“ und bricht ab, wenn NSIS, MSI oder `.deb` größer sind; die Werte stehen in der Zusammenfassung des Laufs.

| Paket         | Größe                     | Gemessen                                    |
| ------------- | ------------------------- | ------------------------------------------- |
| NSIS (`.exe`) | 7,7 MB (7.742.263 Byte)   | CI-Lauf 127, `windows-latest`               |
| MSI           | 10,1 MB (10.055.680 Byte) | CI-Lauf 127, `windows-latest`               |
| `.deb`        | 10,4 MB (10.423.032 Byte) | CI-Lauf 127, `ubuntu-24.04` (lokal 10,3 MB) |
| AppImage      | 88,8 MB (88.754.680 Byte) | CI-Lauf 127, `ubuntu-24.04` (lokal 88,7 MB) |

Das Programm selbst ist rund 14 MB groß, die Wörterbücher mit dem Quelltext von igerman98 6 MB.

**Das AppImage überschreitet den Zielwert; der Auftraggeber hat es als Ausnahme zugelassen** (ADR 0028, Nachtrag vom 19. November 2026). CI misst es nur und bewertet es nicht (`--report-only AppImage`). Ein AppImage muss alles mitbringen, was es nicht vom System erwarten darf: allein WebKitGTK (`libwebkit2gtk-4.1.so`, 91 MB), JavaScriptCore (32 MB) und die ICU-Daten (30 MB) machen ausgepackt rund 150 MB aus, komprimiert den größten Teil der 88,8 MB. Das `.deb` und die Windows-Installer nehmen diese Laufzeit vom System (WebKitGTK aus den Paketquellen, WebView2 von Windows) und bleiben deshalb weit unter 50 MB. Die GStreamer-Medienbibliotheken sind bereits weggelassen (`bundleMediaFramework: false`); kleiner wird ein AppImage mit WebKitGTK nicht.

## Windows

### Warnung bei unsignierten Installern

Die Installer sind nicht signiert (ADR 0028, D11). Deshalb:

- **SmartScreen** warnt beim Start des heruntergeladenen Installers („Der Computer wurde durch Windows geschützt“). Über „Weitere Informationen“ → „Trotzdem ausführen“ lässt sich die Installation fortsetzen. Prüfen Sie vorher, dass die Datei aus einer Quelle stammt, der Sie vertrauen.
- **Smart App Control** (Windows 11) blockiert unsignierte Programme ganz; es gibt dort keine Schaltfläche zum Fortfahren. Wer Project Write testen möchte, muss Smart App Control in „Windows-Sicherheit“ → „App- und Browsersteuerung“ ausschalten. Achtung: Smart App Control lässt sich danach nur mit einer Neuinstallation von Windows wieder einschalten. Für Rechner mit Smart App Control ist die Testversion deshalb nicht gedacht.

Eine Signierung lässt sich später nachrüsten, ohne den Build umzubauen (ADR 0028).

### Installieren

- **NSIS-Installer:** `Project Write_<Version>_x64-setup.exe` starten. Er installiert ohne Administratorrechte, standardmäßig in `%LOCALAPPDATA%\Project Write`, und legt eine Verknüpfung im Startmenü an. Sprache des Installers: Deutsch oder Englisch, nach der Sprache des Systems.
- **MSI-Paket:** `Project Write_<Version>_x64_en-US.msi` per Doppelklick starten (fragt nach Administratorrechten) und installiert nach `C:\Program Files\Project Write`. Für die Verteilung im Unternehmen geht es auch still: `msiexec /i "Project Write_<Version>_x64_en-US.msi" /qn`.
- **Still (ohne Fenster):** `"Project Write_<Version>_x64-setup.exe" /S`.

Project Write braucht die **WebView2-Laufzeit** von Microsoft. Unter Windows 10 und 11 ist sie in aller Regel vorhanden. Fehlt sie, lädt der Installer sie einmalig von Microsoft nach (`downloadBootstrapper`) – das ist die einzige Netzverbindung, und sie geht vom Installer aus, nicht von der App.

Beide Installer melden die Dateiendung `.pwrite` an (Doppelklick öffnet das Dokument in Project Write); der CI-Test warnt, falls die Zuordnung fehlt.

### Deinstallieren

- Über „Einstellungen“ → „Apps“ → „Installierte Apps“ → „Project Write“ → „Deinstallieren“, oder in der Systemsteuerung unter „Programme und Features“.
- Still: NSIS mit `"%LOCALAPPDATA%\Project Write\uninstall.exe" /S`, MSI mit `msiexec /x "Project Write_<Version>_x64_en-US.msi" /qn`.

Die Deinstallation entfernt das Programm, die Wörterbücher, die Lizenzdateien, die Verknüpfungen und den Eintrag der Dateiendung. Der NSIS-Deinstaller bietet zusätzlich an, die App-Daten (Einstellungen, Protokolle unter `%APPDATA%\app.projectwrite.desktop` und `%LOCALAPPDATA%\app.projectwrite.desktop`) zu löschen; ohne Haken bleiben sie erhalten. Die **Bibliothek** gehört nie dazu (siehe unten).

## Linux

### `.deb` (Debian, Ubuntu)

Installieren – `apt` lädt die benötigten Bibliotheken (WebKitGTK 4.1, GTK 3 und weitere) aus den Paketquellen der Distribution:

```sh
sudo apt install "./Project Write_<Version>_amd64.deb"
```

Das Paket heißt `project-write` und hängt von `libwebkit2gtk-4.1-0` und `libgtk-3-0` ab. Das Programm liegt danach unter `/usr/bin/project-write`, Wörterbücher und Lizenzdateien unter `/usr/lib/Project Write/`, der Lizenztext zusätzlich unter `/usr/share/doc/project-write/copyright`; der Starter erscheint im Anwendungsmenü, und `.pwrite`-Dateien sind mit Project Write verknüpft.

Deinstallieren:

```sh
sudo apt remove project-write   # oder: sudo apt purge project-write
```

### AppImage

Ein AppImage wird nicht installiert, sondern als Datei ausgeführt:

```sh
chmod +x "Project Write_<Version>_amd64.AppImage"
"./Project Write_<Version>_amd64.AppImage"
```

AppImages brauchen FUSE 2 (`libfuse2`, unter Ubuntu 24.04 `libfuse2t64`). Ohne FUSE startet es mit `APPIMAGE_EXTRACT_AND_RUN=1 "./Project Write_<Version>_amd64.AppImage"` oder nach `--appimage-extract` über `squashfs-root/AppRun`. Zum „Deinstallieren“ löscht man die Datei. Das AppImage meldet keine Dateiendung und keinen Menüeintrag an.

Einstellungen und Protokolle liegen unter Linux in `~/.config/app.projectwrite.desktop` und `~/.local/share/app.projectwrite.desktop`; sie bleiben nach der Deinstallation erhalten und lassen sich von Hand löschen.

## Die Bibliothek bleibt erhalten

Die Bibliothek mit allen Dokumenten, Medien, der Datenbank und den Backups liegt **außerhalb** des Programmordners – standardmäßig in `Dokumente/ProjectWrite` (Abschnitt 9; unter Windows im Ordner „Dokumente“ des Nutzers, unter Linux in `XDG_DOCUMENTS_DIR`, sonst `~/Documents`) oder an dem Ort, den Sie beim Anlegen gewählt haben.

**Keine Deinstallation löscht oder ändert die Bibliothek** – weder NSIS noch MSI noch `apt remove`/`apt purge`, auch nicht mit dem Haken „App-Daten löschen“. Nach einer Neuinstallation öffnen Sie sie wieder über „Bibliothek öffnen“. Wollen Sie die Bibliothek wirklich entfernen, löschen Sie den Ordner selbst.

Die Bibliothek ist in Version 1.0 nicht verschlüsselt (Abschnitt 20). Schützen Sie den Rechner deshalb mit der Datenträgerverschlüsselung des Systems (BitLocker, LUKS).

## Wie CI Installation und Deinstallation prüft

Zwei Jobs in `.github/workflows/ci.yml` (Abschnitt 25, Phase 14: „Installation und Deinstallation sind auf einem sauberen System je Betriebssystem getestet. Die Deinstallation lässt die Bibliothek unangetastet.“):

1. **Installers** (`windows-latest`, `ubuntu-24.04`): baut die Release-Pakete (`npx tauri build --bundles nsis,msi` bzw. `deb,appimage`, ohne Feature `e2e`), prüft die Lizenzseite (`scripts/installer-license.mjs --check`), misst jede Paketgröße gegen 50 MB (`scripts/installer-test.mjs size`, das AppImage nur gemessen) und lädt die Pakete als Artefakt `installers-<OS>` hoch (7 Tage aufbewahrt).
2. **Install and uninstall** (`windows-latest`, `ubuntu-24.04`): ein **frischer Runner** ohne Rust, Build-Werkzeuge und Entwicklungsbibliotheken lädt die Pakete herunter und führt `scripts/installer-test.mjs` je Paket aus (NSIS und MSI nacheinander unter Windows, `.deb` und AppImage unter Linux). Das Skript bricht beim ersten Fehler ab und prüft:
   1. sauberes System: App nicht installiert, kein Ordner `Dokumente/ProjectWrite` (das Skript fasst eine vorhandene Bibliothek nie an);
   2. stille Installation (`/S`, `msiexec /qn`, `apt-get install`; das AppImage wird nach `~/Applications` kopiert);
   3. installierte Dateien: Programm, jede Wörterbuchdatei byte-gleich mit `src-tauri/dictionaries`, die drei Lizenzdateien byte-gleich mit dem Repository, kein Testplugin im Programm; unter Windows Deinstallationseintrag mit Version und Herausgeber, unter Linux Starter, MIME-Typ und alle gemeinsamen Bibliotheken auflösbar (`ldd`);
   4. Kurzstart: das installierte Programm läuft nach 15 Sekunden noch und hat seine Startzeile ins Protokoll geschrieben (Linux in `xvfb` mit D-Bus-Sitzung, das AppImage selbst ohne FUSE); abschaltbar mit `--no-smoke`;
   5. eine Platzhalter-Bibliothek am Standardort: `library.json`, eine SQLite-Datei, Dokumente mit Umlauten im Namen, ein Medium und ein leerer Ordner; Pfad, Größe, SHA-256 und Änderungszeit jedes Eintrags werden festgehalten;
   6. stille Deinstallation (NSIS-Deinstaller `/S`, `msiexec /x /qn`, `apt-get purge`; das AppImage wird gelöscht);
   7. Programmdateien entfernt: Installationsordner, Deinstallationseintrag und Verknüpfungen (Windows) bzw. jede Datei des Pakets laut `dpkg -L` (Linux);
   8. die Bibliothek ist unverändert, Eintrag für Eintrag.

Lokal unter Linux (als Nutzer mit `sudo`): `node scripts/installer-test.mjs deb src-tauri/target/release/bundle` und `node scripts/installer-test.mjs appimage src-tauri/target/release/bundle`.
