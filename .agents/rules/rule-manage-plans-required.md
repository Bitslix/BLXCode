# Manage-Plans-Skill bei Plan-Arbeiten verpflichtend nutzen

## Ziel

Bei allen Tätigkeiten rund um Pläne muss der Agent den Skill `manage-plans` einbeziehen. Pläne sind nicht nur Markdown-Dateien, sondern aktive Arbeitsobjekte mit Tasks, Status und Live-Fortschritt.

## Vorgaben

- **Skill immer einbeziehen**: Sobald eine Aufgabe Pläne betrifft, muss der Agent den Skill `manage-plans` laden und anwenden.
- **Gilt für alle Plan-Tätigkeiten**:
  - neue Pläne erstellen
  - bestehende Pläne lesen oder ändern
  - Plan-Phasen umsetzen
  - Tasks aus Plänen verfolgen
  - Status von Tasks oder Phasen aktualisieren
  - abgeschlossene Phasen dokumentieren
  - Plan-Fortschritt synchronisieren

- **Plan als Arbeitszustand behandeln**: Sobald aktiv an einem Plan gearbeitet wird, muss der Plan selbst als `in progress` markiert bzw. in einen aktiven Arbeitszustand gebracht werden.

- **Tasks live tracken**: Planbezogene Tasks müssen während der Umsetzung aktuell gehalten werden. Der Agent soll nicht erst am Ende den Fortschritt nachtragen, sondern Statusänderungen zeitnah schreiben.

- **Task-Status synchron halten**: Wenn ein Task begonnen, blockiert, abgeschlossen oder verworfen wird, muss dieser Status sowohl im Task-Tracking als auch im Plan selbst nachvollziehbar sein.

- **Plan-Phasen sauber abschließen**: Nach jeder abgeschlossenen Phase muss der Agent prüfen, ob:
  - zugehörige Tasks aktualisiert wurden
  - der Plan-Fortschritt korrekt ist
  - offene Folgeaufgaben ergänzt wurden
  - Blocker oder Entscheidungen dokumentiert wurden

- **Plan nicht passiv lassen**: Ein Plan darf während aktiver Umsetzung nicht im initialen/pending Zustand verbleiben.

## Vermeiden

- An einem Plan arbeiten, ohne `manage-plans` zu lesen oder anzuwenden.
- Tasks nur im Chat zu erwähnen, ohne sie im Plan-/Task-System zu aktualisieren.
- Einen Plan umzusetzen, während der Plan selbst weiterhin als nicht gestartet markiert ist.
- Abgeschlossene Phasen ohne Status-Update oder Plan-Synchronisierung stehen lassen.
- Plan- und Task-Status auseinanderlaufen lassen.
- Fortschritt erst am Ende gesammelt nachtragen, obwohl während der Arbeit klare Statuswechsel stattgefunden haben.

## Bei Plan-Umsetzung

Vor Beginn der eigentlichen Umsetzung:

1. `manage-plans` Skill laden.
2. Den betroffenen Plan lesen.
3. Den Plan als `in progress` markieren.
4. Die zugehörigen Tasks laden oder aus dem Plan synchronisieren.
5. Während der Umsetzung Task-Status live aktualisieren.
6. Nach jeder Phase Plan und Tasks synchronisieren.

Der Plan ist die führende Arbeitsgrundlage. Der Chat allein reicht nicht als Fortschrittsnachweis.