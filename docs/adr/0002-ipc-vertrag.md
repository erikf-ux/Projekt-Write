# ADR 0002 – IPC-Vertrag: validierte Eingaben, einheitliches Fehlerformat, erzeugte Typen

- Status: angenommen
- Datum: 2026-11-02
- Phase: 1

## Kontext

Abschnitt 8 verlangt: Nur Rust greift auf Datenbank und Dateisystem zu, Rust validiert alle Eingaben, alle Befehle melden Fehler im Format `{ code, messageKey, details? }`, TypeScript-Typen werden aus Rust erzeugt. Abschnitt 20 verlangt, dass eigene Befehle ausdrücklich freigegeben werden.

Problem: Tauri deserialisiert Befehlsargumente selbst. Schlägt das fehl (falscher Typ, fehlendes Feld), antwortet Tauri mit einem **Text** statt mit unserem Fehlerformat. Ebenso antwortet Tauri mit Text, wenn ein Befehl von der Capability nicht erlaubt ist.

## Optionen

1. Argumente als `serde_json::Value` annehmen und im Befehl auswerten – verliert die Typisierung in der Signatur.
2. Rohen `tauri::ipc::Request` auswerten – viel Handarbeit je Befehl.
3. Eigener Argumenttyp `Input<T>`, der `tauri::ipc::CommandArg` implementiert, selbst deserialisiert, `Validate` aufruft und jeden Fehler als `CommandError` zurückgibt.

## Entscheidung

Option 3.

- `src-tauri/src/commands/input.rs`: `Input<T: DeserializeOwned + Validate>`. Jeder Befehl nimmt seine Nutzlast als ein Argument `request: Input<…>`. Eingabetypen nutzen `deny_unknown_fields`. Weil `Validate` Pflicht ist, kann kein Befehl Eingaben ohne Prüfung annehmen.
- `src-tauri/src/error.rs`: `CommandError { code, messageKey, details? }` mit festen `ErrorCode`-Werten. `details` ist eine flache Text-Tabelle mit gekürzten Werten (höchstens 200 Byte). Interne Fehler (SQL, E/A) werden in Rust protokolliert und als `DATABASE` oder `INTERNAL` ohne Inhalt gemeldet. So erreichen weder Stacktraces noch absolute Pfade die Oberfläche.
- `src/services/ipc.ts` ist die einzige Stelle im Frontend, die `invoke` aufruft (ESLint-Regel `no-restricted-imports`). Antworten, die nicht dem Format entsprechen – etwa die Text-Ablehnung von Tauri bei fehlender Berechtigung –, werden zu `IPC_REJECTED` normalisiert, ihr Text wird verworfen. `call()` wirft nie, sondern liefert `{ ok, value | error }`.
- Typen: `ts-rs` 12 schreibt bei `cargo test` nach `src/types/generated/`. CI prüft mit `git diff --exit-code`, dass die committeten Typen aktuell sind. Die Zuordnung Befehlsname → Request/Response steht einmal in `src/services/ipc.ts` und nutzt nur erzeugte Typen.
- Freigabe: `build.rs` nutzt das App-Manifest (`AppManifest::commands`). Damit ist jeder eigene Befehl verboten, bis eine Capability ihn erlaubt. Die einzige Capability `main-window` erlaubt genau `allow-system-check`. `tests/security_config.rs` prüft, dass Befehlsliste, App-Manifest und Capability übereinstimmen und keine Plugin-Rechte (`fs`, `shell`, `sql`, `path` …) vergeben sind.

## Folgen

- Neue Befehle brauchen vier Schritte: Eingabetyp mit `Validate`, Eintrag in `commands::COMMAND_NAMES` und `generate_handler!`, Eintrag im App-Manifest, Eintrag in der Capability. Der Test erzwingt die Übereinstimmung.
- Das Frontend erhält Pfade nur als Anzeigetext; Befehle arbeiten mit IDs (gilt ab Phase 2).
