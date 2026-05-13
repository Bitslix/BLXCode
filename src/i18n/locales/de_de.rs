use crate::i18n::I18nKey;

#[must_use]
pub fn msg(key: I18nKey) -> &'static str {
    match key {
        I18nKey::Decline => "Ablehnen",
        I18nKey::Accept => "Annehmen",
        I18nKey::BtnClose => "Schließen",
        I18nKey::BtnApply => "Übernehmen",
        I18nKey::BtnSave => "Speichern",

        I18nKey::WsAria => "Workspace",
        I18nKey::WsEmptyTitle => "Kein Workspace geöffnet",
        I18nKey::WsEmptyLead => "Wähle links einen Workspace oder starte später mit Dateien.",
        I18nKey::WsEmptyNote => {
            "Befehlspalette ist aktiv (Ctrl+Shift+P); weitere Anbindungen folgen."
        }
        I18nKey::WsKwCmdPalette => "Befehlspalette",
        I18nKey::WsKwQuickOpen => "Schnell öffnen",
        I18nKey::WsKwSidePanel => "Seitenpanel",
        I18nKey::WsKwAgent => "Agent",
        I18nKey::WsKwBrowser => "Browser",
        I18nKey::WsKwMemory => "Gedächtnis",
        I18nKey::WsKwTerminal => "Terminal",

        I18nKey::SbAria => "Workspaces",
        I18nKey::SbExpand => "Sidebar einblenden",
        I18nKey::SbCollapse => "Sidebar ausblenden",
        I18nKey::SbHeading => "Workspaces",

        I18nKey::RpRailAria => "Rechtes Panel",
        I18nKey::RpExpand => "Rechtes Panel einblenden",
        I18nKey::RpCollapse => "Rechtes Panel ausblenden",
        I18nKey::RpSplitterAria => "Breite rechtes Panel",
        I18nKey::RpTabsAria => "Rechter Bereich",
        I18nKey::TabAgent => "Agent",
        I18nKey::TabBrowser => "Browser",
        I18nKey::TabMemory => "Gedächtnis",

        I18nKey::AgAriaPane => "Agent Harness",
        I18nKey::AgSandbox => "Tool-Sandbox ",
        I18nKey::AgNoPath => "(kein Pfad)",
        I18nKey::AgScopedReadHint => {
            "Nutze „READ:relativer/pfad“ für scoped Lesezugriff."
        }
        I18nKey::AgPromptPh => "Ziel formulieren oder READ:README.md …",
        I18nKey::AgSend => "Senden",
        I18nKey::AgCancel => "Abbrechen",
        I18nKey::AgErrNeedPrompt => "Bitte Prompt eingeben.",
        I18nKey::AgYou => "Du",
        I18nKey::AgAssistant => "Agent",
        I18nKey::AgErrColon => "Fehler:",

        I18nKey::BrToolbarAria => "Eingebetteter Browser",
        I18nKey::BrBack => "Zurück",
        I18nKey::BrFwd => "Weiter",
        I18nKey::BrReload => "Neu laden",
        I18nKey::BrGo => "Los",
        I18nKey::BrTabsAria => "Browser-Tabs",
        I18nKey::BrNewTab => "Neuer Tab",
        I18nKey::BrNewTabBtnAria => "Neuer Tab",
        I18nKey::BrCloseTab => "Tab schließen",
        I18nKey::BrPreparing => "Browser wird vorbereitet…",
        I18nKey::BrNativeAria => "Nativer Browser",
        I18nKey::BrFrameTitle => "Eingebetteter Browser",
        I18nKey::BrNewHint => "Adresse oben eingeben, Enter oder „Los“.",
        I18nKey::BrShortcutsHeading => "Schnellwahl",

        I18nKey::PlFilterPh => "Kommando filtern …",
        I18nKey::PlHint => "Escape schließt • Pfeiltasten • Enter",
        I18nKey::PlNoHits => "Keine Treffer",

        I18nKey::CmdSetTitle => "Harness-Einstellungen",
        I18nKey::CmdSetSub => {
            "Kategorisierte UI (Allgemein, Layout, Sprache, Agent)"
        }
        I18nKey::CmdRtpTitle => "Rechtes Panel ein-/ausblenden",
        I18nKey::CmdRtpSub => "Inspector-Spalte umschalten",
        I18nKey::CmdAgentTitle => "Rechter Reiter: Agent",
        I18nKey::CmdAgentSub => "Chat / Composer Harness",
        I18nKey::CmdBrowseTitle => "Rechter Reiter: Browser",
        I18nKey::CmdBrowseSub => "Eingebettete Webview synchronisieren",
        I18nKey::CmdMemoryTitle => "Rechter Reiter: Gedächtnis",
        I18nKey::CmdMemorySub => "Reservierter Bereich",

        I18nKey::HsCloseSettingsAria => "Einstellungen schließen",
        I18nKey::HsTitle => "Harness-Einstellungen",
        I18nKey::HsAriaCats => "Kategorien",
        I18nKey::HsCatGeneral => "Allgemein",
        I18nKey::HsCatLayout => "Layout",
        I18nKey::HsCatLanguage => "Sprache",
        I18nKey::HsCatAgent => "Agent",
        I18nKey::GenHeading => "Allgemein",
        I18nKey::GenApiNote => {
            "API-Schlüssel liegen nur im Desktop-Host (Umgebungsvariable `BLX_ANTHROPIC_API_KEY`), nie im Browser-Storage."
        }
        I18nKey::GenEulaStatus => "EULA-Status",
        I18nKey::GenMoreSoon => "Weitere Allgemeine Regler folgen später.",
        I18nKey::LayHeading => "Layout",
        I18nKey::LayBrowserUrl => "Standard-URL des eingebetteten Browsers",
        I18nKey::LayDefaultIntro => "Voreinstellung:",
        I18nKey::LangHeading => "Sprache",
        I18nKey::LangUiLang => "UI-Sprache",
        I18nKey::AgHeading => "Agent",
        I18nKey::AgWsRootLabel => "Workspace-/Sandbox-Stamm",
        I18nKey::AgWsPlaceholder => "/abs/path/zum/repo",
        I18nKey::AgReadBuiltin => {
            "„READ:relative/pfad.txt“ löst eingebaute Lesehilfen aus."
        }
        I18nKey::AgHooksHeading => "Terminal-Hooks",
        I18nKey::AgHooksDesc => {
            "Installiert Titel- und Session-Capture-Hooks für Claude und Codex. Damit folgt der Tab-Titel dem aktuellen Prompt und Agent-Sessions lassen sich nach einem Neustart fortsetzen."
        }
        I18nKey::AgHooksInstall => "Hooks installieren",
        I18nKey::AgHooksUninstall => "Deinstallieren",
        I18nKey::AgHooksStatusInstalled => "installiert",
        I18nKey::AgHooksStatusMissing => "nicht installiert",
        I18nKey::AgHooksStatusUnknown => "unbekannt",
        I18nKey::AgHooksBusy => "Wird ausgeführt…",
        I18nKey::HarnessLoading => "Laden …",

        I18nKey::AuthGateChecking => "Sitzung wird geprüft…",
        I18nKey::AuthLoginHeading => "Anmelden",
        I18nKey::AuthTabEmail => "E-Mail",
        I18nKey::AuthTabDevice => "Anderes Gerät",
        I18nKey::AuthEmailLabel => "E-Mail",
        I18nKey::AuthPasswordLabel => "Passwort",
        I18nKey::AuthSubmit => "Anmelden",
        I18nKey::AuthDeviceIntro => {
            "Fordere einen Gerätecode an, öffne den Link im Browser und bestätige dort — die App wartet automatisch."
        },
        I18nKey::AuthDeviceStart => "Code anfordern",
        I18nKey::AuthDeviceCode => "Dein Code",
        I18nKey::AuthDeviceCopyAria => "Code in Zwischenablage kopieren",
        I18nKey::AuthDeviceCopied => "Kopiert",

        I18nKey::AuthOpenVerify => "Verifikation im Browser öffnen",
        I18nKey::AuthPolling => "Warte auf Bestätigung …",
        I18nKey::AuthFail => "Anmeldung fehlgeschlagen.",
        I18nKey::SbSignOut => "Abmelden",
        I18nKey::SbUserMenuAria => "Kontomenü öffnen",
        I18nKey::SbAccount => "Konto",

        I18nKey::SbAddWorkspaceAria => "Neuen Workspace anlegen",
        I18nKey::WzTitle => "Workspace anlegen",
        I18nKey::WzSubLayout => "Layout und Arbeitsverzeichnis wählen.",
        I18nKey::WzSubFleet => "Agenten für {n} Terminals zuweisen.",
        I18nKey::WzTemplatesHeading => "Layout-Vorlagen",
        I18nKey::WzNameLabel => "Name (optional)",
        I18nKey::WzNamePh => "z. B. Backend-Refactor",
        I18nKey::WzCwdLabel => "Arbeitsverzeichnis",
        I18nKey::WzNavPh => "cd …",
        I18nKey::WzGo => "Los",
        I18nKey::WzNavHint => "Nur `cd`, `cd ..`, `cd /abs` oder `cd rel` — im Browser ohne echtes Dateisystem nur Pfadeingabe.",
        I18nKey::WzCdErr => "Nur cd-Befehle werden unterstützt.",
        I18nKey::WzCwdEmpty => "Bitte ein Arbeitsverzeichnis setzen.",
        I18nKey::WzNext => "Weiter",
        I18nKey::WzBack => "Zurück",
        I18nKey::WzCancel => "Abbrechen",
        I18nKey::WzSkipAgents => "Agents überspringen",
        I18nKey::WzLaunch => "Workspace starten",
        I18nKey::WzPresetSingle => "1 Terminal",
        I18nKey::WzPreset2 => "2 Terminals",
        I18nKey::WzPreset4 => "4 Terminals",
        I18nKey::WzPreset6 => "6 Terminals",
        I18nKey::WzPreset8 => "8 Terminals",
        I18nKey::WzPreset10 => "10 Terminals",
        I18nKey::WzPreset12 => "12 Terminals",
        I18nKey::WzPreset14 => "14 Terminals",
        I18nKey::WzPreset16 => "16 Terminals",
        I18nKey::WzFleetTitle => "AI Agent Fleet",
        I18nKey::WzFleetUtil => "Auslastung",
        I18nKey::WzFleetNoAgents => "Keine Agenten gewählt",
        I18nKey::WzFleetOptimal => "Slots vollständig zugewiesen",
        I18nKey::WzFleetSumWrong => "Die Summe muss genau {n} ergeben.",
        I18nKey::WzFleetSelectAll => "Alle wählen",
        I18nKey::WzFleetOneEach => "1 je Agent",
        I18nKey::WzFillEvenly => "Gleichmäßig",
        I18nKey::WzFleetClear => "Leeren",
        I18nKey::WzFleetAll => "Alle {n}",
        I18nKey::WzAgentClaude => "Claude",
        I18nKey::WzAgentSubClaude => "anthropic",
        I18nKey::WzAgentCodex => "Codex",
        I18nKey::WzAgentSubCodex => "openai",
        I18nKey::WzAgentGemini => "Gemini",
        I18nKey::WzAgentSubGemini => "google",
        I18nKey::WzAgentOpencode => "OpenCode",
        I18nKey::WzAgentSubOpencode => "opencode",
        I18nKey::WzAgentCursor => "Cursor",
        I18nKey::WzAgentSubCursor => "cursor",
        I18nKey::WsPtyNoDesktop => "Interaktive Shell nur in der Desktop-App (Tauri). Hier ist kein natives PTY verfügbar.",
        I18nKey::WsPtySpawnFailed => "Terminal konnte nicht gestartet werden.",
        I18nKey::WsTermSlot => "Terminal",
        I18nKey::WsTermBootstrapFailed => {
            "Terminal-Oberfläche konnte nicht geladen werden. Siehe Konsole; Bootstrap-Skript oder xterm-CDN könnte blockiert sein."
        }

        I18nKey::EulaAccepted => "Akzeptiert",
        I18nKey::EulaUnknown => "Unbekannt",
    }
}
