# ADR 0005 – Isolation Pattern

- Status: angenommen
- Datum: 2026-11-02
- Phase: 1

## Kontext

Abschnitt 20 verlangt, das Isolation Pattern von Tauri in Phase 1 zu prüfen. Beim Isolation Pattern läuft jede IPC-Nachricht durch ein abgeschottetes Iframe mit einem eigenen Skript (`__TAURI_ISOLATION_HOOK__`). Das Iframe verschlüsselt die Nachricht (AES-GCM, Schlüssel je Programmstart). Ziel ist Schutz gegen kompromittierte Frontend-Abhängigkeiten: Sie sollen keine IPC-Nachrichten am Hook vorbei schicken können.

Abschnitt 20 schreibt zugleich die CSP vor, darunter `frame-src 'none'`. Jede Lockerung braucht eine ADR.

## Prüfung

### Quelltext (Tauri 2.12.5, tauri-codegen 2.7.4)

1. Tauri lädt das Isolations-Iframe über ein eigenes Schema `isolation-<UUID>`. Die UUID wird **bei jedem Build neu erzeugt** (`tauri-codegen/src/embedded_assets.rs`: `format!("isolation-{}", uuid::Uuid::new_v4())`).
2. Tauri ergänzt dieses Schema zur Laufzeit nur in `default-src` (`tauri/src/manager/mod.rs`), nicht in `frame-src`.
3. Ist `frame-src` gesetzt, gilt für Iframes nur `frame-src`; `default-src` ist dann für Iframes ohne Wirkung. Mit `frame-src 'none'` ist das Isolations-Iframe also verboten.
4. Eine feste Ausnahme in `frame-src` ist nicht möglich, weil das Schema je Build zufällig ist. Funktionieren würde das Pattern nur, wenn `frame-src` ganz entfällt. Dann gälte für Iframes `default-src 'self'` plus Isolations-Schema – eine Lockerung gegenüber Abschnitt 20.

### Versuch (Linux, WebKitGTK 2.x, Xvfb, 2. November 2026)

Zwei Debug-Builds mit Isolationsanwendung (`window.__TAURI_ISOLATION_HOOK__ = (payload) => payload`), gebaut mit `--features e2e,tauri/isolation` und `--config` (ohne Änderung der Repository-Dateien), getestet mit derselben E2E-Suite (`PW_E2E_APP` in `tests/e2e/wdio.conf.ts`):

| Variante | CSP                                  | Ergebnis                                                                                                                                                       |
| -------- | ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A        | unverändert (mit `frame-src 'none'`) | **1 von 6 Tests grün.** Jede IPC-Nachricht hängt: Die Systemprüfung antwortet nie, `invoke` läuft in die 120-s-Zeitgrenze. Nur der CSP-Test (ohne IPC) besteht |
| B        | `frame-src` entfernt                 | **6 von 6 Tests grün**, IPC funktioniert                                                                                                                       |

Nebenbefund: `tauri build` trägt bei gesetztem `pattern` das Feature `isolation` selbsttätig in `Cargo.toml` (für `tauri` und `tauri-build`) ein und erweitert `Cargo.lock` um 136 Zeilen (AES-GCM u. a.). Beides wurde nach dem Versuch zurückgesetzt.

### Nutzen im Verhältnis zur Architektur

- Rust ist die Vertrauensgrenze: Jede Eingabe wird dort deserialisiert und validiert (`Input<T>` + `Validate`, ADR 0002). Ein Hook im Iframe würde dieselben Prüfungen ein zweites Mal in JavaScript ausführen.
- Die Capability erlaubt nur die eigenen Befehle; es gibt keine Plugin-Rechte (`fs`, `shell`, `sql`, `path` …). Ein kompromittiertes Paket kann also nur erlaubte Befehle mit gültigen Daten aufrufen – das kann es auch mit dem Isolation Pattern, denn der Hook sieht solche Aufrufe als gültig an.
- Die CSP verbietet fremde Skripte, `eval` und Netzzugriffe. Die Lieferkette wird über Lockfiles, exakte Versionen und `npm audit` in CI überwacht.
- Kosten: zusätzliche Verschlüsselung und `postMessage` je Aufruf, keine ES-Module im Isolationsskript, eine weitere Schicht für Tests (`mockIPC`, E2E).

## Optionen

1. Isolation Pattern aktivieren und `frame-src` aus der CSP streichen (CSP-Lockerung).
2. Isolation Pattern nicht aktivieren, CSP unverändert lassen.

## Entscheidung

Option 2: **Das Isolation Pattern wird in Phase 1 nicht aktiviert.** Die vorgeschriebene CSP bleibt unverändert, einschließlich `frame-src 'none'`. Der Schutz gegen schädliche IPC-Aufrufe liegt in Rust (Validierung, App-Manifest, minimale Capability).

Option 1 ist technisch belegt (Variante B). Sie bleibt dem Auftraggeber offen, setzt aber eine bewusste Lockerung von Abschnitt 20 voraus: Ohne `frame-src` dürften Iframes aus `'self'` und dem Isolations-Schema geladen werden.

`tests/security_config.rs` prüft die CSP wortgetreu; eine spätere Aktivierung müsste diese ADR ablösen und die CSP-Änderung begründen.

## Neubewertung

- Vor Phase 8 (PDF.js und weitere große Frontend-Abhängigkeiten).
- Vor Phase 14 (Auslieferung).
- Sobald Tauri das Isolations-Schema in `frame-src` ergänzt oder ein festes Schema erlaubt.
