# ADR 0006 – Bibliothek: Sperre, Speicherort und Rollback-Modus

- Status: angenommen
- Datum: 2026-11-03
- Phase: 2
- Löst ab: den In-Memory-Teil von ADR 0004 (die Laufzeit nutzt jetzt die Datenbank der Bibliothek)

## Kontext

Abschnitt 9 verlangt: eine Bibliothek ist nur in einer App-Instanz geöffnet (`tauri-plugin-single-instance` und `library.lock`); Sync-Ordner (OneDrive, Dropbox, iCloud Drive, Google Drive) und Netzlaufwerke werden erkannt, lösen eine Warnung aus und laufen, wenn der Nutzer sie trotzdem wählt, mit `journal_mode=DELETE` statt WAL. `library.json` trägt Bibliotheks-ID und Formatversion.

## Entscheidungen

### Sperre

- **Betriebssystem-Dateisperre** auf `library.lock` (`std::fs::File::try_lock`, unter Unix `flock`, unter Windows `LockFileEx`; daher `rust-version = 1.89`). Das System gibt die Sperre beim Prozessende frei, auch nach Absturz oder `kill`; eine veraltete Sperrdatei blockiert nie. Der Inhalt (Prozess-ID, Version, Zeit) dient nur der Diagnose.
- Verworfen: PID-Datei mit Zeitstempel (Zombie- und PID-Wiederverwendungsprobleme, Rennen beim Anlegen).
- Eine zweite Instanz – derselbe oder ein anderer Prozess – erhält `LIBRARY_LOCKED` und kann es erneut versuchen. Getestet mit einem zweiten Prozess, regulär beendet und hart abgebrochen (`tests/library_lock.rs`).
- Unterstützt ein Ort keine Dateisperren, wird die Bibliothek nicht geöffnet (`LIBRARY_LOCK_UNSUPPORTED`), statt ohne Schutz zu arbeiten.
- `tauri-plugin-single-instance` 2.5.5 sorgt zusätzlich dafür, dass ein zweiter Programmstart an die laufende Instanz übergibt und endet; die laufende Instanz holt ihr Fenster nach vorn. Unter Linux braucht das Plugin eine D-Bus-Sitzung (ohne Sitzungsbus startet es normal; dann greift die Dateisperre). Die Pfadübergabe (Dateizuordnung) folgt in Phase 5.

### Speicherort

Erkennung in `filesystem/location.rs`, Reihenfolge der Signale:

1. Netzlaufwerk: Windows `GetDriveTypeW` (über `GetVolumePathNameW`) und UNC-Pfade; Linux Einhängetabelle `/proc/self/mountinfo` (NFS, CIFS/SMB, sshfs, rclone, GVFS u. a.).
2. Windows Cloud Files API (`CfGetSyncRootInfoByPath`): erkennt jeden registrierten Sync-Anbieter samt Namen.
3. Von Anbietern bekannt gemachte Wurzeln: Umgebungsvariablen `OneDrive*`, `iCloudDrive`, Dropbox `info.json`.
4. Ordnernamen der Anbieter (`OneDrive - Firma`, `Dropbox`, `Google Drive`/`My Drive`, `iCloudDrive`, `Library/CloudStorage/…`, `Mobile Documents`, Nextcloud, ownCloud, pCloud).

Namensbasierte Erkennung kann eine Warnung auch für einen gleichnamigen, nicht synchronisierten Ordner auslösen; das ist gewollt (lieber eine Warnung zu viel). macOS (vorerst zurückgestellt) erkennt nur Ordnernamen und `Mobile Documents`, keine Netzlaufwerke.

- Der Journalmodus wird bei **jedem** Öffnen aus dem Ort bestimmt, nicht gespeichert.
- Anlegen und Umziehen an einen solchen Ort verlangen eine Bestätigung (`acceptWarning`), sonst `LOCATION_NOT_ACCEPTED`. Die Bestätigung wird in der Bibliothek gespeichert (`storage.acceptedLocation`); eine geöffnete Bibliothek an einem nicht bestätigten Ort zeigt einen Hinweis, bis der Nutzer ihn ausblendet.

### Struktur und Öffnen

- `library.json` wird **zuletzt** geschrieben: Ein Ordner ist erst dann eine Bibliothek. Ein unterbrochenes Anlegen hinterlässt ein Gerüst ohne `library.json`, das beim nächsten Anlegen weiterverwendet wird.
- Anlegen nur in einem leeren Ordner (Systemdateien wie `.DS_Store`, `desktop.ini` zählen nicht). Wählt der Nutzer einen Ordner mit anderen Dateien, entsteht die Bibliothek im Unterordner `ProjectWrite` – Nutzerdateien und Bibliothek mischen sich nie.
- Beim Öffnen werden alle festen Pfade der Struktur auf Verknüpfungen geprüft (`LIBRARY_UNSAFE_PATH`), `tmp/` geleert (ohne Verknüpfungen zu folgen), `quick_check` ausgeführt, eine neuere Formatversion oder ein neueres Schema abgelehnt, vor Migrationen gesichert (database.md, Abschnitt 3) und die Dokumentordner mit der Datenbank verglichen.
- Ordner außer `database/` und `documents/*` entstehen erst, wenn eine Phase hineinschreibt (Abschnitt 2: nichts auf Vorrat).
- Nur Unicode-Pfade (`PATH_NOT_UNICODE` sonst), weil der Pfad in `settings.json` und als Anzeigetext gebraucht wird.

### Umzug

Abschnitt 9: kopieren, prüfen, umschalten (`services/library/relocate.rs`): Schreibzugriffe sind während des Umzugs gesperrt; Dateien über Temporärnamen mit `fsync` kopieren und unterwegs hashen; Datenbank mit `VACUUM INTO`; jede Kopie erneut lesen und per SHA-256 vergleichen; Datenbankkopie mit `integrity_check`, Schemaversion und Dokumentanzahl prüfen; `library.json` zuletzt; dann alte Bibliothek schließen, neue öffnen (bei Fehlschlag die alte wieder öffnen). Die alte `library.json` erhält `movedTo`, damit die alte Kopie nur nach Bestätigung geöffnet wird. Abbruch und Fehler entfernen genau das, was der Umzug angelegt hat. Fortschritt über einen Tauri-Kanal (keine Event-Rechte nötig).

## Folgen

- Die Systemprüfung prüft die Datenbank der geöffneten Bibliothek; ohne Bibliothek eine In-Memory-Datenbank.
- Wird ein Prozess gestartet, während die App eine Bibliothek schließt, teilt das Kind unter Unix kurz die offenen Dateien (bis `exec`) und hält die Sperre für diesen Moment. Für die App unschädlich; der Mehrprozess-Test läuft deshalb in einer eigenen Testdatei.
- Netzwerkdateisysteme mit unzuverlässigen Sperren (alte NFS-Versionen) sind ein Restrisiko; die Warnung empfiehlt ohnehin einen lokalen Ort.
