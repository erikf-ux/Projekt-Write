# ADR 0018 – Sicherheitsupdate von SQLite über eine gepatchte Kopie von `sqlx-sqlite`

- Status: angenommen (vorübergehend)
- Datum: 10. November 2026
- Phase: 7 (Abschluss)

## Kontext

CI-Lauf 100 (Abschluss Phase 7) scheiterte im Job „Dependency audit“: `cargo audit` meldet **RUSTSEC-2026-0459** (CVE-2026-119375, mittel, CVSS 6,8) für `libsqlite3-sys` 0.37.0. Diese Version bündelt SQLite 3.51.3. SQLite vor 3.54.0 kann nicht initialisierten Speicher des Seitencaches zurückgeben, wenn eine Datenbankdatei zusammen mit einer `-wal`-Datei geöffnet wird, deren Seitengröße abweicht. Ein präpariertes Dateipaar lässt gewöhnliche Abfragen alte Bytes aus dem Speicher des Prozesses liefern. Der Angriff ist lokal und setzt voraus, dass das Opfer die Dateien öffnet.

Project Write öffnet die Datenbank seiner Bibliothek im WAL-Modus (lokal; in Sync-Ordnern und auf Netzlaufwerken mit Rollback-Journal, ADR 0006). Wer eine Bibliothek öffnet, die ein anderer geschrieben hat – etwa einen kopierten Ordner –, ist betroffen. Die Lücke gibt nur Speicher der eigenen App frei; die App baut keine Netzverbindungen auf. Die Bytes könnten aber in der Oberfläche erscheinen oder in die Bibliothek zurückgeschrieben werden.

Behoben ist sie in `libsqlite3-sys` 0.39.0 (SQLite 3.54.0) und 0.39.1 (SQLite 3.54.1). `sqlx-sqlite` 0.9.0, die neueste Version, verlangt aber `libsqlite3-sys` `>=0.30.1, <0.38.0`. Ein gewöhnliches Update ist deshalb nicht möglich, bis SQLx eine neue Version veröffentlicht.

## Optionen

1. **Warnung unterdrücken** (`ignore` in `cargo audit` und `cargo deny`) bis zu einer neuen SQLx-Version. Ohne Code, aber die App würde ein bekannt verwundbares SQLite ausliefern.
2. **Prüfung vor dem Öffnen** (Seitengröße von `-wal` und Datenbank vergleichen) als Schutz im eigenen Code, zusammen mit Option 1. Schützt nur den Pfad, den die Prüfung kennt, und hängt an Einzelheiten der Lücke, die nur aus der Meldung bekannt sind.
3. **Gepatchte Kopie von `sqlx-sqlite`** über `[patch.crates-io]`, die `libsqlite3-sys` 0.39 zulässt. Behebt die Lücke selbst.
4. **System-SQLite** statt der gebündelten Fassung. Windows bringt kein passendes SQLite mit; die Version hinge vom Rechner ab. Widerspricht der Vorgabe „SQLite, mit der App gebaut, über SQLx“ (Abschnitt 7).

## Entscheidung

Option 3. `src-tauri/vendor/sqlx-sqlite/` enthält `sqlx-sqlite` 0.9.0 genau wie auf crates.io veröffentlicht (Quelltext und Lizenztexte unverändert, MIT OR Apache-2.0). Nur eine Zeile in `Cargo.toml` ändert sich:

```diff
 [dependencies.libsqlite3-sys]
-version = ">=0.30.1, <0.38.0"
+version = ">=0.39.0, <0.40.0"
```

Die Untergrenze 0.39.0 stellt sicher, dass kein Lockfile-Update auf eine verwundbare Version zurückfällt. `src-tauri/Cargo.toml` bindet die Kopie mit `[patch.crates-io]` ein; `Cargo.lock` nennt `libsqlite3-sys` 0.39.1 (SQLite 3.54.1).

Geprüft: `sqlx-sqlite` übersetzt unverändert gegen `libsqlite3-sys` 0.39.1. Alle 348 Rust-Tests bestehen damit, darunter Migrationen samt Prüfsummen, die Prüfung auf FTS5 in der mitgebauten SQLite-Version, Absturztests, Struktur und Referenzbibliothek. Clippy (Linux, `e2e`, Windows-Ziel), die E2E-Tests und die Prüfung der Referenzbibliothek mit Fingerabdruck sind im Bericht der Phase 7 aufgeführt.

## Folgen

- Die Kopie ist vorübergehend. **Entfernen**, sobald eine SQLx-Version `libsqlite3-sys` 0.39 oder neuer zulässt: Ordner `src-tauri/vendor/sqlx-sqlite/` und Eintrag `[patch.crates-io]` löschen, SQLx aktualisieren. Offene Aufgabe in `docs/status.md`.
- Die Kopie wird nicht gepflegt. Ändert sich an `sqlx-sqlite` 0.9.0 nichts, gibt es auch nichts nachzuziehen. Eine neue SQLx-Version ersetzt die Kopie, statt dass sie nachgebaut wird.
- `THIRD-PARTY-LICENSES` führt `sqlx-sqlite` weiter mit seiner Lizenz und `libsqlite3-sys` 0.39.1. `cargo audit` und `cargo deny` prüfen ohne Ausnahme.
- `deny.toml` verbietet weiter `rusqlite`. `libsqlite3-sys` kommt nur über SQLx in den Baum (Abschnitt 7).
