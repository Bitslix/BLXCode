# Linux BrowserTabDock Boot-Crash Fix

## Summary

Behebt den Linux-only Boot-Crash: `BrowserTabDock` wurde beim Workbench-Start per CSS versteckt, mountete aber sofort `<iframe>`-Elemente mit der Standard-URL — WebKitGTK in der Tauri-Haupt-Webview stürzte ab. Fix: sticky Lazy Mount + Sichtbarkeits-Gating für iframe-Loads (analog zum `native_child`-Pfad auf Windows/macOS).

## Decisions

- Kein Linux-native Child-Webview in diesem Fix (separat: `linux-native-browser` in v2-roadmap).
- Sticky Lazy Mount: Dock wird beim ersten Browser-Tab-Besuch gemountet und bleibt danach im DOM (Tab-State ohne Reload).
- iframe-`src` und iframable-Effect nur bei sichtbarem Browser-Layer (`right_active_tab == Browser && !right_collapsed`).
- Rein frontend-seitig; kein Backend-Change.

## Implementation Notes

- `src/workbench/right_panel.rs`: `browser_dock_mounted` Signal + `<Show>`-Wrapper um `BrowserTabDock`.
- `src/workbench/browser_tab.rs`: `browser_layer_visible(wb)`; iframable-Effect und `prop:src` gated (`about:blank` wenn unsichtbar).

## Tests

- Linux: Boot ohne Browser-Tab; Browser-Tab öffnen, navigieren, Tab wechseln, Panel collapsen.
- Windows/macOS: Keine Regression bei native Child-Webview.

## Tasks

- [x] `sticky-lazy-mount` - Sticky `<Show>` + Mount-Signal in right_panel.rs
- [x] `visibility-helper` - browser_layer_visible(wb) Hilfsfunktion
- [x] `gate-iframable-effect` - iframable-Effect nur bei sichtbarem Layer
- [x] `gate-iframe-src` - iframe prop:src gated, sonst about:blank
- [x] `manual-linux-test` - cargo check WASM + Tauri grün; GUI-Verifikation durch Nutzer
- [x] `plan-doc` - Plan-Datei + PLANS.md Index
