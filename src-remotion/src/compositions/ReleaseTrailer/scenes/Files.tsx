import { AbsoluteFill, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background, FeatureCard } from "../components";

const DURATION = 120;
const FEATURE_ENTER = 12;

const HIGHLIGHT: { line: number; col: number; len: number; delay: number; color: string; label: string }[] = [
  { line: 4, col: 4, len: 14, delay: 50, color: "var(--accent-cyan)", label: "selection" },
  { line: 7, col: 8, len: 22, delay: 60, color: "var(--accent-purple)", label: "fold #1" },
  { line: 12, col: 6, len: 18, delay: 70, color: "var(--accent-pink)", label: "fold #2" },
];

const LINES: { indent: number; tokens: { type: "kw" | "fn" | "punct" | "param" | "type" | "ident" | "comment"; text: string }[] }[] = [
  { indent: 0, tokens: [{ type: "kw", text: "use " }, { type: "ident", text: "agent::{ChatHistory, Briefing}" }, { type: "punct", text: ";" }] },
  { indent: 0, tokens: [] },
  { indent: 0, tokens: [{ type: "comment", text: "// Compact the running conversation" }] },
  { indent: 0, tokens: [{ type: "kw", text: "pub fn " }, { type: "fn", text: "summarize_turn" }, { type: "punct", text: "(" }, { type: "param", text: "history: &ChatHistory" }, { type: "punct", text: ") -> " }, { type: "type", text: "Briefing" }, { type: "punct", text: " {" }] },
  { indent: 2, tokens: [{ type: "ident", text: "let " }, { type: "param", text: "brief " }, { type: "punct", text: "= " }, { type: "fn", text: "agent" }, { type: "punct", text: "." }, { type: "fn", text: "oneshot_complete" }, { type: "punct", text: "(" }, { type: "param", text: "history" }, { type: "punct", text: ");" }] },
  { indent: 2, tokens: [{ type: "ident", text: "if " }, { type: "param", text: "brief.is_empty()" }, { type: "punct", text: " { " }, { type: "kw", text: "return " }, { type: "param", text: "Briefing::default()" }, { type: "punct", text: ";" }] },
  { indent: 2, tokens: [{ type: "ident", text: "let " }, { type: "param", text: "truncated " }, { type: "punct", text: "= " }, { type: "fn", text: "truncate_to_budget" }, { type: "punct", text: "(" }, { type: "param", text: "&brief" }, { type: "punct", text: ", " }, { type: "param", text: "TOKEN_BUDGET" }, { type: "punct", text: ");" }] },
  { indent: 2, tokens: [{ type: "fn", text: "truncated" }] },
  { indent: 1, tokens: [{ type: "punct", text: "}" }] },
  { indent: 0, tokens: [] },
  { indent: 0, tokens: [{ type: "comment", text: "// Auto-compact fires once per crossing" }] },
  { indent: 0, tokens: [{ type: "kw", text: "fn " }, { type: "fn", text: "should_compact" }, { type: "punct", text: "(" }, { type: "param", text: "occupancy: f32" }, { type: "punct", text: ") -> " }, { type: "type", text: "bool" }, { type: "punct", text: " {" }] },
  { indent: 2, tokens: [{ type: "param", text: "occupancy >= " }, { type: "fn", text: "COMPACT_THRESHOLD" }, { type: "punct", text: " && " }, { type: "param", text: "!state.latched" }] },
  { indent: 1, tokens: [{ type: "punct", text: "}" }] },
];

const tokenColor = (type: string) => {
  switch (type) {
    case "kw":
      return "#ff79c6";
    case "fn":
      return "#50fa7b";
    case "param":
      return "#7dcfff";
    case "type":
      return "#ffb86c";
    case "comment":
      return "#565f89";
    case "ident":
      return "#c8d3f5";
    default:
      return "#c8d3f5";
  }
};

