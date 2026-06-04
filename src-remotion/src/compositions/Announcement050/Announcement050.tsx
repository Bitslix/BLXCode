import type { CSSProperties, ReactNode } from "react";
import {
  AbsoluteFill,
  Audio,
  Easing,
  Img,
  interpolate,
  Sequence,
  spring,
  staticFile,
  useCurrentFrame,
} from "remotion";
import { DroboOrb3D } from "../ReleaseTrailer/components";
import "./Announcement050.css";

const FPS = 30;
const INTRO_DURATION = 158;
const FEATURE_DURATION = 190;
const OUTRO_DURATION = 198;
const MUSIC_SRC = "assets/paoloargento-jazz-commercials-217891.mp3";
const BLX_LOGO_SRC = "blxcode.png";
const MUSIC_PLAYBACK_RATE = 0.932;

type Accent = "purple" | "cyan" | "pink" | "blue" | "green" | "amber" | "red";
type VisualKind =
  | "agent"
  | "mcp"
  | "kanban"
  | "stats"
  | "providers"
  | "voice"
  | "memory"
  | "themes"
  | "context"
  | "editor"
  | "remote"
  | "roles"
  | "updates"
  | "canvas"
  | "mermaid"
  | "heartbeat"
  | "settings"
  | "terminals"
  | "more";

type FeatureSceneData = {
  id: string;
  kicker: string;
  title: string;
  body: string;
  accent: Accent;
  tags: string[];
  visual: VisualKind;
};

const accentColor: Record<Accent, string> = {
  purple: "var(--a-purple)",
  cyan: "var(--a-cyan)",
  pink: "var(--a-pink)",
  blue: "var(--a-blue)",
  green: "var(--a-green)",
  amber: "var(--a-amber)",
  red: "var(--a-red)",
};

const modelAccentColor: Record<string, string> = {
  "var(--a-purple)": "#826da8",
  "var(--a-cyan)": "#6f9aad",
  "var(--a-pink)": "#a66f91",
  "var(--a-blue)": "#6d7fa9",
  "var(--a-green)": "#6f9f82",
  "var(--a-amber)": "#a9825f",
  "var(--a-red)": "#9c6568",
};

const modelSecondaryColor: Record<string, string> = {
  "var(--a-purple)": "#6e789d",
  "var(--a-cyan)": "#9b78a0",
  "var(--a-pink)": "#6f87a0",
  "var(--a-blue)": "#8f789f",
  "var(--a-green)": "#7b7897",
  "var(--a-amber)": "#71839c",
  "var(--a-red)": "#7f829c",
};

const toModelColor = (color: string) => modelAccentColor[color] ?? color;
const toModelSecondaryColor = (color: string) => modelSecondaryColor[color] ?? "#7b7897";

const features: FeatureSceneData[] = [
  {
    id: "agent",
    kicker: "Better Personal Agent",
    title: "Give your agent identity, role, and voice",
    body: "Name your BLXCode Agent, assign a role, choose gender and voice, then let that personality carry through the workspace.",
    accent: "cyan",
    tags: ["name", "role", "gender", "voice"],
    visual: "agent",
  },
  {
    id: "mcp",
    kicker: "[NEW] MCP",
    title: "Add MCP servers once. Use them app-wide.",
    body: "Register stdio or HTTP MCP servers in Settings and expose them to the BLXCode Agent and terminal CLI agents.",
    accent: "purple",
    tags: ["Settings -> MCP", "connection test", "agent tools", "CLI configs"],
    visual: "mcp",
  },
  {
    id: "kanban",
    kicker: "[NEW] Kanban",
    title: "Plans and tasks now have a real board",
    body: "Multi-Kanban per workspace, Markdown-backed plans, nested task lanes, drag and drop, search, and direct agent access.",
    accent: "amber",
    tags: ["multi-board", "drag and drop", ".agents/plans", "agent access"],
    visual: "kanban",
  },
  {
    id: "stats",
    kicker: "[UPDATE] Agent Header",
    title: "Live session stats at the top",
    body: "Provider, model, context window, turns, tool calls, subagents, and cost now live where you need them.",
    accent: "cyan",
    tags: ["context meter", "session cost", "tool calls", "subagents"],
    visual: "stats",
  },
  {
    id: "providers",
    kicker: "Providers",
    title: "Local models and cloud scale in one loop",
    body: "BLXCode now speaks OpenAI-compatible local and cloud providers across chat, utilities, plans, compaction, and subagents.",
    accent: "blue",
    tags: ["OpenAI", "Anthropic", "OpenRouter", "Hugging Face"],
    visual: "providers",
  },
  {
    id: "voice",
    kicker: "[NEW] BLXVoice PTT",
    title: "Speak and send text everywhere",
    body: "Hold a key, talk, and route the transcript to the agent composer, terminal, active input, or clipboard.",
    accent: "pink",
    tags: ["push-to-talk", "Whisper", "live partials", "clipboard"],
    visual: "voice",
  },
  {
    id: "memory",
    kicker: "[NEW] Memory View",
    title: "Memory moves into the center",
    body: "A centered tab, wider category handling, default workspace index, and split view for keeping terminals visible.",
    accent: "blue",
    tags: ["center tab", "split view", "workspace index", "categories"],
    visual: "memory",
  },
  {
    id: "themes",
    kicker: "[UPDATE] Themes",
    title: "A redesigned app theme plus new light looks",
    body: "The flagship BLXCode look gets a Tokyo Night x Dracula refresh, with a larger theme grid and new light themes.",
    accent: "purple",
    tags: ["new dark", "light themes", "roundings", "font"],
    visual: "themes",
  },
  {
    id: "context",
    kicker: "[NEW] Drag and Drop Context",
    title: "Files, diffs, and commits become agent context",
    body: "Drag from the file browser, file diff, or Git commit graph and drop directly into the BLXCode Agent.",
    accent: "green",
    tags: ["file context", "diff context", "commit context", "composer"],
    visual: "context",
  },
  {
    id: "editor",
    kicker: "[UPDATE] CodeMirror",
    title: "Native editing and preview share one engine",
    body: "CodeMirror now powers both the file editor and read-only preview with syntax highlighting, folding, selection, shortcuts, and Vim mode.",
    accent: "green",
    tags: ["CodeMirror 6", "Vim mode", "preview", "shortcuts"],
    visual: "editor",
  },
  {
    id: "remote",
    kicker: "[NEW] Remote SSH",
    title: "Remote workspaces with GitHub-aware workflows",
    body: "Redesigned SSH settings, saved connection cards, remote directories, secret handling, and GitHub-aware commit actions.",
    accent: "blue",
    tags: ["SSH", "presets", "GitHub", "remote dev"],
    visual: "remote",
  },
  {
    id: "roles",
    kicker: "[UPDATE] ADE Awareness",
    title: "Agent roles for real coordination",
    body: "Architect, Branch Steward, Coordinator, and more roles can now control deeper parts of the ADE, including settings and terminal agents.",
    accent: "purple",
    tags: ["Architect", "Branch Steward", "Coordinator", "ADE control"],
    visual: "roles",
  },
  {
    id: "updates",
    kicker: "[NEW] Beta Channel",
    title: "Pre-releases from inside BLXCode",
    body: "Switch between Stable and Beta in App Updates, then let BLXCode check prereleases and notify you when a build is ready.",
    accent: "amber",
    tags: ["Stable", "Beta", "auto-update", "notifications"],
    visual: "updates",
  },
  {
    id: "canvas",
    kicker: "[NEW] Canvas and Swarm Views",
    title: "See the work. Coordinate the agents.",
    body: "Canvas and Swarm views turn plans, files, memory, and agents into a visual control surface for complex work.",
    accent: "cyan",
    tags: ["canvas", "swarm", "agents", "workspace graph"],
    visual: "canvas",
  },
  {
    id: "mermaid-diagram",
    kicker: "[NEW] Mermaid Diagram View",
    title: "Mermaid diagrams in a centered tab",
    body: "Open Mermaid source, rendered preview, and split view in a centered tab, then add the diagram to agent context.",
    accent: "cyan",
    tags: ["centered tab", "source + preview", "split view", "agent context"],
    visual: "mermaid",
  },
  {
    id: "heartbeat",
    kicker: "[NEW] HeartBeat",
    title: "Background services for smarter workspaces",
    body: "HeartBeat registers services and runs them on schedule. The first service is the Memory Indexer.",
    accent: "pink",
    tags: ["services", "Memory Indexer", "Run now", "status"],
    visual: "heartbeat",
  },
  {
    id: "settings",
    kicker: "[UPDATE] Settings",
    title: "Cleaner screens and better dialogs",
    body: "App settings, providers, MCP, Memory, Voice, Code Editor, HeartBeat, Remote SSH, and dialogs were rebuilt for faster control.",
    accent: "blue",
    tags: ["refactor", "dialogs", "master detail", "tokens"],
    visual: "settings",
  },
  {
    id: "terminals",
    kicker: "[UPDATE] Terminal Agent Control",
    title: "The agent can coordinate CLI agents end to end",
    body: "Named terminals, context handoff, raw keys, output reads, waits, settled-output checks, and interruption control.",
    accent: "amber",
    tags: ["Claude Code", "Codex", "Gemini", "OpenCode"],
    visual: "terminals",
  },
  {
    id: "more",
    kicker: "And a lot more",
    title: "Fixes, polish, diagrams, logs, and performance",
    body: "v0.5.0 also brings Mermaid diagrams, notifications, statusline upgrades, AI plans, app logs, Git graph polish, and many fixes.",
    accent: "purple",
    tags: ["Mermaid", "notifications", "AI plans", "app logs"],
    visual: "more",
  },
];

