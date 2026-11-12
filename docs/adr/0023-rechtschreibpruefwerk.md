# ADR 0023 – Prüfwerk für die Rechtschreibung: `spellbook` statt Hunspell-Bindings

- Status: angenommen
- Datum: 12. November 2026
- Phase: 12 (erster Schritt)

## Kontext

Phase 12 beginnt laut Abschnitt 25 mit einem Vergleich von `spellbook` und Hunspell-Bindings. Verlangt ist ein deutscher Korpus aus mindestens 1.000 richtigen Wörtern, darunter Komposita, und 200 Fehlern; das Ergebnis wird eine ADR. Abschnitt 7 sieht `spellbook` vor, nennt es aber noch Alpha. Abnahme von Phase 12: höchstens 3 % der richtigen Wörter markiert, mindestens 95 % der Fehler erkannt.

## Vorgehen

- **Korpus** (`src-tauri/tests/data/language/`, selbst zusammengestellt):
  - `de-richtig.txt`: 1.198 verschiedene richtige Wörter. Dazu gehören Substantive in gebeugten Formen, Verben in verschiedenen Formen, Adjektive mit Steigerung, Funktionswörter, Wörter mit Umlauten, ß und Fremdwörter sowie rund 370 Komposita.
  - `de-fehler.txt`: 212 Fehler mit der richtigen Form und der Art. Die Arten sind Vertauschung, fehlender oder doppelter Buchstabe, Nachbartaste, Umlaut als `ae`/`oe`/`ue` oder ohne Punkte, `ss` statt `ß` und alte Rechtschreibung (`daß`), fehlendes Fugen-s und Fehler in Komposita.
- **Wörterbuch:** igerman98 `de_DE` in der Fassung 20161207, das Debian-Paket `hunspell-de-de`. Es dient nur dem Vergleich und kommt nicht ins Repository.
- **Werkzeug:** `tools/spell-compare/`, ein eigenes Cargo-Projekt außerhalb der App, das nicht ausgeliefert wird. Es misst markierte richtige Wörter, erkannte Fehler, den Platz der richtigen Form unter den Vorschlägen und Zeiten. Aufruf: `cargo run --release -- /usr/share/hunspell/de_DE`.
- **Umgebung:** Cloud-Container, 4 vCPU, Release-Build.

## Ergebnis

| Prüfwerk                                  | richtige Wörter markiert | Fehler erkannt       | richtige Form an 1. Stelle | unter den ersten 5 | fehlt | Prüfung (Median / 95 %) | Vorschläge (Median / 95 % / max)   |
| ----------------------------------------- | ------------------------ | -------------------- | -------------------------- | ------------------ | ----- | ----------------------- | ---------------------------------- |
| `spellbook` 0.4.2                         | 0 von 1.198 (0,0 %)      | 209 von 212 (98,6 %) | 168                        | 203                | 5     | 0,4–0,6 µs / 13–17 µs   | 33–36 ms / 129 ms / 323–343 ms     |
| Hunspell 1.7.2 über `hunspell-rs` 0.4.0   | 0 von 1.198 (0,0 %)      | 209 von 212 (98,6 %) | 174                        | 205                | 6     | 0,9–1,0 µs / 40–45 µs   | 19 ms / 65–69 ms / 85–88 ms        |
| `spellbook`, umsortiert nach Abschnitt 14 | 0 von 1.198 (0,0 %)      | 209 von 212 (98,6 %) | 175                        | 209                | 3     | wie oben                | 33–34 ms / 123–129 ms / 304–341 ms |

Beide Prüfwerke erfüllen die Abnahme mit Abstand und markieren und erkennen dieselben Wörter.

**Die drei nicht erkannten Fehler sind echte Wörter.** „Wettre“ ist eine Form von „wettern“, „Flugzug“ ein gültiges Kompositum, „Aufhabe“ eine Form von „aufhaben“; am Satzanfang werden sie großgeschrieben. Kein Wörterbuch kann sie als Fehler erkennen. Ich lasse sie im Korpus stehen, weil sie diese Grenze zeigen.

**Wo die Vorschläge versagen:** Beide Prüfwerke finden ohne Hilfe oft keine Umlaut- und ß-Formen. Das betrifft `Fruehstueck`, `Grosse` (→ Größe), `Gruesse` (→ Grüße), `Tur`, `Bucher` und `Wohnungstur`. Bei `Wohnungstur` schlägt Hunspell „Wohnung stur“ vor.

Das Umsortieren aus Abschnitt 14 behebt das bis auf drei Fälle. Dabei stehen zuerst die Umlaut- und ß-Varianten des Wortes, bis zu zwei Änderungen weit, wenn das Wörterbuch sie kennt. Dann folgen Einwort-Vorschläge vor solchen mit Leerzeichen oder Bindestrich, höchstens fünf. Die drei Ausnahmen sind `Mitter` → Mutter, `Sinner` → Sommer und `Geburtag` → Geburtstag. Eigene Wörterbücher als zweite Stufe kommen in der App hinzu.

## Entscheidung

**`spellbook` 0.4.2** (MPL-2.0), hinter der Schnittstelle `LanguageEngine`, mit dem Umsortieren aus Abschnitt 14.

Begründung:

- **Genauigkeit:** gleiche Erkennung wie Hunspell; nach dem Umsortieren auch die besten Vorschläge aller drei.
- **Rust ohne native Bibliothek:** Hunspell braucht `libhunspell` und zum Bauen der Bindings bindgen mit libclang. Unter Windows (Priorität) und macOS müsste die App Hunspell mitbauen und ausliefern. `spellbook` baut auf allen Systemen gleich und passt zu `cargo deny`, das MPL-2.0 erlaubt.
- **Lizenz:** MPL-2.0 ist ein schwaches Copyleft auf Dateiebene, wie bei den MPL-Crates von Tauri. Hunspell selbst steht unter MPL 1.1, GPL 2.0 oder LGPL 2.1 und würde als C++-Bibliothek gebunden.
- **Zeiten:** Hunspell liefert Vorschläge etwa doppelt so schnell. `spellbook` braucht bis zu 340 ms für eine Liste, wie in der Stichprobe von Abschnitt 14 (330 ms). Das passt zur Vorgabe, Vorschläge und Autokorrektur nie im Eingabe-Thread und asynchron zu rechnen. Die Prüfung selbst (unter 20 µs) liegt weit unter der Tippverzögerung.

**Risiko:** `spellbook` gilt als Alpha. Die Schnittstelle `LanguageEngine` erlaubt einen späteren Wechsel zu Hunspell ohne Änderung an Editor und Oberfläche. Der Korpus und `tools/spell-compare` bleiben, um jede neue Fassung beider Prüfwerke erneut zu messen.

## Folgen

- Neue Abhängigkeit im nächsten Schritt: `spellbook` 0.4.2 in `src-tauri` (Begründung in `docs/dependencies.md`, wenn sie eingebaut wird).
- Wörterbücher: igerman98 steht unter GPL. Nach Abschnitt 7 wird es als getrenntes Datenpaket mit eigenem Lizenztext ausgeliefert, nie in den Programmcode einkompiliert. Woher die App ihre Wörterbücher bezieht und wie sie sie mitliefert, entscheidet eine eigene ADR in Phase 12.
- Der Korpus wird in Phase 12 zur Abnahme genutzt und erweitert, etwa um die Fälle der Autokorrektur und die Satzende-Testdaten.
- `tools/spell-compare` braucht `libhunspell-dev` und `libclang-dev`. Es ist nicht Teil von CI und nicht der App.
