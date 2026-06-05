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
import "../Announcement050/Announcement050.css";
import "./Announcement051.css";

const FPS = 30;
const INTRO_DURATION = 94;
const FEATURE_DURATION = 111;
const OUTRO_DURATION = 110;
const BLX_LOGO_SRC = "blxcode.png";
const MUSIC_SRC = "assets/white_records-funny-eastern-short-music-vlog-background-hip-hop-beat-29-sec-148905.mp3";

type Accent = "purple" | "cyan" | "pink" | "blue" | "green" | "amber";
type VisualKind =
  | "worktrees"
  | "plugins"
  | "run"
  | "sessions"
  | "models"
  | "mermaid"
  | "polish"
  | "safety";

type SceneData = {
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
};

const modelAccentColor: Record<string, string> = {
  "var(--a-purple)": "#826da8",
  "var(--a-cyan)": "#6f9aad",
  "var(--a-pink)": "#a66f91",
  "var(--a-blue)": "#6d7fa9",
  "var(--a-green)": "#6f9f82",
  "var(--a-amber)": "#a9825f",
};

const scenes: SceneData[] = [
  {
    id: "worktrees",
    kicker: "Git Worktree Workspaces",
    title: "Parallel branches. Real workspaces.",
    body: "Local and Remote SSH Git worktrees become first-class BLXCode workspaces.",
    accent: "green",
    tags: ["local", "Remote SSH", "experiments", "safe remove"],
    visual: "worktrees",
  },
  {
    id: "plugins",
    kicker: "Plugin Packages",
    title: "Plugins get package controls.",
    body: "Built-in and GitHub-installed packages now live in Settings with clear lifecycle actions.",
    accent: "purple",
    tags: ["Settings -> Plugins", "built-in", "GitHub", "validated"],
    visual: "plugins",
  },
  {
    id: "run",
    kicker: "Titlebar Run Menu",
    title: "Run from the titlebar.",
    body: "Detected dev, test, build, run, and debug commands launch into a visible terminal slot.",
    accent: "amber",
    tags: ["dev", "test", "build", "debug"],
    visual: "run",
  },
  {
    id: "sessions",
    kicker: "Multi-Agent Chat Sessions",
    title: "One workspace. Many agent threads.",
    body: "Multiple isolated conversations per workspace, each with its own state, routing, title, and notifications.",
    accent: "cyan",
    tags: ["isolated state", "routing", "persistence", "notifications"],
    visual: "sessions",
  },
  {
    id: "models",
    kicker: "Provider-Grouped Models",
    title: "Models finally scan cleanly.",
    body: "OpenRouter, OpenAI, and Anthropic are grouped with logos, search, favorites, and smarter switching.",
    accent: "blue",
    tags: ["OpenRouter", "OpenAI", "Anthropic", "favorites"],
    visual: "models",
  },
  {
    id: "mermaid",
    kicker: "Interactive Mermaid",
    title: "Mermaid becomes interactive.",
    body: "Pan, zoom, edit, save, and revert diagrams directly from gallery and preview surfaces.",
    accent: "cyan",
    tags: ["pan", "zoom", "live preview", "safe drafts"],
    visual: "mermaid",
  },
];

export const BLXCODE_051_ANNOUNCEMENT_DURATION =
  INTRO_DURATION + scenes.length * FEATURE_DURATION + OUTRO_DURATION;

const clamp = {
  extrapolateLeft: "clamp" as const,
  extrapolateRight: "clamp" as const,
};

const ease = Easing.bezier(0.16, 1, 0.3, 1);

const enter = (frame: number, start = 0, duration = 26) =>
  interpolate(frame, [start, start + duration], [0, 1], {
    ...clamp,
    easing: ease,
  });

const exit = (frame: number, duration: number, frames = 20) =>
  interpolate(frame, [duration - frames, duration], [1, 0], {
    ...clamp,
    easing: Easing.in(Easing.cubic),
  });

const toModelColor = (color: string) => modelAccentColor[color] ?? color;

