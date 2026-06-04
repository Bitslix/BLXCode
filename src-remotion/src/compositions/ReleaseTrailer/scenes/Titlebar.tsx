import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background, FeatureCard } from "../components";

const DURATION = 120;
const FEATURE_ENTER = 12;

const NAVIGATE_ITEMS: { label: string; shortcut?: string; delay: number }[] = [
  { label: "Terminals", shortcut: "prefix + t", delay: 28 },
  { label: "New terminal", shortcut: "prefix + n", delay: 34 },
  { label: "Plans", shortcut: "prefix + p", delay: 40 },
  { label: "Memory", delay: 46 },
  { label: "Skills", delay: 52 },
  { label: "Settings", shortcut: "prefix + ,", delay: 58 },
  { label: "Fullscreen", shortcut: "F11", delay: 64 },
];

const BREADCRUMBS: { name: string; color: string; delay: number }[] = [
  { name: "blxcode", color: "#bd93f9", delay: 0 },
  { name: "src", color: "#7dcfff", delay: 4 },
  { name: "agent_panel", color: "#ff79c6", delay: 8 },
  { name: "composer", color: "#50fa7b", delay: 12 },
];

export const TitlebarScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  const barEnter = spring({
    frame: frame - 2,
    fps,
    config: { damping: 18, stiffness: 100, mass: 0.7 },
  });

  const navigateOpen = interpolate(frame, [25, 35], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill className="titlebar-scene" style={{ opacity: exitOpacity }}>
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
            id: "titlebar",
            kicker: "Titlebar",
            title: "Custom cross-platform app titlebar",
            body: "Brand cluster, native window controls, NAVIGATE quick menu, centered workspace breadcrumbs, and a live focused-terminal crumb.",
            accent: "purple",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div
          style={{
            width: 880,
            opacity: barEnter,
            transform: `translateY(${(1 - barEnter) * 20}px)`,
          }}
        >
          {/* Window */}
          <div
            style={{
              background: "var(--bg-panel)",
              border: "1px solid var(--border-strong)",
              borderRadius: 16,
              overflow: "hidden",
              boxShadow: "0 40px 100px -20px rgba(0, 0, 0, 0.6)",
            }}
          >
            {/* Titlebar */}
            <div
              style={{
                display: "flex",
                alignItems: "center",
                padding: "0 16px",
                height: 52,
                background: "var(--bg-panel-header)",
                borderBottom: "1px solid var(--border)",
                position: "relative",
              }}
            >
              {/* Brand cluster */}
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  width: 180,
                }}
              >
                <div
                  style={{
                    width: 26,
                    height: 26,
                    borderRadius: 8,
                    background:
                      "linear-gradient(135deg, var(--accent-purple) 0%, var(--accent-pink) 100%)",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    fontSize: 16,
                    fontWeight: 800,
                    color: "#16161e",
                  }}
                >
                  b
                </div>
                <div
                  style={{
                    fontFamily: "var(--font-sans)",
                    fontSize: 15,
                    fontWeight: 700,
                    color: "var(--text-bright)",
                    letterSpacing: "-0.01em",
                  }}
                >
                  blxcode
                </div>
              </div>

              {/* Sidebar + panel toggles */}
              <div style={{ display: "flex", alignItems: "center", gap: 6, width: 70 }}>
                <TitlebarBtn symbol="◧" />
                <TitlebarBtn symbol="◨" />
              </div>

              {/* Breadcrumbs */}
              <div
                style={{
                  flex: 1,
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: 6,
                }}
              >
                {BREADCRUMBS.map((b, i) => {
                  const enter = spring({
                    frame: frame - 8 - i * 4,
                    fps,
                    config: { damping: 16, stiffness: 110, mass: 0.5 },
                  });
                  return (
                    <div
                      key={b.name}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 6,
                        opacity: enter,
                        transform: `translateY(${(1 - enter) * 8}px)`,
                      }}
                    >
                      <span
                        style={{
                          width: 6,
                          height: 6,
                          borderRadius: "50%",
                          background: b.color,
                          boxShadow: `0 0 6px ${b.color}`,
                        }}
                      />
                      <span
                        style={{
                          fontFamily: "var(--font-mono)",
                          fontSize: 14,
                          color: i === BREADCRUMBS.length - 1 ? "var(--text-bright)" : "var(--text-muted)",
                          fontWeight: i === BREADCRUMBS.length - 1 ? 600 : 400,
                        }}
                      >
                        {b.name}
                      </span>
                      {i < BREADCRUMBS.length - 1 && (
                        <span style={{ color: "var(--text-faint)", fontSize: 12 }}>›</span>
                      )}
                    </div>
                  );
                })}
                {/* Focused terminal crumb */}
                <div
                  style={{
                    marginLeft: 12,
                    padding: "4px 10px",
                    borderRadius: 6,
                    background: "var(--bg-raised)",
                    border: "1px solid var(--border)",
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    opacity: interpolate(frame, [40, 55], [0, 1], {
                      extrapolateLeft: "clamp",
                      extrapolateRight: "clamp",
                    }),
                  }}
                >
                  <span
                    style={{
                      width: 6,
                      height: 6,
                      borderRadius: "50%",
                      background: "var(--accent-pink)",
                    }}
                  />
                  <span
                    style={{
                      fontFamily: "var(--font-mono)",
                      fontSize: 13,
                      color: "var(--text-muted)",
                    }}
                  >
                    #3 · Mia
                  </span>
                </div>
              </div>

              {/* NAVIGATE button */}
              <button
                style={{
                  padding: "6px 12px",
                  borderRadius: 6,
                  border: "1px solid var(--border-strong)",
                  background: navigateOpen > 0.5 ? "var(--accent-purple)" : "var(--bg-raised)",
                  color: navigateOpen > 0.5 ? "#16161e" : "var(--text)",
                  fontFamily: "var(--font-mono)",
                  fontSize: 12,
                  fontWeight: 600,
                  letterSpacing: "0.1em",
                }}
              >
                NAVIGATE
              </button>

              {/* Window controls */}
              <div style={{ display: "flex", alignItems: "center", gap: 6, marginLeft: 12 }}>
                <TitlebarBtn symbol="—" />
                <TitlebarBtn symbol="◻" />
                <TitlebarBtn symbol="✕" />
              </div>
            </div>

            {/* NAVIGATE popover */}
            <div
              style={{
                position: "relative",
                height: navigateOpen * 460,
                transition: "height 0.2s",
                overflow: "hidden",
              }}
            >
              <div
                style={{
                  position: "absolute",
                  top: 8,
                  right: 100,
                  width: 280,
                  background: "var(--bg-raised)",
                  border: "1px solid var(--border-strong)",
                  borderRadius: 12,
                  padding: 8,
                  boxShadow: "0 30px 80px -20px rgba(0, 0, 0, 0.6)",
                  opacity: navigateOpen,
                  transform: `translateY(${(1 - navigateOpen) * -8}px)`,
                }}
              >
                <div
                  style={{
                    fontFamily: "var(--font-mono)",
                    fontSize: 11,
                    color: "var(--text-faint)",
                    letterSpacing: "0.16em",
                    textTransform: "uppercase",
                    padding: "8px 10px 6px 10px",
                  }}
                >
                  Quick menu
                </div>
                {NAVIGATE_ITEMS.map((item) => {
                  const enter = spring({
                    frame: frame - item.delay,
                    fps,
                    config: { damping: 16, stiffness: 120, mass: 0.5 },
                  });
                  return (
                    <div
                      key={item.label}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 10,
                        padding: "8px 10px",
                        borderRadius: 6,
                        opacity: enter,
                        transform: `translateX(${(1 - enter) * 8}px)`,
                      }}
                    >
                      <span
                        style={{
                          fontSize: 14,
                          color: "var(--text)",
                          flex: 1,
                        }}
                      >
                        {item.label}
                      </span>
                      {item.shortcut && (
                        <span
                          style={{
                            fontFamily: "var(--font-mono)",
                            fontSize: 11,
                            color: "var(--text-faint)",
                          }}
                        >
                          {item.shortcut}
                        </span>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .titlebar-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

const TitlebarBtn: React.FC<{ symbol: string }> = ({ symbol }) => (
  <div
    style={{
      width: 30,
      height: 30,
      borderRadius: 6,
      display: "flex",
      alignItems: "center",
      justifyContent: "center",
      fontSize: 12,
      color: "var(--text-muted)",
      background: "transparent",
    }}
  >
    {symbol}
  </div>
);

export const TITLEBAR_DURATION = DURATION;