export const BLXCODE_050_ANNOUNCEMENT_DURATION =
  INTRO_DURATION + features.length * FEATURE_DURATION + OUTRO_DURATION;

const clamp = {
  extrapolateLeft: "clamp" as const,
  extrapolateRight: "clamp" as const,
};

const ease = Easing.bezier(0.16, 1, 0.3, 1);

const enterValue = (frame: number, start = 0, duration = 28) =>
  interpolate(frame, [start, start + duration], [0, 1], {
    ...clamp,
    easing: ease,
  });

const exitValue = (frame: number, durationInFrames: number, frames = 24) =>
  interpolate(frame, [durationInFrames - frames, durationInFrames], [1, 0], {
    ...clamp,
    easing: Easing.in(Easing.cubic),
  });

const numberText = (value: number, digits = 0) => value.toFixed(digits);

const Background: React.FC<{ accent: string; intensity?: number }> = ({ accent, intensity = 1 }) => {
  const frame = useCurrentFrame();
  const drift = Math.sin(frame / 52) * 18;
  const drift2 = Math.cos(frame / 67) * 14;
  return (
    <AbsoluteFill>
      <div className="a050-bg" />
      <div
        style={{
          position: "absolute",
          width: 560,
          height: 560,
          borderRadius: 999,
          left: 140 + drift,
          top: 120 + drift2,
          background: `radial-gradient(circle, ${accent}44 0%, ${accent}00 66%)`,
          filter: "blur(36px)",
          opacity: 0.8 * intensity,
        }}
      />
      <div
        style={{
          position: "absolute",
          width: 620,
          height: 620,
          borderRadius: 999,
          right: 60 - drift2,
          bottom: 40 + drift,
          background: "radial-gradient(circle, rgba(125, 207, 255, 0.20) 0%, transparent 68%)",
          filter: "blur(40px)",
          opacity: 0.85 * intensity,
        }}
      />
      <div className="a050-grid" />
      <div className="a050-vignette" />
      <div className="a050-noise" />
    </AbsoluteFill>
  );
};

const Shell: React.FC<{ children: ReactNode; accent: string; label?: string }> = ({
  children,
  accent,
  label = "workspace / blxcode-v050",
}) => (
  <div className="a050-shell">
    <div className="a050-topbar">
      <div className="a050-brand">
        <Img src={staticFile(BLX_LOGO_SRC)} className="a050-logo a050-logo-img" />
        BLXCode
      </div>
      <div className="a050-crumb">{label}</div>
      <div style={{ flex: 1 }} />
      <div className="a050-crumb">NAVIGATE</div>
      <div className="a050-dot" style={{ color: accent, background: accent }} />
    </div>
    {children}
  </div>
);

const Window: React.FC<{ title: string; children: ReactNode; style?: CSSProperties }> = ({
  title,
  children,
  style,
}) => (
  <div className="a050-window" style={style}>
    <div className="a050-window-head">
      <div className="a050-win-dot" style={{ background: "#ff5555" }} />
      <div className="a050-win-dot" style={{ background: "#ffb86c" }} />
      <div className="a050-win-dot" style={{ background: "#50fa7b" }} />
      <div className="a050-window-title">{title}</div>
    </div>
    {children}
  </div>
);

const Chip: React.FC<{ children: ReactNode; accent?: string; style?: CSSProperties }> = ({
  children,
  accent,
  style,
}) => (
  <div
    className="a050-chip"
    style={{
      borderColor: accent ? `${accent}66` : undefined,
      color: accent ?? undefined,
      background: accent ? `${accent}18` : undefined,
      ...style,
    }}
  >
    {children}
  </div>
);

const Progress: React.FC<{ progress: number; accent: string }> = ({ progress, accent }) => (
  <div className="a050-progress">
    <div className="a050-progress-fill" style={{ width: `${progress}%`, background: accent }} />
  </div>
);

const Lines: React.FC<{ rows?: number; accent?: string; delay?: number }> = ({
  rows = 4,
  accent = "rgba(200, 211, 245, 0.16)",
  delay = 0,
}) => {
  const frame = useCurrentFrame();
  return (
    <>
      {Array.from({ length: rows }).map((_, i) => {
        const p = enterValue(frame, delay + i * 4, 18);
        return (
          <div
            key={i}
            className="a050-line"
            style={{
              width: `${interpolate(p, [0, 1], [10, 88 - i * 7])}%`,
              opacity: p,
              background: i === 0 ? accent : undefined,
              marginBottom: 14,
            }}
          />
        );
      })}
    </>
  );
};

const AgentModel: React.FC<{
  accent: string;
  size?: number;
  recording?: boolean;
  label?: string;
  compact?: boolean;
}> = ({ accent, size = 220, recording = false, label = "BLXCode Agent", compact = false }) => {
  const frame = useCurrentFrame();
  const breathe = 0.5 + Math.sin(frame / 14) * 0.5;
  const modelAccent = toModelColor(accent);
  const modelAccent2 = toModelSecondaryColor(accent);
  return (
    <div
      className="a050-agent-model"
      style={{
        width: size,
        height: size,
        borderRadius: compact ? 18 : 32,
        borderColor: `${modelAccent}${recording ? "99" : "66"}`,
        boxShadow: `0 22px 56px ${modelAccent}${recording ? "30" : "1f"}`,
      }}
    >
      <div
        className="a050-agent-model__halo"
        style={{
          background: `radial-gradient(circle, ${modelAccent}${recording ? "33" : "22"} 0%, ${modelAccent2}${Math.round(22 + breathe * 18)
            .toString(16)
            .padStart(2, "0")} 46%, transparent 74%)`,
        }}
      />
      <div
        style={{
          position: "relative",
          zIndex: 1,
          transform: `scale(${compact ? 1.12 : 1.02}) translateY(${compact ? -2 : -6}px)`,
        }}
      >
        <DroboOrb3D accent={modelAccent} accent2={modelAccent2} recording={recording} size={size} />
      </div>
      {!compact && (
        <div className="a050-agent-model__label">
          <span>{label}</span>
          <b style={{ background: accent }} />
        </div>
      )}
    </div>
  );
};

const FeatureCopy: React.FC<{ feature: FeatureSceneData; index: number; accent: string }> = ({
  feature,
  index,
  accent,
}) => {
  const frame = useCurrentFrame();
  const titleIn = enterValue(frame, 12, 28);
  const bodyIn = enterValue(frame, 24, 28);
  const tagIn = enterValue(frame, 42, 24);
  return (
    <div
      className="a050-copy"
      style={{
        opacity: titleIn,
        transform: `translateX(${interpolate(titleIn, [0, 1], [48, 0])}px)`,
      }}
    >
      <div className="a050-kicker" style={{ color: accent }}>
        {String(index + 1).padStart(2, "0")} / {feature.kicker}
      </div>
      <h2 className="a050-title">{feature.title}</h2>
      <p
        className="a050-body"
        style={{
          opacity: bodyIn,
          transform: `translateY(${interpolate(bodyIn, [0, 1], [18, 0])}px)`,
        }}
      >
        {feature.body}
      </p>
      <div
        className="a050-chip-row"
        style={{
          opacity: tagIn,
          transform: `translateY(${interpolate(tagIn, [0, 1], [16, 0])}px)`,
        }}
      >
        {feature.tags.map((tag, i) => (
          <Chip key={tag} accent={i === 0 ? accent : undefined}>
            {tag}
          </Chip>
        ))}
      </div>
    </div>
  );
};