const Background: React.FC<{ accent: string; intensity?: number }> = ({ accent, intensity = 1 }) => {
  const frame = useCurrentFrame();
  const drift = Math.sin(frame / 50) * 18;
  const drift2 = Math.cos(frame / 72) * 16;
  return (
    <AbsoluteFill>
      <div className="a050-bg" />
      <div
        style={{
          position: "absolute",
          width: 560,
          height: 560,
          borderRadius: 999,
          left: 138 + drift,
          top: 106 + drift2,
          background: `radial-gradient(circle, ${accent}3f 0%, transparent 66%)`,
          filter: "blur(38px)",
          opacity: 0.82 * intensity,
        }}
      />
      <div
        style={{
          position: "absolute",
          width: 620,
          height: 620,
          borderRadius: 999,
          right: 48 - drift2,
          bottom: 28 + drift,
          background: "radial-gradient(circle, rgba(125, 207, 255, 0.19) 0%, transparent 68%)",
          filter: "blur(42px)",
          opacity: 0.86 * intensity,
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
  label = "workspace / blxcode-v051",
}) => (
  <div className="a050-shell">
    <div className="a050-topbar">
      <div className="a050-brand">
        <Img src={staticFile(BLX_LOGO_SRC)} className="a050-logo a050-logo-img" />
        BLXCode
      </div>
      <div className="a050-crumb">{label}</div>
      <div style={{ flex: 1 }} />
      <div className="a050-crumb">AGENT-READY WORKSPACES</div>
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

const MiniLine: React.FC<{ width: number; accent?: string; delay?: number }> = ({
  width,
  accent,
  delay = 0,
}) => {
  const frame = useCurrentFrame();
  const p = enter(frame, delay, 18);
  return (
    <div
      className="a050-line"
      style={{
        width: `${interpolate(p, [0, 1], [8, width])}%`,
        opacity: p,
        background: accent ?? undefined,
      }}
    />
  );
};

const ColorBurst: React.FC<{ accent: string; variant?: "intro" | "outro" }> = ({ accent, variant = "intro" }) => {
  const frame = useCurrentFrame();
  const spin = frame * (variant === "intro" ? 0.34 : -0.22);
  const sweep = interpolate(frame, [0, variant === "intro" ? INTRO_DURATION : OUTRO_DURATION], [-18, 18], clamp);
  return (
    <>
      <div
        className="a051-color-plane a051-color-plane--cyan"
        style={{
          transform: `translate(${sweep}px, ${Math.sin(frame / 18) * 10}px) rotate(${spin}deg)`,
        }}
      />
      <div
        className="a051-color-plane a051-color-plane--pink"
        style={{
          transform: `translate(${-sweep * 1.4}px, ${Math.cos(frame / 20) * 12}px) rotate(${-spin * 0.7}deg)`,
        }}
      />
      <div
        className="a051-color-plane a051-color-plane--accent"
        style={{
          background: `linear-gradient(135deg, ${accent}88, transparent 68%)`,
          transform: `translate(${Math.cos(frame / 16) * 16}px, ${Math.sin(frame / 23) * 18}px) rotate(${spin * 0.45}deg)`,
        }}
      />
    </>
  );
};

const PosterOrb: React.FC<{ accent: string; size: number; side?: "left" | "right" }> = ({
  accent,
  size,
  side = "right",
}) => {
  const frame = useCurrentFrame();
  const p = enter(frame, 8, 28);
  const float = Math.sin(frame / 18) * 14;
  return (
    <div
      className={`a051-poster-orb a051-poster-orb--${side}`}
      style={{
        width: size,
        height: size,
        opacity: p,
        transform: `translateY(${interpolate(p, [0, 1], [50, float])}px) rotate(${interpolate(p, [0, 1], [-8, 0])}deg)`,
        borderColor: `${accent}66`,
        boxShadow: `0 48px 140px ${accent}24, inset 0 0 0 1px rgba(248,248,242,0.08)`,
      }}
    >
      <DroboOrb3D accent={toModelColor(accent)} accent2="#6f9aad" recording size={size} />
    </div>
  );
};

const TvOnEffect: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const open = enter(frame, 0, 22);
  const fade = interpolate(frame, [18, 34], [1, 0], clamp);
  const lineOpacity =
    frame < 8 ? interpolate(frame, [0, 8], [0, 1], clamp) : interpolate(frame, [8, 28], [1, 0], clamp);
  const flicker = 0.74 + Math.sin(frame * 2.6) * 0.18 + Math.sin(frame * 5.1) * 0.08;
  return (
    <div
      className="a051-tv-on"
      style={{
        opacity: fade,
      }}
    >
      <div
        className="a051-tv-on__iris"
        style={{
          transform: `scaleY(${interpolate(open, [0, 1], [0.006, 1])})`,
          opacity: Math.max(0, Math.min(1, flicker)),
        }}
      />
      <div
        className="a051-tv-on__line"
        style={{
          opacity: lineOpacity,
          background: `linear-gradient(90deg, transparent, ${accent}, var(--a-cyan), transparent)`,
          boxShadow: `0 0 28px ${accent}, 0 0 70px var(--a-cyan)`,
        }}
      />
    </div>
  );
};

const FeatureCopy: React.FC<{ scene: SceneData; index: number; accent: string }> = ({
  scene,
  index,
  accent,
}) => {
  const frame = useCurrentFrame();
  const titleIn = enter(frame, 12, 28);
  const bodyIn = enter(frame, 24, 28);
  const tagIn = enter(frame, 40, 24);
  return (
    <div
      className="a050-copy"
      style={{
        opacity: titleIn,
        transform: `translateX(${interpolate(titleIn, [0, 1], [46, 0])}px)`,
      }}
    >
      <div className="a050-kicker" style={{ color: accent }}>
        {String(index + 1).padStart(2, "0")} / {scene.kicker}
      </div>
      <h2 className="a051-title">
        <span style={{ color: accent }}>{scene.title.split(".")[0]}</span>
        {scene.title.includes(".") ? "." : ""}
        {scene.title.split(".").slice(1).join(".")}
      </h2>
      <p
        className="a051-body"
        style={{
          opacity: bodyIn,
          transform: `translateY(${interpolate(bodyIn, [0, 1], [16, 0])}px)`,
        }}
      >
        {scene.body}
      </p>
      <div
        className="a050-chip-row"
        style={{
          opacity: tagIn,
          transform: `translateY(${interpolate(tagIn, [0, 1], [14, 0])}px)`,
        }}
      >
        {scene.tags.map((tag, i) => (
          <Chip key={tag} accent={i === 0 ? accent : undefined}>
            {tag}
          </Chip>
        ))}
      </div>
    </div>
  );
};

const VisualFrame: React.FC<{ title: string; accent: string; children: ReactNode }> = ({
  title,
  accent,
  children,
}) => {
  const frame = useCurrentFrame();
  const inValue = spring({
    frame: frame - 4,
    fps: FPS,
    config: { damping: 19, stiffness: 92, mass: 0.75 },
  });
  return (
    <div
      className="a051-visual"
      style={{
        opacity: inValue,
        transform: `perspective(1200px) rotateY(${interpolate(inValue, [0, 1], [-8, 0])}deg) translateY(${interpolate(inValue, [0, 1], [32, 0])}px)`,
      }}
    >
      <Window title={title}>
        <div className="a051-window-body">{children}</div>
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

const WorktreesVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const branches = [
    ["main", "clean", "current"],
    ["feature/worktrees", "remote ssh", "agent task"],
    ["experiment/mermaid", "local", "preview"],
    ["fix/titlebar-run", "dirty", "protected"],
  ];
  return (
    <VisualFrame title="Worktree Workspaces" accent={accent}>
      <div style={{ display: "grid", gridTemplateRows: "120px 1fr", gap: 18, height: "100%" }}>
        <div className="a050-card" style={{ padding: 20 }}>
          <div className="a051-row">
            <Chip accent={accent}>Create worktree</Chip>
            <Chip>Open in BLXCode</Chip>
            <Chip>Refresh</Chip>
            <Chip>Safe remove</Chip>
          </div>
          <div style={{ marginTop: 18 }}>
            <MiniLine width={84} accent={accent} />
            <MiniLine width={62} delay={5} />
          </div>
        </div>
        <div className="a051-card-grid">
          {branches.map(([name, meta, state], i) => {
            const p = enter(frame, 12 + i * 9, 24);
            return (
              <div
                key={name}
                className="a051-lane"
                style={{
                  opacity: p,
                  transform: `translateX(${interpolate(p, [0, 1], [36, 0])}px)`,
                  borderColor: i === 1 || i === 3 ? `${accent}66` : undefined,
                  background: i === 3 ? "rgba(80, 250, 123, 0.08)" : undefined,
                }}
              >
                <div className="a051-row">
                  <div
                    style={{
                      width: 12,
                      height: 12,
                      borderRadius: 999,
                      background: i === 3 ? "var(--a-amber)" : accent,
                      boxShadow: `0 0 18px ${i === 3 ? "var(--a-amber)" : accent}`,
                    }}
                  />
                  <div className="a050-mini-title">{name}</div>
                  <div style={{ flex: 1 }} />
                  <div className="a051-pill">{meta}</div>
                  <div className="a051-pill" style={{ color: i === 3 ? "var(--a-amber)" : accent }}>
                    {state}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </VisualFrame>
  );
};

const PluginsVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const plugins = [
    ["GitHub", "built-in", "enabled"],
    ["OpenAI Docs", "built-in", "enabled"],
    ["Team Commands", "github", "disabled"],
    ["Release Tools", "github", "remove"],
  ];
  return (
    <VisualFrame title="Settings / Plugins" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "180px 1fr", gap: 18, height: "100%" }}>
        <div className="a051-sidebar">
          {["General", "Providers", "MCP", "Plugins", "Remote SSH"].map((item, i) => (
            <div
              key={item}
              className="a051-pill"
              style={{
                marginBottom: 10,
                color: item === "Plugins" ? accent : undefined,
                borderColor: item === "Plugins" ? `${accent}66` : undefined,
                background: item === "Plugins" ? `${accent}16` : undefined,
                opacity: enter(frame, i * 4, 18),
              }}
            >
              {item}
            </div>
          ))}
        </div>
        <div className="a051-card-grid">
          {plugins.map(([name, source, state], i) => {
            const p = enter(frame, 18 + i * 8, 22);
            return (
              <div
                key={name}
                className="a051-lane"
                style={{
                  opacity: p,
                  transform: `translateY(${interpolate(p, [0, 1], [24, 0])}px)`,
                }}
              >
                <div className="a051-row">
                  <div className="a050-mini-title">{name}</div>
                  <div className="a051-pill">{source}</div>
                  <div style={{ flex: 1 }} />
                  <Chip accent={state === "enabled" ? accent : undefined}>{state}</Chip>
                </div>
                <div style={{ marginTop: 14 }}>
                  <MiniLine width={72 - i * 6} accent={i === 0 ? accent : undefined} delay={22 + i * 6} />
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </VisualFrame>
  );
};

const RunVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const commands = ["pnpm dev", "pnpm test", "cargo build", "cargo tauri dev"];
  const active = Math.floor(frame / 28) % commands.length;
  return (
    <VisualFrame title="Titlebar Run Menu" accent={accent}>
      <div style={{ display: "grid", gridTemplateRows: "170px 1fr", gap: 18, height: "100%" }}>
        <div className="a050-card" style={{ padding: 22 }}>
          <div className="a051-row">
            <div className="a050-brand">
              <Img src={staticFile(BLX_LOGO_SRC)} className="a050-logo a050-logo-img" />
              workspace
            </div>
            <div style={{ flex: 1 }} />
            <Chip accent={accent}>Run</Chip>
          </div>
          <div style={{ marginTop: 18, display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
            {commands.map((cmd, i) => (
              <div
                key={cmd}
                className="a051-pill"
                style={{
                  color: active === i ? accent : undefined,
                  borderColor: active === i ? `${accent}77` : undefined,
                  background: active === i ? `${accent}18` : undefined,
                }}
              >
                {cmd}
              </div>
            ))}
          </div>
        </div>
        <div className="a051-terminal">
          <div style={{ color: accent }}>$ {commands[active]}</div>
          <div>detected from active workspace</div>
          <div>launching in visible terminal slot...</div>
          <div style={{ color: "var(--a-green)" }}>cwd: selected worktree directory</div>
          <div style={{ color: "var(--a-muted)" }}>works locally and over SSH</div>
        </div>
      </div>
    </VisualFrame>
  );
};

const SessionsVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const sessions = [
    ["Architecture", "OpenAI / GPT-5", "7 turns"],
    ["Release notes", "Anthropic / Sonnet", "12 turns"],
    ["Mermaid cleanup", "OpenRouter / Qwen", "3 turns"],
  ];
  return (
    <VisualFrame title="BLXCode Agent Sessions" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "230px 1fr", gap: 18, height: "100%" }}>
        <div className="a051-sidebar">
          {sessions.map(([name, provider], i) => {
            const p = enter(frame, 10 + i * 8, 20);
            return (
              <div
                key={name}
                className="a051-lane"
                style={{
                  marginBottom: 12,
                  opacity: p,
                  borderColor: i === 0 ? `${accent}66` : undefined,
                  background: i === 0 ? `${accent}12` : undefined,
                }}
              >
                <div className="a050-mini-title" style={{ fontSize: 15 }}>{name}</div>
                <div className="a050-code" style={{ fontSize: 11, marginTop: 4 }}>{provider}</div>
              </div>
            );
          })}
        </div>
        <div className="a050-card" style={{ padding: 20, position: "relative", overflow: "hidden" }}>
          <div style={{ display: "grid", gridTemplateColumns: "130px 1fr", gap: 16, alignItems: "center" }}>
            <div
              className="a050-agent-model"
              style={{
                width: 126,
                height: 126,
                borderRadius: 22,
                borderColor: `${accent}66`,
                boxShadow: `0 22px 56px ${accent}22`,
              }}
            >
              <DroboOrb3D accent={toModelColor(accent)} accent2="#7b7897" recording size={126} />
            </div>
            <div>
              <div className="a050-label">isolated conversation</div>
              <div className="a050-mini-title" style={{ fontSize: 30, marginTop: 8 }}>Architecture</div>
              <div className="a050-code" style={{ marginTop: 10 }}>separate state, backend routing, title, notifications</div>
            </div>
          </div>
          <div style={{ marginTop: 26, display: "grid", gap: 14 }}>
            {["Map the worktree model", "Plan migration edge cases", "Notify when agent finishes"].map((line, i) => {
              const p = enter(frame, 36 + i * 10, 20);
              return (
                <div
                  key={line}
                  className="a051-lane"
                  style={{
                    opacity: p,
                    transform: `translateX(${interpolate(p, [0, 1], [24, 0])}px)`,
                    borderColor: i === 2 ? `${accent}66` : undefined,
                  }}
                >
                  <div className="a051-row">
                    <div className="a051-pill" style={{ color: accent }}>agent</div>
                    <div className="a050-code">{line}</div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </VisualFrame>
  );
};

const ModelsVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const providers = [
    ["OpenAI", "gpt-5", true],
    ["Anthropic", "claude-sonnet-4.5", false],
    ["OpenRouter", "qwen3-coder", false],
  ];
  return (
    <VisualFrame title="Model Picker" accent={accent}>
      <div style={{ display: "grid", gridTemplateRows: "52px 1fr", gap: 16, height: "100%" }}>
        <div className="a051-search">Search models, providers, favorites...</div>
        <div className="a051-card-grid">
          {providers.map(([provider, model, favorite], i) => {
            const p = enter(frame, 12 + i * 10, 24);
            return (
              <div
                key={provider as string}
                className="a051-lane"
                style={{
                  opacity: p,
                  transform: `translateY(${interpolate(p, [0, 1], [24, 0])}px)`,
                  borderColor: i === 2 ? `${accent}66` : undefined,
                }}
              >
                <div className="a051-row">
                  <div
                    style={{
                      width: 44,
                      height: 44,
                      borderRadius: 13,
                      display: "grid",
                      placeItems: "center",
                      background: i === 0 ? `${accent}22` : "rgba(200, 211, 245, 0.08)",
                      color: i === 0 ? accent : "var(--a-muted)",
                      fontWeight: 850,
                    }}
                  >
                    {(provider as string).slice(0, 2)}
                  </div>
                  <div>
                    <div className="a050-mini-title">{provider}</div>
                    <div className="a050-code" style={{ fontSize: 12 }}>{model}</div>
                  </div>
                  <div style={{ flex: 1 }} />
                  <div className="a051-pill" style={{ color: favorite ? "var(--a-amber)" : undefined }}>
                    {favorite ? "favorite" : "provider group"}
                  </div>
                </div>
              </div>
            );
          })}
          <div className="a051-lane" style={{ borderColor: `${accent}66`, background: `${accent}10` }}>
            <div className="a051-row">
              <Chip accent={accent}>Selecting Anthropic updates provider + model</Chip>
            </div>
          </div>
        </div>
      </div>
    </VisualFrame>
  );
};

const MermaidVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const draw = enter(frame, 12, 36);
  const pan = Math.sin(frame / 34) * 5;
  const sourceIn = enter(frame, 28, 22);
  return (
    <VisualFrame title="Diagram Gallery / Mermaid" accent={accent}>
      <div className="a051-mermaid-app">
        <div className="a051-mermaid-top">
          <div className="a051-mermaid-tab">Test-Flow</div>
          <div className="a051-mermaid-actions">
            <span>Close</span>
            <span>Export .md</span>
            <span>Export .pdf</span>
          </div>
        </div>
        <div className="a051-mermaid-main">
          <div className="a051-mermaid-canvas">
            <div className="a051-mermaid-card">
              <b>Test-Flow</b>
              <span>flowchart</span>
            </div>
            <div className="a051-mermaid-title">Test-Flow</div>
            <svg
              className="a051-mermaid-flow"
              viewBox="0 0 340 480"
              style={{
                transform: `translate(${pan}px, ${-pan * 0.5}px) scale(${interpolate(draw, [0, 1], [0.92, 1])})`,
                opacity: draw,
              }}
            >
              <defs>
                <marker id="a051-arrow" viewBox="0 0 8 8" refX="6.8" refY="4" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
                  <path d="M0 0 L8 4 L0 8 z" fill="#8188aa" />
                </marker>
              </defs>
              <path d="M170 78 L170 116" stroke="#8188aa" strokeWidth="2" markerEnd="url(#a051-arrow)" />
              <path d="M170 166 C170 190 124 190 124 222" stroke="#8188aa" strokeWidth="2" fill="none" markerEnd="url(#a051-arrow)" />
              <path d="M170 166 C170 190 224 190 224 222" stroke="#8188aa" strokeWidth="2" fill="none" markerEnd="url(#a051-arrow)" />
              <path d="M124 284 L124 328" stroke="#8188aa" strokeWidth="2" markerEnd="url(#a051-arrow)" />
              <path d="M124 378 L124 416" stroke="#8188aa" strokeWidth="2" markerEnd="url(#a051-arrow)" />
              <path d="M224 284 C304 248 300 166 214 158" stroke="#8188aa" strokeWidth="2" fill="none" markerEnd="url(#a051-arrow)" />

              <rect x="138" y="42" width="64" height="34" rx="17" fill="#eef1fb" stroke="#9aa2c0" />
              <text x="170" y="64" textAnchor="middle">Start</text>
              <rect x="98" y="118" width="144" height="46" rx="3" fill="#f5f6fc" stroke="#c7cce0" />
              <text x="170" y="146" textAnchor="middle">Aktion ausfuhren</text>
              <path d="M124 220 L166 262 L124 304 L82 262 Z" fill="#f5f6fc" stroke="#c7cce0" />
              <text x="124" y="267" textAnchor="middle">Erfolg?</text>
              <text x="103" y="329" textAnchor="middle" fill="#565d82">Ja</text>
              <text x="216" y="224" textAnchor="middle" fill="#565d82">Nein</text>
              <rect x="54" y="330" width="140" height="46" rx="3" fill="#f5f6fc" stroke="#c7cce0" />
              <text x="124" y="358" textAnchor="middle">Ergebnis zeigen</text>
              <rect x="204" y="330" width="112" height="46" rx="3" fill="#f5f6fc" stroke="#c7cce0" />
              <text x="260" y="358" textAnchor="middle">Wiederholen</text>
              <rect x="102" y="418" width="44" height="30" rx="15" fill="#eef1fb" stroke="#9aa2c0" />
              <text x="124" y="438" textAnchor="middle">Ende</text>
            </svg>
            <div className="a051-mermaid-zoom">- <b>100%</b> +</div>
          </div>
          <div
            className="a051-mermaid-source"
            style={{
              opacity: sourceIn,
              transform: `translateX(${interpolate(sourceIn, [0, 1], [24, 0])}px)`,
            }}
          >
            <div className="a051-mermaid-source-head">Mermaid</div>
            {[
              "flowchart TD",
              "A([Start]) --> B[Aktion ausfuhren]",
              "B --> C{Erfolg?}",
              "C -- Ja --> D[Ergebnis zeigen]",
              "C -- Nein --> E[Wiederholen]",
              "E --> B",
              "D --> F([Ende])",
            ].map((line, i) => (
              <div key={line} className="a051-mermaid-code-line">
                <span>{i + 1}</span>
                <code>{line}</code>
              </div>
            ))}
          </div>
        </div>
      </div>
    </VisualFrame>
  );
};

const PolishVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const items = [
    ["Light readability", "higher contrast muted text"],
    ["Right panel", "cleaner surfaces"],
    ["Mermaid behavior", "better gallery and preview"],
    ["Remote terminals", "open in selected worktree"],
  ];
  return (
    <VisualFrame title="Workspace Polish" accent={accent}>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, height: "100%" }}>
        {items.map(([title, body], i) => {
          const p = enter(frame, 10 + i * 9, 22);
          return (
            <div
              key={title}
              className="a051-metric"
              style={{
                opacity: p,
                transform: `translateY(${interpolate(p, [0, 1], [24, 0])}px)`,
                borderColor: i === 0 || i === 3 ? `${accent}66` : undefined,
                background: i === 0 ? "rgba(248, 248, 242, 0.08)" : undefined,
              }}
            >
              <div className="a050-label">{title}</div>
              <div className="a050-mini-title" style={{ fontSize: 24, marginTop: 12 }}>{body}</div>
              <div style={{ marginTop: 20 }}>
                <div className="a051-track">
                  <div
                    className="a051-track-fill"
                    style={{
                      width: `${interpolate(p, [0, 1], [18, 78 + i * 4])}%`,
                      background: i === 0 ? accent : "rgba(200, 211, 245, 0.36)",
                    }}
                  />
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </VisualFrame>
  );
};

const SafetyVisual: React.FC<{ accent: string }> = ({ accent }) => {
  const frame = useCurrentFrame();
  const safeguards = [
    ["Runtime plugins", "discover commands only"],
    ["GitHub packages", "validated before copy"],
    ["Dirty worktrees", "commit, stash, or discard first"],
  ];
  return (
    <VisualFrame title="Guardrails" accent={accent}>
      <div style={{ display: "grid", gridTemplateRows: "1fr 170px", gap: 18, height: "100%" }}>
        <div className="a051-card-grid">
          {safeguards.map(([title, body], i) => {
            const p = enter(frame, 14 + i * 12, 24);
            return (
              <div
                key={title}
                className="a051-lane"
                style={{
                  opacity: p,
                  transform: `translateX(${interpolate(p, [0, 1], [32, 0])}px)`,
                  borderColor: `${accent}55`,
                }}
              >
                <div className="a051-row">
                  <div
                    style={{
                      width: 34,
                      height: 34,
                      borderRadius: 12,
                      background: `${accent}18`,
                      border: `1px solid ${accent}55`,
                      display: "grid",
                      placeItems: "center",
                      color: accent,
                      fontWeight: 900,
                    }}
                  >
                    {i + 1}
                  </div>
                  <div>
                    <div className="a050-mini-title">{title}</div>
                    <div className="a050-code" style={{ fontSize: 13 }}>{body}</div>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
        <div className="a051-terminal">
          <div style={{ color: accent }}>local-first / agent-ready / open-source</div>
          <div>workspace changes stay visible and recoverable</div>
          <div style={{ color: "var(--a-muted)" }}>BLXCode v0.5.1</div>
        </div>
      </div>
    </VisualFrame>
  );
};

const FeatureVisual: React.FC<{ scene: SceneData; accent: string }> = ({ scene, accent }) => {
  switch (scene.visual) {
    case "worktrees":
      return <WorktreesVisual accent={accent} />;
    case "plugins":
      return <PluginsVisual accent={accent} />;
    case "run":
      return <RunVisual accent={accent} />;
    case "sessions":
      return <SessionsVisual accent={accent} />;
    case "models":
      return <ModelsVisual accent={accent} />;
    case "mermaid":
      return <MermaidVisual accent={accent} />;
    case "polish":
      return <PolishVisual accent={accent} />;
    case "safety":
      return <SafetyVisual accent={accent} />;
    default:
      return <WorktreesVisual accent={accent} />;
  }
};

const FeatureScene: React.FC<{ scene: SceneData; index: number }> = ({ scene, index }) => {
  const frame = useCurrentFrame();
  const accent = accentColor[scene.accent];
  const out = exit(frame, FEATURE_DURATION, 22);
  const progress = interpolate(frame, [0, FEATURE_DURATION], [0, 100], clamp);
  return (
    <AbsoluteFill
      className="a051"
      style={{
        opacity: out,
        transform: `scale(${interpolate(out, [0, 1], [0.985, 1])})`,
      }}
    >
      <Background accent={accent} />
      <Shell accent={accent}>
        <div className="a051-stage">
          <FeatureCopy scene={scene} index={index} accent={accent} />
          <FeatureVisual scene={scene} accent={accent} />
        </div>
        <div className="a051-footer">
          <span>BLXCode v0.5.1 announcement</span>
          <span>{Math.round(progress)}% / {scene.id}</span>
        </div>
      </Shell>
    </AbsoluteFill>
  );
};

const Intro: React.FC = () => {
  const frame = useCurrentFrame();
  const wordIn = enter(frame, 18, 24);
  const secondIn = enter(frame, 32, 22);
  const stripIn = enter(frame, 48, 20);
  const out = exit(frame, INTRO_DURATION, 18);
  const accent = "var(--a-green)";
  return (
    <AbsoluteFill
      className="a051"
      style={{
        opacity: out,
        transform: `scale(${interpolate(out, [0, 1], [0.982, 1])})`,
      }}
    >
      <Background accent={accent} intensity={1.18} />
      <ColorBurst accent={accent} />
      <PosterOrb accent="var(--a-purple)" size={540} />
      <TvOnEffect accent={accent} />
      <div className="a051-hero-mark">
        <Img src={staticFile(BLX_LOGO_SRC)} className="a050-logo a050-logo-img" />
        BLXCode
      </div>
      <div className="a051-intro a051-intro--poster">
        <div
          style={{
            opacity: wordIn,
            transform: `translateY(${interpolate(wordIn, [0, 1], [44, 0])}px)`,
          }}
        >
          <div className="a051-release-chip">v0.5.1</div>
          <h1 className="a051-poster-title">
            <span>workspace</span>
            <span className="a051-gradient-text">goes agent-ready</span>
          </h1>
          <p
            className="a051-poster-copy"
            style={{
              opacity: secondIn,
              transform: `translateY(${interpolate(secondIn, [0, 1], [24, 0])}px)`,
            }}
          >
            Worktrees, plugins, run commands, isolated agents, provider-grouped models, and live Mermaid diagrams.
          </p>
        </div>
      </div>
      <div
        className="a051-signal-strip"
        style={{
          opacity: stripIn,
          transform: `translateY(${interpolate(stripIn, [0, 1], [22, 0])}px)`,
        }}
      >
        {["Git worktrees", "Remote SSH", "Plugins", "Run menu", "Multi-agent", "Mermaid"].map((item, i) => (
          <span key={item} style={{ color: i % 2 === 0 ? accent : "var(--a-cyan)" }}>
            {item}
          </span>
        ))}
      </div>
      <div className="a051-footer">
        <span>silent cut</span>
        <span>agent-ready workspace system</span>
      </div>
    </AbsoluteFill>
  );
};

const Outro: React.FC = () => {
  const frame = useCurrentFrame();
  const inValue = enter(frame, 8, 24);
  const tagIn = enter(frame, 42, 18);
  const out = exit(frame, OUTRO_DURATION, 20);
  const accent = "var(--a-pink)";
  return (
    <AbsoluteFill className="a051" style={{ opacity: out }}>
      <Background accent={accent} intensity={1.14} />
      <ColorBurst accent={accent} variant="outro" />
      <Shell accent={accent} label="release / v0.5.1">
        <div
          className="a051-outro-poster"
          style={{
            opacity: inValue,
            transform: `translateY(${interpolate(inValue, [0, 1], [34, 0])}px)`,
          }}
        >
          <div>
            <div className="a050-kicker" style={{ color: "var(--a-cyan)" }}>BLXCode v0.5.1</div>
            <h2 className="a051-outro-title">
              <span>not a chat panel.</span>
              <span className="a051-gradient-text">an agent workspace.</span>
            </h2>
            <p className="a051-body">
              Local-first, guarded by design, built for parallel coding agents.
            </p>
            <div
              className="a050-chip-row"
              style={{
                opacity: tagIn,
                transform: `translateY(${interpolate(tagIn, [0, 1], [18, 0])}px)`,
              }}
            >
              {["#BLXCode", "#OpenSource", "#CodingAgents", "#Rust", "#Tauri", "#Git", "#SSH", "#Mermaid"].map(
                (tag, i) => (
                  <Chip key={tag} accent={i === 0 || i === 2 ? accent : undefined}>
                    {tag}
                  </Chip>
                ),
              )}
            </div>
            <div
              className="a051-music-credit"
              style={{
                opacity: tagIn,
                transform: `translateY(${interpolate(tagIn, [0, 1], [18, 0])}px)`,
              }}
            >
              Music by White_Records from Pixabay
            </div>
          </div>
          <PosterOrb accent={accent} size={430} side="left" />
        </div>
        <div className="a051-footer">
          <span>BLXCode v0.5.1</span>
          <span>worktrees / plugins / agents / diagrams</span>
        </div>
      </Shell>
    </AbsoluteFill>
  );
};

export const BLXCode051Announcement: React.FC = () => {
  return (
    <AbsoluteFill className="a051">
      <Audio src={staticFile(MUSIC_SRC)} volume={0.85} />
      <Sequence durationInFrames={INTRO_DURATION} premountFor={15}>
        <Intro />
      </Sequence>
      {scenes.map((scene, index) => {
        const offset = INTRO_DURATION + index * FEATURE_DURATION;
        return (
          <Sequence key={scene.id} from={offset} durationInFrames={FEATURE_DURATION} premountFor={15}>
            <FeatureScene scene={scene} index={index} />
          </Sequence>
        );
      })}
      <Sequence
        from={INTRO_DURATION + scenes.length * FEATURE_DURATION}
        durationInFrames={OUTRO_DURATION}
        premountFor={15}
      >
        <Outro />
      </Sequence>
    </AbsoluteFill>
  );
};
