/**
 * blxcode-dark (default) color tokens — sourced from themes/tokens.css so the
 * trailer matches the actual app. Tokyo Night night-blue base + Dracula accent.
 */
export const tokens = {
  bg: {
    app: "#16161e",
    raised: "#1a1b26",
    panel: "#1f2030",
    panelHeader: "#24263a",
  },
  border: {
    subtle: "rgba(189, 200, 245, 0.09)",
    strong: "rgba(189, 200, 245, 0.16)",
    focus: "#bd93f9",
  },
  text: {
    default: "#c8d3f5",
    muted: "#a9b1d6",
    faint: "#565f89",
    bright: "#f8f8f2",
  },
  accent: {
    purple: "#bd93f9",
    purpleHover: "#cda9fb",
    purpleSoft: "rgba(189, 147, 249, 0.18)",
    cyan: "#7dcfff",
    cyanSoft: "rgba(125, 207, 255, 0.12)",
    pink: "#ff79c6",
    blue: "#7aa2f7",
  },
  status: {
    success: "#50fa7b",
    warning: "#ffb86c",
    danger: "#ff5555",
  },
  lanes: ["#bd93f9", "#7dcfff", "#50fa7b", "#ffb86c", "#ff5555", "#7aa2f7"],
  agents: {
    claude: "#ffb86c",
    codex: "#50fa7b",
    gemini: "#7aa2f7",
    copilot: "#bd93f9",
    cursor: "#ff79c6",
  },
  fonts: {
    sans: '"Inter", system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif',
    mono: '"JetBrains Mono", "Cascadia Mono", "Cascadia Code", Consolas, "SF Mono", Menlo, ui-monospace, monospace',
  },
} as const;
