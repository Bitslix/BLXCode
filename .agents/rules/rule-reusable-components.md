# Wiederverwendbare Komponenten

## Ziel

UI und Hilfslogik so gestalten, dass sie an mehreren Stellen nutzbar sind — ohne Duplikate und ohne unnötige Kopplung an einen einzigen Screen.

## Vorgaben

- **Ordner pro Komponente (Angular-ähnlich)**: Jede eigenständige UI-Komponente liegt in einem **eigenen Unterordner** mit allem, was zu ihr gehört — übersichtlich und greifbar wie ein Angular-`component/`. Beispielstruktur:

  ```text
  workbench/
    sidebar/
      mod.rs              # oder mod.rs + view.rs — Haupteinstieg der Komponente
      sidebar.css         # nur Styles dieser Komponente (Name an Komponente anpassen)
      # optional: kleine Hilfen nur für diese Komponente, z. B. types.rs
  ```

  Namenskonvention: Ordnername in `kebab-case` oder wie im bestehenden Modulbaum üblich; **eine CSS-Datei (oder wenige klar benannte) im selben Ordner** für komponentenspezifische Regeln. Globale Tokens/Themes bleiben in den projektweiten Styles (z. B. `styles.css`); hier nur Layout/Overrides, die wirklich zu dieser Komponente gehören.

- **Einbindung der CSS-Datei**: Technisch per Projekt-Standard (z. B. `@import` aus der zentralen Einstiegs-CSS, oder Trunk-`<link data-trunk rel="css" …>` in `index.html`, oder Leptos-`Style`/äquivalent) — wichtig ist: **Pfad und Lebensdauer** zur Komponente gehören zusammen; keine „verwaisten“ Styles in weit entfernten Ordnern.

- **Props/Schnittstelle klar**: Öffentliche API einer Komponente (Props, Callbacks, optionale Slots/Children) bewusst halten — keine versteckten globalen Abhängigkeiten, wenn es vermeidbar ist.
- **Generisch dort, wo es sich lohnt**: Wiederkehrende Muster (Buttons, Panels, Listenzeilen, Modals) als eigene Komponenten oder kleine Bausteine auslagern statt copy-paste.
- **Styling konsistent**: Bestehende Klassen/Tokens und Layout-Muster des Projekts nutzen; keine parallelen „Einmal“-Styles für dasselbe UI-Muster.
- **Zustand**: Lokal halten, was nur diese Komponente betrifft; geteilten Zustand über Eltern, Context oder etablierte Services — nicht über implizite Seiteneffekte in tiefer verschachtelten Kindern.

## Vermeiden

- Eine Komponente als einzelne `.rs`-Datei **ohne** zugehörigen Ordner, obwohl sie eigenes CSS braucht — dann fehlt die gleiche Sortierung wie bei den anderen Bausteinen.
- Komponenten-CSS in generischen Sammeldateien verstreuen, obwohl es nur eine konkrete Komponente betrifft (erschwert Finden und Refactor).

- Große Komponenten, die mehrere unabhängige UI-Blöcke enthalten — lieber zerlegen und zusammensetzen.
- Harte Verdrahtung zu einem konkreten Parent (fest eingebaute Routen, feste Texte ohne i18n), wenn dieselbe UI woanders gebraucht werden könnte.

## Rust / Tauri

Analog: wiederkehrende invoke-Patterns, Fehlerbehandlung oder kleine Hilfsfunktionen in gemeinsame Module ziehen statt zu duplizieren — ohne dabei eine Monolith-Datei zu erzeugen (siehe `rule-no-monolith-structure.md`).