const VisualWrapper: React.FC<{ children: ReactNode; title: string; accent: string }> = ({
  children,
  title,
  accent,
}) => {
  const frame = useCurrentFrame();
  const inValue = spring({
    frame: frame - 4,
    fps: FPS,
    config: { damping: 19, stiffness: 92, mass: 0.75 },
  });
  return (
    <div
      className="a050-visual"
      style={{
        opacity: inValue,
        transform: `perspective(1200px) rotateY(${interpolate(inValue, [0, 1], [-8, 0])}deg) translateY(${interpolate(inValue, [0, 1], [32, 0])}px)`,
      }}
    >
      <Window title={title}>
        <div style={{ position: "absolute", inset: "48px 0 0", padding: 24 }}>{children}</div>
        <div
          style={{
            position: "absolute",
            inset: 0,
            pointerEvents: "none",
            borderRadius: 28,
            boxShadow: `inset 0 0 0 1px ${accent}22`,
          }}
        />
      </Window>
    </div>
  );
};

const AgentVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const save = enterValue(frame, 108, 30);
  const identity = [
    { label: "Name", value: "Nova", hint: "How the agent appears in chat" },
    { label: "Role", value: "Architect", hint: "Default behavior and decisions" },
    { label: "Gender", value: "Female", hint: "Voice profile and persona" },
    { label: "Voice", value: "BLXVoice / calm", hint: "Spoken responses and PTT" },
  ];
  const roles = ["Architect", "Coordinator", "Branch Steward"];
  const voices = ["calm", "bright", "direct"];
  return (
    <VisualWrapper title="BLXCode Agent" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "320px 1fr", gap: 20, height: "100%" }}>
        <div
          className="a050-card"
          style={{
            padding: 24,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            gap: 18,
            background: `linear-gradient(180deg, ${accent}12, rgba(18, 20, 32, 0.78))`,
          }}
        >
          <AgentModel accent={accent} size={238} recording label="Nova" />
          <div style={{ width: "100%", textAlign: "center" }}>
            <div className="a050-mini-title" style={{ fontSize: 24 }}>Nova</div>
            <div className="a050-code" style={{ marginTop: 8, color: accent }}>Architect · BLXVoice calm</div>
          </div>
          <div style={{ width: "100%", display: "flex", justifyContent: "center", gap: 8 }}>
            {voices.map((voice, i) => (
              <Chip key={voice} accent={i === 0 ? accent : undefined}>
                {voice}
              </Chip>
            ))}
          </div>
          <div className="a050-code" style={{ color: "var(--a-muted)", textAlign: "center" }}>
            identity saved to workspace<br />
            spoken by BLXVoice
          </div>
        </div>

        <div style={{ display: "grid", gridTemplateRows: "1fr 178px", gap: 18 }}>
          <div className="a050-card" style={{ padding: 22 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 16 }}>
              <div>
                <div className="a050-label">agent identity</div>
                <div className="a050-mini-title" style={{ marginTop: 6 }}>Personal profile builder</div>
              </div>
              <div style={{ flex: 1 }} />
              <Chip accent={accent}>workspace-aware</Chip>
            </div>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
              {identity.map((item, i) => {
                const p = enterValue(frame, 20 + i * 10, 24);
                return (
                  <div
                    key={item.label}
                    className="a050-card"
                    style={{
                      padding: 16,
                      minHeight: 116,
                      opacity: p,
                      transform: `translateY(${interpolate(p, [0, 1], [24, 0])}px)`,
                      borderColor: i === 0 || i === 3 ? `${accent}66` : undefined,
                      background: i === 0 || i === 3 ? `${accent}10` : undefined,
                    }}
                  >
                    <div className="a050-label">{item.label}</div>
                    <div className="a050-mini-title" style={{ marginTop: 10 }}>{item.value}</div>
                    <div className="a050-code" style={{ marginTop: 12, color: "var(--a-muted)" }}>{item.hint}</div>
                  </div>
                );
              })}
            </div>
          </div>

          <div className="a050-card" style={{ padding: 18, display: "grid", gridTemplateColumns: "1.1fr 1fr", gap: 18 }}>
            <div>
              <div className="a050-label">role preset</div>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 8, marginTop: 12 }}>
                {roles.map((role, i) => {
                  const p = enterValue(frame, 64 + i * 8, 20);
                  return (
                    <Chip key={role} accent={i === 0 ? accent : undefined} style={{ opacity: p }}>
                      {role}
                    </Chip>
                  );
                })}
              </div>
              <div
                className="a050-code"
                style={{
                  marginTop: 18,
                  color: "var(--a-muted)",
                  opacity: save,
                  transform: `translateY(${interpolate(save, [0, 1], [12, 0])}px)`,
                }}
              >
                profile.apply("Nova")<br />
                voice.route("BLXVoice")
              </div>
            </div>
            <div>
              <div className="a050-label">voice preview</div>
              <div style={{ display: "flex", alignItems: "end", gap: 6, height: 72, marginTop: 16 }}>
                {Array.from({ length: 16 }).map((_, i) => {
                  const wave = 0.45 + Math.sin(frame / 7 + i * 0.8) * 0.35;
                  const p = enterValue(frame, 78 + i, 18);
                  return (
                    <div
                      key={i}
                      style={{
                        width: 7,
                        height: 14 + wave * 44 * p,
                        borderRadius: 999,
                        background: i % 3 === 0 ? accent : "rgba(125, 207, 255, 0.58)",
                        opacity: 0.35 + p * 0.65,
                      }}
                    />
                  );
                })}
              </div>
              <Chip accent={accent} style={{ marginTop: 12, opacity: save }}>
                ready to speak
              </Chip>
            </div>
          </div>
        </div>
      </div>
    </VisualWrapper>
  );
};

const MpcVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const connect = enterValue(frame, 48, 34);
  const servers = ["docs-search", "browser-tools", "linear", "db-inspect"];
  return (
    <VisualWrapper title="Settings / MCP" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "360px 1fr", gap: 24, height: "100%" }}>
        <div className="a050-card" style={{ padding: 22 }}>
          <div className="a050-label">enabled servers</div>
          {servers.map((server, i) => {
            const p = enterValue(frame, 16 + i * 8, 18);
            return (
              <div
                key={server}
                className="a050-card"
                style={{
                  padding: 14,
                  marginTop: 14,
                  opacity: p,
                  transform: `translateY(${interpolate(p, [0, 1], [18, 0])}px)`,
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <div className="a050-dot" style={{ background: i < 3 ? "var(--a-green)" : "var(--a-faint)" }} />
                  <div className="a050-mini-title">{server}</div>
                  <div style={{ flex: 1 }} />
                  <span className="a050-code">{i < 3 ? "on" : "off"}</span>
                </div>
              </div>
            );
          })}
          <div style={{ marginTop: 22 }}>
            <Chip accent={accent}>connection test passed</Chip>
          </div>
        </div>
        <div className="a050-card" style={{ position: "relative", padding: 26, overflow: "hidden" }}>
          <div className="a050-label">app-wide tool injection</div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 18, marginTop: 24 }}>
            {["BLXCode Agent", "Claude CLI", "Codex CLI", "OpenCode CLI"].map((target, i) => (
              <div key={target} className="a050-card" style={{ padding: 18, minHeight: 120 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                  {i === 0 && <AgentModel accent={accent} size={52} recording compact />}
                  <div className="a050-mini-title">{target}</div>
                </div>
                <div className="a050-code" style={{ marginTop: 12, color: i === 0 ? accent : "var(--a-muted)" }}>
                  mcp.docs-search.query<br />
                  mcp.browser-tools.open
                </div>
              </div>
            ))}
          </div>
          <div
            style={{
              position: "absolute",
              left: 30,
              right: 30,
              top: 108,
              height: 2,
              background: `linear-gradient(90deg, ${accent}, transparent)`,
              transform: `scaleX(${connect})`,
              transformOrigin: "left center",
              opacity: connect,
            }}
          />
        </div>
      </div>
    </VisualWrapper>
  );
};

const KanbanVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const drag = enterValue(frame, 58, 42);
  const columns = [
    { name: "Backlog", cards: ["Agent roles", "Mermaid view"] },
    { name: "In progress", cards: ["MCP server UI", "PTT routing"] },
    { name: "Review", cards: ["Memory split view"] },
    { name: "Done", cards: ["Theme grid", "CodeMirror preview"] },
  ];
  return (
    <VisualWrapper title="Workspace Kanban" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 16, height: "100%" }}>
        {columns.map((column, i) => (
          <div key={column.name} className="a050-card" style={{ padding: 15, background: i === 1 ? `${accent}10` : undefined }}>
            <div className="a050-label" style={{ color: i === 1 ? accent : undefined }}>{column.name}</div>
            {column.cards.map((card, j) => {
              const p = enterValue(frame, 16 + i * 5 + j * 5, 20);
              const isDragged = i === 1 && j === 0;
              return (
                <div
                  key={card}
                  className="a050-card"
                  style={{
                    padding: 14,
                    marginTop: 15,
                    minHeight: 98,
                    opacity: p,
                    transform: isDragged
                      ? `translate(${drag * 210}px, ${drag * 116}px) rotate(${drag * 3}deg)`
                      : `translateY(${interpolate(p, [0, 1], [20, 0])}px)`,
                    borderColor: isDragged ? accent : undefined,
                    boxShadow: isDragged ? `0 22px 46px ${accent}22` : undefined,
                    zIndex: isDragged ? 3 : 1,
                  }}
                >
                  <div className="a050-mini-title">{card}</div>
                  <div className="a050-code" style={{ marginTop: 12 }}>tasks: {2 + i + j}</div>
                </div>
              );
            })}
            {i === 2 && (
              <div
                style={{
                  marginTop: 15,
                  border: `2px dashed ${accent}`,
                  borderRadius: 14,
                  height: 92,
                  opacity: drag,
                  background: `${accent}10`,
                }}
              />
            )}
          </div>
        ))}
      </div>
    </VisualWrapper>
  );
};

const StatsVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const context = interpolate(frame, [24, 120], [22, 71], clamp);
  const rows = [
    ["provider", "Anthropic"],
    ["model", "Claude Opus"],
    ["turns", numberText(3 + enterValue(frame, 26, 90) * 18)],
    ["tool calls", numberText(8 + enterValue(frame, 34, 98) * 76)],
    ["subagents", "Architect, Tom"],
    ["cost", `$${(0.04 + enterValue(frame, 40, 100) * 0.38).toFixed(3)}`],
  ];
  return (
    <VisualWrapper title="Agent Header Stats" accent={accent}>
      <div style={{ display: "grid", gridTemplateRows: "142px 1fr", gap: 18, height: "100%" }}>
        <div className="a050-card" style={{ padding: 22 }}>
          <div style={{ display: "flex", gap: 12, alignItems: "center" }}>
            <AgentModel accent={accent} size={78} recording compact />
            <div style={{ flex: 1 }}>
              <div className="a050-mini-title">BLXCode Agent</div>
              <div className="a050-code">context {numberText(context)}% / 200k</div>
              <div style={{ marginTop: 12 }}>
                <Progress progress={context} accent={context > 70 ? "var(--a-amber)" : accent} />
              </div>
            </div>
            <Chip accent={accent}>thinking</Chip>
          </div>
        </div>
        <div className="a050-card" style={{ padding: 24 }}>
          {rows.map(([label, value], i) => {
            const p = enterValue(frame, 18 + i * 7, 22);
            return (
              <div
                key={label}
                style={{
                  display: "grid",
                  gridTemplateColumns: "180px 1fr",
                  padding: "14px 0",
                  borderBottom: i < rows.length - 1 ? "1px solid rgba(200,211,245,0.08)" : undefined,
                  opacity: p,
                  transform: `translateX(${interpolate(p, [0, 1], [22, 0])}px)`,
                }}
              >
                <div className="a050-label">{label}</div>
                <div className="a050-mini-title" style={{ color: i === 3 ? accent : undefined }}>{value}</div>
              </div>
            );
          })}
        </div>
      </div>
    </VisualWrapper>
  );
};

const ProvidersVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const local = ["Ollama", "LM Studio"];
  const cloud = ["OpenAI", "Anthropic", "OpenRouter", "Hugging Face", "Cloudflare", "Together", "Portkey"];
  const list = [...local, ...cloud];
  return (
    <VisualWrapper title="Agent Providers" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 16 }}>
        {list.map((name, i) => {
          const p = enterValue(frame, 12 + i * 5, 18);
          const isLocal = local.includes(name);
          return (
            <div
              key={name}
              className="a050-card"
              style={{
                padding: 18,
                minHeight: 150,
                opacity: p,
                transform: `translateY(${interpolate(p, [0, 1], [28, 0])}px)`,
                borderColor: isLocal ? `${accent}66` : undefined,
                background: isLocal ? `${accent}12` : undefined,
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                <div className="a050-logo" style={{ width: 34, height: 34, background: isLocal ? accent : "var(--a-panel-2)" }}>
                  {name.slice(0, 1)}
                </div>
                <div>
                  <div className="a050-mini-title">{name}</div>
                  <div className="a050-code">{isLocal ? "local / server url" : "cloud / api key"}</div>
                </div>
              </div>
              <div style={{ marginTop: 16 }}>
                <div className="a050-label">{isLocal ? "Server URL" : "Model"}</div>
                <div className="a050-card" style={{ padding: 12, marginTop: 10 }}>
                  <span className="a050-code">
                    {isLocal ? "http://localhost:11434/v1" : "context + pricing metadata"}
                  </span>
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </VisualWrapper>
  );
};

const VoiceVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const waveBars = Array.from({ length: 42 });
  const route = enterValue(frame, 86, 44);
  return (
    <VisualWrapper title="BLXVoice Push-to-Talk" accent={accent}>
      <div style={{ display: "grid", gridTemplateRows: "250px 1fr", gap: 22, height: "100%" }}>
        <div className="a050-card" style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: 8, padding: 26 }}>
          {waveBars.map((_, i) => {
            const height = 22 + Math.abs(Math.sin((frame + i * 4) / 7)) * 110;
            return (
              <div
                key={i}
                style={{
                  width: 9,
                  height,
                  borderRadius: 999,
                  background: i % 5 === 0 ? accent : "rgba(200,211,245,0.22)",
                  opacity: 0.7 + Math.sin((frame + i) / 11) * 0.18,
                }}
              />
            );
          })}
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 18 }}>
          {["Agent composer", "Active terminal", "Text input", "Clipboard"].map((target, i) => {
            const p = enterValue(frame, 34 + i * 9, 22);
            return (
              <div
                key={target}
                className="a050-card"
                style={{
                  padding: 20,
                  opacity: p,
                  borderColor: i === Math.floor(route * 3) ? accent : undefined,
                  background: i === Math.floor(route * 3) ? `${accent}14` : undefined,
                }}
              >
                <div className="a050-label">route</div>
                <div className="a050-mini-title">{target}</div>
                <div className="a050-code" style={{ marginTop: 14 }}>
                  "create a plan for MCP..."
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </VisualWrapper>
  );
};

const MemoryVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const split = enterValue(frame, 82, 34);
  return (
    <VisualWrapper title="Memory Center Tab" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: `${300 - split * 40}px 1fr ${split * 220}px`, gap: 16, height: "100%" }}>
        <div className="a050-card" style={{ padding: 18 }}>
          <div className="a050-label">.agents/memory</div>
          {["README.md", "rules", "skills", "plans", "architecture"].map((item, i) => (
            <div key={item} className="a050-card" style={{ padding: 12, marginTop: 12, borderColor: i === 0 ? accent : undefined }}>
              <div className="a050-code" style={{ color: i === 0 ? accent : undefined }}>{item}</div>
            </div>
          ))}
        </div>
        <div className="a050-card" style={{ padding: 26 }}>
          <div className="a050-mini-title"># Workspace Memory Index</div>
          <div className="a050-code" style={{ marginTop: 20 }}>
            <Lines rows={7} accent={accent} delay={18} />
          </div>
          <div style={{ marginTop: 16 }}>
            <Chip accent={accent}>split view enabled</Chip>
          </div>
        </div>
        <div className="a050-term" style={{ padding: 16, opacity: split, overflow: "hidden" }}>
          <div className="a050-label">terminal grid</div>
          <div className="a050-code" style={{ marginTop: 16 }}>
            $ cargo check<br />
            memory indexer: idle<br />
            agent: ready
          </div>
        </div>
      </div>
    </VisualWrapper>
  );
};

const ThemesVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const themeColors = [
    "#bd93f9", "#7dcfff", "#ff79c6", "#7aa2f7", "#50fa7b", "#ffb86c",
    "#f8f8f2", "#d7e3fc", "#c6d0f5", "#f2d5cf", "#b4befe", "#a6e3a1",
    "#89b4fa", "#f5c2e7", "#fab387", "#94e2d5", "#e0def4", "#c4a7e7",
    "#9ccfd8", "#ebbcba", "#31748f", "#f6c177", "#ea9a97", "#56949f",
    "#286983", "#907aa9", "#d7827e", "#575279", "#797593", "#9893a5",
    "#b4637a", "#56949f",
  ];
  return (
    <VisualWrapper title="Appearance / Themes" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(8, 1fr)", gap: 12 }}>
        {themeColors.map((color, i) => {
          const p = enterValue(frame, 10 + i * 2, 16);
          const isLight = i > 15;
          return (
            <div
              key={`${color}-${i}`}
              className="a050-card"
              style={{
                height: 112,
                padding: 10,
                opacity: p,
                transform: `scale(${interpolate(p, [0, 1], [0.84, 1])})`,
                background: isLight ? "#f7f7fb" : "#202234",
                borderColor: i === 0 ? accent : undefined,
              }}
            >
              <div style={{ height: 42, borderRadius: 10, background: color }} />
              <div className="a050-line" style={{ marginTop: 12, width: "70%", background: isLight ? "#c9d0e8" : "rgba(200,211,245,0.18)" }} />
              <div className="a050-line" style={{ marginTop: 8, width: "46%", background: isLight ? "#dfe4f4" : "rgba(200,211,245,0.1)" }} />
            </div>
          );
        })}
      </div>
    </VisualWrapper>
  );
};

const ContextVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const drag1 = enterValue(frame, 40, 38);
  const drag2 = enterValue(frame, 70, 38);
  const drag3 = enterValue(frame, 100, 38);
  const item = (label: string, y: number, p: number, color = accent) => (
    <div
      className="a050-card"
      style={{
        position: "absolute",
        left: 38 + p * 510,
        top: y + p * 130,
        width: 230,
        padding: 16,
        borderColor: color,
        background: `${color}14`,
        boxShadow: `0 18px 40px ${color}24`,
      }}
    >
      <div className="a050-label">context</div>
      <div className="a050-mini-title">{label}</div>
    </div>
  );
  return (
    <VisualWrapper title="Drag and Drop Context" accent={accent}>
      <div style={{ position: "relative", height: "100%" }}>
        <div className="a050-card" style={{ position: "absolute", left: 22, top: 20, width: 300, height: 600, padding: 18 }}>
          <div className="a050-label">sources</div>
          <Lines rows={9} delay={12} />
        </div>
        <div className="a050-card" style={{ position: "absolute", right: 22, bottom: 22, width: 360, height: 260, padding: 22, borderColor: accent }}>
          <div className="a050-mini-title">Agent context tray</div>
          <div style={{ marginTop: 18, display: "flex", flexWrap: "wrap", gap: 10 }}>
            <Chip accent={drag1 > 0.95 ? accent : undefined}>file</Chip>
            <Chip accent={drag2 > 0.95 ? accent : undefined}>diff</Chip>
            <Chip accent={drag3 > 0.95 ? accent : undefined}>commit</Chip>
          </div>
          <div className="a050-code" style={{ marginTop: 28 }}>Ready to send with prompt</div>
        </div>
        {item("src/app.rs", 80, drag1)}
        {item("file diff", 210, drag2, "var(--a-amber)")}
        {item("commit a13f9c2", 340, drag3, "var(--a-blue)")}
      </div>
    </VisualWrapper>
  );
};

const EditorVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const cursor = Math.floor(frame / 18) % 8;
  const code = [
    "pub async fn index_workspace(path: PathBuf) -> Result<()> {",
    "    let memory = MemoryIndexer::new(path).await?;",
    "    let summary = memory.refresh_notes().await?;",
    "    app.emit(\"heartbeat://memory\", summary)?;",
    "    Ok(())",
    "}",
    "",
    "// preview uses the same CodeMirror bundle",
  ];
  return (
    <VisualWrapper title="CodeMirror Editor" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 18, height: "100%" }}>
        {["edit", "preview"].map((mode) => (
          <div key={mode} className="a050-card" style={{ overflow: "hidden" }}>
            <div style={{ display: "flex", padding: "12px 16px", borderBottom: "1px solid rgba(200,211,245,0.08)" }}>
              <div className="a050-label">{mode}</div>
              <div style={{ flex: 1 }} />
              {mode === "edit" && <Chip accent={accent} style={{ padding: "5px 9px", fontSize: 11 }}>VIM</Chip>}
            </div>
            <div className="a050-code" style={{ padding: 18 }}>
              {code.map((line, i) => (
                <div key={`${mode}-${line}-${i}`} style={{ display: "flex", color: i === cursor ? accent : undefined }}>
                  <span style={{ width: 34, color: "var(--a-faint)" }}>{i + 1}</span>
                  <span>{line || " "}</span>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </VisualWrapper>
  );
};

const RemoteVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const tunnel = enterValue(frame, 58, 52);
  return (
    <VisualWrapper title="Remote SSH" accent={accent}>
      <div style={{ position: "relative", height: "100%" }}>
        <div className="a050-card" style={{ position: "absolute", left: 22, top: 28, width: 410, padding: 22 }}>
          <div className="a050-label">saved preset</div>
          <div className="a050-mini-title">prod-buildbox</div>
          <div className="a050-code" style={{ marginTop: 16 }}>
            iptoux@10.0.0.42:22<br />
            auth: keychain secret<br />
            dir: ~/Development/blxcode
          </div>
          <div style={{ marginTop: 22 }}><Chip accent={accent}>Connect</Chip></div>
        </div>
        <div
          style={{
            position: "absolute",
            left: 430,
            top: 280,
            width: 310,
            height: 3,
            background: accent,
            transform: `scaleX(${tunnel})`,
            transformOrigin: "left center",
            boxShadow: `0 0 22px ${accent}`,
          }}
        />
        <div className="a050-card" style={{ position: "absolute", right: 24, bottom: 34, width: 410, padding: 22 }}>
          <div className="a050-label">git commit graph</div>
          {["a13f9c2 Add MCP support", "3de19aa Refactor settings", "9be204c Open on GitHub"].map((commit, i) => (
            <div key={commit} style={{ display: "flex", gap: 12, alignItems: "center", marginTop: 15 }}>
              <div className="a050-dot" style={{ background: i === 2 ? accent : "var(--a-faint)" }} />
              <div className="a050-code" style={{ color: i === 2 ? accent : undefined }}>{commit}</div>
            </div>
          ))}
        </div>
      </div>
    </VisualWrapper>
  );
};

const RolesVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const roles = ["Architect", "Branch Steward", "Coordinator", "Reviewer", "Implementer", "Operator"];
  const radius = 205;
  return (
    <VisualWrapper title="ADE-aware Agent Roles" accent={accent}>
      <div style={{ position: "relative", height: "100%", display: "flex", alignItems: "center", justifyContent: "center" }}>
        <AgentModel accent={accent} size={230} recording label="Role Controller" />
        {roles.map((role, i) => {
          const p = enterValue(frame, 20 + i * 7, 22);
          const angle = (Math.PI * 2 * i) / roles.length + frame / 160;
          const x = Math.cos(angle) * radius;
          const y = Math.sin(angle) * radius;
          return (
            <div
              key={role}
              className="a050-card"
              style={{
                position: "absolute",
                left: `calc(50% + ${x}px - 104px)`,
                top: `calc(50% + ${y}px - 32px)`,
                width: 208,
                padding: "14px 10px",
                textAlign: "center",
                opacity: p,
                borderColor: i < 3 ? accent : undefined,
              }}
            >
              <div className="a050-mini-title" style={{ fontSize: 15 }}>{role}</div>
            </div>
          );
        })}
        <div className="a050-card" style={{ position: "absolute", right: 28, bottom: 28, padding: 18, width: 310 }}>
          <div className="a050-label">ADE controls</div>
          <div className="a050-code" style={{ marginTop: 14 }}>
            settings.update<br />
            terminal.coordinate<br />
            plans.manage<br />
            memory.query
          </div>
        </div>
      </div>
    </VisualWrapper>
  );
};

const UpdatesVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const beta = enterValue(frame, 42, 24);
  const notify = enterValue(frame, 92, 24);
  return (
    <VisualWrapper title="App Updates" accent={accent}>
      <div style={{ padding: 28 }}>
        <div className="a050-card" style={{ padding: 24, maxWidth: 660 }}>
          <div className="a050-label">update channel</div>
          <div style={{ display: "flex", gap: 10, marginTop: 20 }}>
            <Chip>Stable</Chip>
            <Chip accent={accent} style={{ transform: `scale(${1 + beta * 0.06})` }}>Beta</Chip>
          </div>
          <div style={{ marginTop: 34 }}>
            <Progress progress={25 + beta * 64} accent={accent} />
          </div>
          <div className="a050-code" style={{ marginTop: 18 }}>
            checking GitHub prereleases...<br />
            found BLXCode v0.5.0-beta
          </div>
        </div>
        <div
          className="a050-card"
          style={{
            position: "absolute",
            right: 58,
            top: 88,
            width: 360,
            padding: 22,
            opacity: notify,
            transform: `translateY(${interpolate(notify, [0, 1], [-24, 0])}px)`,
            borderColor: accent,
          }}
        >
          <div className="a050-label">notification</div>
          <div className="a050-mini-title">Update available</div>
          <div className="a050-code" style={{ marginTop: 12 }}>Open release notes</div>
        </div>
      </div>
    </VisualWrapper>
  );
};

const CanvasVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const nodes = [
    ["Agent", 420, 170],
    ["Plan", 250, 320],
    ["Task", 520, 360],
    ["Memory", 700, 230],
    ["File", 760, 440],
    ["Swarm", 410, 510],
  ] as const;
  return (
    <VisualWrapper title="Canvas and Swarm" accent={accent}>
      <div style={{ position: "relative", height: "100%" }}>
        <svg width="100%" height="100%" style={{ position: "absolute", inset: 0 }}>
          {nodes.slice(1).map((node, i) => {
            const p = enterValue(frame, 36 + i * 8, 22);
            return (
              <line
                key={node[0]}
                x1={420}
                y1={170}
                x2={node[1]}
                y2={node[2]}
                stroke={accent}
                strokeOpacity={0.18 + p * 0.42}
                strokeWidth={2}
              />
            );
          })}
        </svg>
        {nodes.map(([name, x, y], i) => {
          const p = enterValue(frame, 12 + i * 9, 24);
          if (i === 0) {
            return (
              <div
                key={name}
                style={{
                  position: "absolute",
                  left: x - 64,
                  top: y - 64,
                  opacity: p,
                  transform: `scale(${interpolate(p, [0, 1], [0.72, 1])})`,
                }}
              >
                <AgentModel accent={accent} size={128} recording compact />
                <div
                  className="a050-chip"
                  style={{
                    position: "absolute",
                    left: "50%",
                    bottom: -12,
                    transform: "translateX(-50%)",
                    borderColor: `${accent}66`,
                    color: accent,
                    background: `${accent}18`,
                  }}
                >
                  Agent
                </div>
              </div>
            );
          }
          return (
            <div
              key={name}
              className="a050-card"
              style={{
                position: "absolute",
                left: x - 72,
                top: y - 38,
                width: 144,
                padding: 16,
                textAlign: "center",
                opacity: p,
                transform: `scale(${interpolate(p, [0, 1], [0.72, 1])})`,
                borderColor: i === 5 ? accent : undefined,
                background: i === 5 ? `${accent}13` : undefined,
              }}
            >
              <div className="a050-mini-title">{name}</div>
            </div>
          );
        })}
      </div>
    </VisualWrapper>
  );
};

const MermaidVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const draw = enterValue(frame, 40, 56);
  const focus = enterValue(frame, 92, 30);
  const codeLines = [
    "flowchart LR",
    "  Idea[Release idea] --> Plan",
    "  Plan --> Tasks{Tasks ready?}",
    "  Tasks -->|yes| Build[Build video]",
    "  Tasks -->|review| Agent[BLXCode Agent]",
    "  Agent --> Memory[(Memory)]",
  ];
  const nodes = [
    { id: "Idea", x: 118, y: 150, w: 150, h: 58 },
    { id: "Plan", x: 330, y: 150, w: 128, h: 58 },
    { id: "Tasks", x: 535, y: 144, w: 132, h: 70 },
    { id: "Build", x: 760, y: 94, w: 142, h: 58 },
    { id: "Agent", x: 760, y: 218, w: 142, h: 58 },
    { id: "Memory", x: 548, y: 346, w: 150, h: 58 },
  ];
  const edges = [
    ["M 268 179 C 288 179 306 179 330 179", 0],
    ["M 458 179 C 490 179 506 179 535 179", 1],
    ["M 667 170 C 700 142 724 123 760 123", 2],
    ["M 667 192 C 704 210 724 247 760 247", 3],
    ["M 760 247 C 674 276 620 306 623 346", 4],
  ] as const;
  return (
    <VisualWrapper title="Centered Tab / Mermaid Diagram" accent={accent}>
      <div style={{ display: "grid", gridTemplateRows: "64px 1fr", gap: 16, height: "100%" }}>
        <div className="a050-card" style={{ padding: "12px 16px", display: "flex", alignItems: "center", gap: 12 }}>
          {["Files", "Agent", "Mermaid Diagram", "Memory"].map((tab, i) => {
            const active = i === 2;
            return (
              <div
                key={tab}
                className="a050-chip"
                style={{
                  borderColor: active ? `${accent}88` : "rgba(200, 211, 245, 0.12)",
                  background: active ? `${accent}18` : "rgba(21, 22, 32, 0.5)",
                  color: active ? accent : "var(--a-muted)",
                  transform: active ? `translateY(${Math.sin(frame / 18) * 2}px)` : undefined,
                }}
              >
                {tab}
              </div>
            );
          })}
          <div style={{ flex: 1 }} />
          <Chip accent={accent}>centered tab</Chip>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "380px 1fr", gap: 20, minHeight: 0 }}>
        <div className="a050-card" style={{ padding: 20, display: "grid", gridTemplateRows: "auto 1fr auto", gap: 16 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <div>
              <div className="a050-label">diagram source</div>
              <div className="a050-mini-title" style={{ marginTop: 6 }}>release-flow.mmd</div>
            </div>
            <div style={{ flex: 1 }} />
            <Chip accent={accent}>Mermaid</Chip>
          </div>
          <div className="a050-term" style={{ padding: 18, overflow: "hidden" }}>
            {codeLines.map((line, i) => {
              const p = enterValue(frame, 16 + i * 7, 18);
              return (
                <div
                  key={line}
                  style={{
                    color: i === 0 ? accent : "var(--a-text)",
                    opacity: p,
                    transform: `translateX(${interpolate(p, [0, 1], [-18, 0])}px)`,
                    whiteSpace: "pre",
                  }}
                >
                  <span style={{ color: "var(--a-faint)", display: "inline-block", width: 28 }}>{i + 1}</span>
                  {line}
                </div>
              );
            })}
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 8 }}>
            {["Source", "Preview", "Split"].map((mode, i) => (
              <Chip
                key={mode}
                accent={i === 2 ? accent : undefined}
                style={{
                  textAlign: "center",
                  opacity: enterValue(frame, 72 + i * 6, 18),
                }}
              >
                {mode}
              </Chip>
            ))}
          </div>
        </div>

        <div className="a050-card" style={{ position: "relative", padding: 24, overflow: "hidden" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
            <div>
              <div className="a050-label">live preview</div>
              <div className="a050-mini-title" style={{ marginTop: 6 }}>Rendered Mermaid graph</div>
            </div>
            <div style={{ flex: 1 }} />
            <Chip accent={accent}>add as context</Chip>
          </div>

          <svg
            viewBox="0 0 980 500"
            style={{
              position: "absolute",
              left: 14,
              right: 14,
              top: 96,
              width: "calc(100% - 28px)",
              height: 438,
            }}
          >
            <defs>
              <marker id="a050-mermaid-arrow" markerWidth="10" markerHeight="10" refX="8" refY="3" orient="auto">
                <path d="M0,0 L0,6 L9,3 z" fill={accent} opacity="0.78" />
              </marker>
              <filter id="a050-mermaid-glow" x="-30%" y="-30%" width="160%" height="160%">
                <feGaussianBlur stdDeviation="5" result="blur" />
                <feMerge>
                  <feMergeNode in="blur" />
                  <feMergeNode in="SourceGraphic" />
                </feMerge>
              </filter>
            </defs>
            {edges.map(([d, offset]) => (
              <path
                key={d}
                d={d}
                fill="none"
                stroke={accent}
                strokeWidth="3"
                strokeOpacity={0.2 + draw * 0.58}
                strokeDasharray="12 10"
                strokeDashoffset={interpolate(draw, [0, 1], [120 - offset * 12, 0])}
                markerEnd="url(#a050-mermaid-arrow)"
              />
            ))}
            {nodes.map((node, i) => {
              const p = enterValue(frame, 34 + i * 8, 22);
              const isDecision = node.id === "Tasks";
              const isAgent = node.id === "Agent";
              return (
                <g
                  key={node.id}
                  opacity={p}
                  transform={`translate(${node.x + node.w / 2} ${node.y + node.h / 2}) scale(${interpolate(p, [0, 1], [0.74, 1])}) translate(${-node.w / 2} ${-node.h / 2})`}
                  filter={isAgent ? "url(#a050-mermaid-glow)" : undefined}
                >
                  {isDecision ? (
                    <path
                      d={`M${node.w / 2} 0 L${node.w} ${node.h / 2} L${node.w / 2} ${node.h} L0 ${node.h / 2} Z`}
                      fill={`${accent}16`}
                      stroke={accent}
                      strokeOpacity="0.8"
                    />
                  ) : (
                    <rect
                      width={node.w}
                      height={node.h}
                      rx={18}
                      fill={isAgent ? `${accent}20` : "rgba(21, 22, 32, 0.82)"}
                      stroke={isAgent ? accent : "rgba(200, 211, 245, 0.25)"}
                      strokeOpacity={isAgent ? 0.95 : 1}
                    />
                  )}
                  <text
                    x={node.w / 2}
                    y={node.h / 2 + 6}
                    textAnchor="middle"
                    fill={isAgent ? accent : "#f8f8f2"}
                    fontFamily="Inter, sans-serif"
                    fontSize="21"
                    fontWeight="800"
                  >
                    {node.id}
                  </text>
                </g>
              );
            })}
          </svg>

          <div
            className="a050-card"
            style={{
              position: "absolute",
              right: 28,
              bottom: 28,
              width: 286,
              padding: 18,
              opacity: focus,
              transform: `translateY(${interpolate(focus, [0, 1], [18, 0])}px)`,
              borderColor: `${accent}66`,
              background: `linear-gradient(180deg, ${accent}14, rgba(12, 14, 22, 0.86))`,
            }}
          >
            <div className="a050-label">workspace handoff</div>
            <div className="a050-code" style={{ marginTop: 10 }}>
              selected diagram<br />
              context.attach("release-flow")
            </div>
          </div>
        </div>
        </div>
      </div>
    </VisualWrapper>
  );
};

const HeartbeatVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const pulse = 0.5 + Math.sin(frame / 9) * 0.5;
  const run = enterValue(frame, 66, 45);
  return (
    <VisualWrapper title="Settings / HeartBeat" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "320px 1fr", gap: 22, height: "100%" }}>
        <div className="a050-card" style={{ display: "flex", alignItems: "center", justifyContent: "center", flexDirection: "column", gap: 22 }}>
          <div
            style={{
              width: 190 + pulse * 18,
              height: 190 + pulse * 18,
              borderRadius: "50%",
              border: `3px solid ${accent}`,
              boxShadow: `0 0 ${30 + pulse * 40}px ${accent}55`,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontSize: 62,
              color: accent,
              fontWeight: 900,
            }}
          >
            HB
          </div>
          <Chip accent={accent}>service runtime</Chip>
        </div>
        <div className="a050-card" style={{ padding: 24 }}>
          <div className="a050-label">registered services</div>
          <div className="a050-card" style={{ padding: 18, marginTop: 20, borderColor: accent }}>
            <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
              <div className="a050-dot" style={{ background: accent }} />
              <div className="a050-mini-title">Memory Indexer</div>
              <div style={{ flex: 1 }} />
              <Chip accent={accent}>running</Chip>
            </div>
            <div style={{ marginTop: 22 }}><Progress progress={28 + run * 70} accent={accent} /></div>
            <div className="a050-code" style={{ marginTop: 18 }}>
              last run: today<br />
              next run: scheduled<br />
              notes: rules, skills, plans
            </div>
          </div>
          <div style={{ marginTop: 22 }}><Chip>Run now</Chip></div>
        </div>
      </div>
    </VisualWrapper>
  );
};

const SettingsVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const sections = ["App", "Appearance", "Agent Provider", "BLXCode Agent", "MCP", "Memory", "Voice", "Code Editor", "HeartBeat", "Remote SSH"];
  return (
    <VisualWrapper title="Settings Refactor" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "260px 1fr", gap: 18, height: "100%" }}>
        <div className="a050-card" style={{ padding: 14 }}>
          {sections.map((section, i) => {
            const p = enterValue(frame, 10 + i * 4, 16);
            return (
              <div
                key={section}
                className="a050-card"
                style={{
                  padding: "12px 14px",
                  marginBottom: 8,
                  opacity: p,
                  borderColor: i === 4 ? accent : undefined,
                  background: i === 4 ? `${accent}12` : undefined,
                }}
              >
                <div className="a050-code" style={{ color: i === 4 ? accent : undefined }}>{section}</div>
              </div>
            );
          })}
        </div>
        <div className="a050-card" style={{ padding: 24 }}>
          <div className="a050-mini-title">MCP server</div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginTop: 22 }}>
            <div className="a050-card" style={{ padding: 16 }}>
              <div className="a050-label">command</div>
              <div className="a050-code" style={{ marginTop: 12 }}>npx @modelcontextprotocol/server</div>
            </div>
            <div className="a050-card" style={{ padding: 16 }}>
              <div className="a050-label">environment</div>
              <div className="a050-code" style={{ marginTop: 12 }}>TOKEN=stored safely</div>
            </div>
          </div>
          <div style={{ marginTop: 28 }}><Lines rows={6} accent={accent} delay={58} /></div>
        </div>
      </div>
    </VisualWrapper>
  );
};

const TerminalsVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const agents = ["Devon / Claude", "Tom / Codex", "Mia / Gemini", "Kai / OpenCode"];
  return (
    <VisualWrapper title="Terminal Agent Control" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 18, height: "100%" }}>
        {agents.map((agent, i) => {
          const p = enterValue(frame, 14 + i * 10, 26);
          return (
            <div
              key={agent}
              className="a050-term"
              style={{
                padding: 18,
                opacity: p,
                transform: `translateY(${interpolate(p, [0, 1], [24, 0])}px)`,
                borderColor: i === 1 ? accent : undefined,
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <div className="a050-dot" style={{ background: i === 1 ? accent : "var(--a-green)" }} />
                <div className="a050-mini-title">{agent}</div>
              </div>
              <div className="a050-code" style={{ marginTop: 18 }}>
                $ blx agent read-output<br />
                sequence_id: {1000 + i * 27}<br />
                context attached: plan.md<br />
                {i === 1 ? "raw keys: Ctrl+C" : "status: settled"}
              </div>
            </div>
          );
        })}
      </div>
    </VisualWrapper>
  );
};

const MoreVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const items = ["Mermaid diagrams", "Notifications", "Status line", "AI plans", "App logs", "Git graph", "Help menu", "Performance", "Fixes"];
  return (
    <VisualWrapper title="More in v0.5.0" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 18 }}>
        {items.map((item, i) => {
          const p = enterValue(frame, 12 + i * 7, 20);
          return (
            <div
              key={item}
              className="a050-card"
              style={{
                padding: 24,
                height: 142,
                opacity: p,
                transform: `translateY(${interpolate(p, [0, 1], [22, 0])}px)`,
                borderColor: i % 3 === 0 ? accent : undefined,
              }}
            >
              <div className="a050-label">included</div>
              <div className="a050-mini-title" style={{ marginTop: 12 }}>{item}</div>
            </div>
          );
        })}
      </div>
    </VisualWrapper>
  );
};

const FeatureVisual: React.FC<{ feature: FeatureSceneData; accent: string }> = ({ feature, accent }) => {
  switch (feature.visual) {
    case "agent":
      return <AgentVisual accent={accent} />;
    case "mcp":
      return <MpcVisual accent={accent} />;
    case "kanban":
      return <KanbanVisual accent={accent} />;
    case "stats":
      return <StatsVisual accent={accent} />;
    case "providers":
      return <ProvidersVisual accent={accent} />;
    case "voice":
      return <VoiceVisual accent={accent} />;
    case "memory":
      return <MemoryVisual accent={accent} />;
    case "themes":
      return <ThemesVisual accent={accent} />;
    case "context":
      return <ContextVisual accent={accent} />;
    case "editor":
      return <EditorVisual accent={accent} />;
    case "remote":
      return <RemoteVisual accent={accent} />;
    case "roles":
      return <RolesVisual accent={accent} />;
    case "updates":
      return <UpdatesVisual accent={accent} />;
    case "canvas":
      return <CanvasVisual accent={accent} />;
    case "mermaid":
      return <MermaidVisual accent={accent} />;
    case "heartbeat":
      return <HeartbeatVisual accent={accent} />;
    case "settings":
      return <SettingsVisual accent={accent} />;
    case "terminals":
      return <TerminalsVisual accent={accent} />;
    case "more":
      return <MoreVisual accent={accent} />;
    default:
      return <MoreVisual accent={accent} />;
  }
};

