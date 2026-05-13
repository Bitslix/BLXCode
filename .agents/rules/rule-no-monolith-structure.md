# Keine Monolith-Struktur

## Ziel

Code soll in klar abgegrenzte Module und Dateien verteilt sein — keine „God-Files“, keine unkontrolliert wachsenden Einstiegspunkte.

## Vorgaben

- **Kleine, fokussierte Einheiten**: Eine Datei deckt idealerweise ein Thema ab (z. B. eine Komponente, ein Service, ein Protokoll-Typ-Block). Wenn eine Datei schwer zu überblicken wird, Inhalte auslagern.
- **UI-Komponenten**: Eigenen **Subfolder pro Komponente** nutzen; komponentenspezifische **CSS-Dateien im selben Ordner** ablegen (Angular-ähnliche Kapselung). Details: `rule-reusable-components.md`.
- **Schichten und Grenzen**: UI (Leptos), Tauri-Commands/State, reine Logik und I/O klar trennen. Keine Vermischung von DOM/Signals mit direktem Dateizugriff oder HTTP-Details in derselben Schicht ohne klare Abstraktion.
- **Workspace-Kontext**: `blxcode-ui` (WASM) und `blxcode` (Tauri) bleiben getrennt; gemeinsame Typen nur dort, wo es bereits etabliert ist (z. B. `agent_wire` / Protokoll), nicht durch Kopieren von halben Stacks.
- **Kein „alles in `main`/`lib`“**: `main.rs` / `lib.rs` nur verdrahten und registrieren; Implementierung in Untermodulen.

## Vermeiden

- Eine Komponente oder eine Rust-Datei mit hunderten Zeilen Geschäftslogik, mehreren unabhängigen Features und globalem Zustand.
- Zyklische Abhängigkeiten zwischen Modulen — bei Bedarf gemeinsame kleine Hilfsmodule oder Traits, nicht gegenseitige `use`-Ketten.

## Bei neuen Features

Zuerst passendes bestehendes Modul wählen oder ein neues Untermodul anlegen; nur erweitern, wenn die Verantwortung wirklich zusammengehört.
