# Architekturentscheidungen (ADR)

Kurze Entscheidungsnotizen nach dem Muster Kontext → Optionen → Entscheidung → Folgen.
Eine ADR wird nicht nachträglich umgeschrieben. Ändert sich eine Entscheidung, entsteht eine neue ADR, die die alte ablöst.

| Nr.                                           | Titel                                                                        | Status     |
| --------------------------------------------- | ---------------------------------------------------------------------------- | ---------- |
| [0001](0001-versionen-und-kompatibilitaet.md) | Versionen und Kompatibilität des Technologiestacks                           | angenommen |
| [0002](0002-ipc-vertrag.md)                   | IPC-Vertrag: validierte Eingaben, einheitliches Fehlerformat, erzeugte Typen | angenommen |
| [0003](0003-e2e-tests-ohne-global-tauri.md)   | E2E-Tests mit eingebettetem WebDriver, ohne `withGlobalTauri`                | angenommen |
| [0004](0004-datenbank-vor-der-bibliothek.md)  | Datenbank vor der Bibliothek: In-Memory in Phase 1, erste Migration          | angenommen |
| [0005](0005-isolation-pattern.md)             | Isolation Pattern                                                            | angenommen |
