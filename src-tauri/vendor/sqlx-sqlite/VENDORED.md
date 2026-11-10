# Vendored sqlx-sqlite 0.9.0

This is the crate `sqlx-sqlite` 0.9.0 as published on crates.io
(MIT OR Apache-2.0, licence texts alongside), used through
`[patch.crates-io]` in `src-tauri/Cargo.toml` (ADR 0018).

One line differs from the published crate, in `Cargo.toml`:

```diff
 [dependencies.libsqlite3-sys]
-version = ">=0.30.1, <0.38.0"
+version = ">=0.39.0, <0.40.0"
```

`libsqlite3-sys` 0.39 bundles SQLite 3.54, which fixes RUSTSEC-2026-0459
(CVE-2026-119375). The source in `src/` and the licence texts are unchanged.

Remove this directory and the patch entry once an sqlx release accepts
`libsqlite3-sys` 0.39 or later.
