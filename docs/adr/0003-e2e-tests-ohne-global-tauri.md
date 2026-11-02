# ADR 0003 – E2E-Tests mit eingebettetem WebDriver, ohne `withGlobalTauri`

- Status: angenommen
- Datum: 2026-11-02
- Phase: 1

## Kontext

Abschnitt 7 nennt WebdriverIO mit `@wdio/tauri-service`. Abschnitt 20 verlangt `withGlobalTauri: false` und Testplugins nur in Debug-Builds.

**Gefundener Widerspruch:** Die Dokumentation von `@wdio/tauri-service` 1.5.1 verlangt das Plugin `tauri-plugin-wdio` samt JavaScript-Teil und `withGlobalTauri: true`. Der JavaScript-Teil müsste ins App-Bundle und fängt `invoke` ab.

Untersucht wurde der Quelltext des Dienstes (`dist/esm/index.js`): Das Plugin wird nur für `browser.tauri.execute()` und das Mocking gebraucht. Ohne Plugin protokolliert der Dienst eine Warnung und funktioniert für normale WebDriver-Befehle weiter. Das zweite Plugin `tauri-plugin-wdio-webdriver` (nur Rust) stellt einen eingebetteten WebDriver-Server bereit, bindet an `127.0.0.1`, braucht keine Berechtigungen und kein `withGlobalTauri` und läuft auf Windows, macOS und Linux. `tauri-driver` dagegen unterstützt macOS nicht.

## Entscheidung

- E2E-Tests nutzen `@wdio/tauri-service` mit `driverProvider: 'embedded'` und **nur** `tauri-plugin-wdio-webdriver`. `tauri-plugin-wdio` und `@wdio/tauri-plugin` werden nicht eingebunden. `browser.tauri.*` wird nicht benutzt; Tests nutzen `browser.execute`.
- Das Plugin ist eine optionale Abhängigkeit hinter dem Cargo-Feature `e2e`. `src/lib.rs` bricht die Übersetzung mit `compile_error!` ab, wenn `e2e` ohne `debug_assertions` gebaut wird.
- CI prüft im Job `release-check`: (1) `cargo check --release --features e2e` muss scheitern, (2) der Abhängigkeitsgraph des Release-Builds enthält kein Testplugin, (3) das Release-Binary enthält keine Plugin-Kennungen (`scripts/check-release-no-test-plugins.sh`). Positivkontrolle lokal: Das E2E-Debug-Binary enthält die Kennung 78-mal, der Scan schlägt dort also an.
- Der Webview der E2E-Tests schreibt Caches nach `tests/e2e/.tmp/` (XDG-Variablen unter Linux, `WEBVIEW2_USER_DATA_FOLDER` unter Windows). Unter macOS lässt sich der WebKit-Datenordner nicht umlenken; dort entstehen Cache-Dateien im Benutzerordner der CI-Maschine.

## Bekannte Grenze

Der eingebettete WebDriver erzeugt Tastatureingaben als synthetische JavaScript-Ereignisse. Browser führen für solche Ereignisse keine Standardaktion aus; `Tab` verschiebt den Fokus deshalb nicht. Die Tabulator-Reihenfolge wird im E2E-Test strukturell geprüft (erstes fokussierbares Element), der sichtbare Fokus durch programmatisches Fokussieren. Echte Tastaturnavigation (Abschnitt 25, Phase 3) braucht eine zusätzliche Prüfung, etwa mit `tauri-driver` und nativen Eingaben unter Linux und Windows oder manuell.

## Plattformunterschiede

- **Farben werden an Pixeln gemessen:** Die CI-Fenster unter macOS und Windows rendern nicht von selbst. `getComputedStyle` lieferte dort nach einer Änderung Werte von vorher (CI-Lauf 15), unter macOS teils sogar nach einem erzwungenen Frame (CI-Lauf 17), während die gerenderten Pixel stimmten. Unter macOS zeigte sogar ein einzelner Screenshot noch den vorherigen Zustand (CI-Lauf 18). Alle Farbprüfungen (Text-Kontrast, Hintergründe, Fokusring) messen deshalb an einem **eingeschwungenen** Screenshot – so lange neu aufgenommen, bis zwei aufeinanderfolgende pixelgleich sind und die Hintergrundfarbe des erwarteten Themes sichtbar ist (höchstens 15 Versuche im Abstand von 300 ms) –, also an dem, was Nutzer sehen: Hintergrund = häufigstes Pixel im Textfeld, Textfarbe = Pixel mit dem höchsten Kontrast dazu. Geprüft in Chromium bei 1- und 2-facher Skalierung: liefert exakt die Token-Werte. PNG-Leser: `tests/e2e/png.ts` (nur `node:zlib`, keine neue Abhängigkeit).
- Der Fokusring (2 px laut `index.css`, unter macOS 3 px gezeichnet) wird ebenfalls an Pixeln geprüft: mindestens 2 px in Akzentfarbe links vom Button, mindestens 3:1 gegen den Kartenhintergrund.
- Der Kontrast wird im Theme geprüft, mit dem die App startet (folgt dem System), und nach einem Wechsel zur Laufzeit in beide Richtungen (dieselben DOM-Schritte wie `applyTheme()`).
- Unter Windows lädt Tauri die App von `http://tauri.localhost`, sonst von `tauri://localhost`. IPC-Tests müssen den passenden Ursprung verwenden, sonst gilt die Anfrage als fremd und wird abgelehnt.

## Folgen

- Mocking über `browser.tauri.mock()` steht nicht zur Verfügung. Frontend-Tests mit gemocktem IPC laufen in Vitest mit `mockIPC`.
- Wird `withGlobalTauri` je nötig, braucht das eine neue ADR.
