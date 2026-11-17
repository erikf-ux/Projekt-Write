# ADR 0026 – LanguageTool als Zusatzpaket über eine vorhandene Installation

- Status: angenommen
- Datum: 17. November 2026
- Phase: 12

## Kontext

Abschnitt 14 sieht für Grammatik, Zeichensetzung und Kontextfehler LanguageTool als optionales Zusatzpaket vor. Für LanguageTool gilt:

- Lizenz LGPL-2.1;
- es braucht Java;
- das Paket ist rund 250 MB groß, mit n-Gramm-Daten mehrere GB.

Abschnitt 2 erlaubt Netzverbindungen nur für Updates und optionale Zusatzpakete nach Bestätigung. Ein Herunterladen ließ sich in Phase 12 nicht umsetzen und prüfen: Die Umgebung erreichte languagetool.org nicht. Der Nutzer entschied sich deshalb für Variante 2: Die App nutzt eine Installation, die schon auf dem Rechner liegt.

## Entscheidung

1. **Einrichten:**
   - Der Nutzer wählt in den Einstellungen den Ordner einer LanguageTool-Installation.
   - Gültig ist ein Ordner mit `languagetool-server.jar`.
   - Ordner und Schalter stehen in `settings.json` unter `languageTool`. Nur Rust liest und schreibt sie.
   - Die App lädt nichts herunter und öffnet keine Verbindung nach außen.
2. **Start:**
   - Wann: beim ersten Abfragen des Zustands, wenn eine Installation gewählt und eingeschaltet ist; das geschieht beim Öffnen des Arbeitsbereichs.
   - Java: aus `JAVA_HOME` oder `java` im Suchpfad, mindestens Version 17.
   - Server: `java -cp languagetool-server.jar org.languagetool.server.HTTPServer --port N`, mit einem freien Port N. Der Server lauscht nur auf `127.0.0.1` (Voreinstellung von LanguageTool ohne `--public`).
   - Bereitschaft: Ein Hintergrund-Thread wartet bis zu 120 s, bis `/v2/languages` antwortet.
   - Anzeige während des Starts: Die Oberfläche fragt alle 2 s nach dem Zustand.
   - Beim Wechsel auf „läuft“ steigt die Epoche, damit alle Absätze auch die Fundstellen von LanguageTool bekommen.
3. **Prüfen:**
   - Aufruf: `language_check` ruft LanguageTool zusätzlich zum eigenen Prüfwerk auf, je Absatz über `POST /v2/check`.
   - HTTP-Client: ein kleiner HTTP/1.1-Client in Rust, ohne neue Abhängigkeit, mit Zeitlimit von 20 s.
   - Verworfene Fundstellen:
     - Rechtschreibung und Stil (`misspelling`, `style`, Kategorien `TYPOS`, `STYLE`), denn die Rechtschreibung bleibt beim eigenen Prüfwerk mit den eigenen Wörterbüchern;
     - Fundstellen, die eine eigene Fundstelle überlappen.
   - Umrechnung: LanguageTool zählt UTF-16-Einheiten; sie werden in Byte-Offsets umgerechnet.
   - Regeln: Sie heißen `lt:<ID>`. Das Kontextmenü zeigt die Erklärung von LanguageTool vor den Vorschlägen.
4. **Fehler:**
   - Wann: Java fehlt, der Server startet nicht oder endet.
   - Folge: Der Zustand wird „fehlgeschlagen“; die Rechtschreibprüfung läuft ohne LanguageTool weiter.
   - Anzeige: Die Einstellungen nennen den Grund, die Statusleiste zeigt „LanguageTool nicht verfügbar“.
   - Neuer Versuch: Aus- und Einschalten startet neu.
5. **Ende:** Der Server wird beendet:
   - beim Beenden der App;
   - beim Ausschalten;
   - beim Vergessen der Installation;
   - beim Wechsel der Installation.

## Alternativen

- **Variante 1, Herunterladen nach Bestätigung, mit fester Prüfsumme:** entspricht Abschnitt 14 am genauesten, war aber in Phase 12 nicht prüfbar. Sie bleibt als Erweiterung möglich; Prüfwerk, Start und Oberfläche bleiben dann gleich, nur das Einrichten kommt hinzu.
- **Öffentliche LanguageTool-API:** Texte würden das Gerät verlassen. Ausgeschlossen durch Abschnitt 2.
- **Java mitliefern:** Das Installationspaket würde deutlich größer als 50 MB. Verworfen.

## Folgen

- Keine neue Abhängigkeit.
- Keine LGPL-Komponente im Installationspaket: LanguageTool liegt beim Nutzer und läuft als eigener Prozess.
- Die App funktioniert ohne LanguageTool vollständig.
- In der CI ist LanguageTool nicht installiert. Getestet werden mit einem lokalen Testserver: Zustandslogik, Erkennen der Installation, Java-Versionen, HTTP-Client und Umwandlung der Antwort. Ein Test mit einer echten Installation steht aus.
