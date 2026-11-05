# ADR 0016 – Wiederherstellung, Papierkorb, Archiv und Historie

- Status: angenommen
- Datum: 5. November 2026
- Phase: 6

## Kontext

Phase 6 umfasst laut Abschnitt 25 die Wiederherstellung der Stufen 1 bis 4 (Glossar, Abschnitt 4), Löschen und Papierkorb, Archivieren sowie die Dokumenthistorie mit Aufbewahrung und Versionsvergleich. Abnahme: Ein harter Abbruch beim Tippen kostet höchstens 2 s Text, der nächste Start bietet die Wiederherstellung an; ein gelöschtes Element kehrt mit Zuordnungen und Verbindungen zurück; die Aufbewahrung arbeitet richtig, getestet mit simulierter Zeit; jede Wiederherstellung sichert zuerst den aktuellen Stand. Regeln stehen in den Abschnitten 9 (Speichern, Recovery-Entwürfe), 13 (Dokumentverwaltung, Historie) und 18 (`deleted_at`, `archived_at`).

## Entscheidungen

1. **Umfang.** Stufe 5 (Backup) gehört zu Phase 10. Zuordnungen zu Subnetzen und Tags entstehen in Phase 7; für sie gilt dasselbe Muster wie für die Links: Die Zeile bleibt im Papierkorb, also bleiben auch ihre Zuordnungen. Die Explorer-Einträge „Nicht zugeordnet“ und „Medien“ kommen mit den Phasen 7 und 8.
2. **Papierkorb (Stufe 3).** Löschen setzt `deleted_at`; Zeile, Datei, Links, Historie und Favorit bleiben. Wiederherstellen setzt die Spalte zurück, und alles ist wieder da – auch die Rückverweise, die die Abfrage nur für Dokumente außerhalb des Papierkorbs zeigt. Endgültig entfernt wird nach `trash.retentionDays` (Bibliothekseinstellung, Standard 30, 1–3650) beim Öffnen der Bibliothek und beim Anzeigen des Papierkorbs, oder auf Wunsch („Endgültig löschen“, „Papierkorb leeren“, beides mit Rückfrage). Endgültig: erst die Zeile (Kaskade über Links und Historie), dann die Datei und ein Recovery-Entwurf. Ein gelöschtes offenes Dokument verliert seinen Tab, nachdem gespeichert ist.
3. **Archiv.** Archivieren setzt `archived_at`. Archivierte Dokumente fehlen in „Favoriten“, „Zuletzt verwendet“, „Alle Dokumente“ und in der Dokumentauswahl für Links und stehen unter „Archiv“; sie lassen sich öffnen und bearbeiten. Aus dem Archiv holen setzt die Spalte zurück. Ein archiviertes Dokument kann in den Papierkorb; wiederhergestellt ist es weiter archiviert.
4. **Historie (Stufe 4).** Migration 0006 legt `document_history` nach dem Entwurf in `docs/database.md` an. Versionen entstehen
   - `auto`: beim Speichern, wenn die neueste Version älter als 10 Minuten ist – das ergibt eine Version je 10 Minuten aktiver Bearbeitung, denn ohne Bearbeitung wird nicht gespeichert;
   - `close`: beim Schließen des Tabs;
   - `beforeConversion`: vor dem Umwandeln (alter Inhalt im alten Format);
   - `beforeRestore`: vor jeder Wiederherstellung einer Version oder eines Entwurfs;
   - `named`: „Version speichern“ mit einem Namen.

   Gespeichert wird der Inhalt der Datei, komprimiert mit Deflate (`flate2`, reines Rust, `docs/dependencies.md`). Gleicht der Inhalt der neuesten Version, entsteht keine neue; eine benannte wird dann zur Benennung dieser Version.

