export type Accent = "purple" | "cyan" | "pink" | "blue" | "success" | "warning" | "danger";

export type Feature = {
  id: string;
  kicker: string;
  title: string;
  body: string;
  accent: Accent;
};

export const features: Feature[] = [
  {
    id: "titlebar",
    kicker: "Titlebar",
    title: "Custom cross-platform app titlebar",
    body: "Brand cluster, native window controls, NAVIGATE quick menu, workspace breadcrumbs, and a live focused-terminal crumb.",
    accent: "purple",
  },
  {
    id: "theme",
    kicker: "Redesigned",
    title: "A new Tokyo Night × Dracula look",
    body: "Flagship dark/light themes reworked. The old default lives on as BLXCode Legacy.",
    accent: "purple",
  },
  {
    id: "orb",
    kicker: "Agent",
    title: "3D Drobo agent orb",
    body: "Interactive Three.js model loaded from Drobo.glb, recolors from theme tokens, follows the cursor, 2D fallback available.",
    accent: "cyan",
  },
  {
    id: "composer",
    kicker: "Composer",
    title: "Modern composer + Push-to-Talk",
    body: "Auto-growing textarea, footer model picker, plan/build modes, thinking-level, hold-to-speak transcription into any target.",
    accent: "pink",
  },
  {
    id: "cli-agents",
    kicker: "Terminals",
    title: "Per-terminal CLI-agent model + effort",
    body: "Fleet rows pick the model and reasoning effort for claude, codex, gemini, opencode, and cursor. Persists on workspaces and presets.",
    accent: "warning",
  },
  {
    id: "terminals",
    kicker: "Workspaces",
    title: "Named terminals",
    body: "Devon, Tom, Mia — friendly names with deterministic slot mapping. Double-click a terminal header to rename. The Agent knows the names too.",
    accent: "warning",
  },
  {
    id: "plans",
    kicker: "Plans",
    title: "AI Plan & AI Tasks",
    body: "Turn a short prompt into a full Markdown plan with an optional task list, right inside the Plans panel.",
    accent: "blue",
  },
  {
    id: "changed-files",
    kicker: "Timeline",
    title: "Changed-files summary card",
    body: "Model rounds that mutate files end with a totals card and a collapsible directory tree. Click any file to open the diff view.",
    accent: "success",
  },
  {
    id: "stats",
    kicker: "Agent",
    title: "Session stats + context-window meter",
    body: "Live provider/model chip, session start, context used/max with a thin meter that warns at 70% and danders at 85%, total turns and cost.",
    accent: "cyan",
  },
  {
    id: "tool-loop",
    kicker: "Settings",
    title: "Configurable tool-loop limit",
    body: "Per-turn ceiling on tool-call rounds is now a setting (1-500, default 36). No more hard-coded 36-round wall.",
    accent: "danger",
  },
  {
    id: "files",
    kicker: "Editor",
    title: "File preview = CodeMirror 6",
    body: "Preview and edit use the same engine now — same highlighting, gutter, folding. highlight.js (~127 KiB) is gone.",
    accent: "success",
  },
  {
    id: "memory",
    kicker: "Memory",
    title: "Centered memory tabs + workspace index",
    body: "Open notes in a centered tab, auto-load the workspace's README index, and only keep one category group open at a time.",
    accent: "blue",
  },
  {
    id: "filter",
    kicker: "Rules · Skills · Plans",
    title: "Unified category + live search",
    body: "Rules and skills share the same filter row, themed separator, and search. The Plans panel adds live search on top of its status tabs.",
    accent: "purple",
  },
  {
    id: "git",
    kicker: "Git",
    title: "VS Code-style commit graph",
    body: "Colored lanes, click-to-expand file lists, hover cards with author, date, refs, and Open on GitHub.",
    accent: "success",
  },
  {
    id: "remote",
    kicker: "Remote",
    title: "SSH workspaces, master/detail settings",
    body: "tmux-persistent or keepalive sessions, password / key / agent auth, OS-keychain secret storage, and a grid-of-cards settings view.",
    accent: "blue",
  },
  {
    id: "roundings",
    kicker: "Appearance",
    title: "Roundings & Font",
    body: "Sharp / Default / Rounded / Extra corner scale plus a monospace picker (JetBrains Mono, Cascadia Code, Fira Code, …) — re-rounds the whole workbench instantly.",
    accent: "purple",
  },
  {
    id: "themes",
    kicker: "32 themes",
    title: "Ten new lights + Claude Code",
    body: "Brand light counterparts, custom cool designs, and a warm-charcoal Claude Code look. Each ships a complete token set.",
    accent: "cyan",
  },
];

export const accentVar: Record<Accent, string> = {
  purple: "var(--accent-purple)",
  cyan: "var(--accent-cyan)",
  pink: "var(--accent-pink)",
  blue: "var(--accent-blue)",
  success: "var(--status-success)",
  warning: "var(--status-warning)",
  danger: "var(--status-danger)",
};
