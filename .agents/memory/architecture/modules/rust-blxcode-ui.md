---
title: blxcode-ui (rust)
enabled: true
tags: ["architecture"]
managed: static
kind: rust
stale: false
git_rev: 2bd555e33cd66810235467822b853e084e4b7a45
source_paths: ["src/agent_wire.rs", "src/app.rs", "src/boot_loading.rs", "src/config/app.config.rs", "src/config/mod.rs", "src/i18n/eula.rs", "src/i18n/keys.rs", "src/i18n/locale.rs", "src/i18n/locales/de_de.rs", "src/i18n/locales/en_us.rs", "src/i18n/locales/es_es.rs", "src/i18n/locales/fr_fr.rs", "src/i18n/locales/hu_hu.rs", "src/i18n/locales/it_it.rs", "src/i18n/locales/ja_jp.rs", "src/i18n/locales/ko_kr.rs", "src/i18n/locales/mod.rs", "src/i18n/locales/pl_pl.rs", "src/i18n/locales/pt_br.rs", "src/i18n/locales/ru_ru.rs", "src/i18n/locales/zh_cn.rs", "src/i18n/locales/zh_tw.rs", "src/i18n/mod.rs", "src/i18n/resolve.rs", "src/main.rs", "src/memory_paths.rs", "src/open_http.rs", "src/quit.rs", "src/service/mod.rs", "src/service/service.i18n.rs", "src/skills_rules_wire.rs", "src/tauri_bridge.rs", "src/theme/catalog.rs", "src/theme/i18n.rs", "src/theme/mod.rs", "src/workbench/agent_accent.rs", "src/workbench/agent_context_handoff.rs", "src/workbench/agent_model_picker/mod.rs", "src/workbench/agent_panel/ask_user_card/mod.rs", "src/workbench/agent_panel/client_tools.rs", "src/workbench/agent_panel/context_list.rs", "src/workbench/agent_panel/image_context.rs", "src/workbench/agent_panel/mod.rs", "src/workbench/agent_panel/reducer.rs", "src/workbench/agent_panel/task_list.rs", "src/workbench/agent_panel/timeline.rs", "src/workbench/agent_panel/turn_metrics_bar/mod.rs", "src/workbench/agent_panel/voice_orb/mod.rs", "src/workbench/agent_panel/voice_orb/state.rs", "src/workbench/agent_provider_pane/mod.rs", "src/workbench/agent_timeline.rs", "src/workbench/api_keys_pane/mod.rs", "src/workbench/app_prefs.rs", "src/workbench/appearance_settings_pane/mod.rs", "src/workbench/appearance_settings_pane/theme_preview_card.rs", "src/workbench/browser_tab.rs", "src/workbench/chat_markdown.rs", "src/workbench/close_terminals_tab_dialog/mod.rs", "src/workbench/create_workspace_wizard.rs", "src/workbench/file_diff/mod.rs", "src/workbench/file_diff_section/mod.rs", "src/workbench/file_preview/code_context_menu.rs", "src/workbench/file_preview/code_view.rs", "src/workbench/file_preview/header.rs", "src/workbench/file_preview/hljs_glue.rs", "src/workbench/file_preview/image_view.rs", "src/workbench/file_preview/markdown_view.rs", "src/workbench/file_preview/mermaid_glue.rs", "src/workbench/file_preview/mermaid_view.rs", "src/workbench/file_preview/mod.rs", "src/workbench/file_preview/util.rs", "src/workbench/file_preview/video_view.rs", "src/workbench/git_graph/mod.rs", "src/workbench/git_sync_controls.rs", "src/workbench/harness_chords.rs", "src/workbench/harness_image_pane/mod.rs", "src/workbench/harness_ui.rs", "src/workbench/harness_voice_pane/mod.rs", "src/workbench/memory_graph/graph_glue.rs", "src/workbench/memory_graph/mod.rs", "src/workbench/memory_panel.rs", "src/workbench/mod.rs", "src/workbench/notification_sound.rs", "src/workbench/path_nav.rs", "src/workbench/plans_panel/mod.rs", "src/workbench/pointer_agents.rs", "src/workbench/post_update_notes.rs", "src/workbench/project_explorer/mod.rs", "src/workbench/right_panel.rs", "src/workbench/sidebar.rs", "src/workbench/sidebar_resizer/mod.rs", "src/workbench/sidebar_view_section/mod.rs", "src/workbench/skills_rules_panel/install_dialog.rs", "src/workbench/skills_rules_panel/mod.rs", "src/workbench/skills_rules_panel/rule_card.rs", "src/workbench/skills_rules_panel/rules_pointers.rs", "src/workbench/skills_rules_panel/rules_tab.rs", "src/workbench/skills_rules_panel/skill_card.rs", "src/workbench/skills_rules_panel/skills_tab.rs", "src/workbench/skills_rules_panel/state.rs", "src/workbench/state.rs", "src/workbench/terminal_cell.rs", "src/workbench/terminal_context_menu.rs", "src/workbench/terminal_glue.rs", "src/workbench/terminal_slot_dnd.rs", "src/workbench/terminal_slot_drag_overlay.rs", "src/workbench/theme_service.rs", "src/workbench/toast.rs", "src/workbench/update_dialog.rs", "src/workbench/update_service.rs", "src/workbench/voice_app_controls/mod.rs", "src/workbench/workspace_panel.rs", "src/workbench/workspace_settings_pane/category_colors.rs", "src/workbench/workspace_settings_pane/mod.rs"]
---
# blxcode-ui (rust)

