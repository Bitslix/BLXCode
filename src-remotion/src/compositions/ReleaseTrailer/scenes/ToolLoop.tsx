import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background, FeatureCard } from "../components";

const DURATION = 120;
const FEATURE_ENTER = 12;

export const ToolLoopScene: React.FC = () => {
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

  const value = Math.floor(
    interpolate(frame, [16, 56], [1, 96], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })
  );
  const valueOpacity = interpolate(frame, [16, 28], [0, 1], {
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  const tickCount = 25;
  const ticks = Array.from({ length: tickCount }, (_, i) => i);
  const limit = 96;

  return (
    <AbsoluteFill className="tool-loop-scene" style={{ opacity: exitOpacity }}>
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
            id: "tool-loop",
            kicker: "Settings",
            title: "Configurable tool-loop limit",
            body: "Per-turn ceiling on tool-call rounds is now a setting (1-500, default 36). No more hard-coded 36-round wall.",
            accent: "danger",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div
          style={{
            width: 620,
            opacity: cardEnter,
            transform: `translateY(${(1 - cardEnter) * 20}px)`,
            background: "var(--bg-panel)",
            border: "1px solid var(--border-strong)",
            borderRadius: 20,
            padding: 28,
            boxShadow: "0 30px 80px -20px rgba(0, 0, 0, 0.5)",
            display: "flex",
            flexDirection: "column",
            gap: 22,
          }}
        >
          {/* Field */}
          <div>
            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 12,
                color: "var(--text-faint)",
                letterSpacing: "0.12em",
                textTransform: "uppercase",
                marginBottom: 12,
              }}
            >
              tool-loop limit
            </div>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 14,
                padding: "16px 18px",
                background: "var(--bg-raised)",
                border: "1px solid var(--border-strong)",
                borderRadius: 12,
              }}
            >
              <div
                style={{
                  fontFamily: "var(--font-sans)",
                  fontSize: 56,
                  fontWeight: 700,
                  color: "var(--text-bright)",
                  fontFeatureSettings: '"tnum"',
                  letterSpacing: "-0.02em",
                  flex: 1,
                  opacity: valueOpacity,
                }}
              >
                {value}
              </div>
              <div
                style={{
                  fontFamily: "var(--font-mono)",
                  fontSize: 12,
                  color: "var(--text-faint)",
                  textTransform: "uppercase",
                  letterSpacing: "0.1em",
                }}
              >
                rounds
              </div>
            </div>
          </div>

          {/* Slider */}
          <div>
            <div
              style={{
                position: "relative",
                height: 8,
                borderRadius: 4,
                background: "var(--bg-raised)",
                overflow: "hidden",
                border: "1px solid var(--border)",
              }}
            >
              <div
                style={{
                  position: "absolute",
                  inset: 0,
                  width: `${(value / 500) * 100}%`,
                  background: "linear-gradient(90deg, var(--accent-cyan) 0%, var(--accent-purple) 100%)",
                }}
              />
            </div>
            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                marginTop: 8,
                fontFamily: "var(--font-mono)",
                fontSize: 11,
                color: "var(--text-faint)",
              }}
            >
              <span>1</span>
              <span>100</span>
              <span>200</span>
              <span>300</span>
              <span>400</span>
              <span>500</span>
            </div>
          </div>

          {/* Ticks */}
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              flexWrap: "wrap",
            }}
          >
            {ticks.map((i) => {
              const enter = spring({
                frame: frame - 18 - i * 0.6,
                fps,
                config: { damping: 16, stiffness: 140, mass: 0.4 },
              });
              const isLimit = i === limit;
              return (
                <div
                  key={i}
                  style={{
                    width: 18,
                    height: 18,
                    borderRadius: 4,
                    background: isLimit
                      ? "var(--status-danger)"
                      : i < limit
                        ? "var(--accent-purple)"
                        : "var(--bg-raised)",
                    border: isLimit
                      ? "1px solid var(--status-danger)"
                      : "1px solid var(--border)",
                    opacity: enter,
                    boxShadow: isLimit ? "0 0 8px var(--status-danger)" : "none",
                    transform: isLimit ? "scale(1.2)" : undefined,
                  }}
                />
              );
            })}
          </div>

          {/* Callouts */}
          <div
            style={{
              display: "flex",
              gap: 12,
              fontFamily: "var(--font-mono)",
              fontSize: 12,
              color: "var(--text-muted)",
            }}
          >
            <div
              style={{
                flex: 1,
                padding: "10px 12px",
                borderRadius: 8,
                background: "var(--bg-raised)",
                border: "1px solid var(--border)",
              }}
            >
              <div
                style={{
                  color: "var(--text-faint)",
                  letterSpacing: "0.1em",
                  textTransform: "uppercase",
                  fontSize: 10,
                }}
              >
                default
              </div>
              <div style={{ color: "var(--text-bright)", fontSize: 18, fontWeight: 600 }}>36</div>
            </div>
            <div
              style={{
                flex: 1,
                padding: "10px 12px",
                borderRadius: 8,
                background: "var(--bg-raised)",
                border: "1px solid var(--border)",
              }}
            >
              <div
                style={{
                  color: "var(--text-faint)",
                  letterSpacing: "0.1em",
                  textTransform: "uppercase",
                  fontSize: 10,
                }}
              >
                range
              </div>
              <div style={{ color: "var(--text-bright)", fontSize: 18, fontWeight: 600 }}>1 – 500</div>
            </div>
            <div
              style={{
                flex: 1,
                padding: "10px 12px",
                borderRadius: 8,
                background: "var(--bg-raised)",
                border: "1px solid var(--border)",
              }}
            >
              <div
                style={{
                  color: "var(--text-faint)",
                  letterSpacing: "0.1em",
                  textTransform: "uppercase",
                  fontSize: 10,
                }}
              >
                clamped
              </div>
              <div style={{ color: "var(--status-success)", fontSize: 14, fontWeight: 600 }}>on save</div>
            </div>
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .tool-loop-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const TOOL_LOOP_DURATION = DURATION;
