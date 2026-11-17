# ADR 0024 – Wörterbücher: Herkunft, Lizenzen, Auslieferung

- Status: angenommen
- Datum: 17. November 2026
- Phase: 12

## Kontext

Abschnitt 14 verlangt mitgelieferte Wörterbücher für Deutsch (`de-DE`, `de-AT`, `de-CH`) und Englisch (`en-US`, `en-GB`). Der Nutzer kann weitere Hunspell-Wörterbücher (`.aff`, `.dic`) als Sprachen hinzufügen. Abschnitt 7 schreibt vor, Wörterbücher als getrennte Datenpakete mit eigenem Lizenztext auszuliefern, nie in den Programmcode einkompiliert. Deutsche Wörterbücher stehen in der Regel unter GPL. Die App lädt nichts nach, außer optionalen Zusatzpaketen nach Bestätigung (Abschnitt 2). Das Prüfwerk ist `spellbook` 0.4.2 (ADR 0023).

## Entscheidung

1. **Herkunft:** Die Dateien stammen aus den Paketen von Ubuntu 24.04 (noble). Die Paketquelle prüft ihre Signaturen; die Prüfsummen der Quellarchive stimmen mit der `.dsc`-Datei überein.

   | Sprache                   | Paket und Fassung               | Quelle                            | Lizenz                                                         |
   | ------------------------- | ------------------------------- | --------------------------------- | -------------------------------------------------------------- |
   | `de-DE`, `de-AT`, `de-CH` | `hunspell-de-*` 20161207-12     | igerman98 20161207 (Björn Jacke)  | GPL-2.0-or-later, Affixdatei GPLv2 und GPLv3                   |
   | `en-US`                   | `hunspell-en-us` 1:2020.12.07-2 | SCOWL 2020.12.07 (Kevin Atkinson) | eigene, freizügige Lizenz von SCOWL                            |
   | `en-GB`                   | `hunspell-en-gb` 1:24.2.1-1     | libreoffice-dictionaries 24.2.1   | laut Debian die SCOWL-Lizenz, laut README des Wörterbuchs LGPL |

   igerman98 ist dieselbe Fassung, mit der ADR 0023 gemessen hat.

2. **Ablage im Repository:** `src-tauri/dictionaries/<Tag>/<Tag>.aff`, `<Tag>.dic` und `LICENSE` mit dem Copyright-Text des Pakets und den vollen Lizenztexten (GPL-2.0, GPL-3.0, LGPL-2.1). `de-CH` hat zusätzlich `README.swiss`. Die Dateien sind unverändert, nur umbenannt (`de_DE` → `de-DE`). `SHA256SUMS` hält ihre Prüfsummen; ein Rust-Test vergleicht sie. `.gitattributes` schützt sie vor einer Umwandlung der Zeilenenden. Zusammen rund 6 MB.

3. **Quelltext für die GPL:** igerman98 wird aus Wortlisten erzeugt. Die `.dic`- und `.aff`-Dateien sind also nicht die bevorzugte Form zur Bearbeitung. Deshalb liegen das Quellarchiv (`igerman98_20161207.orig.tar.bz2`), die Debian-Änderungen (`…-12.debian.tar.xz`) und die `.dsc` in `dictionaries/sources/` und werden mit ausgeliefert. Das ist der vollständige Quelltext genau der ausgelieferten Dateien. Bei `en-US` und `en-GB` sind `.aff` und `.dic` selbst die gepflegte Form.

4. **Auslieferung:** Tauri legt den Ordner als Ressource neben das Programm (`bundle.resources`). Die App liest ihn zur Laufzeit aus dem Ressourcenordner. Im Programmcode steckt kein Wörterbuch. Ein Wörterbuch wird bei der ersten Prüfung in seiner Sprache geladen und bleibt im Speicher. Das dauert 20 bis 35 ms je Sprache (Container, Debug- und Release-Build gleich, weil `spellbook` auch im Debug-Build optimiert wird).

5. **Eigene Sprachwörterbücher:** Ein importiertes Paar `.aff`/`.dic` liegt im Einstellungsordner der App unter `dictionaries/<Tag>/`, wie die importierten Themes. Es gilt für alle Bibliotheken und hat bei gleichem Tag Vorrang vor dem mitgelieferten.
   - **Prüfung beim Import:** Der Tag muss die Form eines Sprach-Tags haben, er wird auch als Ordnername benutzt. Die Größe ist begrenzt (Affixdatei 16 MB, Wortdatei 64 MB). Kodierungen: UTF-8, ISO 8859-1 und ISO 8859-15; alles wird nach UTF-8 umgewandelt. Erst nach dem Lesen durch `spellbook` wird atomar geschrieben.
   - **Entfernen:** Ein mitgeliefertes Wörterbuch lässt sich nicht entfernen.
   - **Sprachwahl:** `de` oder eine unbekannte Region wie `de-LU` nutzt `de-DE`, `en` nutzt `en-US`, sonst die erste Variante derselben Sprache.

## Alternativen

- **Herunterladen beim Bauen** mit festen Prüfsummen: weniger Dateien im Repository, aber jeder Build und jeder CI-Lauf hinge vom Netz ab. Verworfen.
- **Wörterbücher von LibreOffice** (`de_DE_frami`): neuer als igerman98, aber nicht mit dem Korpus gemessen. Ein Wechsel ist später ein Austausch der Dateien, mit erneutem Lauf des Korpus.
- **Wörterbuch einkompilieren** (`include_str!`): durch Abschnitt 7 ausgeschlossen.

## Offene Punkte vor der Weitergabe (Phase 14, mit D4)

- **en-GB:** Debian nennt die freizügige SCOWL-Lizenz, das README des Wörterbuchs die LGPL. Beide Texte liegen in `en-GB/LICENSE`, und beide erlauben die Weitergabe als getrennte Datei. Abschnitt 7 sagt „Stoppe bei Zweifeln“. Weitergegeben wird vor Phase 14 nichts, deshalb steht der Punkt bei D4.
- **GPL und App-Lizenz:** Die Wörterbücher sind Daten, die die App liest; sie werden nicht gelinkt. Das gilt als bloße Zusammenstellung. Mit D4 ist zu bestätigen, dass das für die gewählte App-Lizenz passt.

## Folgen

- Neue Abhängigkeit: `spellbook` 0.4.2 (MPL-2.0) in `src-tauri`, Begründung in `docs/dependencies.md`.
- `THIRD-PARTY-LICENSES` nennt die Wörterbücher als Datenpakete.
- Das Installationspaket wird um etwa 2 MB größer (komprimiert); Ziel bleibt ≤ 50 MB.
- Tests: Prüfsummen, Laden aller fünf Sprachen, Korpus mit Abnahme (`tests/language.rs`), Import und Umwandlung der Kodierung (Unit-Tests).