const FeatureScene: React.FC<{ feature: FeatureSceneData; index: number }> = ({ feature, index }) => {
  const frame = useCurrentFrame();
  const accent = accentColor[feature.accent];
  const opacity = exitValue(frame, FEATURE_DURATION);
  const wipe = enterValue(frame, 0, 24);
  return (
    <AbsoluteFill className="a050" style={{ opacity }}>
      <Background accent={accent} />
      <Shell accent={accent}>
        <div className="a050-stage">
          <div
            style={{
              opacity: wipe,
              transform: `translateX(${interpolate(wipe, [0, 1], [-42, 0])}px)`,
            }}
          >
            <FeatureVisual feature={feature} accent={accent} />
          </div>
          <FeatureCopy feature={feature} index={index} accent={accent} />
        </div>
      </Shell>
      <ProgressRail index={index + 1} total={features.length} accent={accent} />
    </AbsoluteFill>
  );
};

const ProgressRail: React.FC<{ index: number; total: number; accent: string }> = ({
  index,
  total,
  accent,
}) => (
  <div className="a050-footer">
    <div>BLXCode v0.5.0 announcement</div>
    <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
      <span>{String(index).padStart(2, "0")} / {String(total).padStart(2, "0")}</span>
      <div style={{ width: 240, height: 4, borderRadius: 999, background: "rgba(200,211,245,0.12)", overflow: "hidden" }}>
        <div style={{ width: `${(index / total) * 100}%`, height: "100%", background: accent }} />
      </div>
    </div>
  </div>
);

const Intro: React.FC = () => {
  const frame = useCurrentFrame();
  const title = spring({
    frame: frame - 10,
    fps: FPS,
    config: { damping: 18, stiffness: 78, mass: 0.9 },
  });
  const sub = enterValue(frame, 54, 36);
  const shell = enterValue(frame, 94, 42);
  const opacity = exitValue(frame, INTRO_DURATION, 30);
  return (
    <AbsoluteFill className="a050" style={{ opacity }}>
      <Background accent="var(--a-purple)" intensity={1.15} />
      <AbsoluteFill style={{ alignItems: "center", justifyContent: "center", textAlign: "center", padding: "0 120px" }}>
        <div style={{ transform: `translateY(${interpolate(title, [0, 1], [42, 0])}px) scale(${interpolate(title, [0, 1], [0.9, 1])})`, opacity: title }}>
          <div className="a050-kicker" style={{ color: "var(--a-cyan)", marginBottom: 24 }}>release announcement</div>
          <h1 className="a050-intro-title">
            BLXCode <span className="a050-intro-gradient">v0.5.0</span>
          </h1>
        </div>
        <p
          className="a050-body"
          style={{
            maxWidth: 1180,
            marginTop: 36,
            opacity: sub,
            transform: `translateY(${interpolate(sub, [0, 1], [24, 0])}px)`,
          }}
        >
          The agentic workspace just got bigger: MCP, Kanban, voice, memory, local models, remote SSH, HeartBeat, Mermaid View, and a sharper app foundation.
        </p>
        <div
          style={{
            marginTop: 40,
            display: "flex",
            gap: 12,
            opacity: shell,
            transform: `translateY(${interpolate(shell, [0, 1], [20, 0])}px)`,
          }}
        >
          {["Agents", "Tasks", "Memory", "Terminals", "Providers"].map((label, i) => (
            <Chip key={label} accent={i === 0 ? "var(--a-purple)" : undefined}>{label}</Chip>
          ))}
        </div>
      </AbsoluteFill>
    </AbsoluteFill>
  );
};

const Outro: React.FC = () => {
  const frame = useCurrentFrame();
  const accent = "var(--a-purple)";
  const logo = spring({
    frame: frame - 8,
    fps: FPS,
    config: { damping: 17, stiffness: 82, mass: 0.82 },
  });
  const cta = enterValue(frame, 54, 34);
  const links = enterValue(frame, 78, 26);
  const grid = enterValue(frame, 102, 30);
  return (
    <AbsoluteFill className="a050">
      <Background accent={accent} intensity={1.2} />
      <Shell accent={accent} label="release / v0.5.0">
        <div style={{ height: "calc(100% - 58px)", display: "grid", gridTemplateColumns: "1fr 1fr", gap: 44, padding: 70, alignItems: "center" }}>
          <div style={{ opacity: logo, transform: `translateY(${interpolate(logo, [0, 1], [30, 0])}px)` }}>
            <div style={{ display: "flex", alignItems: "center", gap: 22 }}>
              <Img
                src={staticFile(BLX_LOGO_SRC)}
                className="a050-logo a050-logo-img"
                style={{ width: 98, height: 98, borderRadius: 24 }}
              />
              <div>
                <div className="a050-intro-title" style={{ fontSize: 100 }}>BLXCode</div>
                <div className="a050-kicker" style={{ color: "var(--a-cyan)", marginTop: 8 }}>v0.5.0</div>
              </div>
            </div>
            <p className="a050-body" style={{ marginTop: 34, maxWidth: 780 }}>
              Build with agents. Manage the workspace. Stay in flow.
            </p>
            <div style={{ marginTop: 30, opacity: cta }}>
              <Chip accent="var(--a-green)" style={{ fontSize: 18, padding: "12px 18px" }}>Available now · Windows / macOS / Linux</Chip>
            </div>
            <div
              style={{
                display: "grid",
                gap: 12,
                marginTop: 30,
                opacity: links,
                transform: `translateY(${interpolate(links, [0, 1], [18, 0])}px)`,
              }}
            >
              {["bitslix.com", "blxcode.com", "github.com/bitslix/BLXcode"].map((link, i) => (
                <div
                  key={link}
                  className="a050-card"
                  style={{
                    padding: "14px 18px",
                    maxWidth: 620,
                    borderColor: i === 2 ? "var(--a-cyan)" : "rgba(200, 211, 245, 0.16)",
                    background: i === 2 ? "rgba(125, 207, 255, 0.08)" : undefined,
                  }}
                >
                  <div className="a050-code" style={{ color: i === 2 ? "var(--a-cyan)" : "var(--a-text)", fontSize: 18 }}>
                    {link}
                  </div>
                </div>
              ))}
              <div
                className="a050-card"
                style={{
                  padding: "13px 18px",
                  maxWidth: 620,
                  borderColor: "rgba(200, 211, 245, 0.14)",
                  background: "rgba(21, 22, 32, 0.42)",
                }}
              >
                <div className="a050-label">music credit</div>
                <div className="a050-code" style={{ marginTop: 6, color: "var(--a-muted)", fontSize: 15 }}>
                  Music by Paolo Argento from Pixabay
                </div>
              </div>
            </div>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 18, opacity: grid }}>
            {["MCP", "Kanban", "Memory", "BLXVoice", "Remote SSH", "HeartBeat", "Mermaid View", "Agent Roles"].map((item, i) => (
              <div key={item} className="a050-card" style={{ padding: 24, minHeight: 128, borderColor: i % 3 === 0 ? accent : undefined }}>
                <div className="a050-label">v0.5.0</div>
                <div className="a050-mini-title" style={{ marginTop: 12 }}>{item}</div>
              </div>
            ))}
          </div>
        </div>
      </Shell>
      <div className="a050-footer">
        <div>Update now</div>
        <div>BLXCode announcement trailer</div>
      </div>
    </AbsoluteFill>
  );
};

export const BLXCode050Announcement: React.FC = () => {
  let offset = INTRO_DURATION;
  return (
    <AbsoluteFill className="a050">
      <Audio
        src={staticFile(MUSIC_SRC)}
        playbackRate={MUSIC_PLAYBACK_RATE}
        preservePitch
        volume={(frame) =>
          interpolate(
            frame,
            [0, 36, BLXCODE_050_ANNOUNCEMENT_DURATION - 60, BLXCODE_050_ANNOUNCEMENT_DURATION - 1],
            [0, 0.48, 0.48, 0],
            clamp,
          )
        }
      />
      <Sequence durationInFrames={INTRO_DURATION} premountFor={15}>
        <Intro />
      </Sequence>
      {features.map((feature, index) => {
        const from = offset;
        offset += FEATURE_DURATION;
        return (
          <Sequence
            key={feature.id}
            from={from}
            durationInFrames={FEATURE_DURATION}
            premountFor={15}
          >
            <FeatureScene feature={feature} index={index} />
          </Sequence>
        );
      })}
      <Sequence from={offset} durationInFrames={OUTRO_DURATION} premountFor={15}>
        <Outro />
      </Sequence>
    </AbsoluteFill>
  );
};