Manual notes about this unit can live above or below the generated block.

<!-- architecture:static:begin -->
## `blxcode-ui`

- Kind: `rust`
- Manifest: `Cargo.toml`
- Root: `.`
- Source root: `src`
- Source files: 116
- Root declarations: `agent_wire`, `app`, `boot_loading`, `config`, `i18n`, `memory_paths`, `open_http`, `quit`, `service`, `skills_rules_wire`, `tauri_bridge`, `theme`, `workbench`

### Top-Level Modules

- `agent_wire` (1 files)
- `app` (1 files)
- `boot_loading` (1 files)
- `config` (2 files) — submodules: `app.config`; declarations: `app_config`
- `i18n` (19 files) — submodules: `eula`, `keys`, `locale`, `locales`, `resolve`; declarations: `de_de`, `en_us`, `es_es`, `eula`, `fr_fr`, `hu_hu`, `it_it`, `ja_jp`, `keys`, `ko_kr`, `locale`, `locales`, `pl_pl`, `pt_br`, `resolve`, `ru_ru`, `tests`, `zh_cn`, `zh_tw`; 13 deeper source files aggregated here
- `memory_paths` (1 files)
- `open_http` (1 files)
- `quit` (1 files)
- `service` (2 files) — submodules: `service.i18n`; declarations: `i18n`
- `skills_rules_wire` (1 files)
- `tauri_bridge` (1 files)
- `theme` (3 files) — submodules: `catalog`, `i18n`; declarations: `catalog`, `i18n`
- `workbench` (81 files) — submodules: `agent_accent`, `agent_context_handoff`, `agent_model_picker`, `agent_panel`, `agent_provider_pane`, `agent_timeline`, `api_keys_pane`, `app_prefs`, `appearance_settings_pane`, `browser_tab`, `chat_markdown`, `close_terminals_tab_dialog`, `commit_dialog`, `confirm_dialog`, `create_workspace_wizard`, `file_diff`, `file_diff_section`, `file_preview`, `git_graph`, `git_sync_controls`, `harness_chords`, `harness_image_pane`, `harness_ui`, `harness_voice_pane`, `memory_graph`, `memory_panel`, `notification_sound`, `path_nav`, `plans_panel`, `pointer_agents`, `post_update_notes`, `project_explorer`, `right_panel`, `sidebar`, `sidebar_resizer`, `sidebar_view_section`, `skills_rules_panel`, `state`, `terminal_cell`, `terminal_context_menu`, `terminal_glue`, `terminal_slot_dnd`, `terminal_slot_drag_overlay`, `theme_service`, `toast`, `update_dialog`, `update_service`, `voice_app_controls`, `workspace_panel`, `workspace_settings_pane`; declarations: `agent_accent`, `agent_context_handoff`, `agent_model_picker`, `agent_panel`, `agent_provider_pane`, `agent_timeline`, `api_keys_pane`, `app_prefs`, `appearance_settings_pane`, `ask_user_card`, `browser_tab`, `category_colors`, `center_tab_tests`, `chat_markdown`, `client_tools`, `close_terminals_tab_dialog`, `code_context_menu`, `code_view`, `commit_dialog`, `confirm_dialog`, `context_list`, `create_workspace_wizard`, `file_diff`, `file_diff_section`, `file_preview`, `git_graph`, `git_sync_controls`, `graph_glue`, `harness_chords`, `harness_image_pane`, `harness_ui`, `harness_voice_pane`, `header`, `hljs_glue`, `image_context`, `image_view`, `install_dialog`, `markdown_view`, `memory_graph`, `memory_panel`, `mermaid_glue`, `mermaid_view`, `notification_sound`, `path_nav`, `plans_panel`, `post_update_notes`, `project_explorer`, `reducer`, `right_panel`, `rule_card`, `rules_pointers`, `rules_tab`, `sidebar`, `sidebar_resizer`, `sidebar_view_section`, `skill_card`, `skills_rules_panel`, `skills_tab`, `state`, `task_list`, `terminal_cell`, `terminal_context_menu`, `terminal_glue`, `terminal_slot_dnd`, `terminal_slot_drag_overlay`, `terminal_slot_tests`, `tests`, `theme_preview_card`, `theme_service`, `timeline`, `toast`, `update_dialog`, `update_service`, `util`, `video_view`, `voice_app_controls`, `voice_orb`, `workspace_panel`, `workspace_settings_pane`; 30 deeper source files aggregated here