export const FilesScene: React.FC = () => {
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
    <AbsoluteFill className="files-scene" style={{ opacity: exitOpacity }}>
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
            id: "files",
            kicker: "Editor",
            title: "File preview = CodeMirror 6",
            body: "Preview and edit use the same engine now — same highlighting, gutter, folding. highlight.js (~127 KiB) is gone.",
            accent: "success",
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
            overflow: "hidden",
            boxShadow: "0 30px 80px -20px rgba(0, 0, 0, 0.5)",
          }}
        >
          {/* Header */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 10,
              padding: "10px 16px",
              background: "var(--bg-panel-header)",
              borderBottom: "1px solid var(--border)",
            }}
          >
            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 14,
                color: "var(--text-muted)",
              }}
            >
              agent/compact.rs
            </div>
            <div style={{ flex: 1 }} />
            <div
              style={{
                padding: "3px 8px",
                borderRadius: 999,
                background: "var(--bg-raised)",
                border: "1px solid var(--border)",
                fontFamily: "var(--font-mono)",
                fontSize: 11,
                color: "var(--text-faint)",
                textTransform: "uppercase",
                letterSpacing: "0.1em",
              }}
            >
              read-only
            </div>
            <div
              style={{
                padding: "3px 8px",
                borderRadius: 999,
                background: "var(--accent-purple-soft)",
                border: "1px solid var(--accent-purple-soft)",
                fontFamily: "var(--font-mono)",
                fontSize: 11,
                color: "var(--accent-purple)",
                textTransform: "uppercase",
                letterSpacing: "0.1em",
              }}
            >
              ⌘ + preview
            </div>
          </div>

          {/* Code body */}
          <div
            style={{
              padding: "16px 0",
              fontFamily: "var(--font-mono)",
              fontSize: 15,
              lineHeight: 1.7,
              position: "relative",
            }}
          >
            {LINES.map((line, i) => (
              <div
                key={i}
                style={{
                  display: "flex",
                  gap: 16,
                  paddingLeft: 16,
                  paddingRight: 16,
                  background: i % 2 === 0 ? "transparent" : "rgba(189, 200, 245, 0.02)",
                }}
              >
                <span
                  style={{
                    width: 32,
                    textAlign: "right",
                    color: "var(--text-faint)",
                    fontFeatureSettings: '"tnum"',
                  }}
                >
                  {i + 1}
                </span>
                <div style={{ flex: 1, paddingLeft: line.indent * 12 }}>
                  {line.tokens.map((tok, j) => (
                    <span key={j} style={{ color: tokenColor(tok.type) }}>
                      {tok.text}
                    </span>
                  ))}
                </div>
              </div>
            ))}

            {/* Highlights */}
            {HIGHLIGHT.map((h, i) => {
              const enter = spring({
                frame: frame - h.delay,
                fps,
                config: { damping: 16, stiffness: 130, mass: 0.5 },
              });
              return (
                <div
                  key={i}
                  style={{
                    position: "absolute",
                    left: 16 + 32 + 16 + h.col * 9.4,
                    top: 16 + (h.line - 1) * 25.5,
                    height: 22,
                    width: h.len * 9.4,
                    background: `${h.color === "var(--accent-cyan)" ? "rgba(125, 207, 255, 0.18)" : h.color === "var(--accent-purple)" ? "rgba(189, 147, 249, 0.18)" : "rgba(255, 121, 198, 0.18)"}`,
                    border: `1px solid ${h.color === "var(--accent-cyan)" ? "rgba(125, 207, 255, 0.4)" : h.color === "var(--accent-purple)" ? "rgba(189, 147, 249, 0.4)" : "rgba(255, 121, 198, 0.4)"}`,
                    borderRadius: 3,
                    opacity: enter,
                    pointerEvents: "none",
                  }}
                >
                  <div
                    style={{
                      position: "absolute",
                      bottom: -22,
                      right: 0,
                      fontFamily: "var(--font-mono)",
                      fontSize: 10,
                      color: h.color,
                      textTransform: "uppercase",
                      letterSpacing: "0.12em",
                    }}
                  >
                    {h.label}
                  </div>
                </div>
              );
            })}
          </div>

          {/* Footer */}
          <div
            style={{
              padding: "8px 16px",
              background: "var(--bg-raised)",
              borderTop: "1px solid var(--border)",
              fontFamily: "var(--font-mono)",
              fontSize: 12,
              color: "var(--text-faint)",
              display: "flex",
              alignItems: "center",
              gap: 16,
            }}
          >
            <span>Rust · utf-8</span>
            <span>14 lines</span>
            <span>ln 8, col 18</span>
            <span style={{ flex: 1 }} />
            <span style={{ color: "var(--status-danger)" }}>
              −highlight.js · −127 KiB
            </span>
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .files-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const FILES_DURATION = DURATION;
