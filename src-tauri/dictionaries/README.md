# Wörterbücher

Mitgelieferte Hunspell-Wörterbücher der Rechtschreibprüfung von Project Write. Es sind Datenpakete, kein Programmcode: Die App liest sie zur Laufzeit. Jeder Ordner enthält die Affix- und die Wortdatei sowie `LICENSE` mit Herkunft und Lizenztext. Herkunft und Entscheidung: `docs/adr/0024-woerterbuecher.md`.

| Ordner  | Quelle                                             | Lizenz                                     |
| ------- | -------------------------------------------------- | ------------------------------------------ |
| `de-DE` | igerman98 20161207 (Ubuntu-Paket `hunspell-de-de`) | GPL-2.0-or-later                           |
| `de-AT` | igerman98 20161207 (Ubuntu-Paket `hunspell-de-at`) | GPL-2.0-or-later                           |
| `de-CH` | igerman98 20161207 (Ubuntu-Paket `hunspell-de-ch`) | GPL-2.0-or-later                           |
| `en-US` | SCOWL 2020.12.07 (Ubuntu-Paket `hunspell-en-us`)   | SCOWL-Lizenz                               |
| `en-GB` | libreoffice-dictionaries 24.2.1 (`hunspell-en-gb`) | SCOWL-Lizenz laut Debian, LGPL laut README |

`sources/` enthält den Quelltext von igerman98, aus dem die deutschen Wörterbücher erzeugt sind (GPL). `SHA256SUMS` hält die Prüfsummen aller Dateien; `tests/language.rs` vergleicht sie.

Die Dateien werden nicht von Hand geändert. Eine neue Fassung kommt als Ganzes, mit neuen Prüfsummen und einem neuen Lauf des Korpus (`cargo test --test language`).
