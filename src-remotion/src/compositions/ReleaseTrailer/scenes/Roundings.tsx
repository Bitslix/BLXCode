import { AbsoluteFill, interpolate, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { Background } from "../components/Background";
import { FeatureCard } from "../components/FeatureCard";

const DURATION = 150;
const FEATURE_ENTER = 18;

const SCALES: { id: string; label: string; radius: number; delay: number; font: string }[] = [
  { id: "sharp", label: "Sharp", radius: 2, delay: 0, font: "var(--font-mono)" },
  { id: "default", label: "Default", radius: 8, delay: 0.1, font: "var(--font-mono)" },
  { id: "rounded", label: "Rounded", radius: 16, delay: 0.2, font: "var(--font-mono)" },
  { id: "extra", label: "Extra", radius: 24, delay: 0.3, font: "var(--font-mono)" },
];

export const RoundingsScene: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  const exit = spring({
    frame: frame - (DURATION - 18),
    fps,
    config: { damping: 24, stiffness: 100, mass: 0.7 },
  });
  const exitOpacity = Math.max(0, 1 - Math.max(0, exit - 1));

  // Active scale cycles: 0=sharp, 1=default, 2=rounded, 3=extra
  const activeIndex = Math.min(3, Math.floor((frame - 25) / 16));

  // Font swap moment
  const fontSwap = interpolate(frame, [95, 115], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" });

  return (
    <AbsoluteFill className="roundings-scene" style={{ opacity: exitOpacity }}>
      <Background fadeIn={0} />
      <AbsoluteFill style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", padding: "0 100px" }}>
        <FeatureCard
          feature={{
            id: "roundings",
            kicker: "Appearance",
            title: "Roundings & Font",
            body: "Pick a corner-radius scale and a monospace font. Theme-independent, persisted, re-rounds instantly.",
            accent: "purple",
          }}
          enterAt={FEATURE_ENTER}
          durationInFrames={DURATION}
          side="left"
        />

        <div style={{ width: 600, display: "flex", flexDirection: "column", gap: 32 }}>
          {/* Roundings row */}
          <div>
            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 16,
                color: "var(--text-faint)",
                letterSpacing: "0.12em",
                textTransform: "uppercase",
                marginBottom: 14,
              }}
            >
              Roundings
            </div>
            <div style={{ display: "flex", gap: 18, alignItems: "flex-end" }}>
              {SCALES.map((scale, i) => {
                const enter = spring({
                  frame: frame - 15 - i * 4,
                  fps,
                  config: { damping: 16, stiffness: 100, mass: 0.7 },
                });
                const isActive = i === activeIndex;
                return (
                  <div
                    key={scale.id}
                    style={{
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "center",
                      gap: 12,
                      opacity: enter,
                      transform: `translateY(${(1 - enter) * 16}px)`,
                    }}
                  >
                    <div
                      style={{
                        width: 110,
                        height: 110,
                        borderRadius: scale.radius,
                        background: isActive
                          ? "linear-gradient(135deg, var(--accent-purple) 0%, var(--accent-pink) 100%)"
                          : "var(--bg-raised)",
                        border: `2px solid ${isActive ? "var(--accent-purple)" : "var(--border-strong)"}`,
                        boxShadow: isActive
                          ? "0 12px 30px -8px rgba(189, 147, 249, 0.5), 0 0 0 4px rgba(189, 147, 249, 0.15)"
                          : "none",
                        transition: "background 0.2s, border 0.2s, box-shadow 0.2s",
                      }}
                    />
                    <div
                      style={{
                        fontFamily: "var(--font-sans)",
                        fontSize: 18,
                        color: isActive ? "var(--text-bright)" : "var(--text-muted)",
                        fontWeight: isActive ? 600 : 400,
                      }}
                    >
                      {scale.label}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Font row */}
          <div>
            <div
              style={{
                fontFamily: "var(--font-mono)",
                fontSize: 16,
                color: "var(--text-faint)",
                letterSpacing: "0.12em",
                textTransform: "uppercase",
                marginBottom: 14,
              }}
            >
              Font
            </div>
            <div
              style={{
                padding: 18,
                background: "var(--bg-panel)",
                border: "1px solid var(--border-strong)",
                borderRadius: 14,
                fontFamily: fontSwap > 0.5 ? '"JetBrains Mono", monospace' : "var(--font-sans)",
                fontSize: 28,
                color: "var(--text-bright)",
                display: "flex",
                alignItems: "center",
                gap: 16,
                transition: "font-family 0.3s",
              }}
            >
              <span style={{ flex: 1, fontFamily: fontSwap > 0.5 ? '"JetBrains Mono", monospace' : "var(--font-sans)" }}>
                {`const greet = (name) => \`hello, \${name}!\`;`}
              </span>
              <span
                style={{
                  padding: "6px 12px",
                  borderRadius: 8,
                  background: "var(--accent-purple-soft)",
                  color: "var(--accent-purple)",
                  fontFamily: "var(--font-sans)",
                  fontSize: 16,
                  fontWeight: 500,
                }}
              >
                JetBrains Mono
              </span>
            </div>
          </div>
        </div>
      </AbsoluteFill>
      <style>{`
        .roundings-scene {
          color: var(--text);
          font-family: var(--font-sans);
        }
      `}</style>
    </AbsoluteFill>
  );
};

export const ROUNDINGS_DURATION = DURATION;
