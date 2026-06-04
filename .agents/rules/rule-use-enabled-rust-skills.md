# Rust-Skills und aktive Workspace-Skills nutzen

## Ziel

Der Agent soll bei Rust-bezogenen Aufgaben konsequent die verfügbaren Rust-Skills sowie alle aktivierten Workspace-Skills aus `.agents/skills/index.json` berücksichtigen, bevor er Code analysiert, ändert oder generiert.

## Vorgaben

- **Aktive Skills prüfen**: Vor relevanten Aufgaben zuerst `.agents/skills/index.json` lesen und alle dort aktivierten Skills berücksichtigen.
- **Rust-Skills bevorzugt nutzen**: Bei Rust-, Tauri-, Leptos-, WASM- oder Cargo-bezogenen Aufgaben müssen vorhandene Rust-Skills geladen und angewendet werden.
- **Nur aktivierte Skills verwenden**: Deaktivierte Skills aus `index.json` sind zu ignorieren, auch wenn die Skill-Dateien noch im Workspace vorhanden sind.
- **Skill-Inhalt lesen, nicht raten**: Der Agent darf nicht nur anhand des Skill-Namens handeln. Relevante `SKILL.md`-Dateien müssen gelesen werden, bevor daraus Regeln oder Vorgehensweisen abgeleitet werden.
- **Regeln vor Skills**: Diese und andere aktive Rules bleiben bindend. Falls ein Skill einer aktiven Rule widerspricht, hat die Rule Vorrang.
- **Kontextabhängig anwenden**: Nicht jeden Skill blind laden. Rust-/Build-/Architektur-/Tauri-/Leptos-Skills sind aber immer relevant, sobald die Aufgabe diese Bereiche berührt.

## Vermeiden

- Rust-Code ohne vorheriges Prüfen relevanter Rust-Skills ändern.
- Aktivierte Skills aus `index.json` ignorieren.
- Deaktivierte Skills trotzdem verwenden, nur weil sie im Dateisystem vorhanden sind.
- Skill-Namen interpretieren, ohne die zugehörige `SKILL.md` gelesen zu haben.
- Gegen aktive Rules arbeiten, weil ein Skill etwas anderes empfiehlt.

## Bei neuen Rust-Features

Vor der Umsetzung zuerst die aktiven Skills aus `.agents/skills/index.json` prüfen, relevante Rust-Skills lesen und deren Vorgaben in Planung, Dateistruktur, Tests und Implementierung einfließen lassen.