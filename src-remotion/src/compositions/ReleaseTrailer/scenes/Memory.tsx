import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background, FeatureCard } from "../components";

const DURATION = 120;
const FEATURE_ENTER = 12;

type Note = { name: string; category: string; date: string; delay: number; active?: boolean };

const NOTES: Note[] = [
  { name: "README.md", category: "index", date: "today", delay: 14, active: true },
  { name: "agent-panel.md", category: "agent", date: "yesterday", delay: 22 },
  { name: "voice-ptt.md", category: "voice", date: "2 days ago", delay: 30 },
  { name: "memory-center.md", category: "memory", date: "3 days ago", delay: 38 },
  { name: "code-architecture.md", category: "code", date: "1 week ago", delay: 46 },
];

const CATEGORIES: { id: string; label: string; open: boolean; count: number }[] = [
  { id: "agent", label: "agent", open: true, count: 3 },
  { id: "voice", label: "voice", open: false, count: 2 },
  { id: "memory", label: "memory", open: false, count: 4 },
  { id: "code", label: "code", open: false, count: 2 },
  { id: "learnings", label: "learnings", open: false, count: 5 },
];

export const MemoryScene: React.FC = () => {
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

  const tabOpen = interpolate(frame, [20, 35], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill className="memory-scene" style={{ opacity: exitOpacity }}>
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
            id: "memory",
            kicker: "Memory",
            title: "Centered memory tabs + workspace index",
            body: "Open notes in a centered tab, auto-load the workspace's README index, and only keep one category group open at a time.",
            accent: "blue",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div
          style={{
            width: 740,
            opacity: cardEnter,
            transform: `translateY(${(1 - cardEnter) * 20}px)`,
            position: "relative",
          }}
        >
          {/* Main panel */}
          <div
            style={{
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
                gap: 12,
                padding: "12px 16px",
                background: "var(--bg-panel-header)",
                borderBottom: "1px solid var(--border)",
              }}
            >
              <span style={{ fontSize: 14, color: "var(--text-faint)" }}>⌘</span>
              <span
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: 14,
                  color: "var(--text-bright)",
                  fontWeight: 600,
                }}
              >
                Memory
              </span>
              <div style={{ flex: 1 }} />
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  padding: "4px 10px",
                  borderRadius: 999,
                  background: "var(--accent-blue-soft)",
                  border: "1px solid var(--accent-blue)",
                  fontFamily: "var(--font-mono)",
                  fontSize: 11,
                  color: "var(--accent-blue)",
                  textTransform: "uppercase",
                  letterSpacing: "0.1em",
                }}
              >
                <span
                  style={{
                    width: 5,
                    height: 5,
                    borderRadius: "50%",
                    background: "var(--accent-blue)",
                  }}
                />
                5 files · 4 cats
              </div>
            </div>

            <div style={{ display: "flex" }}>
              {/* File tree */}
              <div
                style={{
                  width: 240,
                  padding: 12,
                  borderRight: "1px solid var(--border)",
                  background: "var(--bg-raised)",
                }}
              >
                <div
                  style={{
                    fontFamily: "var(--font-mono)",
                    fontSize: 10,
                    color: "var(--text-faint)",
                    letterSpacing: "0.16em",
                    textTransform: "uppercase",
                    marginBottom: 10,
                  }}
                >
                  .agents/memory
                </div>
                {CATEGORIES.map((cat, i) => {
                  const enter = spring({
                    frame: frame - 10 - i * 3,
                    fps,
                    config: { damping: 16, stiffness: 120, mass: 0.5 },
                  });
                  return (
                    <div key={cat.id} style={{ marginBottom: 8, opacity: enter }}>
                      <div
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 6,
                          padding: "4px 6px",
                          borderRadius: 6,
                          background: cat.open ? "var(--accent-purple-soft)" : "transparent",
                        }}
                      >
                        <span
                          style={{
                            fontSize: 10,
                            color: "var(--text-faint)",
                            transition: "transform 0.2s",
                            transform: cat.open ? "rotate(90deg)" : "rotate(0deg)",
                          }}
                        >
                          ▸
                        </span>
                        <span
                          style={{
                            flex: 1,
                            fontFamily: "var(--font-mono)",
                            fontSize: 13,
                            color: cat.open ? "var(--accent-purple)" : "var(--text)",
                          }}
                        >
                          {cat.label}
                        </span>
                        <span
                          style={{
                            fontFamily: "var(--font-mono)",
                            fontSize: 10,
                            color: "var(--text-faint)",
                          }}
                        >
                          {cat.count}
                        </span>
                      </div>
                      {cat.open && (
                        <div style={{ paddingLeft: 16, marginTop: 4 }}>
                          {NOTES.filter((n) => n.category === cat.id || cat.id === "agent").slice(0, 2).map((n, j) => (
                            <div
                              key={j}
                              style={{
                                display: "flex",
                                alignItems: "center",
                                gap: 6,
                                padding: "3px 6px",
                                borderRadius: 4,
                                background: n.active ? "var(--accent-blue-soft)" : "transparent",
                              }}
                            >
                              <span style={{ fontSize: 11, color: "var(--text-faint)" }}>📄</span>
                              <span
                                style={{
                                  fontFamily: "var(--font-mono)",
                                  fontSize: 12,
                                  color: n.active ? "var(--accent-blue)" : "var(--text-muted)",
                                }}
                              >
                                {n.name}
                              </span>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>

              {/* Preview */}
              <div style={{ flex: 1, padding: 20 }}>
                <div
                  style={{
                    fontFamily: "var(--font-sans)",
                    fontSize: 22,
                    fontWeight: 700,
                    color: "var(--text-bright)",
                    marginBottom: 12,
                  }}
                >
                  # Memory · index
                </div>
                <div
                  style={{
                    fontFamily: "var(--font-sans)",
                    fontSize: 14,
                    color: "var(--text-muted)",
                    lineHeight: 1.6,
                  }}
                >
                  Workspace notes for <code style={{ color: "var(--accent-cyan)" }}>blxcode</code>.
                  <br />
                  <br />
                  • <code style={{ color: "var(--accent-purple)" }}>agent/</code> — agent subsystem
                  notes
                  <br />
                  • <code style={{ color: "var(--accent-purple)" }}>voice/</code> — PTT and TTS
                  behavior
                  <br />
                  • <code style={{ color: "var(--accent-purple)" }}>memory/</code> — memory panel
                  and indexing
                </div>
                <div
                  style={{
                    marginTop: 20,
                    padding: "10px 14px",
                    borderRadius: 10,
                    background: "var(--bg-raised)",
                    border: "1px solid var(--border)",
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    fontFamily: "var(--font-mono)",
                    fontSize: 12,
                    color: "var(--text-faint)",
                  }}
                >
                  <span>📄</span> auto-loaded from <code style={{ color: "var(--accent-blue)" }}>.agents/memory/README.md</code>
                </div>
              </div>
            </div>
          </div>

          {/* Centered tab indicator */}
          <div
            style={{
              position: "absolute",
              top: 0,
              right: -40,
              width: 80 * tabOpen,
              height: 32,
              background: "var(--accent-blue)",
              borderRadius: 8,
              boxShadow: "0 8px 24px -4px rgba(122, 162, 247, 0.5)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              fontFamily: "var(--font-mono)",
              fontSize: 11,
              color: "#16161e",
              fontWeight: 600,
              overflow: "hidden",
              opacity: tabOpen,
            }}
          >
            {tabOpen > 0.5 ? "MEMORY" : ""}
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .memory-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const MEMORY_DURATION = DURATION;
