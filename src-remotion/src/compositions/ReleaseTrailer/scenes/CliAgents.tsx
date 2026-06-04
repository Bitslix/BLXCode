import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background, FeatureCard } from "../components";

const DURATION = 120;
const FEATURE_ENTER = 12;

type Agent = {
  id: string;
  name: string;
  model: string;
  effort: string;
  env: string;
  accent: string;
  delay: number;
};

const AGENTS: Agent[] = [
  {
    id: "claude",
    name: "Claude",
    model: "opus 4.6",
    effort: "max effort",
    env: "CLAUDE_CODE_EFFORT_LEVEL",
    accent: "#ffb86c",
    delay: 16,
  },
  {
    id: "codex",
    name: "Codex",
    model: "gpt-5.2-codex",
    effort: "high",
    env: "-c model_reasoning_effort",
    accent: "#50fa7b",
    delay: 24,
  },
  {
    id: "gemini",
    name: "Gemini",
    model: "gemini-3-pro",
    effort: "model-only",
    env: "—",
    accent: "#7aa2f7",
    delay: 32,
  },
  {
    id: "opencode",
    name: "OpenCode",
    model: "qwen3-coder",
    effort: "model-only",
    env: "—",
    accent: "#bd93f9",
    delay: 40,
  },
  {
    id: "cursor",
    name: "Cursor",
    model: "sonnet 4.5",
    effort: "model-only",
    env: "—",
    accent: "#ff79c6",
    delay: 48,
  },
];

export const CliAgentsScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  const cardEnter = spring({
    frame: frame - 4,
    fps,
    config: { damping: 18, stiffness: 90, mass: 0.7 },
  });

  return (
    <AbsoluteFill className="cli-agents-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />
      <AbsoluteFill
        style={{
          flexDirection: "row",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 100px",
        }}
      >
        <FeatureCard
          feature={{
            id: "cli-agents",
            kicker: "Terminals",
            title: "Per-terminal CLI-agent model + effort",
            body: "Fleet rows pick the model and reasoning effort for claude, codex, gemini, opencode, and cursor. Persists on workspaces and presets.",
            accent: "warning",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div
          style={{
            width: 720,
            opacity: cardEnter,
            transform: `translateY(${(1 - cardEnter) * 20}px)`,
            background: "var(--bg-panel)",
            border: "1px solid var(--border-strong)",
            borderRadius: 20,
            padding: 20,
            boxShadow: "0 30px 80px -20px rgba(0, 0, 0, 0.5)",
          }}
        >
          {/* Header */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 12,
              padding: "0 4px 14px 4px",
              borderBottom: "1px solid var(--border)",
              fontFamily: "var(--font-mono)",
              fontSize: 12,
              color: "var(--text-faint)",
              letterSpacing: "0.12em",
              textTransform: "uppercase",
            }}
          >
            <span style={{ width: 22 }}>slot</span>
            <span style={{ width: 80 }}>agent</span>
            <span style={{ flex: 1 }}>model</span>
            <span style={{ width: 130 }}>effort</span>
            <span style={{ width: 230 }}>launch profile</span>
          </div>

          {/* Rows */}
          {AGENTS.map((agent, i) => {
            const enter = spring({
              frame: frame - agent.delay,
              fps,
              config: { damping: 16, stiffness: 120, mass: 0.5 },
            });
            return (
              <div
                key={agent.id}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 12,
                  padding: "12px 4px",
                  borderBottom: i < AGENTS.length - 1 ? "1px solid var(--border)" : "none",
                  opacity: enter,
                  transform: `translateX(${(1 - enter) * 16}px)`,
                }}
              >
                <div
                  style={{
                    width: 22,
                    height: 22,
                    borderRadius: 6,
                    background: "var(--bg-raised)",
                    border: "1px solid var(--border)",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    fontSize: 11,
                    color: "var(--text-muted)",
                    fontFamily: "var(--font-mono)",
                  }}
                >
                  {i + 1}
                </div>
                <div
                  style={{
                    width: 80,
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                  }}
                >
                  <span
                    style={{
                      width: 10,
                      height: 10,
                      borderRadius: "50%",
                      background: agent.accent,
                      boxShadow: `0 0 8px ${agent.accent}`,
                    }}
                  />
                  <span
                    style={{
                      fontFamily: "var(--font-sans)",
                      fontSize: 15,
                      fontWeight: 600,
                      color: "var(--text-bright)",
                    }}
                  >
                    {agent.name}
                  </span>
                </div>
                <div
                  style={{
                    flex: 1,
                    padding: "6px 10px",
                    borderRadius: 6,
                    background: "var(--bg-raised)",
                    border: "1px solid var(--border)",
                    fontFamily: "var(--font-mono)",
                    fontSize: 14,
                    color: "var(--text)",
                  }}
                >
                  {agent.model}
                </div>
                <div
                  style={{
                    width: 130,
                    padding: "6px 10px",
                    borderRadius: 6,
                    background:
                      agent.effort === "model-only" ? "var(--bg-raised)" : "var(--accent-purple-soft)",
                    border: `1px solid ${
                      agent.effort === "model-only" ? "var(--border)" : "var(--accent-purple)"
                    }`,
                    fontFamily: "var(--font-mono)",
                    fontSize: 13,
                    color: agent.effort === "model-only" ? "var(--text-faint)" : "var(--accent-purple)",
                    fontStyle: agent.effort === "model-only" ? "italic" : "normal",
                  }}
                >
                  {agent.effort}
                </div>
                <div
                  style={{
                    width: 230,
                    fontFamily: "var(--font-mono)",
                    fontSize: 12,
                    color: "var(--text-muted)",
                    whiteSpace: "nowrap",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                  }}
                >
                  {agent.env}
                </div>
              </div>
            );
          })}
        </div>
      </AbsoluteFill>
      <style>{`
        .cli-agents-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const CLI_AGENTS_DURATION = DURATION;
