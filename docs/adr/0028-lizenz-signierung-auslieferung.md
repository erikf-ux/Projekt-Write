# ADR 0028 – Lizenz, Signierung, Updates und Plattformen der Auslieferung (D4, D11)

- Status: angenommen
- Datum: 18. November 2026
- Phase: 14

## Kontext

Vor Phase 14 sind zwei Entscheidungen des Auftraggebers einzuholen:

- **D4:** die Lizenz der App.
- **D11:** die Signierung der Auslieferung.

Dazu kommen zwei Fragen, die sich daraus ergeben:

- ob die App signierte Updates bekommt;
- wie mit macOS umzugehen ist, das seit dem 2. November 2026 zurückgestellt ist, während Phase 14 Installer für alle Plattformen nennt.

Der Auftraggeber hat am 18. November 2026 entschieden (Antworten auf die Rückfrage zu Phase 14).

## Entscheidung

1. **Lizenz (D4):** Project Write steht unter **MIT OR Apache-2.0**, nach Wahl des Nutzers.
   - **Lizenzdateien:** `LICENSE-MIT` und `LICENSE-APACHE` im Wurzelordner.
   - **Angaben:** `license = "MIT OR Apache-2.0"` in `Cargo.toml` und `package.json`.
   - **Rechteinhaber:** „The Project Write authors“. Ein Name wird erst eingetragen, wenn der Auftraggeber ihn nennt.
   - **Verträglichkeit mit den Fremdlizenzen:**
     - **GPL-Wörterbücher:** Sie sind getrennte Datenpakete, die die App liest; sie werden nicht gelinkt (bloße Zusammenstellung). Sie werden mit Lizenztext und vollständigem Quelltext (igerman98) ausgeliefert (ADR 0024).
     - **MPL-2.0-Bibliotheken** (`spellbook`, einige Crates im Tauri-Baum): Copyleft auf Dateiebene. Erfüllt, solange ihr Quelltext öffentlich verfügbar ist und `THIRD-PARTY-LICENSES` sie nennt; geändert wird keine Datei.
     - **ELK.js:** Es steht unter „EPL-2.0 OR GPL-3.0-or-later“; gewählt ist EPL-2.0. Unverändert verwendet, mit Lizenztext in der Lizenzliste.
     - **en-GB-Wörterbuch:** Debian nennt die SCOWL-Lizenz, das README die LGPL. Beide Texte liegen bei; beide erlauben die Weitergabe als getrennte Datei.

     Die Liste aller Ausnahmen von MIT und Apache-2.0 steht in `THIRD-PARTY-LICENSES` und `docs/dependencies.md`.
2. **Signierung (D11):** Die Installer werden **vorerst nicht signiert**.
   - Unter Windows warnt SmartScreen beim Download; Smart App Control unter Windows 11 blockiert die App ganz.
   - Die Installer sind für Test und eigene Nutzung gedacht. Eine Signierung (Azure Trusted Signing oder ein eigenes Zertifikat) lässt sich später nachrüsten, ohne den Build umzubauen.
3. **Updates:** Die App bekommt **noch keine Update-Funktion**.
   - Es gibt also keine Netzverbindung, keinen Update-Endpunkt und keinen Signaturschlüssel.
   - Neue Versionen installiert man von Hand über den Installer. Die Ausnahme „Updateprüfung“ aus Abschnitt 2 wird damit nicht genutzt; der Netzwerktest erwartet im Normalbetrieb überhaupt keine Verbindung.
4. **Plattformen:** **Windows und Linux.** macOS bleibt zurückgestellt; der Bericht führt Installer und Tests für macOS als offen.
   - Windows: Installer NSIS und MSI.
   - Linux: `.deb` und AppImage.

## Folgen

- Die offenen Punkte D4 und D11 aus `docs/status.md` sind entschieden.
- Phase 14 baut unsignierte Installer für Windows und Linux in CI. Installation und Deinstallation werden auf frischen CI-Runnern geprüft.
- Kommt später eine Update-Funktion, braucht sie eine eigene ADR: Endpunkt, Schlüsselverwahrung, Bestätigung durch den Nutzer und Anpassung des Netzwerktests.

## Nachtrag 19. November 2026

Nach dem Bericht zu Phase 14 hat der Auftraggeber zwei offene Punkte entschieden:

1. **AppImage:** Der Zielwert „Installationspaket ≤ 50 MB“ (Abschnitt 6) gilt für das AppImage **nicht**; es ist eine Ausnahme. Ein AppImage muss WebKitGTK, JavaScriptCore und ICU selbst mitbringen und ist deshalb rund 89 MB groß (`docs/installation.md`). Für NSIS, MSI und `.deb` gilt der Zielwert weiter; CI bricht ab, wenn eines davon größer ist, und misst das AppImage nur (`--report-only AppImage`).
2. **Rechteinhaber:** In `LICENSE-MIT` und `LICENSE-APACHE` bleibt **„The Project Write authors“** stehen.

Außerdem hat er die offenen Handtests freigegeben. Sie brauchen echte Geräte und eine Person davor; die Liste mit Vorgehen und Protokoll steht in `docs/handtests.md`.
