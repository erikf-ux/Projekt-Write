# ADR 0007 – Atomares Speichern und Dokument-Repository

- Status: angenommen
- Datum: 2026-11-03
- Phase: 2

## Kontext

Abschnitt 9: „Jeder Schreibvorgang ist atomar: Temporärdatei im selben Ordner, Daten auf den Datenträger schreiben (`fsync`), dann umbenennen.“ Abnahme Phase 2: „100 harte Prozessabbrüche während des Speicherns: keine beschädigte Datei, `integrity_check` meldet `ok`.“ Datei und Datenbankzeile eines Dokuments lassen sich nicht in einer gemeinsamen Transaktion ändern.

## Entscheidungen

- `filesystem::atomic::write_atomic`: `.<name>.<zufall>.pwtmp` mit `create_new` (folgt keiner vorhandenen Datei oder Verknüpfung) → schreiben → `sync_all` → umbenennen → Unix: Ordner mit `fsync` synchronisieren. Windows: `MoveFileExW` mit `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH` (statt `std::fs::rename`, damit die Umbenennung selbst auf dem Datenträger ist); bei kurzzeitigen Freigabekonflikten (Virenscanner, Indexdienst, Sync-Programme) bis etwa eine Sekunde Wiederholungen. Pfade werden vor dem `\\?\`-Präfix absolut und normalisiert (in CI belegt: `/` in einem Präfixpfad ergab Fehler 123).
- Neue Dateien sind unter Unix nur für den Nutzer lesbar (`0600`, Ordner `0700`).
- **Reihenfolge Datei vor Zeile** (database.md, Abschnitt 6): Ein Absturz hinterlässt höchstens eine Datei ohne Zeile (gemeldet, nicht gelöscht) oder eine Zeile mit veralteter Prüfsumme (beim nächsten Lesen korrigiert; die Datei gilt). Umgekehrt (Zeile zuerst) könnte ein Dokument ohne Datei entstehen – das wäre ein sichtbarer Datenverlust.
- Liegengebliebene Temporärdateien entfernt das Öffnen der Bibliothek: Ihr Ziel ist dank Umbenennung unversehrt, der Inhalt stammt aus einem nicht abgeschlossenen Speichern. Den Verlust ungespeicherter Eingaben deckt die Absturz-Wiederherstellung über `recovery/` ab (Phase 6).
- Alle Schreibvorgänge einer Bibliothek laufen hinter einer gemeinsamen Sperre; zwei Speichervorgänge desselben Dokuments können sich nicht überholen. Unveränderter Inhalt wird nicht neu geschrieben.
- Dateinamen sind `<uuid>.<ext>` und werden aus der Zeile abgeleitet; Titel nur in der Datenbank (Umbenennen berührt die Datei nicht).
- Grenzen: Titel 1–300 Zeichen ohne Steuerzeichen, Inhalt höchstens 64 MiB (Speicherschutz; 50.000 Wörter sind rund 400 KB).

## Absturztest

`tests/crash_safety.rs` startet die Testdatei selbst ein zweites Mal als Hilfsprozess, der ohne Pause speichert (Inhalte 1 KB bis 512 KB, jedes zehnte Mal ein neues Dokument), und bricht ihn 100-mal hart ab (`SIGKILL`/`TerminateProcess`) zu zufälligen, aber aus einem festen Startwert abgeleiteten Zeitpunkten. Jede Datei trägt Länge und SHA-256 ihres Inhalts im Kopf und wird nach jedem Abbruch vollständig geprüft. Lokal (Linux): 100 von 100 Abbrüchen trafen ein laufendes Speichern.

## Folgen

- Phase 4/5 laden und speichern Dokumente nur über das Repository.
- Externe Dateien (Phase 5) nutzen dieselbe Funktion, müssen aber zusätzlich Kodierung, Zeilenenden und Dateirechte erhalten.