5. **Aufbewahrung** als reine Funktion über (Versionen, Zeitpunkt, Regel), getestet mit simulierter Zeit: alle Versionen der letzten 24 Stunden; bis 30 Tage die neueste je Kalendertag (UTC); bis 365 Tage die neueste je ISO-Woche; ältere entfallen. Benannte Versionen und die neueste Version bleiben immer. Die drei Grenzen sind Bibliothekseinstellungen (`history.keepAllHours`, `history.dailyDays`, `history.weeklyDays`). Rust wendet sie nach jeder neuen Version auf das Dokument an und beim Öffnen der Bibliothek auf alle.
6. **Versionsvergleich** nebeneinander im Frontend, zeilenweise mit einem eigenen Myers-Diff (keine Abhängigkeit). Verglichen wird der Text: TXT und Markdown wie gespeichert, `.pwrite` als Markdown-Text seines Inhalts. Zwei beliebige Versionen oder eine Version mit dem aktuellen Stand.
7. **Version wiederherstellen** sichert zuerst den aktuellen Stand als `beforeRestore` und schreibt dann die Version – auch in ihrem alten Format, wenn sie vor einer Umwandlung entstand (gleicher Ablauf wie beim Umwandeln: neue Datei, Zeile, alte Datei zuletzt). Ein offener Tab lädt neu.
8. **Recovery-Entwürfe (Stufe 2).** Das Frontend schickt den Text eines Tabs mit ungespeicherten Änderungen spätestens 1 s nach der ersten ungespeicherten Eingabe an Rust (gedrosselt, nicht entprellt: auch beim ununterbrochenen Tippen höchstens alle 1 s). Rust schreibt atomar nach `recovery/<id>.json`: Art, Dokument-ID oder Pfad der externen Datei, Format, Kodierung und Zeilenenden, Prüfsumme des gespeicherten Stands, Zeit, Text. Damit kostet ein harter Abbruch höchstens etwa 1 s Text plus die Schreibzeit. Gilt auch für externe Dateien. Ist ein Tab gespeichert oder verworfen, löscht das Frontend den Entwurf.
9. **Angebot beim Start.** Nach dem Öffnen der Bibliothek listet Rust die Entwürfe, deren Text vom gespeicherten Stand abweicht; Entwürfe ohne Abweichung oder zu gelöschten Dokumenten entfernt es. Die Oberfläche fragt „Wiederherstellung (Absturz)“: Wiederherstellen oder Verwerfen, je Entwurf oder alle. Wiederherstellen eines Bibliotheksdokuments sichert zuerst den aktuellen Stand als `beforeRestore` und speichert dann den Entwurf; bei einer externen Datei öffnet der Tab mit dem Entwurf als ungespeicherter Änderung, die Datei bleibt bis `Strg + S` unverändert.
10. **Sitzung (Stufe 1)** in `settings.json` (`session`): ID der Bibliothek, Tabs in Reihenfolge (Bibliotheksdokument per ID, externe Datei per Pfad – die Pfade bleiben in Rust, das Frontend schickt Sitzungs-IDs), aktiver Tab, je Tab Ansicht und Auswahl (Anker, Kopf). Gespeichert wird 2 s nach einer Änderung der Tabs und vor dem Beenden. Beim nächsten Start mit derselben Bibliothek öffnet die Arbeitsfläche die Tabs wieder und setzt Auswahl und Ansicht; fehlende Dokumente und Dateien entfallen still. Das Layout (Seitenleiste, Fenster, Zoom) speichert schon `settings.json` seit Phase 3.
11. **Befehle und Oberfläche.** Explorer-Einträge „Archiv“ und „Papierkorb“; Kontextmenü und Befehlspalette: Archivieren, Aus dem Archiv holen, Löschen (`Entf` im Explorer), Wiederherstellen, Endgültig löschen, Papierkorb leeren, Version speichern, Versionen (Historie). Die Historie ist ein Dialog mit Liste, Vergleich nebeneinander und Wiederherstellen. In Oberfläche und Code steht immer die Stufe (Abschnitt 4).
12. **Eine neue Abhängigkeit:** `flate2` (Deflate) für die Historie; im Baum schon zur Übersetzungszeit (über `png`).

## Folgen

- `docs/database.md`: Migration 0006 `document_history`; die Einstellungen `trash.retentionDays` und `history.*` werden genutzt.
- Externe Dateien haben weiter keine Historie (Abschnitt 9), aber Recovery-Entwürfe.
- Ein Recovery-Entwurf enthält den Text im Klartext im Bibliotheksordner, wie die Dokumente selbst (keine Verschlüsselung in 1.0, Abschnitt 20).
