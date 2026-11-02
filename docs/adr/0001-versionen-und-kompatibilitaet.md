# ADR 0001 – Versionen und Kompatibilität des Technologiestacks

- Status: angenommen
- Datum: 2026-11-02
- Phase: 1

## Kontext

Abschnitt 7 verlangt stabile, miteinander kompatible Versionen, geprüft beim Projektstart. Vorabversionen sind ausgeschlossen. Am 2. November 2026 gab es bei mehreren Paketen eine neuere Hauptversion, die mit anderen Teilen des Stacks nicht zusammenpasst.

## Geprüfte Konflikte

| Paket                                | Neueste stabile Version                       | Konflikt                                                        | Gewählt    |
| ------------------------------------ | --------------------------------------------- | --------------------------------------------------------------- | ---------- |
| TypeScript                           | 7.0.2 (nativer Compiler)                      | `typescript-eslint` 8.72.1 verlangt `typescript >=4.8.4 <6.1.0` | **6.0.3**  |
| WebdriverIO (`@wdio/cli` u. a.)      | 10.1.0                                        | `@wdio/tauri-service` 1.5.1 verlangt `webdriverio ^9`           | **9.32.0** |
| Tauri (Crate)                        | 2.12.5 (crates.io zeigt auch `3.0.0-alpha.5`) | Alpha ausgeschlossen (Abschnitt 7)                              | **2.12.5** |
| `tauri-build`                        | 2.7.4                                         | –                                                               | 2.7.4      |
| `@tauri-apps/api`, `@tauri-apps/cli` | 2.12.4                                        | Minor muss zur Crate passen (2.12)                              | 2.12.4     |
| SQLx                                 | 0.9.0                                         | –                                                               | 0.9.0      |

Alle übrigen Versionen stehen in `docs/dependencies.md`. Lockfiles (`package-lock.json`, `src-tauri/Cargo.lock`) sind committet, `npm` speichert exakte Versionen (`.npmrc`: `save-exact=true`).

## Entscheidung

1. TypeScript 6.0.x statt 7.0.x, bis `typescript-eslint` TypeScript 7 unterstützt. Der Strict-Modus ist unverändert verfügbar.
2. WebdriverIO 9.x statt 10.x, bis `@wdio/tauri-service` WebdriverIO 10 unterstützt.
3. SQLx mit `default-features = false` und nur `runtime-tokio`, `sqlite-bundled`, `migrate`, `macros`. Das Sammel-Feature `sqlite` wird bewusst nicht genutzt, weil es `sqlite-load-extension` einschaltet. Die mitgebaute SQLite (`libsqlite3-sys` 0.37.0) wird mit `SQLITE_ENABLE_FTS5` übersetzt. Ihr Build-Skript setzt zwar `SQLITE_ENABLE_LOAD_EXTENSION=1`, SQLx stellt das Laden von Erweiterungen ohne das Feature aber nicht bereit, und SQLite lässt die SQL-Funktion `load_extension()` ohne ausdrückliche Freigabe ausgeschaltet.
4. SQLx 0.9 akzeptiert nur statische SQL-Texte (`SqlSafeStr`). Dynamisches SQL braucht die ausdrückliche Markierung `AssertSqlSafe` und wird damit im Review sichtbar. Der Produktivcode nutzt in Phase 1 kein dynamisches SQL.
5. Rust-Edition 2024, `rust-version = "1.88"` (Let-Chains). Entwickelt und getestet mit Rust 1.97.0.
6. Node.js 22 (LTS), `engines.node >= 22.12.0`.

## Folgen

- Beim Wechsel auf TypeScript 7 oder WebdriverIO 10 ist eine neue ADR nötig.
- Die Bundle-Kennung `app.projectwrite.desktop` in `tauri.conf.json` ist **vorläufig**. Sie bestimmt Konfigurations- und Datenordner des Systems und ist nach der ersten Weitergabe schwer änderbar (Abschnitt 2, Regel 3). Sie wird vor Phase 14 zusammen mit D4 und D11 festgelegt.
