import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background } from "../components/Background";
import { FeatureCard } from "../components/FeatureCard";

const DURATION = 150;
const FEATURE_ENTER = 18;

export const PlansScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  // Dialog enter
  const dialogEnter = spring({
    frame: frame - 8,
    fps,
    config: { damping: 18, stiffness: 90, mass: 0.8 },
  });
  const dialogOpacity = interpolate(frame, [8, 28], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  // Shimmer phase
  const shimmerActive = frame > 30 && frame < 65;
  const shimmer = shimmerActive ? Math.sin((frame / 6)) * 0.5 + 0.5 : 1;
  const shimmerWidth = shimmerActive ? 20 + shimmer * 60 : 0;

  // Markdown preview appear
  const previewEnter = interpolate(frame, [60, 78], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });
  const previewY = interpolate(frame, [60, 78], [20, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  // Tasks appear staggered
  const taskReveal = (i: number) => interpolate(frame, [75 + i * 5, 85 + i * 5], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  return (
    <AbsoluteFill className="plans-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />
      <AbsoluteFill style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", padding: "0 100px" }}>
        <FeatureCard
          feature={{
            id: "plans",
            kicker: "Plans",
            title: "AI Plan & AI Tasks",
            body: "Turn a short prompt into a full Markdown plan with an optional task list, right inside the Plans panel.",
            accent: "blue",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div
          style={{
            width: 640,
            opacity: dialogOpacity,
            transform: `scale(${0.94 + 0.06 * dialogEnter}) translateY(${(1 - dialogEnter) * 20}px)`,
            background: "var(--bg-panel)",
            border: "1px solid var(--border-strong)",
            borderRadius: 20,
            boxShadow: "0 40px 100px -20px rgba(0, 0, 0, 0.6)",
            overflow: "hidden",
          }}
        >
          {/* Header */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 12,
              padding: "16px 20px",
              borderBottom: "1px solid var(--border)",
            }}
          >
            <div
              style={{
                width: 32,
                height: 32,
                borderRadius: 8,
                background: "var(--accent-blue-soft)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                fontSize: 18,
              }}
            >
              ✦
            </div>
            <div style={{ fontSize: 20, fontWeight: 600, color: "var(--text-bright)", flex: 1 }}>
              AI Plan
            </div>
            <div
              style={{
                fontSize: 14,
                color: "var(--text-muted)",
                padding: "4px 10px",
                borderRadius: 999,
                background: "var(--bg-raised)",
                border: "1px solid var(--border)",
                display: "flex",
                alignItems: "center",
                gap: 6,
              }}
            >
              <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--accent-blue)" }} />
              with tasks
            </div>
          </div>

          {/* Prompt area */}
          <div style={{ padding: 20 }}>
            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 14,
                color: "var(--text-faint)",
                letterSpacing: "0.08em",
                textTransform: "uppercase",
                marginBottom: 8,
              }}
            >
              prompt
            </div>
            <div
              style={{
                position: "relative",
                minHeight: 80,
                padding: 16,
                background: "var(--bg-raised)",
                border: "1px solid var(--border)",
                borderRadius: 12,
                fontSize: 18,
                color: "var(--text)",
                fontFamily: "var(--font-sans)",
                lineHeight: 1.5,
              }}
            >
              add a /health endpoint to the agent service that returns orb mode and last turn duration
              {shimmerActive && (
                <div
                  style={{
                    position: "absolute",
                    inset: 0,
                    borderRadius: 12,
                    background:
                      "linear-gradient(90deg, transparent 0%, rgba(122, 162, 247, 0.2) 50%, transparent 100%)",
                    backgroundSize: `${shimmerWidth}% 100%`,
                    backgroundRepeat: "no-repeat",
                    animation: "shimmer 1.2s linear infinite",
                    pointerEvents: "none",
                  }}
                />
              )}
            </div>

            {/* Markdown preview */}
            <div
              style={{
                marginTop: 18,
                opacity: previewEnter,
                transform: `translateY(${previewY}px)`,
              }}
            >
              <div
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: 14,
                  color: "var(--text-faint)",
                  letterSpacing: "0.08em",
                  textTransform: "uppercase",
                  marginBottom: 8,
                }}
              >
                preview
              </div>
              <div
                style={{
                  padding: 16,
                  background: "var(--bg-raised)",
                  border: "1px solid var(--border)",
                  borderRadius: 12,
                }}
              >
                <div
                  style={{
                    fontSize: 22,
                    fontWeight: 700,
                    color: "var(--text-bright)",
                    marginBottom: 10,
                    fontFamily: "var(--font-sans)",
                  }}
                >
                  Agent health endpoint
                </div>
                <div
                  style={{
                    fontSize: 16,
                    color: "var(--text-muted)",
                    lineHeight: 1.5,
                    marginBottom: 12,
                    fontFamily: "var(--font-sans)",
                  }}
                >
                  Add a small, read-only endpoint that reports the agent&rsquo;s runtime
                  state so external monitors can probe it.
                </div>
                <div
                  style={{
                    fontSize: 16,
                    fontWeight: 600,
                    color: "var(--text-bright)",
                    marginBottom: 8,
                    fontFamily: "var(--font-sans)",
                  }}
                >
                  ## Tasks
                </div>
                {[
                  { id: "agent-health-1", title: "Add /health route + handler" },
                  { id: "agent-health-2", title: "Return orb mode + last turn duration" },
                  { id: "agent-health-3", title: "Cover with unit tests" },
                ].map((task, i) => {
                  const reveal = taskReveal(i);
                  return (
                    <div
                      key={task.id}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 10,
                        padding: "4px 0",
                        fontFamily: "var(--font-mono)",
                        fontSize: 15,
                        color: "var(--text)",
                        opacity: reveal,
                        transform: `translateX(${(1 - reveal) * 12}px)`,
                      }}
                    >
                      <span
                        style={{
                          width: 14,
                          height: 14,
                          borderRadius: 4,
                          border: "1.5px solid var(--accent-blue)",
                          background: reveal > 0.5 ? "var(--accent-blue-soft)" : "transparent",
                        }}
                      />
                      <span style={{ color: "var(--text-faint)" }}>{task.id}</span>
                      <span style={{ color: "var(--text-muted)" }}>—</span>
                      <span>{task.title}</span>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>

          {/* Footer */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 10,
              padding: 16,
              borderTop: "1px solid var(--border)",
              background: "var(--bg-raised)",
            }}
          >
            <div
              style={{
                padding: "10px 18px",
                borderRadius: 10,
                fontSize: 16,
                color: "var(--text-muted)",
                background: "transparent",
              }}
            >
              Cancel
            </div>
            <div
              style={{
                padding: "10px 18px",
                borderRadius: 10,
                fontSize: 16,
                color: "var(--text)",
                background: "var(--bg-panel)",
                border: "1px solid var(--border-strong)",
              }}
            >
              Regenerate
            </div>
            <div style={{ flex: 1 }} />
            <div
              style={{
                padding: "10px 22px",
                borderRadius: 10,
                fontSize: 16,
                fontWeight: 600,
                color: "#16161e",
                background: "linear-gradient(135deg, var(--accent-blue) 0%, var(--accent-purple) 100%)",
                boxShadow: "0 0 20px rgba(122, 162, 247, 0.4)",
              }}
            >
              Save plan
            </div>
          </div>
        </div>
      </AbsoluteFill>

      <style>{`
        @keyframes shimmer {
          0% { background-position: -100% 0; }
          100% { background-position: 200% 0; }
        }
        .plans-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const PLANS_DURATION = DURATION;