### Source Paths

- `src/agent_wire.rs`
- `src/app.rs`
- `src/boot_loading.rs`
- `src/config/app.config.rs`
- `src/config/mod.rs`
- `src/i18n/eula.rs`
- `src/i18n/keys.rs`
- `src/i18n/locale.rs`
- `src/i18n/locales/de_de.rs`
- `src/i18n/locales/en_us.rs`
- `src/i18n/locales/es_es.rs`
- `src/i18n/locales/fr_fr.rs`
- `src/i18n/locales/hu_hu.rs`
- `src/i18n/locales/it_it.rs`
- `src/i18n/locales/ja_jp.rs`
- `src/i18n/locales/ko_kr.rs`
- `src/i18n/locales/mod.rs`
- `src/i18n/locales/pl_pl.rs`
- `src/i18n/locales/pt_br.rs`
- `src/i18n/locales/ru_ru.rs`
- `src/i18n/locales/zh_cn.rs`
- `src/i18n/locales/zh_tw.rs`
- `src/i18n/mod.rs`
- `src/i18n/resolve.rs`
- `src/main.rs`
- `src/memory_paths.rs`
- `src/open_http.rs`
- `src/quit.rs`
- `src/service/mod.rs`
- `src/service/service.i18n.rs`
- `src/skills_rules_wire.rs`
- `src/tauri_bridge.rs`
- `src/theme/catalog.rs`
- `src/theme/i18n.rs`
- `src/theme/mod.rs`
- `src/workbench/agent_accent.rs`
- `src/workbench/agent_context_handoff.rs`
- `src/workbench/agent_model_picker/mod.rs`
- `src/workbench/agent_panel/ask_user_card/mod.rs`
- `src/workbench/agent_panel/client_tools.rs`
- `src/workbench/agent_panel/context_list.rs`
- `src/workbench/agent_panel/image_context.rs`
- `src/workbench/agent_panel/mod.rs`
- `src/workbench/agent_panel/reducer.rs`
- `src/workbench/agent_panel/task_list.rs`
- `src/workbench/agent_panel/timeline.rs`
- `src/workbench/agent_panel/turn_metrics_bar/mod.rs`
- `src/workbench/agent_panel/voice_orb/mod.rs`
- `src/workbench/agent_panel/voice_orb/state.rs`
- `src/workbench/agent_provider_pane/mod.rs`
- `src/workbench/agent_timeline.rs`
- `src/workbench/api_keys_pane/mod.rs`
- `src/workbench/app_prefs.rs`
- `src/workbench/appearance_settings_pane/mod.rs`
- `src/workbench/appearance_settings_pane/theme_preview_card.rs`
- `src/workbench/browser_tab.rs`
- `src/workbench/chat_markdown.rs`
- `src/workbench/close_terminals_tab_dialog/mod.rs`
- `src/workbench/commit_dialog/mod.rs`
- `src/workbench/confirm_dialog/mod.rs`
- `src/workbench/create_workspace_wizard.rs`
- `src/workbench/file_diff/mod.rs`
- `src/workbench/file_diff_section/mod.rs`
- `src/workbench/file_preview/code_context_menu.rs`
- `src/workbench/file_preview/code_view.rs`
- `src/workbench/file_preview/header.rs`
- `src/workbench/file_preview/hljs_glue.rs`
- `src/workbench/file_preview/image_view.rs`
- `src/workbench/file_preview/markdown_view.rs`
- `src/workbench/file_preview/mermaid_glue.rs`
- `src/workbench/file_preview/mermaid_view.rs`
- `src/workbench/file_preview/mod.rs`
- `src/workbench/file_preview/util.rs`
- `src/workbench/file_preview/video_view.rs`
- `src/workbench/git_graph/mod.rs`
- `src/workbench/git_sync_controls.rs`
- `src/workbench/harness_chords.rs`
- `src/workbench/harness_image_pane/mod.rs`
- `src/workbench/harness_ui.rs`
- `src/workbench/harness_voice_pane/mod.rs`
- ... 36 more source paths omitted
<!-- architecture:static:end -->

