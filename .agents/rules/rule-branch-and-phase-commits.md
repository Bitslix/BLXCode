# Branch- und Commit-Abfrage vor Plan-Umsetzung

## Ziel

Bevor ein Agent einen Plan praktisch umsetzt, soll er den Nutzer fragen, ob für die Umsetzung ein neuer Git-Branch angelegt werden soll und ob nach jeder abgeschlossenen Phase automatisch ein Commit erstellt werden soll.

## Vorgaben

- **Vor Umsetzung fragen**: Sobald ein Plan in die Implementierung übergeht, muss der Agent fragen:
  - ob ein neuer Branch für die Umsetzung angelegt werden soll
  - wie der Branch heißen soll, falls der Nutzer keinen Namen vorgibt
  - ob nach jeder abgeschlossenen Phase automatisch ein Commit erfolgen soll

- **Kein Branch ohne Zustimmung**: Der Agent darf keinen neuen Branch anlegen, ohne dass der Nutzer dies ausdrücklich bestätigt hat.

- **Keine automatischen Commits ohne Zustimmung**: Automatische Commits nach Phasen sind nur erlaubt, wenn der Nutzer dies vorher bestätigt hat.

- **Phasen sauber abschließen**: Wenn automatische Phase-Commits aktiviert sind, darf ein Commit erst erfolgen, nachdem die jeweilige Phase sinnvoll abgeschlossen wurde, z. B. nach Implementierung, Tests, Fixes oder Dokumentation der Phase.

- **Commit-Inhalt prüfen**: Vor jedem automatischen Commit muss der Agent prüfen, welche Dateien geändert wurden, und darf keine offensichtlich unpassenden, temporären oder sensiblen Dateien committen.

- **Aussagekräftige Commit Messages**: Commit Messages sollen kurz, verständlich und phasenbezogen sein, z. B.:
  - `feat: add workspace branch setup`
  - `fix: stabilize plan phase handling`
  - `docs: update implementation notes`

- **Bestehenden Branch respektieren**: Wenn bereits auf einem passenden Feature-Branch gearbeitet wird, soll der Agent nicht unnötig einen weiteren Branch anlegen, sondern den Nutzer darauf hinweisen und nachfragen.

## Vermeiden

- Direkt auf `main`, `master`, `stage` oder Release-Branches Änderungen umsetzen, ohne den Nutzer vorher auf das Risiko hinzuweisen.
- Einen Branch automatisch anzulegen, nur weil ein Plan existiert.
- Nach jeder kleinen Dateiänderung zu committen.
- Commits mit unklarem Inhalt wie `update`, `changes`, `wip` oder `fix stuff`.
- Ungeprüfte Dateien, Build-Artefakte, Logs, Secrets oder lokale Konfigurationen zu committen.

## Bei neuen Plänen

Bevor der Agent mit der Umsetzung beginnt, zuerst den Git-Status prüfen und dann den Nutzer fragen:

> Soll ich für die Umsetzung dieses Plans einen neuen Branch anlegen?  
> Und soll ich nach jeder abgeschlossenen Phase automatisch einen Commit erstellen?

Erst nach der Antwort des Nutzers darf die Implementierung beginnen.